import * as Comlink from 'comlink';
import init, { execute_command, run_mss, init_vfs, setup_engine, get_wasm_memory_size, lint_mss } from '../../engine/pkg/engine.js';

const STATE_IDLE = 0;
const STATE_REQ = 1;
const STATE_DONE = 2;
const STATE_ERR = 3;

let sharedBuffer: SharedArrayBuffer;
let sharedInt32: Int32Array;
let dataBuffer: Uint8Array;
let ioWorker: Worker;
let sqliteWorker: Worker;
let aiWorker: Worker;

let queryIdCounter = 0;
const pendingQueries = new Map<number, { resolve: Function, reject: Function }>();
const pendingAiRequests = new Map<number, { resolve: Function, reject: Function }>();
let aiProgressCallback: ((msg: string) => void) | null = null;

const api = {
    async init(logCallback?: (msg: string) => void, progressCallback?: (msg: string) => void) {
        aiProgressCallback = progressCallback || null;
        if (logCallback) {
            const originalConsoleLog = console.log;
            console.log = (...args) => {
                logCallback(args.join(' '));
                originalConsoleLog(...args);
            };
        }

        // Stream compile Wasm if possible, fallback to fetch + instantiate
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                await init(new URL('../../engine/pkg/engine_bg.wasm', import.meta.url));
            } catch (e) {
                console.warn('Wasm instantiateStreaming failed, falling back to arrayBuffer:', e);
                const response = await fetch(new URL('../../engine/pkg/engine_bg.wasm', import.meta.url));
                const buffer = await response.arrayBuffer();
                await init(buffer);
            }
        } else {
            const response = await fetch(new URL('../../engine/pkg/engine_bg.wasm', import.meta.url));
            const buffer = await response.arrayBuffer();
            await init(buffer);
        }
        setup_engine();

        // Initialize SharedArrayBuffer for sync I/O (1MB for data)
        sharedBuffer = new SharedArrayBuffer(8 + 1024 * 1024);
        sharedInt32 = new Int32Array(sharedBuffer);
        dataBuffer = new Uint8Array(sharedBuffer, 8);

        // Initialize I/O Worker
        ioWorker = new Worker(new URL('./io-worker.ts', import.meta.url), { type: 'module' });

        // Initialize SQLite Worker
        sqliteWorker = new Worker(new URL('./sqlite-worker.ts', import.meta.url), { type: 'module' });
        sqliteWorker.onmessage = (e) => {
            if (e.data.type === 'result' || e.data.type === 'export_result' || e.data.type === 'import_result') {
                const pending = pendingQueries.get(e.data.id);
                if (pending) {
                    if (e.data.error) {
                        pending.reject(new Error(e.data.error));
                    } else {
                        pending.resolve(e.data.data !== undefined ? e.data.data : (e.data.rows !== undefined ? e.data.rows : e.data.success));
                    }
                    pendingQueries.delete(e.data.id);
                }
            }
        };
        sqliteWorker.postMessage({ type: 'init' });

        // Initialize AI Worker
        aiWorker = new Worker(new URL('./ai-worker.ts', import.meta.url), { type: 'module' });
        aiWorker.onmessage = (e) => {
            if (e.data.type === 'progress') {
                if (aiProgressCallback) aiProgressCallback(e.data.message);
            } else if (e.data.type === 'ready') {
                const pending = pendingAiRequests.get(e.data.id);
                if (pending) {
                    pending.resolve();
                    pendingAiRequests.delete(e.data.id);
                }
            } else if (e.data.type === 'result') {
                const pending = pendingAiRequests.get(e.data.id);
                if (pending) {
                    pending.resolve(e.data.text);
                    pendingAiRequests.delete(e.data.id);
                }
            } else if (e.data.type === 'error') {
                const pending = pendingAiRequests.get(e.data.id);
                if (pending) {
                    pending.reject(new Error(e.data.error));
                    pendingAiRequests.delete(e.data.id);
                }
            }
        };

        // Check storage persist permission
        if (navigator.storage && navigator.storage.persist) {
            const isPersisted = await navigator.storage.persisted();
            if (!isPersisted) {
                const granted = await navigator.storage.persist();
                console.log(`Storage persist granted: ${granted}`);
            } else {
                console.log('Storage is already persisted.');
            }
        }

        // Get OPFS root to pass to I/O worker
        const root = await navigator.storage.getDirectory();
        ioWorker.postMessage({ type: 'init', buffer: sharedBuffer, root }, [root as any]);

        // Attach sync I/O functions to global for Wasm
        (self as any).readSync = api.readSync;
        (self as any).writeSync = api.writeSync;
        (self as any).truncateSync = api.truncateSync;

        // Implement httpGet and sleep for Wasm
        (self as any).httpGet = async (url: string) => {
            try {
                const res = await fetch(url);
                return await res.text();
            } catch (e) {
                return `Fetch Error: ${e}`;
            }
        };

        (self as any).sleep = async (ms: number) => {
            return new Promise(resolve => setTimeout(resolve, ms));
        };

        await init_vfs();

        // Memory limit monitoring
        setInterval(() => {
            try {
                const pages = get_wasm_memory_size();
                const sizeMb = (pages * 64 * 1024) / (1024 * 1024);
                if (sizeMb > 500) {
                    console.warn(`High Wasm Memory Usage: ${sizeMb.toFixed(2)} MB`);
                }
            } catch (e) {
                // Ignore if not initialized
            }
        }, 10000);

        return "Wasm Initialized with Sync I/O";
    },
    async executeCommand(cmdLine: string) {
        if (cmdLine.trim().startsWith('translate ')) {
            const textToTranslate = cmdLine.substring(10).trim().replace(/^["'](.*)["']$/, '$1');
            try {
                const result = await api.translateText(textToTranslate);
                return `Translation: ${result}`;
            } catch (e: any) {
                return `Translation Error: ${e.message}`;
            }
        }
        if (cmdLine.trim().startsWith('sqlite ')) {
            const sql = cmdLine.substring(7).trim();
            // remove surrounding quotes if any
            const cleanedSql = sql.replace(/^["'](.*)["']$/, '$1');

            if (cleanedSql === '.export') {
                try {
                    const data = await api.exportSqliteDb();
                    const b64 = btoa(String.fromCharCode.apply(null, Array.from(data)));
                    return `Exported DB:\n${b64}`;
                } catch (e: any) {
                    return `SQLite Export Error: ${e.message}`;
                }
            }

            if (cleanedSql.startsWith('.import ')) {
                try {
                    const b64 = cleanedSql.substring(8).trim();
                    const binaryString = atob(b64);
                    const bytes = new Uint8Array(binaryString.length);
                    for (let i = 0; i < binaryString.length; i++) {
                        bytes[i] = binaryString.charCodeAt(i);
                    }
                    await api.importSqliteDb(bytes);
                    return 'SQLite DB imported successfully.';
                } catch (e: any) {
                    return `SQLite Import Error: ${e.message}`;
                }
            }

            let finalSql = cleanedSql;
            if (cleanedSql.startsWith('explain ')) {
                finalSql = 'EXPLAIN QUERY PLAN ' + cleanedSql.substring(8);
            }

            try {
                const rows = await api.querySqlite(finalSql);
                if (!rows || rows.length === 0) return 'Query executed successfully. (0 rows)';

                // Generate ASCII table for rows
                const keys = Object.keys(rows[0]);
                const colWidths = keys.map(k => k.length);

                // Only measure first 1000 rows to prevent blocking the UI for huge datasets
                const sampleRows = rows.slice(0, 1000);
                sampleRows.forEach(row => {
                    keys.forEach((k, i) => {
                        const valLen = String(row[k]).length;
                        if (valLen > colWidths[i]) colWidths[i] = valLen;
                    });
                });

                const buildSeparator = () => '+' + colWidths.map(w => '-'.repeat(w + 2)).join('+') + '+';
                const buildRow = (rowData: any) => '|' + keys.map((k, i) => ' ' + String(rowData[k]).padEnd(colWidths[i], ' ') + ' ').join('|') + '|';

                let output = buildSeparator() + '\n';
                output += '|' + keys.map((k, i) => ' ' + k.padEnd(colWidths[i], ' ') + ' ').join('|') + '|\n';
                output += buildSeparator() + '\n';

                const maxRowsToDisplay = 1000;
                rows.slice(0, maxRowsToDisplay).forEach(row => {
                    output += buildRow(row) + '\n';
                });

                if (rows.length > maxRowsToDisplay) {
                    output += `| ... ${rows.length - maxRowsToDisplay} more rows omitted for streaming performance ... |\n`;
                }
                output += buildSeparator();

                return output;
            } catch (e: any) {
                return `SQLite Error: ${e.message}`;
            }
        }
        try {
            return await execute_command(cmdLine);
        } catch (e) {
            return `Error: ${e}`;
        }
    },
    async querySqlite(sql: string): Promise<any[]> {
        return new Promise((resolve, reject) => {
            const id = queryIdCounter++;
            pendingQueries.set(id, { resolve, reject });
            sqliteWorker.postMessage({ type: 'query', sql, id });
        });
    },
    async exportSqliteDb(): Promise<Uint8Array> {
        return new Promise((resolve, reject) => {
            const id = queryIdCounter++;
            pendingQueries.set(id, { resolve, reject });
            sqliteWorker.postMessage({ type: 'export', id });
        });
    },
    async importSqliteDb(data: Uint8Array): Promise<boolean> {
        return new Promise((resolve, reject) => {
            const id = queryIdCounter++;
            pendingQueries.set(id, { resolve, reject });
            sqliteWorker.postMessage({ type: 'import', data, id });
        });
    },
    async initAiModel(): Promise<void> {
        return new Promise((resolve, reject) => {
            const id = queryIdCounter++;
            pendingAiRequests.set(id, { resolve, reject });
            aiWorker.postMessage({ type: 'init', id });
        });
    },
    async translateText(text: string, targetLang: string = 'jpn_Jpan'): Promise<string> {
        return new Promise((resolve, reject) => {
            const id = queryIdCounter++;
            pendingAiRequests.set(id, { resolve, reject });
            aiWorker.postMessage({ type: 'translate', text, targetLang, id });
        });
    },
    // Sync I/O call for Rust (to be called via JS bridge)
    readSync(path: string, offset: number, length: number): Uint8Array {
        const pathEncoded = new TextEncoder().encode(path);
        dataBuffer.set(pathEncoded);

        sharedInt32[1] = 0; // Op: Read
        sharedInt32[2] = pathEncoded.length;
        sharedInt32[3] = offset;
        sharedInt32[4] = length;

        Atomics.store(sharedInt32, 0, STATE_REQ);
        Atomics.notify(sharedInt32, 0, 1);
        Atomics.wait(sharedInt32, 0, STATE_REQ);

        if (Atomics.load(sharedInt32, 0) === STATE_DONE) {
            const bytesRead = sharedInt32[1];
            const result = new Uint8Array(bytesRead);
            result.set(dataBuffer.slice(0, bytesRead));
            Atomics.store(sharedInt32, 0, STATE_IDLE);
            return result;
        }
        Atomics.store(sharedInt32, 0, STATE_IDLE);
        throw new Error("Sync Read Failed");
    },
    writeSync(path: string, content: Uint8Array, offset: number): number {
        const pathEncoded = new TextEncoder().encode(path);

        if (pathEncoded.length + content.length > dataBuffer.length) {
            throw new Error("Data exceeds SharedArrayBuffer limit");
        }

        // Path followed by content in dataBuffer
        dataBuffer.set(pathEncoded);
        dataBuffer.set(content, pathEncoded.length);

        sharedInt32[1] = 1; // Op: Write
        sharedInt32[2] = pathEncoded.length;
        sharedInt32[3] = offset;
        sharedInt32[4] = content.length;

        Atomics.store(sharedInt32, 0, STATE_REQ);
        Atomics.notify(sharedInt32, 0, 1);
        Atomics.wait(sharedInt32, 0, STATE_REQ);

        if (Atomics.load(sharedInt32, 0) === STATE_DONE) {
            const bytesWritten = sharedInt32[1];
            Atomics.store(sharedInt32, 0, STATE_IDLE);
            return bytesWritten;
        }
        Atomics.store(sharedInt32, 0, STATE_IDLE);
        throw new Error("Sync Write Failed");
    },
    truncateSync(path: string, size: number): void {
        const pathEncoded = new TextEncoder().encode(path);
        dataBuffer.set(pathEncoded);

        sharedInt32[1] = 2; // Op: Truncate
        sharedInt32[2] = pathEncoded.length;
        sharedInt32[3] = size;

        Atomics.store(sharedInt32, 0, STATE_REQ);
        Atomics.notify(sharedInt32, 0, 1);
        Atomics.wait(sharedInt32, 0, STATE_REQ);

        if (Atomics.load(sharedInt32, 0) === STATE_DONE) {
            Atomics.store(sharedInt32, 0, STATE_IDLE);
            return;
        }
        Atomics.store(sharedInt32, 0, STATE_IDLE);
        throw new Error("Sync Truncate Failed");
    },
    async lintMss(code: string) {
        try {
            return lint_mss(code);
        } catch (e) {
            return `Error: ${e}`;
        }
    },
    async runMss(code: string, timeoutMs: number = 0) {
        let timerId: any = null;
        if (timeoutMs > 0) {
            timerId = setTimeout(() => {
                api.interrupt();
            }, timeoutMs);
        }

        try {
            return await run_mss(code);
        } finally {
            if (timerId !== null) {
                clearTimeout(timerId);
            }
            const { clear_interrupt } = await import('../../engine/pkg/engine.js');
            clear_interrupt();
        }
    },
    async interrupt() {
        // We'll need to import 'interrupt' from engine
        const { interrupt } = await import('../../engine/pkg/engine.js');
        interrupt();
    },
    async saveToCache(key: string, data: string) {
        const path = `.cache/mermaid/${key}.svg`;
        try {
            // We can use the write command we'll add to execute_command
            // Or use direct VFS access if exposed
            return await execute_command(`write "${path}" "${data}"`);
        } catch (e) {
            return `Cache Error: ${e}`;
        }
    }
};

Comlink.expose(api);
