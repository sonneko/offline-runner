<script lang="ts">
    import { onMount, createEventDispatcher } from 'svelte';
    import { EditorView, basicSetup } from 'codemirror';
    import { keymap } from '@codemirror/view';
    import { indentWithTab } from '@codemirror/commands';
    import { oneDark } from '@codemirror/theme-one-dark';
    import { mss } from '../lib/mss-lang';

    export let workerApi: any;

    let editorContainer: HTMLElement;
    let view: EditorView;
    let currentPath: string | null = null;
    const dispatch = createEventDispatcher();

    onMount(() => {
        view = new EditorView({
            doc: '',
            extensions: [
                basicSetup,
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
                oneDark,
                mss()
            ],
            parent: editorContainer
        });

        return () => view.destroy();
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

        // Use stat to get file size
        const statResult = await workerApi.executeCommand(`stat "${path}"`);
        const sizeMatch = statResult.match(/Size:\s+(\d+)/);
        let size = 0;
        if (sizeMatch && sizeMatch[1]) {
            size = parseInt(sizeMatch[1], 10);
        }

        // Lazy load for large files (e.g. > 500KB)
        if (size > 500 * 1024) {
            const headResult = await workerApi.executeCommand(`head -n 1000 "${path}"`);
            if (!headResult.startsWith('head: ')) {
                setContent(`// Large file lazy loaded (First 1000 lines). Total size: ${size} bytes.\n// Editing large files is limited in this view.\n\n` + headResult);
            } else {
                console.error(headResult);
            }
        } else {
            const result = await workerApi.executeCommand(`cat "${path}"`);
            if (!result.startsWith('cat: ')) {
                setContent(result);
            } else {
                console.error(result);
            }
        }
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
        // Use the 'write' command we implemented in the backend
        const result = await workerApi.executeCommand(`write "${path}" "${content}"`);
        console.log(result);
        dispatch('save', { path });
    }

    export function newFile() {
        currentPath = null;
        setContent('');
    }
</script>

<div class="editor-wrapper">
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
