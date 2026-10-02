<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from './lib/state.svelte';
  import { TARGET } from './lib/platform';
  import Gate from './lib/components/Gate.svelte';
  import Outline from './lib/components/Outline.svelte';
  import Inspector from './lib/components/Inspector.svelte';
  import VarPalette from './lib/components/VarPalette.svelte';
  import Preview from './lib/components/Preview.svelte';
  import IssuesPanel from './lib/components/IssuesPanel.svelte';
  import CodeMode from './lib/components/CodeMode.svelte';

  let newMenu = $state(false);

  onMount(() => {
    app.init();
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const k = e.key.toLowerCase();
      if (k === 's') {
        e.preventDefault();
        if (app.mode === 'gui') app.save(e.shiftKey);
      } else if (k === 'z' && app.mode === 'gui' && !(e.target as HTMLElement)?.closest('.cm-editor')) {
        e.preventDefault();
        e.shiftKey ? app.redo() : app.undo();
      } else if (k === 'y' && app.mode === 'gui' && !(e.target as HTMLElement)?.closest('.cm-editor')) {
        e.preventDefault();
        app.redo();
      }
    };
    window.addEventListener('keydown', onKey);
    const onUnload = (e: BeforeUnloadEvent) => {
      if (app.dirty && TARGET === 'desktop') e.preventDefault();
    };
    window.addEventListener('beforeunload', onUnload);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('beforeunload', onUnload);
    };
  });

  async function newDoc(t: string) {
    newMenu = false;
    if (app.dirty && !confirm('保存していない変更があります。破棄して新規作成しますか？')) return;
    await app.newDocument(t);
  }
</script>

<div class="app">
  <header class="toolbar">
    <span class="brand">formDoc</span>
    <span class="target small">{TARGET === 'desktop' ? 'デスクトップ版' : 'Web版'}</span>

    <div class="modes" role="tablist">
      <button role="tab" class:on={app.mode === 'gui'} onclick={() => app.enterGuiMode()}>部品で作成</button>
      <button role="tab" class:on={app.mode === 'code'} onclick={() => app.enterCodeMode()}>コードで作成（Typst）</button>
    </div>

    {#if app.mode === 'gui'}
      <div class="group">
        <span class="new">
          <button onclick={() => (newMenu = !newMenu)}>新規</button>
          {#if newMenu}
            <div class="menu">
              {#each app.catalog?.templates ?? [] as t}
                <button class="ghost" onclick={() => newDoc(t.id)} title={t.description}>{t.name}</button>
              {/each}
            </div>
          {/if}
        </span>
        <button onclick={() => app.open()}>開く…</button>
        <button onclick={() => app.save()} title="Ctrl+S">保存{app.dirty ? ' *' : ''}</button>
        <Gate cap="nativeSaveDialog"><button onclick={() => app.save(true)} title="Ctrl+Shift+S">名前を付けて保存…</button></Gate>
      </div>
      <div class="group">
        <button onclick={() => app.undo()} title="元に戻す (Ctrl+Z)">↶</button>
        <button onclick={() => app.redo()} title="やり直し (Ctrl+Y)">↷</button>
      </div>
      <div class="group">
        <button onclick={() => app.convertToCode()} title="この文書を、同じ体裁のTypstコードに変換してコードモードで開きます">Typstに変換</button>
        <button onclick={() => app.exportTypst()}>Typst書き出し</button>
      </div>
    {/if}

    <span class="spacer"></span>
    {#if app.filePath}<span class="path small muted" title={app.filePath}>{app.filePath}</span>{/if}
    <button class="primary" disabled={!app.result?.exportable} onclick={() => app.exportPdf()}
      title={app.result?.exportable ? 'PDFを出力' : 'エラーを解消するとPDFを出力できます'}>PDF出力</button>
  </header>

  {#if app.loading}
    <div class="loading">{app.loading}</div>
  {:else}
    <main class:code={app.mode === 'code'}>
      {#if app.mode === 'gui'}
        <aside class="left"><Outline /></aside>
        <section class="center">
          <div class="inspector"><Inspector /></div>
          <div class="vars"><VarPalette /></div>
        </section>
      {:else}
        <section class="codepane"><CodeMode /></section>
      {/if}
      <section class="right"><Preview /></section>
      <footer class="bottom"><IssuesPanel /></footer>
    </main>
  {/if}

  {#if app.message}
    <div class="toast {app.message.kind}" role="status">{app.message.text}</div>
  {/if}
</div>

<style>
  .app { display: flex; flex-direction: column; height: 100vh; }
  .toolbar {
    display: flex; align-items: center; gap: 10px; padding: 6px 10px; background: var(--panel);
    border-bottom: 1px solid var(--line); flex-wrap: wrap;
  }
  .brand { font-weight: 800; letter-spacing: 0.04em; color: var(--accent); }
  .target { color: var(--muted); border: 1px solid var(--line); border-radius: 10px; padding: 0 8px; }
  .modes { display: inline-flex; border: 1px solid var(--line-strong); border-radius: var(--radius); overflow: hidden; }
  .modes button { border: none; border-radius: 0; }
  .modes button.on { background: var(--accent); color: #fff; }
  .group { display: inline-flex; gap: 4px; padding-left: 10px; border-left: 1px solid var(--line); }
  .new { position: relative; }
  .menu {
    position: absolute; top: 110%; left: 0; z-index: 30; background: var(--panel); border: 1px solid var(--line-strong);
    border-radius: var(--radius); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15); padding: 4px; display: flex; flex-direction: column; min-width: 140px;
  }
  .menu button { text-align: left; }
  .spacer { flex: 1; }
  .path { max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .loading { flex: 1; display: grid; place-items: center; color: var(--muted); }

  main {
    flex: 1; min-height: 0; display: grid;
    grid-template-columns: 280px minmax(320px, 1fr) minmax(420px, 1.3fr);
    grid-template-rows: 1fr 170px;
    grid-template-areas: 'left center right' 'bottom bottom bottom';
  }
  main.code { grid-template-columns: minmax(420px, 1fr) minmax(420px, 1fr); grid-template-areas: 'codepane right' 'bottom bottom'; }
  .left { grid-area: left; background: var(--panel); border-right: 1px solid var(--line); min-height: 0; }
  .center { grid-area: center; display: grid; grid-template-rows: 1fr 200px; min-height: 0; border-right: 1px solid var(--line); background: var(--panel); }
  .inspector { min-height: 0; }
  .vars { border-top: 1px solid var(--line); min-height: 0; }
  .codepane { grid-area: codepane; min-height: 0; background: var(--panel); border-right: 1px solid var(--line); }
  .right { grid-area: right; min-height: 0; }
  .bottom { grid-area: bottom; background: var(--panel); border-top: 1px solid var(--line); min-height: 0; }

  .toast {
    position: fixed; bottom: 190px; left: 50%; transform: translateX(-50%); padding: 8px 16px; border-radius: var(--radius);
    background: #263238; color: #fff; box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25); z-index: 50; max-width: 70vw;
  }
  .toast.error { background: var(--error); }
</style>
