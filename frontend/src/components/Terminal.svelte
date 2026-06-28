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
                case '\t': // Tab
                    const lastArg = input.split(' ').pop() || '';
                    if (lastArg.length > 0) {
                        const filesStr = await workerApi.executeCommand("_list_files");
                        const files = filesStr.split('\n');
                        const matches = files.filter((f: string) => f.startsWith(lastArg));
                        if (matches.length === 1) {
                            const completion = matches[0].slice(lastArg.length);
                            input += completion;
                            term.write(completion);
                        } else if (matches.length > 1) {
                            term.write('\r\n' + matches.join('  ') + '\r\n$ ' + input);
                        }
                    }
                    break;
                case '\r': // Enter
                    term.write('\r\n');
                    if (input.trim()) {
                        const result = await workerApi.executeCommand(input.trim());
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

<div class="terminal-wrapper">
    <div bind:this={terminalElement} class="terminal-container"></div>
    <div class="virtual-keys">
        <button on:click={() => term.focus()}>Focus</button>
        <button on:click={() => term.onData('\x1b')}>Esc</button>
        <button on:click={() => term.onData('\t')}>Tab</button>
        <button on:click={() => term.onData('\x03')}>Ctrl+C</button>
    </div>
</div>

<style>
    .terminal-wrapper {
        display: flex;
        flex-direction: column;
        height: 100%;
        width: 100%;
    }
    .terminal-container {
        flex: 1;
        width: 100%;
        background: #1e1e1e;
    }
    .virtual-keys {
        height: 40px;
        background: #252525;
        display: flex;
        align-items: center;
        padding: 0 10px;
        gap: 5px;
        border-top: 1px solid #333;
    }
    .virtual-keys button {
        background: #444;
        color: white;
        border: none;
        padding: 4px 8px;
        border-radius: 3px;
        font-size: 12px;
    }
</style>
