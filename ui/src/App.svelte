<script lang="ts">
  import { onMount } from 'svelte';
  import { app } from './lib/state.svelte';
  import { TARGET, has } from './lib/platform';
  import Gate from './lib/components/Gate.svelte';
  import Outline from './lib/components/Outline.svelte';
  import Inspector from './lib/components/Inspector.svelte';
  import VarPalette from './lib/components/VarPalette.svelte';
  import Preview from './lib/components/Preview.svelte';
  import IssuesPanel from './lib/components/IssuesPanel.svelte';
  import CodeMode from './lib/components/CodeMode.svelte';
  import InsertDialog from './lib/components/InsertDialog.svelte';
  import SaveTemplateDialog from './lib/components/SaveTemplateDialog.svelte';
  import SettingsDialog from './lib/components/SettingsDialog.svelte';
  import ShapeEditor from './lib/components/ShapeEditor.svelte';

  let recentMenu = $state(false);
  /** 印刷の用紙の大きさ（文書の1ページ目に合わせる） */
  const printSize = $derived(app.result?.pages[0] ? { w: app.result.pages[0].width_pt, h: app.result.pages[0].height_pt } : null);

  const docTitle = $derived(`${app.dirty ? '● ' : ''}${app.doc?.meta.title || '無題'} - formDoc`);
  $effect(() => {
    document.title = docTitle;
  });

  onMount(() => {
    app.init();
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || app.dialog) return;
      const k = e.key.toLowerCase();
      const inCode = !!(e.target as HTMLElement)?.closest('.cm-editor');
      if (k === 's') {
        e.preventDefault();
        app.save(e.shiftKey);
      } else if (k === 'p') {
        // ブラウザ標準の印刷（画面全体）ではなく、文書だけを印刷する
        e.preventDefault();
        app.print();
      } else if (k === 'z' && !inCode) {
        e.preventDefault();
        e.shiftKey ? app.redo() : app.undo();
      } else if (k === 'y' && !inCode) {
        e.preventDefault();
        app.redo();
      }
    };
    window.addEventListener('keydown', onKey);

    // 未保存の変更があるときは閉じさせない（Web：タブ・ウィンドウを閉じる／再読み込み）
    const onUnload = (e: BeforeUnloadEvent) => {
      if (app.dirty) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', onUnload);

    // ブラウザ（WebView2）標準の右クリックメニューは出さない。入力欄では切り取り・貼り付けのために残す
    const onContext = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (!t?.closest('input, textarea, [contenteditable], .cm-editor')) e.preventDefault();
    };
    window.addEventListener('contextmenu', onContext);

    // デスクトップ：ウィンドウを閉じる前に確認する
    let unlistenClose: (() => void) | null = null;
    if (TARGET === 'desktop') {
      (async () => {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const { ask } = await import('@tauri-apps/plugin-dialog');
        const win = getCurrentWindow();
        unlistenClose = await win.onCloseRequested(async (ev) => {
          if (!app.dirty) return;
          ev.preventDefault();
          const ok = await ask('保存していない変更があります。破棄して閉じますか？', { title: 'formDoc', kind: 'warning', okLabel: '破棄して閉じる', cancelLabel: 'キャンセル' });
          if (ok) await win.destroy();
        });
      })();
    }
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('beforeunload', onUnload);
      window.removeEventListener('contextmenu', onContext);
      unlistenClose?.();
    };
  });

  function openRecent(p: string) {
    recentMenu = false;
    app.openPath(p, true);
  }
</script>

<div class="app">
  <header class="toolbar">
    <span class="brand">formDoc</span>
    <span class="target small">{TARGET === 'desktop' ? 'デスクトップ版' : 'Web版'}</span>

    <div class="modes" role="tablist">
      <button role="tab" class:on={app.mode === 'gui'} onclick={() => app.enterGuiMode()} title="部品を並べて編集します">部品で作成</button>
      <button role="tab" class:on={app.mode === 'code'} onclick={() => app.enterCodeMode()} title="同じ文書を Typst のコードとして編集します">コードで作成（Typst）</button>
    </div>

    <!-- どちらのモードでも編集しているのは同じ文書 -->
    <div class="group">
      <button onclick={() => app.newDocument()} title="新しい文書（文書情報でスタイルを選んで始めます）">新規</button>
      <span class="open">
        <button onclick={() => app.open()}>開く…</button><Gate cap="recentFiles"><button class="drop" onclick={() => (recentMenu = !recentMenu)} title="最近使ったファイル">▾</button></Gate>
        {#if recentMenu}
          <div class="menu-backdrop" role="presentation" onmousedown={() => (recentMenu = false)}></div>
          <div class="menu">
            <div class="small muted head">最近使ったファイル</div>
            {#each app.settings.recent as p}
              <button class="ghost" onclick={() => openRecent(p)} title={p}>
                <span class="fname">{p.split(/[\\/]/).pop()}</span><span class="small muted fpath">{p}</span>
              </button>
            {:else}
              <div class="small muted head">まだありません</div>
            {/each}
          </div>
        {/if}
      </span>
      <button onclick={() => app.save()} title="Ctrl+S">保存{app.dirty ? ' *' : ''}</button>
      <Gate cap="nativeSaveDialog"><button onclick={() => app.save(true)} title="Ctrl+Shift+S">名前を付けて保存…</button></Gate>
    </div>
    <div class="group">
      <button onclick={() => app.undo()} title="元に戻す (Ctrl+Z)">↶</button>
      <button onclick={() => app.redo()} title="やり直し (Ctrl+Y)">↷</button>
    </div>

    <span class="spacer"></span>
    {#if app.filePath}<span class="path small muted" title={app.filePath}>{app.filePath}</span>{/if}
    <button onclick={() => (app.dialog = { kind: 'settings' })} title="設定（テーマ・文字の大きさ・最近使ったファイル・テンプレートのフォルダ）">⚙ 設定</button>
    <button disabled={!app.result?.exportable} onclick={() => app.print()} title={app.result?.exportable ? '文書を印刷 (Ctrl+P)' : 'エラーを解消すると印刷できます'}>印刷</button>
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

{#if app.dialog?.kind === 'insert'}
  <InsertDialog />
{:else if app.dialog?.kind === 'saveTemplate'}
  <SaveTemplateDialog blockId={app.dialog.blockId} />
{:else if app.dialog?.kind === 'settings'}
  <SettingsDialog />
{:else if app.dialog?.kind === 'shapes'}
  <ShapeEditor blockId={app.dialog.blockId} />
{/if}

<!-- 印刷用：文書のページだけ（@media print で画面の代わりに出す）。
     用紙は文書のページと同じ大きさにし、1ページを1枚に収める（端数で次の用紙にはみ出さないよう、少し小さく切る） -->
<svelte:head>
  {#if printSize}{@html `<style>@page { size: ${printSize.w}pt ${printSize.h}pt; margin: 0; }</style>`}{/if}
</svelte:head>
<div class="print-pages" aria-hidden="true">
  {#each app.pageHashes as h, i (h + i)}
    {@const pg = app.result?.pages[i]}
    <div class="print-page" style:width="{pg?.width_pt ?? 595}pt" style:height="{(pg?.height_pt ?? 842) - 1}pt">{@html app.svgs.get(h) ?? ''}</div>
  {/each}
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
  .open { position: relative; display: inline-flex; }
  .open > button:first-child { border-top-right-radius: 0; border-bottom-right-radius: 0; }
  .drop { border-top-left-radius: 0; border-bottom-left-radius: 0; border-left: none; padding: 4px 6px; }
  .menu-backdrop { position: fixed; inset: 0; z-index: 29; }
  .menu {
    position: absolute; top: 110%; left: 0; z-index: 30; background: var(--panel); border: 1px solid var(--line-strong);
    border-radius: var(--radius); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15); padding: 4px; display: flex; flex-direction: column;
    min-width: 320px; max-width: 520px;
  }
  .menu .head { padding: 4px 8px; }
  .menu button { text-align: left; display: flex; flex-direction: column; align-items: flex-start; }
  .fpath { max-width: 480px; overflow: hidden; text-overflow: ellipsis; }
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
    background: #263238; color: #fff; box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25); z-index: 150; max-width: 70vw;
  }
  .toast.error { background: var(--error); }

  .print-pages { display: none; }
  @media print {
    :global(body) { background: #fff !important; }
    .app { display: none; }
    .print-pages { display: block; }
    .print-page { overflow: hidden; break-inside: avoid; break-after: page; page-break-after: always; }
    .print-page:last-child { break-after: auto; page-break-after: auto; }
    .print-page :global(svg) { width: 100%; height: auto; display: block; }
  }
</style>
