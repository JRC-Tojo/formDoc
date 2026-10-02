<script lang="ts">
  // 文書の構成（ブロック列）。見出しは番号付きで、それ以外は見出しの下に字下げして表示する。
  import { app } from '../state.svelte';
  import type { Block } from '../types';

  let dragId = $state<string | null>(null);
  let dropIndex = $state<number | null>(null);
  /** 右クリックメニュー（id が null なら一覧の余白） */
  let ctx = $state<{ x: number; y: number; id: string | null } | null>(null);

  const comps = $derived(app.catalog?.components ?? {});

  /** 見出し番号（§4． / 4.1 / (1) / 1)）をテンプレートと同じ規則で計算 */
  const numbering = $derived.by(() => {
    const out: Record<string, string> = {};
    const c = [Number(app.doc?.meta['chapter-start'] ?? 1) - 1, 0, 0, 0];
    for (const b of app.doc?.blocks ?? []) {
      if (b.kind !== 'heading') continue;
      const lv = Math.min(Math.max(Number(b.props.level ?? 2), 1), 4);
      c[lv - 1]++;
      for (let i = lv; i < 4; i++) c[i] = 0;
      out[b.id] = lv === 1 ? `§${c[0]}．` : lv === 2 ? `${c[0]}.${c[1]}` : lv === 3 ? `(${c[2]})` : `${c[3]})`;
    }
    return out;
  });

  /** 見出し以外のブロックの字下げ（直前の見出しの階層） */
  const depth = $derived.by(() => {
    const out: Record<string, number> = {};
    let cur = 0;
    for (const b of app.doc?.blocks ?? []) {
      if (b.kind === 'heading') {
        cur = Number(b.props.level ?? 2);
        out[b.id] = cur - 1;
      } else out[b.id] = cur;
    }
    return out;
  });

  function title(b: Block): string {
    const r = app.result?.blocks[b.id];
    const p = b.props;
    switch (b.kind) {
      case 'heading':
        return `${p.text ?? ''}${p.symbol ? '　' + p.symbol : ''}`;
      case 'paragraph':
        return String(p.text ?? '').replace(/\s+/g, ' ').slice(0, 40) || '（空の段落）';
      case 'vdef':
      case 'calc':
      case 'sum':
      case 'check':
        return r?.summary || p.name || p.expr || '';
      case 'where':
        return 'ここに， ' + (r?.vars ?? []).join(', ');
      case 'kijun':
        return `【${p.kijun?.abbr ?? ''}】${p.kijun?.loc ?? ''}`;
      case 'table':
      case 'fig-beam':
      case 'fig-isection':
      case 'fig-shapes':
      case 'image':
        return p.caption || p.file || (b.kind === 'fig-isection' ? `I形断面 ${p.H ?? ''}×${p.B ?? ''}×${p.tw ?? ''}×${p.tf ?? ''}` : comps[b.kind]?.label ?? '');
      case 'pagebreak':
        return '改ページ';
      case 'typst':
        return String(p.code ?? '').split('\n')[0].slice(0, 40);
      default:
        return '';
    }
  }

  function status(b: Block): 'error' | 'ng' | 'warn' | 'ok' {
    const r = app.result?.blocks[b.id];
    if (r?.status === 'error' || app.issuesFor(b.id).some((i) => i.severity === 'error')) return 'error';
    if (r?.status === 'ng') return 'ng';
    if (app.issuesFor(b.id).some((i) => i.severity === 'warning')) return 'warn';
    return 'ok';
  }

  function onDragStart(e: DragEvent, id: string) {
    dragId = id;
    e.dataTransfer?.setData('text/plain', id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }

  function onDragOver(e: DragEvent, index: number) {
    if (!dragId) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    dropIndex = e.clientY > rect.top + rect.height / 2 ? index + 1 : index;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    if (dragId != null && dropIndex != null) app.moveBlock(dragId, dropIndex);
    dragId = null;
    dropIndex = null;
  }

  function openAdd() {
    if (!app.style) return app.flash('先に文書情報でスタイルを選んでください', 'error');
    app.dialog = { kind: 'insert' };
  }

  function onContext(e: MouseEvent, id: string | null) {
    e.preventDefault();
    e.stopPropagation();
    if (id) app.selectedId = id;
    ctx = { x: Math.min(e.clientX, window.innerWidth - 230), y: Math.min(e.clientY, window.innerHeight - 260), id };
  }

  /** メニューの項目を実行してから閉じる（先に閉じると対象のIDが消える） */
  function run(fn: () => void) {
    fn();
    ctx = null;
  }
</script>

<div class="outline">
  <div class="head">
    <button class:active={app.selectedId === null} class="docmeta" onclick={() => (app.selectedId = null)}>
      📄 {app.doc?.meta.title || '文書情報'}
    </button>
  </div>

  <ol class="list" ondragleave={() => (dropIndex = null)} oncontextmenu={(e) => onContext(e, null)}>
    {#each app.doc?.blocks ?? [] as b, i (b.id)}
      {@const st = status(b)}
      <li
        class="item {b.kind}"
        class:selected={app.selectedId === b.id}
        class:drop-before={dropIndex === i}
        class:drop-after={dropIndex === i + 1 && i === (app.doc?.blocks.length ?? 0) - 1}
        style:padding-left="{8 + (depth[b.id] ?? 0) * 14}px"
        draggable="true"
        ondragstart={(e) => onDragStart(e, b.id)}
        ondragover={(e) => onDragOver(e, i)}
        ondrop={onDrop}
        ondragend={() => { dragId = null; dropIndex = null; }}
      >
        <button class="row" onclick={() => (app.selectedId = b.id)} oncontextmenu={(e) => onContext(e, b.id)}>
          {#if b.kind === 'heading'}
            <span class="num">{numbering[b.id]}</span>
          {:else}
            <span class="icon" title={comps[b.kind]?.label}>{comps[b.kind]?.icon ?? '•'}</span>
          {/if}
          <span class="text">{title(b)}</span>
          {#if st !== 'ok'}
            <span class="st {st}" title={st === 'error' ? 'エラー' : st === 'ng' ? '照査NG' : '注意'}>
              {st === 'error' ? '✕' : st === 'ng' ? 'NG' : '!'}
            </span>
          {/if}
        </button>
      </li>
    {/each}
  </ol>

  <div class="foot">
    <button class="primary" onclick={openAdd} title="部品・テンプレートを選んで、選択中の部品の下に追加します">＋ 部品を追加…</button>
    {#if app.selected}
      <div class="ops">
        <button title="複製" onclick={() => app.duplicateBlock(app.selectedId!)}>複製</button>
        <button title="削除" onclick={() => app.removeBlock(app.selectedId!)}>削除</button>
      </div>
    {/if}
  </div>
</div>

{#if ctx}
  {@const id = ctx?.id ?? null}
  <div class="ctx-backdrop" role="presentation" onmousedown={() => (ctx = null)} oncontextmenu={(e) => { e.preventDefault(); ctx = null; }}></div>
  <div class="ctx" role="menu" style:left="{ctx.x}px" style:top="{ctx.y}px">
    <button class="ghost" role="menuitem" onclick={() => run(openAdd)}>＋ この下に部品を追加…</button>
    {#if id}
      {@const b = app.doc?.blocks.find((x) => x.id === id)}
      <hr />
      <button class="ghost" role="menuitem" onclick={() => run(() => app.duplicateBlock(id))}>複製</button>
      <button class="ghost" role="menuitem" onclick={() => run(() => app.shiftBlock(id, -1))}>上へ移動</button>
      <button class="ghost" role="menuitem" onclick={() => run(() => app.shiftBlock(id, 1))}>下へ移動</button>
      {#if b?.kind === 'fig-shapes'}
        <button class="ghost" role="menuitem" onclick={() => run(() => (app.dialog = { kind: 'shapes', blockId: id }))}>図を描く…</button>
      {/if}
      <hr />
      <button class="ghost" role="menuitem" onclick={() => run(() => (app.dialog = { kind: 'saveTemplate', blockId: id }))}
        title={b?.kind === 'heading' ? '見出しと、その配下の節をまとめて保存します' : 'この部品を保存します'}>
        テンプレートとして保存…{b?.kind === 'heading' ? '（節ごと）' : ''}
      </button>
      <hr />
      <button class="ghost danger" role="menuitem" onclick={() => run(() => app.removeBlock(id))}>削除</button>
    {/if}
  </div>
{/if}

<style>
  .outline { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { padding: 8px; border-bottom: 1px solid var(--line); }
  .docmeta { width: 100%; text-align: left; overflow: hidden; text-overflow: ellipsis; }
  .docmeta.active { background: var(--accent-weak); border-color: var(--accent); }
  .list { list-style: none; margin: 0; padding: 4px 0; overflow: auto; flex: 1; min-height: 0; }
  .item { position: relative; padding-right: 6px; }
  .item.drop-before::before, .item.drop-after::after {
    content: ''; position: absolute; left: 8px; right: 8px; height: 2px; background: var(--accent);
  }
  .item.drop-before::before { top: 0; }
  .item.drop-after::after { bottom: 0; }
  .row {
    display: flex; align-items: center; gap: 6px; width: 100%; border: none; background: none;
    padding: 4px 6px; border-radius: 4px; text-align: left; cursor: grab;
  }
  .item.selected .row { background: var(--accent-weak); }
  .row:hover { background: var(--panel-2); }
  .item.heading .row { font-weight: 600; }
  .num { color: var(--accent); min-width: 2.4em; }
  .icon { display: inline-block; min-width: 2.2em; text-align: center; color: var(--muted); font-size: 11px; }
  .text { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .st { font-size: 10.5px; font-weight: 700; border-radius: 3px; padding: 0 4px; }
  .st.error { color: #fff; background: var(--error); }
  .st.ng { color: #fff; background: var(--warn); }
  .st.warn { color: var(--warn); background: var(--warn-weak); }
  .foot { border-top: 1px solid var(--line); padding: 8px; display: flex; gap: 6px; justify-content: space-between; }
  .ctx-backdrop { position: fixed; inset: 0; z-index: 60; }
  .ctx {
    position: fixed; z-index: 61; background: var(--panel); border: 1px solid var(--line-strong); border-radius: var(--radius);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.2); padding: 4px; min-width: 210px; display: flex; flex-direction: column;
  }
  .ctx button { text-align: left; }
  .ctx hr { border: none; border-top: 1px solid var(--line); margin: 3px 0; width: 100%; }
  .ctx .danger { color: var(--error); }
  .ops { display: flex; gap: 4px; }
</style>
