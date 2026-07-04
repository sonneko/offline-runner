<script lang="ts">
    import { onMount, createEventDispatcher } from 'svelte';
    import { EditorView, basicSetup } from 'codemirror';
    import { keymap } from '@codemirror/view';
    import { indentWithTab } from '@codemirror/commands';
    import { autocompletion, CompletionContext } from '@codemirror/autocomplete';
    import { oneDark } from '@codemirror/theme-one-dark';
    import { linter, lintGutter } from '@codemirror/lint';
    import type { Diagnostic } from '@codemirror/lint';
    import { mss } from '../lib/mss-lang';

    export let workerApi: any;

    let editorContainer: HTMLElement;
    let view: EditorView;
    let currentPath: string | null = null;
    let openTabs: string[] = [];
    let autoSaveInterval: any;
    const dispatch = createEventDispatcher();

    function initAutoSaveDB(): Promise<IDBDatabase> {
        return new Promise((resolve, reject) => {
            const request = indexedDB.open("EditorAutoSaveDB", 1);
            request.onupgradeneeded = (e: any) => {
                const db = e.target.result;
                if (!db.objectStoreNames.contains("autosave")) {
                    db.createObjectStore("autosave", { keyPath: "path" });
                }
            };
            request.onsuccess = (e: any) => resolve(e.target.result);
            request.onerror = (e) => reject(e);
        });
    }

    async function checkAutoSave(path: string): Promise<string | null> {
        try {
            const db = await initAutoSaveDB();
            return new Promise((resolve) => {
                const tx = db.transaction("autosave", "readonly");
                const store = tx.objectStore("autosave");
                const req = store.get(path);
                req.onsuccess = () => resolve(req.result ? req.result.content : null);
                req.onerror = () => resolve(null);
            });
        } catch {
            return null;
        }
    }

    async function storeAutoSave() {
        if (!currentPath) return;
        try {
            const db = await initAutoSaveDB();
            const tx = db.transaction("autosave", "readwrite");
            const store = tx.objectStore("autosave");
            store.put({ path: currentPath, content: getContent(), timestamp: Date.now() });
        } catch (e) {
            console.warn("Autosave failed", e);
        }
    }

    async function clearAutoSave(path: string) {
        try {
            const db = await initAutoSaveDB();
            const tx = db.transaction("autosave", "readwrite");
            const store = tx.objectStore("autosave");
            store.delete(path);
        } catch {}
    }

    function mssCompletions(context: CompletionContext) {
        let word = context.matchBefore(/\w*/);
        if (!word || (word.from == word.to && !context.explicit))
            return null;

        const builtins = [
            "echo", "ls", "cat", "grep", "find", "head", "tail", "pwd", "cd", "touch", "stat", "xargs",
            "sleep", "http_get", "json_parse"
        ];

        return {
            from: word.from,
            options: builtins.map(b => ({ label: b, type: "function" }))
        };
    }

    const mssLinter = linter(async (view) => {
        if (!workerApi) return [];
        const code = view.state.doc.toString();
        const result = await workerApi.lintMss(code);
        let diagnostics: Diagnostic[] = [];

        if (result && result.startsWith("Error:")) {
            diagnostics.push({
                from: 0,
                to: code.length,
                severity: "error",
                message: result
            });
        }
        return diagnostics;
    });

    onMount(() => {
        view = new EditorView({
            doc: '',
            extensions: [
                basicSetup,
                lintGutter(),
                mssLinter,
                keymap.of([
                    indentWithTab,
                    {
                        key: "Mod-s",
                        run: () => {
                            saveFile();
                            return true;
                        }
                    },
                    {
                        key: "F12",
                        run: () => {
                            jumpToDefinition();
                            return true;
                        }
                    }
                ]),
                autocompletion({ override: [mssCompletions] }),
                oneDark,
                mss()
            ],
            parent: editorContainer
        });

        autoSaveInterval = setInterval(storeAutoSave, 5000); // Autosave every 5 seconds

        return () => {
            clearInterval(autoSaveInterval);
            view.destroy();
        };
    });

    function jumpToDefinition() {
        const state = view.state;
        const selection = state.selection.main;
        const line = state.doc.lineAt(selection.head);

        // Very basic word extraction at cursor
        const wordMatch = line.text.substring(0, selection.head - line.from).match(/([a-zA-Z0-9_]+)$/);
        const wordMatchForward = line.text.substring(selection.head - line.from).match(/^([a-zA-Z0-9_]+)/);

        let word = '';
        if (wordMatch) word += wordMatch[1];
        if (wordMatchForward) word += wordMatchForward[1];

        if (word) {
            const text = state.doc.toString();
            // Look for 'function WORD' or 'WORD =' patterns in MSS
            const regex = new RegExp(`(?:function\\s+${word}\\s*\\(|${word}\\s*=)`, 'g');
            const match = regex.exec(text);

            if (match) {
                view.dispatch({
                    selection: { anchor: match.index, head: match.index },
                    scrollIntoView: true
                });
            }
        }
    }

    export function getContent() {
        return view.state.doc.toString();
    }

    export function setContent(content: string) {
        view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: content }
        });
    }

    export async function loadFile(path: string) {
        if (!workerApi) return;
        currentPath = path;

        if (!openTabs.includes(path)) {
            openTabs = [...openTabs, path];
        }

        // Check for autosaved version
        const autosavedContent = await checkAutoSave(path);

        // Use stat to get file size
        const statResult = await workerApi.executeCommand(`stat "${path}"`);
        const sizeMatch = statResult.match(/Size:\s+(\d+)/);
        let size = 0;
        if (sizeMatch && sizeMatch[1]) {
            size = parseInt(sizeMatch[1], 10);
        }

        let actualContent = '';

        // Lazy load for large files (e.g. > 500KB)
        if (size > 500 * 1024) {
            const headResult = await workerApi.executeCommand(`head -n 1000 "${path}"`);
            if (!headResult.startsWith('head: ')) {
                actualContent = `// Large file lazy loaded (First 1000 lines). Total size: ${size} bytes.\n// Editing large files is limited in this view.\n\n` + headResult;
            } else {
                console.error(headResult);
            }
        } else {
            const result = await workerApi.executeCommand(`cat "${path}"`);
            if (!result.startsWith('cat: ')) {
                actualContent = result;
            } else {
                console.error(result);
            }
        }

        if (autosavedContent && autosavedContent !== actualContent) {
            if (confirm(`An unsaved version of ${path} was found. Do you want to restore it?`)) {
                setContent(autosavedContent);
                return;
            } else {
                clearAutoSave(path);
            }
        }

        setContent(actualContent);
    }

    export async function saveFile() {
        if (!workerApi) {
             alert("Worker not ready");
             return;
        }

        let path = currentPath;
        if (!path) {
            path = prompt("Enter filename to save:", "script.mss");
            if (!path) return;
            currentPath = path;
        }

        const content = getContent();
        if (content.startsWith('// Large file lazy loaded')) {
            alert("Saving large files loaded in lazy mode is not supported to prevent data loss.");
            return;
        }

        // Use the 'write' command we implemented in the backend
        const result = await workerApi.executeCommand(`write "${path}" "${content}"`);
        console.log(result);
        clearAutoSave(path); // Clear autosave on successful manual save
        dispatch('save', { path });
    }

    export function newFile() {
        currentPath = null;
        setContent('');
    }

    async function switchTab(path: string) {
        if (currentPath === path) return;
        await storeAutoSave(); // Save current state before switching
        await loadFile(path);
    }

    async function closeTab(path: string, event: Event) {
        event.stopPropagation();
        openTabs = openTabs.filter(t => t !== path);
        if (currentPath === path) {
            if (openTabs.length > 0) {
                await switchTab(openTabs[openTabs.length - 1]);
            } else {
                newFile();
            }
        }
    }
</script>

<div class="editor-wrapper">
    {#if openTabs.length > 0}
    <div class="tabs">
        {#each openTabs as tab}
            <div class="tab" class:active={tab === currentPath} on:click={() => switchTab(tab)} on:keydown={(e) => e.key === 'Enter' && switchTab(tab)} role="button" tabindex="0">
                <span class="tab-title">{tab.split('/').pop()}</span>
                <span class="tab-close" on:click={(e) => closeTab(tab, e)} on:keydown={(e) => e.key === 'Enter' && closeTab(tab, e)} role="button" tabindex="0">×</span>
            </div>
        {/each}
    </div>
    {/if}
    <div class="toolbar">
        <span class="filename">{currentPath || 'Untitled'}</span>
        <div class="actions">
            <button on:click={newFile}>New</button>
            <button on:click={saveFile}>Save</button>
        </div>
    </div>
    <div bind:this={editorContainer} class="cm-editor-container"></div>
</div>

<style>
    .editor-wrapper {
        display: flex;
        flex-direction: column;
        height: 100%;
        width: 100%;
    }
    .tabs {
        display: flex;
        background: #1e1e1e;
        border-bottom: 1px solid #333;
        overflow-x: auto;
    }
    .tab {
        padding: 5px 10px;
        background: #2a2a2a;
        border-right: 1px solid #333;
        display: flex;
        align-items: center;
        gap: 8px;
        cursor: pointer;
        font-size: 13px;
        color: #aaa;
    }
    .tab.active {
        background: #3a3a3a;
        color: white;
    }
    .tab-close {
        font-size: 16px;
        line-height: 1;
    }
    .tab-close:hover {
        color: #ff5555;
    }
    .toolbar {
        height: 30px;
        background: #252525;
        border-bottom: 1px solid #333;
        display: flex;
        align-items: center;
        padding: 0 10px;
        justify-content: space-between;
        font-size: 12px;
        color: #aaa;
    }
    .actions {
        display: flex;
        gap: 5px;
    }
    .actions button {
        background: #333;
        color: #ccc;
        border: 1px solid #444;
        padding: 2px 8px;
        border-radius: 3px;
        cursor: pointer;
    }
    .actions button:hover {
        background: #444;
        color: white;
    }
    .cm-editor-container {
        flex: 1;
        overflow: hidden;
    }
    :global(.cm-editor) {
        height: 100%;
    }
</style>
