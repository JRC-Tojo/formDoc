<script lang="ts">
  // 文書の構成。見出しは番号付きで、配下の部品を字下げして表示する。
  // 見出しの節と、挿入したテンプレートのまとまり（group）は折りたためる。ドラッグで並べ替える（見出しは節ごと）。
  import { app } from '../state.svelte';
  import { findBlock, sectionEnd } from '../tree';
  import type { Block } from '../types';

  /** 一覧の1行 */
  type Row = {
    b: Block;
    /** 字下げ */
    depth: number;
    /** 入っている並び（null は文書の直下、group のID ならその中） */
    parentId: string | null;
    index: number;
    /** 折りたためる（見出しで配下がある・group） */
    foldable: boolean;
  };

  /** 右クリックメニュー（id が null なら一覧の余白） */
  let ctx = $state<{ x: number; y: number; id: string | null } | null>(null);
  /** ドラッグ中：動かす部品と、落とす位置 */
  let drag = $state<{ id: string; x: number; y: number; active: boolean } | null>(null);
  let drop = $state<{ parentId: string | null; index: number; rowId: string; where: 'before' | 'after' | 'inside' } | null>(null);
  let suppressClick = false;

  const comps = $derived(app.catalog?.components ?? {});

  /** 見出し番号（§4． / 4.1 / (1) / 1)）。テンプレートの中の見出しも文書の順に数える */
  const numbering = $derived.by(() => {
    const out: Record<string, string> = {};
    const c = [Number(app.doc?.meta['chapter-start'] ?? 1) - 1, 0, 0, 0];
    const walk = (list: Block[]) => {
      for (const b of list) {
        if (b.kind === 'heading') {
          const lv = Math.min(Math.max(Number(b.props.level ?? 2), 1), 4);
          c[lv - 1]++;
          for (let i = lv; i < 4; i++) c[i] = 0;
          out[b.id] = lv === 1 ? `§${c[0]}．` : lv === 2 ? `${c[0]}.${c[1]}` : lv === 3 ? `(${c[2]})` : `${c[3]})`;
        }
        if (b.children?.length) walk(b.children);
      }
    };
    walk(app.doc?.blocks ?? []);
    return out;
  });

  /** 表示する行（折りたたんだ節・まとまりの中は出さない） */
  const rows = $derived.by(() => {
    const out: Row[] = [];
    const walk = (list: Block[], parentId: string | null, base: number) => {
      let cur = 0; // 直前の見出しの階層
      for (let i = 0; i < list.length; i++) {
        const b = list[i];
        const isHeading = b.kind === 'heading';
        const lv = Number(b.props.level ?? 2);
        const depth = base + (isHeading ? lv - 1 : cur);
        const end = sectionEnd(list, i);
        const foldable = b.kind === 'group' || (isHeading && end > i + 1);
        out.push({ b, depth, parentId, index: i, foldable });
        if (isHeading) cur = lv;
        if (app.collapsed[b.id]) {
          if (isHeading) i = end - 1; // 節の中身を飛ばす
          continue;
        }
        if (b.kind === 'group') walk(b.children ?? [], b.id, depth + 1);
      }
    };
    walk(app.doc?.blocks ?? [], null, 0);
    return out;
  });

  function title(b: Block): string {
    const r = app.result?.blocks[b.id];
    const p = b.props;
    switch (b.kind) {
      case 'heading':
        return `${p.text ?? ''}${p.symbol ? '　' + p.symbol : ''}`;
      case 'group':
        return `${p.title || 'テンプレート'}（${b.children?.length ?? 0}）`;
      case 'paragraph':
        return String(p.text ?? '').replace(/\s+/g, ' ').slice(0, 40) || '（空の段落）';
      case 'vdef':
      case 'calc':
      case 'sum':
      case 'check':
        return (r?.summary || p.name || p.expr || '') + (p.global ? '　🌐' : '');
      case 'where':
        return 'ここに， ' + (r?.vars ?? []).join(', ');
      case 'kijun':
        return `【${p.kijun?.abbr ?? ''}】${p.kijun?.loc ?? ''}`;
      case 'table':
      case 'fig-shapes':
      case 'image':
        return p.caption || p.file || comps[b.kind]?.label || '';
      case 'pagebreak':
        return '改ページ';
      case 'typst':
        return String(p.code ?? '').split('\n')[0].slice(0, 40);
      default:
        return '';
    }
  }

  /** 部品の状態（まとまり・折りたたんだ節は中身も含める） */
  function status(row: Row): 'error' | 'ng' | 'warn' | 'ok' {
    const ids = [row.b.id];
    const addChildren = (bs: Block[]) => bs.forEach((c) => (ids.push(c.id), addChildren(c.children ?? [])));
    if (row.b.kind === 'group') addChildren(row.b.children ?? []);
    if (row.b.kind === 'heading' && app.collapsed[row.b.id]) {
      const list = row.parentId ? (findBlock(app.doc?.blocks ?? [], row.parentId)?.children ?? []) : (app.doc?.blocks ?? []);
      addChildren(list.slice(row.index + 1, sectionEnd(list, row.index)));
    }
    let st: 'error' | 'ng' | 'warn' | 'ok' = 'ok';
    for (const id of ids) {
      const r = app.result?.blocks[id];
      const is = app.issuesFor(id);
      if (r?.status === 'error' || is.some((i) => i.severity === 'error')) return 'error';
      if (r?.status === 'ng') st = 'ng';
      else if (st === 'ok' && is.some((i) => i.severity === 'warning')) st = 'warn';
    }
    return st;
  }

  function toggle(e: MouseEvent, id: string) {
    e.stopPropagation();
    app.collapsed[id] = !app.collapsed[id];
  }

  // ---------- ドラッグで並べ替え（マウス操作で行う。WebView2 でも動くよう HTML の DnD は使わない） ----------

  function onPointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    // 文字の選択やブラウザ標準のドラッグを始めさせない。押した行にポインタを捕まえ、外に出ても移動を受け取る
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    drag = { id, x: e.clientX, y: e.clientY, active: false };
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag) return;
    if (!drag.active && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 5) return;
    drag.active = true;
    const el = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest('li[data-row]') as HTMLElement | null;
    if (!el) return (drop = null);
    const row = rows[Number(el.dataset.row)];
    if (!row || row.b.id === drag.id) return (drop = null);
    const r = el.getBoundingClientRect();
    const rel = (e.clientY - r.top) / r.height;
    if (row.b.kind === 'group' && !app.collapsed[row.b.id] && rel > 0.5) {
      // 開いているまとまりの下半分 → まとまりの先頭へ
      drop = { parentId: row.b.id, index: 0, rowId: row.b.id, where: 'inside' };
    } else if (rel < 0.5) {
      drop = { parentId: row.parentId, index: row.index, rowId: row.b.id, where: 'before' };
    } else {
      // 折りたたんだ見出しの下 → 節の後ろへ
      const list = row.parentId ? (findBlock(app.doc?.blocks ?? [], row.parentId)?.children ?? []) : (app.doc?.blocks ?? []);
      const index = row.b.kind === 'heading' && app.collapsed[row.b.id] ? sectionEnd(list, row.index) : row.index + 1;
      drop = { parentId: row.parentId, index, rowId: row.b.id, where: 'after' };
    }
  }

  function onPointerUp() {
    if (drag?.active) {
      suppressClick = true;
      if (drop) app.moveBlock(drag.id, drop.parentId, drop.index);
    }
    drag = null;
    drop = null;
  }

  function select(id: string) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    app.selectedId = id;
  }

  function openAdd() {
    if (!app.style) return app.flash('先に文書情報でスタイルを選んでください', 'error');
    app.dialog = { kind: 'insert' };
  }

  function onContext(e: MouseEvent, id: string | null) {
    e.preventDefault();
    e.stopPropagation();
    if (id) app.selectedId = id;
    ctx = { x: Math.min(e.clientX, window.innerWidth - 230), y: Math.min(e.clientY, window.innerHeight - 300), id };
  }

  /** メニューの項目を実行してから閉じる（先に閉じると対象のIDが消える） */
  function run(fn: () => void) {
    fn();
    ctx = null;
  }
</script>

<svelte:window onpointermove={onPointerMove} onpointerup={onPointerUp} onpointercancel={() => ((drag = null), (drop = null))} />

<div class="outline" class:dragging={drag?.active}>
  <div class="head">
    <button class:active={app.selectedId === null} class="docmeta" onclick={() => (app.selectedId = null)}>
      📄 {app.doc?.meta.title || '文書情報'}
    </button>
  </div>

  <ol class="list" oncontextmenu={(e) => onContext(e, null)}>
    {#each rows as row, ri (row.b.id)}
      {@const b = row.b}
      {@const st = status(row)}
      <li
        data-row={ri}
        class="item {b.kind}"
        class:selected={app.selectedId === b.id}
        class:moving={drag?.active && drag.id === b.id}
        class:drop-before={drop?.rowId === b.id && drop.where === 'before'}
        class:drop-after={drop?.rowId === b.id && drop.where === 'after'}
        class:drop-inside={drop?.rowId === b.id && drop.where === 'inside'}
        style:padding-left="{4 + row.depth * 14}px"
      >
        <button class="row" onpointerdown={(e) => onPointerDown(e, b.id)} onclick={() => select(b.id)} oncontextmenu={(e) => onContext(e, b.id)}>
          {#if row.foldable}
            <span class="fold" role="button" tabindex="-1" onpointerdown={(e) => e.stopPropagation()} onclick={(e) => toggle(e, b.id)} onkeydown={() => {}}
              title={app.collapsed[b.id] ? '開く' : '折りたたむ'}>{app.collapsed[b.id] ? '▸' : '▾'}</span>
          {:else}
            <span class="fold"></span>
          {/if}
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
      {@const b = findBlock(app.doc?.blocks ?? [], id)}
      <hr />
      <button class="ghost" role="menuitem" onclick={() => run(() => app.duplicateBlock(id))}>複製</button>
      <button class="ghost" role="menuitem" onclick={() => run(() => app.shiftBlock(id, -1))}>上へ移動</button>
      <button class="ghost" role="menuitem" onclick={() => run(() => app.shiftBlock(id, 1))}>下へ移動</button>
      {#if b?.kind === 'fig-shapes'}
        <button class="ghost" role="menuitem" onclick={() => run(() => (app.dialog = { kind: 'shapes', blockId: id }))}>図を描く…</button>
      {/if}
      {#if b?.kind === 'group'}
        <button class="ghost" role="menuitem" onclick={() => run(() => app.ungroup(id))} title="中の部品を、このまとまりの位置に並べ直します（中の変数は外の節の変数になります）">まとまりを解除</button>
      {/if}
      <hr />
      <button class="ghost" role="menuitem" onclick={() => run(() => (app.dialog = { kind: 'saveTemplate', blockId: id }))}
        title={b?.kind === 'heading' ? '見出しと、その配下の節をまとめて保存します' : 'この部品を保存します'}>
        テンプレートとして保存…{b?.kind === 'heading' ? '（節ごと）' : ''}
      </button>
      <hr />
      <button class="ghost danger" role="menuitem" onclick={() => run(() => app.removeBlock(id))}>削除{b?.kind === 'group' ? '（まとまりごと）' : ''}</button>
    {/if}
  </div>
{/if}

<style>
  .outline { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .outline.dragging { cursor: grabbing; user-select: none; }
  .head { padding: 8px; border-bottom: 1px solid var(--line); }
  .docmeta { width: 100%; text-align: left; overflow: hidden; text-overflow: ellipsis; }
  .docmeta.active { background: var(--accent-weak); border-color: var(--accent); }
  .list { list-style: none; margin: 0; padding: 4px 0; overflow: auto; flex: 1; min-height: 0; }
  .item { position: relative; padding-right: 6px; }
  .item.moving { opacity: 0.4; }
  .item.drop-before::before, .item.drop-after::after {
    content: ''; position: absolute; left: 8px; right: 8px; height: 2px; background: var(--accent);
  }
  .item.drop-before::before { top: 0; }
  .item.drop-after::after { bottom: 0; }
  .item.drop-inside .row { outline: 2px solid var(--accent); }
  .row {
    display: flex; align-items: center; gap: 4px; width: 100%; border: none; background: none;
    padding: 4px 6px 4px 2px; border-radius: 4px; text-align: left; cursor: grab; touch-action: none; user-select: none; -webkit-user-drag: none;
  }
  .item.selected .row { background: var(--accent-weak); }
  .row:hover { background: var(--panel-2); }
  .item.heading .row { font-weight: 600; }
  .item.group .row { color: var(--accent); }
  .fold { width: 1.1em; flex: none; text-align: center; color: var(--muted); cursor: pointer; font-size: 11px; }
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
