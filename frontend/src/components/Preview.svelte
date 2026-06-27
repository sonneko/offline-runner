<script lang="ts">
    import { onMount } from 'svelte';
    import mermaid from 'mermaid';

    export let content = '';
    export let type: 'mermaid' | 'pdf' | 'text' = 'text';

    let previewElement: HTMLElement;

    $: if (previewElement && type === 'mermaid' && content) {
        renderMermaid();
    }

    async function renderMermaid() {
        previewElement.innerHTML = '';
        const { svg } = await mermaid.render('mermaid-svg', content);
        previewElement.innerHTML = svg;
    }

    onMount(() => {
        mermaid.initialize({ startOnLoad: false });
    });
</script>

<div bind:this={previewElement} class="preview-container">
    {#if type === 'text'}
        <pre>{content}</pre>
    {:else if type === 'pdf'}
        <p>PDF Preview Not Implemented yet (requires Blob URL)</p>
    {/if}
</div>

<style>
    .preview-container {
        height: 100%;
        width: 100%;
        overflow: auto;
        padding: 10px;
        background: #fff;
        color: #000;
    }
</style>
