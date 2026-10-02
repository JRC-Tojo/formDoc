<script lang="ts">
  // コードモード（生Typst）。
  // - デスクトップ版: フォルダを開いて「VSCodeで開く」。保存を監視してプレビューを更新する
  // - どちらの版でも: 既製の CodeMirror による簡易エディタ（独自の編集機能は持たない）
  import { onDestroy, onMount } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { app } from '../state.svelte';
  import Gate from './Gate.svelte';

  let host: HTMLDivElement;
  let view: EditorView | null = null;

  onMount(() => {
    view = new EditorView({
      doc: app.codeText,
      extensions: [
        basicSetup,
        EditorView.lineWrapping,
        EditorView.updateListener.of((u) => {
          if (u.docChanged) app.setCode(u.state.doc.toString());
        }),
      ],
      parent: host,
    });
  });
  onDestroy(() => view?.destroy());

  // 外部（GUIからの変換など）で本文が変わったらエディタに反映する
  $effect(() => {
    const text = app.codeText;
    if (view && !app.codeFolder && view.state.doc.toString() !== text) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
  });

  const diags = $derived(app.result?.diagnostics ?? []);

  function jump(line: number | null) {
    if (!view || !line) return;
    const l = view.state.doc.line(Math.min(line, view.state.doc.lines));
    view.dispatch({ selection: { anchor: l.from }, scrollIntoView: true });
    view.focus();
  }
</script>

<div class="code">
  <div class="bar">
    <Gate cap="localFolder">
      <button onclick={() => app.openCodeFolder()} title="プロジェクトフォルダ（main.typ）を開いて監視します">フォルダを開く…</button>
    </Gate>
    <Gate cap="openInVSCode">
      <button onclick={() => app.openInVSCode()} title="フォルダをVSCodeで開き、保存のたびにプレビューを更新します">VSCodeで開く</button>
    </Gate>
    {#if app.codeFolder}
      <span class="folder small mono" title={app.codeFolder}>監視中: {app.codeFolder}</span>
      <button class="small" onclick={() => app.closeCodeFolder()}>監視をやめる</button>
    {/if}
    <span class="spacer"></span>
    <span class="small muted">#import "@local/formdoc:0.1.0": * の関数を使えます</span>
  </div>
  {#if app.codeFolder}
    <div class="external muted">
      <p>外部エディタで <code>main.typ</code> を編集して保存すると、右のプレビューが更新されます。</p>
      <p class="small">GUIと同じ社内標準パッケージ（@local/formdoc）で組版されるため、体裁・計算結果はGUIで作った文書と一致します。</p>
    </div>
  {/if}
  <div class="editor" class:hidden={!!app.codeFolder} bind:this={host}></div>
  {#if diags.length}
    <ul class="diags">
      {#each diags as d}
        <li class={d.severity}>
          <button class="ghost" onclick={() => jump(d.file === '/main.typ' ? d.line : null)}>
            <span class="mono small">{d.file ?? ''}:{d.line ?? '?'}</span> {d.message}
            {#each d.hints as h}<span class="small muted"> （{h}）</span>{/each}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .code { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .bar { display: flex; gap: 6px; align-items: center; padding: 6px 10px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .spacer { flex: 1; }
  .folder { max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .editor { flex: 1; min-height: 0; overflow: auto; }
  .editor.hidden { display: none; }
  .editor :global(.cm-editor) { height: 100%; font-size: 13px; }
  .editor :global(.cm-scroller) { font-family: var(--mono); }
  .external { padding: 20px; flex: 1; }
  .diags { list-style: none; margin: 0; padding: 4px; border-top: 1px solid var(--line); max-height: 30%; overflow: auto; }
  .diags li.error { color: var(--error); }
  .diags li.warning { color: var(--warn); }
  .diags button { text-align: left; width: 100%; }
</style>
