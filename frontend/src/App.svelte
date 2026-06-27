<script lang="ts">
  import * as Comlink from 'comlink';
  import { onMount } from 'svelte';
  import Terminal from './components/Terminal.svelte';
  import Editor from './components/Editor.svelte';
  import Preview from './components/Preview.svelte';

  let workerApi: any;
  let editor: Editor;
  let previewContent = 'Welcome! Run a command or write a script.';
  let previewType: 'mermaid' | 'pdf' | 'text' = 'text';

  onMount(async () => {
    const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    workerApi = Comlink.wrap(worker);
    await workerApi.init();
  });

  async function runScript() {
    if (editor && workerApi) {
      const code = editor.getContent();
      const result = await workerApi.runMss(code);
      previewContent = result;
      previewType = 'text';
    }
  }

  function showMermaid() {
      previewContent = `graph TD
    A[Start] --> B{Is it working?}
    B -- Yes --> C[Great!]
    B -- No --> D[Fix it]`;
      previewType = 'mermaid';
  }
</script>

<main>
  <div class="top-bar">
    <button on:click={runScript}>Run MSS</button>
    <button on:click={showMermaid}>Demo Mermaid</button>
  </div>
  <div class="container">
    <div class="pane editor-pane">
      <Editor bind:this={editor} />
    </div>
    <div class="pane preview-pane">
      <Preview content={previewContent} type={previewType} />
    </div>
    <div class="pane terminal-pane">
      {#if workerApi}
        <Terminal {workerApi} />
      {/if}
    </div>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: sans-serif;
  }
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
  }
  .top-bar {
    height: 40px;
    background: #333;
    display: flex;
    align-items: center;
    padding: 0 10px;
    gap: 10px;
  }
  .container {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .pane {
    flex: 1;
    border: 1px solid #444;
    overflow: hidden;
  }
  .terminal-pane {
    background: #1e1e1e;
  }
</style>
