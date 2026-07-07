<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { PDFDocument, rgb } from 'pdf-lib';
    import svgPanZoom from 'svg-pan-zoom';

    export let content = '';
    export let type: 'mermaid' | 'pdf' | 'text' = 'text';
    export let theme: 'light' | 'dark' = 'dark';

    let previewElement: HTMLElement;
    let svgContainer: HTMLElement;
    let mermaidWorker: Worker;
    let debounceTimer: any;
    let renderId = 0;
    let errorMessage = '';
    let isFullscreen = false;
    let panZoomInstance: any = null;

    $: if (previewElement && type === 'mermaid' && content) {
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(renderMermaid, 500);
    } else {
        errorMessage = '';
    }

    // Re-render when theme changes if currently displaying mermaid
    $: if (theme && type === 'mermaid' && content && previewElement) {
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(renderMermaid, 100);
    }

    $: if (type === 'pdf' && previewElement) {
        generatePdf();
    }

    async function generatePdf() {
        try {
            const pdfDoc = await PDFDocument.create();
            const page = pdfDoc.addPage();

            // Draw some basic content
            page.drawText('iOS PWA Tool PDF Generator', {
                x: 50,
                y: page.getHeight() - 100,
                size: 24,
                color: rgb(0, 0, 0),
            });

            const lines = content.split('\n');
            let y = page.getHeight() - 150;
            for (let i = 0; i < Math.min(lines.length, 30); i++) {
                page.drawText(lines[i] || '', { x: 50, y, size: 12 });
                y -= 20;
            }

            const pdfBytes = await pdfDoc.save();
            const blob = new Blob([pdfBytes], { type: 'application/pdf' });
            const url = URL.createObjectURL(blob);
            previewElement.innerHTML = `<iframe src="${url}" width="100%" height="100%" frameborder="0"></iframe>`;
        } catch (e: any) {
            errorMessage = "PDF Generation failed: " + e.message;
        }
    }

    async function renderMermaid() {
        if (!mermaidWorker) return;
        renderId++;
        errorMessage = '';
        mermaidWorker.postMessage({ id: renderId, content, theme: theme === 'dark' ? 'dark' : 'default' });
    }

    onMount(() => {
        mermaidWorker = new Worker(new URL('../mermaid-worker.ts', import.meta.url), { type: 'module' });
        mermaidWorker.onmessage = (e) => {
            if (e.data.id === renderId) {
                if (e.data.error) {
                    errorMessage = e.data.error;
                } else if (e.data.svg && svgContainer) {
                    if (panZoomInstance) {
                        panZoomInstance.destroy();
                        panZoomInstance = null;
                    }
                    svgContainer.innerHTML = e.data.svg;
                    const svgElement = svgContainer.querySelector('svg');
                    if (svgElement) {
                        svgElement.style.width = '100%';
                        svgElement.style.height = '100%';
                        panZoomInstance = svgPanZoom(svgElement, {
                            zoomEnabled: true,
                            controlIconsEnabled: true,
                            fit: true,
                            center: true
                        });
                    }
                }
            }
        };
    });

    onDestroy(() => {
        if (panZoomInstance) panZoomInstance.destroy();
        if (mermaidWorker) mermaidWorker.terminate();
        clearTimeout(debounceTimer);
    });

    function toggleFullscreen() {
        isFullscreen = !isFullscreen;
        setTimeout(() => {
            if (panZoomInstance) {
                panZoomInstance.resize();
                panZoomInstance.fit();
                panZoomInstance.center();
            }
        }, 100);
    }
</script>

<div bind:this={previewElement} class="preview-container {isFullscreen ? 'fullscreen' : ''}">
    {#if type === 'mermaid' && !errorMessage}
        <button class="fullscreen-btn" on:click={toggleFullscreen}>
            {isFullscreen ? 'Exit Fullscreen' : 'Fullscreen'}
        </button>
    {/if}

    {#if errorMessage}
        <div class="error-ui">
            <h3>Mermaid Syntax Error</h3>
            <pre>{errorMessage}</pre>
        </div>
    {/if}
    {#if type === 'mermaid'}
        <div bind:this={svgContainer} class="svg-container"></div>
    {/if}
    {#if type === 'text'}
        <pre>{content}</pre>
    {/if}
</div>

<style>
    .preview-container {
        height: 100%;
        width: 100%;
        overflow: auto;
        padding: 10px;
        background: var(--bg-color, #1e1e1e);
        color: var(--text-color, #eee);
        position: relative;
    }
    .preview-container.fullscreen {
        position: fixed;
        top: 0;
        left: 0;
        z-index: 1000;
    }
    .svg-container {
        width: 100%;
        height: 100%;
    }
    .fullscreen-btn {
        position: absolute;
        top: 10px;
        right: 10px;
        z-index: 10;
        background: #444;
        color: white;
        border: 1px solid #555;
        padding: 5px 10px;
        cursor: pointer;
        border-radius: 4px;
    }
    .error-ui {
        background: #440000;
        color: #ffcccc;
        padding: 15px;
        border: 1px solid #ff0000;
        border-radius: 5px;
        margin-bottom: 15px;
    }
    .error-ui pre {
        white-space: pre-wrap;
        word-break: break-all;
    }
</style>
