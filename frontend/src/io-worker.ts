// I/O Worker for synchronous OPFS access
const STATE_IDLE = 0;
const STATE_REQ = 1;
const STATE_DONE = 2;
const STATE_ERR = 3;

const MAX_HANDLES = 10;

let sharedBuffer: Int32Array;
let dataBuffer: Uint8Array;
let root: FileSystemDirectoryHandle;
const handles = new Map<string, FileSystemSyncAccessHandle>();
const handleOrder: string[] = [];

self.onmessage = async (e) => {
    const { type, buffer, root: rootHandle } = e.data;
    if (type === 'init') {
        sharedBuffer = new Int32Array(buffer);
        dataBuffer = new Uint8Array(buffer, 8);
        root = rootHandle;
        console.log("I/O Worker initialized");
    }
};

async function getHandle(path: string, create: boolean = false) {
    let handle = handles.get(path);
    if (handle) {
        // Move to end for LRU
        const idx = handleOrder.indexOf(path);
        if (idx > -1) handleOrder.splice(idx, 1);
        handleOrder.push(path);
        return handle;
    }

    const parts = path.split('/').filter(p => p.length > 0);
    const fileName = parts.pop();
    if (!fileName) throw new Error("Invalid Path");

    let currentDir = root;
    for (const part of parts) {
        currentDir = await currentDir.getDirectoryHandle(part, { create });
    }

    const fileHandle = await currentDir.getFileHandle(fileName, { create });
    handle = await (fileHandle as any).createSyncAccessHandle();

    if (handles.size >= MAX_HANDLES) {
        const oldest = handleOrder.shift();
        if (oldest) {
            const h = handles.get(oldest);
            if (h) (h as any).close();
            handles.delete(oldest);
        }
    }

    handles.set(path, handle!);
    handleOrder.push(path);
    return handle;
}

// Continuous loop to check for requests
async function loop() {
    while (true) {
        if (!sharedBuffer) {
            await new Promise(r => setTimeout(r, 100));
            continue;
        }

        // Wait for STATE_REQ
        if (Atomics.load(sharedBuffer, 0) !== STATE_REQ) {
            Atomics.wait(sharedBuffer, 0, STATE_IDLE);
            if (Atomics.load(sharedBuffer, 0) !== STATE_REQ) continue;
        }

        try {
            const op = sharedBuffer[1]; // 0: Read, 1: Write, 2: Truncate
            const pathLen = sharedBuffer[2];
            const path = new TextDecoder().decode(dataBuffer.slice(0, pathLen));

            const handle = await getHandle(path, op !== 0); // create=true if not read

            if (op === 0) { // Read
                const offset = sharedBuffer[3];
                const length = sharedBuffer[4];
                const readBuffer = new Uint8Array(length);
                const bytesRead = handle!.read(readBuffer, { at: offset });
                dataBuffer.set(new Uint8Array(readBuffer.buffer, 0, bytesRead));
                sharedBuffer[1] = bytesRead;
                Atomics.store(sharedBuffer, 0, STATE_DONE);
            } else if (op === 1) { // Write
                const offset = sharedBuffer[3];
                const length = sharedBuffer[4];
                const writeData = dataBuffer.slice(pathLen, pathLen + length);
                const bytesWritten = handle!.write(writeData, { at: offset });
                sharedBuffer[1] = bytesWritten;
                Atomics.store(sharedBuffer, 0, STATE_DONE);
            } else if (op === 2) { // Truncate
                const size = sharedBuffer[3];
                (handle as any).truncate(size);
                Atomics.store(sharedBuffer, 0, STATE_DONE);
            }
        } catch (e: any) {
            console.error("I/O Worker Error:", e);
            Atomics.store(sharedBuffer, 0, STATE_ERR);
        }
        Atomics.notify(sharedBuffer, 0, 1);
    }
}

loop();
