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
  let showCommandPalette = false;
  let commandInput = '';
  let commandInputEl: HTMLInputElement;

  $: if (showCommandPalette && commandInputEl) {
      commandInputEl.focus();
  }

  onMount(async () => {
    const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    workerApi = Comlink.wrap(worker);
    await workerApi.init();

    window.addEventListener('keydown', (e) => {
        if ((e.metaKey || e.ctrlKey) && e.key === 'p') {
            e.preventDefault();
            showCommandPalette = !showCommandPalette;
        }
    });
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

  async function handleCommand(e: KeyboardEvent) {
      if (e.key === 'Enter' && workerApi) {
          const parts = commandInput.trim().split(' ');
          const result = await workerApi.executeCommand(parts[0], parts.slice(1));
          previewContent = result;
          previewType = 'text';
          showCommandPalette = false;
          commandInput = '';
      }
  }
</script>

<main>
  <div class="top-bar">
    <button on:click={runScript}>Run MSS</button>
    <button on:click={showMermaid}>Demo Mermaid</button>
    <div class="info">Press Cmd+P for Command Palette</div>
  </div>
  <div class="container">
    <div class="pane file-tree">
        <div class="pane-header">Files</div>
        <div class="tree-content">
            <div>welcome.txt</div>
        </div>
    </div>
    <div class="pane main-content">
        <div class="upper">
            <div class="pane editor-pane">
                <Editor bind:this={editor} />
            </div>
            <div class="pane preview-pane">
                <Preview content={previewContent} type={previewType} />
            </div>
        </div>
        <div class="pane terminal-pane">
            {#if workerApi}
                <Terminal {workerApi} />
            {/if}
        </div>
    </div>
  </div>

  {#if showCommandPalette}
    <div class="command-palette-overlay" on:click={() => showCommandPalette = false}>
        <div class="command-palette" on:click|stopPropagation>
            <input
                type="text"
                placeholder="Type a command..."
                bind:value={commandInput}
                on:keydown={handleCommand}
                bind:this={commandInputEl}
            />
        </div>
    </div>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: sans-serif;
    background: #121212;
    color: #eee;
  }
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
  }
  .top-bar {
    height: 40px;
    background: #252525;
    display: flex;
    align-items: center;
    padding: 0 10px;
    gap: 10px;
    border-bottom: 1px solid #333;
  }
  .info {
      font-size: 12px;
      color: #888;
      margin-left: auto;
  }
  .container {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .pane {
    display: flex;
    flex-direction: column;
    border: 1px solid #333;
  }
  .pane-header {
      padding: 5px 10px;
      background: #333;
      font-size: 12px;
      text-transform: uppercase;
  }
  .file-tree {
      width: 200px;
      background: #1e1e1e;
  }
  .tree-content {
      padding: 10px;
      font-size: 14px;
  }
  .main-content {
      flex: 1;
  }
  .upper {
      flex: 2;
      display: flex;
  }
  .editor-pane, .preview-pane {
      flex: 1;
  }
  .terminal-pane {
      flex: 1;
      background: #1e1e1e;
      border-top: 1px solid #444;
  }
  .command-palette-overlay {
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: rgba(0,0,0,0.5);
      display: flex;
      justify-content: center;
      padding-top: 50px;
      z-index: 100;
  }
  .command-palette {
      width: 500px;
      background: #252525;
      padding: 10px;
      border-radius: 4px;
      box-shadow: 0 4px 10px rgba(0,0,0,0.5);
  }
  .command-palette input {
      width: 100%;
      background: #333;
      border: 1px solid #444;
      color: white;
      padding: 8px;
      outline: none;
  }
</style>
