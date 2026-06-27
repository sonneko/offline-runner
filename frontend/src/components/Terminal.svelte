<script lang="ts">
    import { onMount } from 'svelte';
    import { Terminal } from 'xterm';
    import { FitAddon } from 'xterm-addon-fit';
    import 'xterm/css/xterm.css';

    export let workerApi: any;

    let terminalElement: HTMLElement;
    let term: Terminal;
    let input = '';

    onMount(() => {
        term = new Terminal({
            cursorBlink: true,
            theme: {
                background: '#1e1e1e'
            }
        });
        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(terminalElement);
        fitAddon.fit();

        term.writeln('Welcome to iOS PWA Tool CLI');
        term.write('\r\n$ ');

        term.onData(async e => {
            switch (e) {
                case '\r': // Enter
                    term.write('\r\n');
                    if (input.trim()) {
                        const parts = input.trim().split(' ');
                        const cmd = parts[0];
                        const args = parts.slice(1);
                        const result = await workerApi.executeCommand(cmd, args);
                        term.writeln(result);
                    }
                    input = '';
                    term.write('$ ');
                    break;
                case '\u007F': // Backspace
                    if (input.length > 0) {
                        input = input.slice(0, -1);
                        term.write('\b \b');
                    }
                    break;
                default:
                    input += e;
                    term.write(e);
            }
        });
    });
</script>

<div bind:this={terminalElement} class="terminal-container"></div>

<style>
    .terminal-container {
        height: 100%;
        width: 100%;
        background: #1e1e1e;
    }
</style>
