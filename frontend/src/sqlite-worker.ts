import sqlite3InitModule from '@sqlite.org/sqlite-wasm';

let sqlite3: any;
let db: any;

self.onmessage = async (e) => {
    if (e.data.type === 'init') {
        try {
            sqlite3 = await sqlite3InitModule({
                print: console.log,
                printErr: console.error,
            });
            if (sqlite3.opfs) {
                db = new sqlite3.oo1.OpfsDb('/mydb.sqlite3');
                console.log('OPFS SQLite DB initialized.');
                self.postMessage({ type: 'ready' });
            } else {
                console.error('OPFS is not available.');
            }
        } catch (err) {
            console.error('SQLite init failed:', err);
        }
    } else if (e.data.type === 'query') {
        if (!db) {
            self.postMessage({ type: 'result', error: 'DB not initialized', id: e.data.id });
            return;
        }
        try {
            const rows: any[] = [];
            db.exec({
                sql: e.data.sql,
                rowMode: 'object',
                callback: function (row: any) {
                    rows.push(row);
                }
            });
            self.postMessage({ type: 'result', rows, id: e.data.id });
        } catch (err: any) {
            self.postMessage({ type: 'result', error: err.message, id: e.data.id });
        }
    } else if (e.data.type === 'export') {
        if (!db) {
            self.postMessage({ type: 'export_result', error: 'DB not initialized', id: e.data.id });
            return;
        }
        try {
            const byteArray = sqlite3.capi.sqlite3_js_db_export(db.pointer);
            self.postMessage({ type: 'export_result', data: byteArray, id: e.data.id });
        } catch (err: any) {
            self.postMessage({ type: 'export_result', error: err.message, id: e.data.id });
        }
    } else if (e.data.type === 'import') {
        if (!sqlite3 || !sqlite3.opfs) {
            self.postMessage({ type: 'import_result', error: 'SQLite/OPFS not available', id: e.data.id });
            return;
        }
        try {
            if (db) db.close();
            // Create a new memory DB
            const memDb = new sqlite3.oo1.DB();
            // Import the provided bytes
            sqlite3.capi.sqlite3_deserialize(memDb.pointer, 'main', e.data.data, e.data.data.length, e.data.data.length, 0);

            // Delete old OPFS file
            try { sqlite3.opfs.unlink('/mydb.sqlite3'); } catch (e) {}

            // Re-create OPFS DB
            db = new sqlite3.oo1.OpfsDb('/mydb.sqlite3');

            // Copy contents from memDb to OPFS DB using vacuum/backup or manual table copying.
            // Modern sqlite3-wasm allows saving a memory DB to OPFS via sqlite3_js_vfs_create_file
            // A simpler way: serialize memdb, then write to OPFS (since opfs provides synchronous IO)
            const exported = sqlite3.capi.sqlite3_js_db_export(memDb.pointer);

            // Using OPFS SyncAccessHandle to write directly to the VFS.
            const root = await navigator.storage.getDirectory();
            const fileHandle = await root.getFileHandle('mydb.sqlite3', { create: true });
            const accessHandle = await fileHandle.createSyncAccessHandle();
            accessHandle.truncate(0);
            accessHandle.write(exported);
            accessHandle.flush();
            accessHandle.close();

            memDb.close();

            // Reopen DB to apply
            db.close();
            db = new sqlite3.oo1.OpfsDb('/mydb.sqlite3');

            self.postMessage({ type: 'import_result', success: true, id: e.data.id });
        } catch (err: any) {
            self.postMessage({ type: 'import_result', error: err.message, id: e.data.id });
        }
    }
};
