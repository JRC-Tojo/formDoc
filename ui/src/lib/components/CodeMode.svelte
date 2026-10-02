<script lang="ts">
  // コードモード：編集中の文書を、そのまま Typst のコードとして見る・編集する（文書は「部品で作成」と同じ1つ）。
  // - 部品ごとに「// @block ID」の目印がある。部品のコードを書き換えると、その部品は Typstコード部品になる
  // - 目印の無いところに行を足すと、新しい Typstコード部品になる
  // - デスクトップ版は「VSCodeで開く」で外部エディタでも編集できる（保存すると文書に反映）
  import { onDestroy, onMount } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { app } from '../state.svelte';
  import Gate from './Gate.svelte';
  import { flatten } from '../tree';

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  /** エディタへの書き込み中（外からの更新でエディタを書き換えるとき、変更通知を無視する） */
  let syncing = false;

  onMount(() => {
    view = new EditorView({
      doc: app.codeText,
      extensions: [
        basicSetup,
        EditorView.lineWrapping,
        EditorView.updateListener.of((u) => {
          if (u.docChanged && !syncing) app.setCode(u.state.doc.toString());
        }),
      ],
      parent: host,
    });
  });
  onDestroy(() => view?.destroy());

  // 文書が外（元に戻す・検証パネルの修正・外部エディタなど）で変わったら、エディタに反映する
  $effect(() => {
    const text = app.codeText;
    if (view && view.state.doc.toString() !== text) {
      syncing = true;
      const sel = Math.min(view.state.selection.main.head, text.length);
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, selection: { anchor: sel } });
      syncing = false;
    }
  });

  // 検証パネルなどで部品を選んだら、その部品のコードへ移動する
  $effect(() => {
    const id = app.selectedId;
    if (!view || !id) return;
    const text = view.state.doc.toString();
    const at = text.search(new RegExp(`^// @(block|group) ${id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'm'));
    if (at >= 0) view.dispatch({ selection: { anchor: at }, scrollIntoView: true });
  });

  /** Typstコード部品の数（コードで書き換えた部品） */
  const typstBlocks = $derived(flatten(app.doc?.blocks ?? []).filter((b) => b.kind === 'typst').length);
</script>

<div class="code">
  <div class="bar">
    <Gate cap="openInVSCode">
      <button onclick={() => app.openInVSCode()} title="この文書を main.typ・style.typ として書き出して VSCode で開きます。VSCode で保存すると、この文書に反映されます">VSCodeで開く</button>
    </Gate>
    {#if app.codeFolder}
      <span class="folder small mono" title={app.codeFolder}>VSCodeと同期中: {app.codeFolder}</span>
      <button class="small" onclick={() => app.closeCodeFolder()}>同期をやめる</button>
    {/if}
    <span class="spacer"></span>
    <button class="small" onclick={() => app.saveCodeFiles()} title="main.typ と style.typ をファイルとして保存します（formDoc の外で Typst として組版できます）">ファイルに保存…</button>
  </div>
  <div class="note small muted">
    「部品で作成」と同じ文書です。部品ごとの「// @block」の目印は消さないでください。部品のコードを書き換えるとその部品は Typstコード部品になり、目印の無いところに書いた行は新しい Typstコード部品になります。
    {#if typstBlocks}<span class="cnt">（Typstコード部品 {typstBlocks} 個）</span>{/if}
  </div>
  <div class="editor" bind:this={host}></div>
</div>

<style>
  .code { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .bar { display: flex; gap: 6px; align-items: center; padding: 6px 10px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .spacer { flex: 1; }
  .folder { max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .note { padding: 4px 10px; border-bottom: 1px solid var(--line); }
  .cnt { color: var(--accent); }
  .editor { flex: 1; min-height: 0; overflow: auto; }
  .editor :global(.cm-editor) { height: 100%; font-size: 13px; }
  .editor :global(.cm-scroller) { font-family: var(--mono); }
</style>
