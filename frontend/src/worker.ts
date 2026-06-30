import * as Comlink from 'comlink';
import init, { execute_command, run_mss, init_vfs, setup_engine, get_wasm_memory_size } from '../../engine/pkg/engine.js';

const STATE_IDLE = 0;
const STATE_REQ = 1;
const STATE_DONE = 2;
const STATE_ERR = 3;

let sharedBuffer: SharedArrayBuffer;
let sharedInt32: Int32Array;
let dataBuffer: Uint8Array;
let ioWorker: Worker;

const api = {
    async init(logCallback?: (msg: string) => void) {
        if (logCallback) {
            const originalConsoleLog = console.log;
            console.log = (...args) => {
                logCallback(args.join(' '));
                originalConsoleLog(...args);
            };
        }

        await init();
        setup_engine();

        // Initialize SharedArrayBuffer for sync I/O (1MB for data)
        sharedBuffer = new SharedArrayBuffer(8 + 1024 * 1024);
        sharedInt32 = new Int32Array(sharedBuffer);
        dataBuffer = new Uint8Array(sharedBuffer, 8);

        // Initialize I/O Worker
        ioWorker = new Worker(new URL('./io-worker.ts', import.meta.url), { type: 'module' });

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
        try {
            return await execute_command(cmdLine);
        } catch (e) {
            return `Error: ${e}`;
        }
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
    async runMss(code: string) {
        return run_mss(code);
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
