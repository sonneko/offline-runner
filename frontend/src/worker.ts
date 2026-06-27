import * as Comlink from 'comlink';
import init, { execute_command, run_mss, init_vfs } from '../../engine/pkg/engine.js';

const api = {
    async init() {
        await init();
        init_vfs();
        return "Wasm Initialized";
    },
    async executeCommand(cmd: string, args: string[]) {
        return execute_command(cmd, args);
    },
    async runMss(code: string) {
        return run_mss(code);
    }
};

Comlink.expose(api);
