<script lang="ts">
    import { onMount, createEventDispatcher } from 'svelte';

    export let workerApi: any;

    const dispatch = createEventDispatcher();
    let files: string[] = [];

    export async function refresh() {
        if (!workerApi) return;
        const result = await workerApi.executeCommand('ls -a');
        if (result) {
            // result is a space-separated string of files from ls
            // We need to clean ANSI escape codes if they are present
            const cleanResult = result.replace(/\x1b\[[0-9;]*m/g, '');
            files = cleanResult.split(/\s+/).filter((f: string) => f.length > 0 && f !== '.' && f !== '..');
        }
    }

    onMount(() => {
        refresh();
        const interval = setInterval(refresh, 5000);
        return () => clearInterval(interval);
    });

    function selectFile(file: string) {
        dispatch('select', { path: file });
    }
</script>

<div class="file-tree-container">
    <div class="header">
        <span>Files</span>
        <button on:click={refresh} title="Refresh">🔄</button>
    </div>
    <ul class="file-list">
        {#each files as file}
            <li on:click={() => selectFile(file)}>
                <span class="icon">📄</span>
                <span class="name">{file}</span>
            </li>
        {/each}
        {#if files.length === 0}
            <li class="empty">No files found</li>
        {/if}
    </ul>
</div>

<style>
    .file-tree-container {
        display: flex;
        flex-direction: column;
        height: 100%;
        background: #1e1e1e;
        color: #ccc;
        font-size: 14px;
        border-right: 1px solid #333;
    }
    .header {
        padding: 10px;
        background: #252525;
        display: flex;
        justify-content: space-between;
        align-items: center;
        font-weight: bold;
        text-transform: uppercase;
        font-size: 11px;
        letter-spacing: 1px;
    }
    .header button {
        background: none;
        border: none;
        color: #888;
        cursor: pointer;
        padding: 2px 5px;
    }
    .header button:hover {
        color: white;
    }
    .file-list {
        list-style: none;
        padding: 0;
        margin: 0;
        overflow-y: auto;
    }
    .file-list li {
        padding: 6px 12px;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 8px;
        border-bottom: 1px solid #252525;
    }
    .file-list li:hover {
        background: #2a2d2e;
        color: white;
    }
    .icon {
        font-size: 12px;
    }
    .empty {
        padding: 20px;
        text-align: center;
        font-style: italic;
        color: #666;
    }
</style>
