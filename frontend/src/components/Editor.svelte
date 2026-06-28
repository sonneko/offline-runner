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
                    }
                ]),
                oneDark,
                mss()
            ],
            parent: editorContainer
        });

        return () => view.destroy();
    });

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
        const result = await workerApi.executeCommand(`cat "${path}"`);
        if (!result.startsWith('cat: ')) {
            setContent(result);
        } else {
            console.error(result);
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
