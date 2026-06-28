<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from 'codemirror';
    import { javascript } from '@codemirror/lang-javascript';
    import { oneDark } from '@codemirror/theme-one-dark';

    let editorElement: HTMLElement;
    let view: EditorView;

    onMount(() => {
        view = new EditorView({
            doc: '// Write your Mini-ShellScript here\n@ls\n',
            extensions: [
                basicSetup,
                javascript(),
                oneDark
            ],
            parent: editorElement
        });
    });

    onDestroy(() => {
        if (view) view.destroy();
    });

    export function getContent() {
        return view.state.doc.toString();
    }
</script>

<div bind:this={editorElement} class="editor-container"></div>

<style>
    .editor-container {
        height: 100%;
        width: 100%;
    }
</style>
