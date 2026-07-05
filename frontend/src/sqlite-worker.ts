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
    }
};
