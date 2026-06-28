<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from 'codemirror';
    import { mssLanguage } from '../lib/mss-lang';
    import { oneDark } from '@codemirror/theme-one-dark';

    let editorElement: HTMLElement;
    let view: EditorView;

    onMount(() => {
        view = new EditorView({
            doc: '// Write your Mini-ShellScript here\n@ls\n',
            extensions: [
                basicSetup,
                mssLanguage,
                oneDark
            ],
            parent: editorElement
        });
    });

    onDestroy(() => {
        if (view) view.destroy();
    });

    export let workerApi: any;
    let currentPath = 'untitled.mss';

    export function getContent() {
        return view.state.doc.toString();
    }

    export async function saveFile() {
        if (!workerApi) return;
        const code = getContent();
        const res = await workerApi.executeCommand(`write "${currentPath}" "${code}"`);
        console.log("Save result:", res);
    }

    onMount(() => {
        window.addEventListener('keydown', (e) => {
            if ((e.metaKey || e.ctrlKey) && e.key === 's') {
                e.preventDefault();
                saveFile();
            }
        });
    });
</script>

<div class="editor-wrapper">
    <div class="editor-toolbar">
        <span>{currentPath}</span>
        <button on:click={saveFile}>Save</button>
    </div>
    <div bind:this={editorElement} class="editor-container"></div>
</div>

<style>
    .editor-wrapper {
        display: flex;
        flex-direction: column;
        height: 100%;
        width: 100%;
    }
    .editor-toolbar {
        height: 30px;
        background: #252525;
        display: flex;
        align-items: center;
        padding: 0 10px;
        font-size: 12px;
        border-bottom: 1px solid #333;
        gap: 10px;
    }
    .editor-toolbar button {
        background: #444;
        color: white;
        border: none;
        padding: 2px 8px;
        border-radius: 3px;
        cursor: pointer;
    }
    .editor-container {
        flex: 1;
        width: 100%;
        overflow: hidden;
    }
</style>
