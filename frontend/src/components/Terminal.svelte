<script lang="ts">
    import { onMount, createEventDispatcher } from 'svelte';
    import { Terminal } from 'xterm';
    import { FitAddon } from 'xterm-addon-fit';
    import 'xterm/css/xterm.css';

    export let workerApi: any;

    let terminalElement: HTMLElement;
    let term: Terminal;
    let fitAddon: FitAddon;
    let fontSize = 14;
    let isDarkMode = true;
    let input = '';
    let history: string[] = [];
    let historyIndex = -1;
    let promptString = '$ ';
    const dispatch = createEventDispatcher();

    export function printLog(message: string) {
        term.writeln('\r\n' + message);
        term.write(promptString + input);
    }

    export function setPrompt(newPrompt: string) {
        promptString = newPrompt;
    }

    async function loadHistory() {
        return new Promise<string[]>((resolve) => {
            const request = indexedDB.open("terminalHistory", 1);
            request.onupgradeneeded = (e: any) => {
                const db = e.target.result;
                if (!db.objectStoreNames.contains("history")) {
                    db.createObjectStore("history", { autoIncrement: true });
                }
            };
            request.onsuccess = (e: any) => {
                const db = e.target.result;
                const transaction = db.transaction("history", "readonly");
                const store = transaction.objectStore("history");
                const getRequest = store.getAll();
                getRequest.onsuccess = () => resolve(getRequest.result);
            };
            request.onerror = () => resolve([]);
        });
    }

    async function saveToHistory(cmd: string) {
        const request = indexedDB.open("terminalHistory", 1);
        request.onsuccess = (e: any) => {
            const db = e.target.result;
            const transaction = db.transaction("history", "readwrite");
            const store = transaction.objectStore("history");
            store.add(cmd);
        };
    }

    async function handleTab() {
        if (!workerApi) return;

        const lastSpaceIndex = input.lastIndexOf(' ');
        const currentWord = lastSpaceIndex === -1 ? input : input.slice(lastSpaceIndex + 1);

        if (currentWord.length === 0) return;

        const lsResult = await workerApi.executeCommand('ls -a');
        const cleanResult = lsResult.replace(/\x1b\[[0-9;]*m/g, '');
        const files = cleanResult.split(/\s+/).filter((f: string) => f.length > 0 && f !== '.' && f !== '..');

        const matches = files.filter((f: string) => f.startsWith(currentWord));

        if (matches.length === 1) {
            const completion = matches[0].slice(currentWord.length);
            input += completion;
            term.write(completion);
        } else if (matches.length > 1) {
            term.write('\r\n' + matches.join('  ') + '\r\n' + promptString + input);
        }
    }

    onMount(async () => {
        history = await loadHistory();
        term = new Terminal({
            cursorBlink: true,
            scrollback: 10000,
            fontSize: fontSize,
            theme: {
                background: '#1e1e1e',
                foreground: '#ffffff'
            },
            macOptionIsMeta: true,
            rightClickSelectsWord: true
        });
        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(terminalElement);
        fitAddon.fit();

        term.onSelectionChange(() => {
            if (term.hasSelection() && navigator.clipboard) {
                navigator.clipboard.writeText(term.getSelection()).catch(() => {});
            }
        });

        window.addEventListener('resize', () => {
            fitAddon.fit();
        });

        term.writeln('Welcome to iOS PWA Tool CLI');
        term.write('\r\n' + promptString);

        term.onData(async e => {
            switch (e) {
                case '\x1b[A': // Up arrow
                    if (history.length > 0 && historyIndex < history.length - 1) {
                        historyIndex++;
                        term.write('\b \b'.repeat(input.length));
                        input = history[history.length - 1 - historyIndex];
                        term.write(input);
                    }
                    break;
                case '\x1b[B': // Down arrow
                    if (historyIndex > 0) {
                        historyIndex--;
                        term.write('\b \b'.repeat(input.length));
                        input = history[history.length - 1 - historyIndex];
                        term.write(input);
                    } else if (historyIndex === 0) {
                        historyIndex = -1;
                        term.write('\b \b'.repeat(input.length));
                        input = '';
                    }
                    break;
                case '\r': // Enter
                    term.write('\r\n');
                    if (input.trim()) {
                        const cmd = input.trim();
                        history.push(cmd);
                        saveToHistory(cmd);
                        historyIndex = -1;
                        const result = await workerApi.executeCommand(cmd);
                        if (result) {
                            term.writeln(result);
                        }
                        dispatch('commandExecuted');
                    }

                    // Check if prompt was changed via env
                    try {
                        const newPrompt = await workerApi.executeCommand('echo $PROMPT');
                        if (newPrompt && newPrompt.trim() !== '') {
                            promptString = newPrompt.trim() + ' ';
                        }
                    } catch (e) {}

                    input = '';
                    term.write(promptString);
                    break;
                case '\u007F': // Backspace
                    if (input.length > 0) {
                        input = input.slice(0, -1);
                        term.write('\b \b');
                    }
                    break;
                case '\t': // Tab
                    await handleTab();
                    break;
                default:
                    if (e >= ' ' || e === '\x1b') { // Basic filter for printable chars or escape sequences
                         if (e.length === 1) {
                            input += e;
                            term.write(e);
                         }
                    }
            }
        });
    });

    function changeFontSize(delta: number) {
        fontSize = Math.max(8, Math.min(32, fontSize + delta));
        term.options.fontSize = fontSize;
        setTimeout(() => fitAddon.fit(), 10);
    }

    function toggleTheme() {
        isDarkMode = !isDarkMode;
        term.options.theme = isDarkMode ? {
            background: '#1e1e1e',
            foreground: '#ffffff'
        } : {
            background: '#ffffff',
            foreground: '#000000',
            cursor: '#000000'
        };
    }
</script>

<div class="terminal-wrapper">
    <div bind:this={terminalElement} class="terminal-container"></div>
    <div class="virtual-keys" on:touchstart|preventDefault={() => {}}>
        <button on:touchstart|preventDefault={() => term.focus()} on:click={() => term.focus()}>Focus</button>
        <button on:touchstart|preventDefault={() => term.onData('\x1b')} on:click={() => term.onData('\x1b')}>Esc</button>
        <button on:touchstart|preventDefault={() => term.onData('\t')} on:click={() => term.onData('\t')}>Tab</button>
        <button on:touchstart|preventDefault={() => { workerApi.interrupt(); term.write('^C\r\n' + promptString); input = ''; }} on:click={() => { workerApi.interrupt(); term.write('^C\r\n' + promptString); input = ''; }}>Ctrl+C</button>
        <div class="spacer"></div>
        <button on:touchstart|preventDefault={() => changeFontSize(-1)} on:click={() => changeFontSize(-1)}>A-</button>
        <button on:touchstart|preventDefault={() => changeFontSize(1)} on:click={() => changeFontSize(1)}>A+</button>
        <button on:touchstart|preventDefault={toggleTheme} on:click={toggleTheme}>Theme</button>
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
        padding: 8px 12px;
        border-radius: 4px;
        font-size: 14px;
        touch-action: manipulation; /* Prevent double-tap zoom for better tap response */
    }
    .virtual-keys button:active {
        background: #555;
    }
    .spacer {
        flex: 1;
    }
</style>
