import * as Comlink from 'comlink';
import init, { execute_command, run_mss, init_vfs } from '../../engine/pkg/engine.js';

const api = {
    async init() {
        await init();
        init_vfs();
        return "Wasm Initialized";
    },
    async executeCommand(cmd: string, args: string[]) {
        try {
            return await execute_command(cmd, args);
        } catch (e) {
            return `Error: ${e}`;
        }
    },
    async runMss(code: string) {
        return run_mss(code);
    },
    async saveToCache(key: string, data: string) {
        const path = `.cache/mermaid/${key}.svg`;
        try {
            // We can use the write command we'll add to execute_command
            // Or use direct VFS access if exposed
            return await execute_command("write", [path, data]);
        } catch (e) {
            return `Cache Error: ${e}`;
        }
    }
};

Comlink.expose(api);
