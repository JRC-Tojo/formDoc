<script lang="ts">
  // 汎用図形の描画エディタ。マウスで描き、点をドラッグして直す。座標は表の値（数値または式）として保存する。
  // 変数や式を使った座標は、評価した値の位置に描いてロックする（ドラッグで数値に戻してしまわないように）。
  import { onMount } from 'svelte';
  import { app } from '../state.svelte';
  import Modal from './Modal.svelte';
  import { evalExpr, formatLabel } from '../expr';
  import { findBlock } from '../tree';

  let { blockId }: { blockId: string } = $props();

  type Shape = {
    kind: string; x1?: string; y1?: string; x2?: string; y2?: string; pts?: string; label?: string; fill?: string;
    /** 繰り返し：横・縦の回数と間隔（式）。式の中で番号 ix・iy（0始まり）を使える */
    nx?: string; dx?: string; ny?: string; dy?: string;
  };
  type Tool = 'select' | 'line' | 'arrow' | 'rect' | 'circle' | 'polygon' | 'dim' | 'text';
  type Pt = { x: number; y: number };

  // 部品テンプレートのまとまりの中の図形もあるため、木全体から探す
  const block = $derived(findBlock(app.doc?.blocks ?? [], blockId));
  const original: Shape[] = JSON.parse(JSON.stringify(findBlock(app.doc?.blocks ?? [], blockId)?.props.shapes ?? []));

  let shapes = $state<Shape[]>(JSON.parse(JSON.stringify(original)));
  let undoStack: string[] = [];
  let redoStack: string[] = [];
  let tool = $state<Tool>('select');
  let sel = $state<number | null>(null);
  const GRIDS = [0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 25, 50, 100];
  let grid = $state(0.5);
  let snap = $state(true);
  let ppu = $state(60); // 1単位あたりのピクセル
  let origin = $state({ x: 80, y: 360 }); // 画面上の原点
  let svgEl: SVGSVGElement;
  let draft = $state<Shape | null>(null);
  let polyPts = $state<Pt[]>([]);
  let cursor = $state<Pt | null>(null);
  let note = $state('');

  const TOOLS: [Tool, string, string][] = [
    ['select', '↖', '選択・移動（点をドラッグで変形、Delete で削除）'],
    ['line', '╱', '線'],
    ['arrow', '➚', '矢印（終点に矢じり）'],
    ['rect', '▭', '矩形'],
    ['circle', '◯', '円（中心からドラッグ）'],
    ['polygon', '⬠', '多角形（クリックで頂点、ダブルクリックか Enter で確定）'],
    ['dim', '↔', '寸法線（文字に {{変数}} も可）'],
    ['text', 'A', '文字（クリックした位置）'],
  ];

  // ---------- 座標の評価 ----------

  // 表示用の評価。正式な値は保存後にエンジンが計算する（プレビュー・PDF はそちら）
  const varValues = $derived(new Map((app.result?.vars ?? []).map((v) => [v.name, v.value])));
  const varTexts = $derived(new Map((app.result?.vars ?? []).map((v) => [v.name, v.text])));

  const NUM = /^-?(\d+\.?\d*|\.\d+)(e-?\d+)?$/i;
  const isNum = (s: string | undefined) => s != null && NUM.test(s.trim());
  function val(s: string | undefined, ix = 0, iy = 0): number | null {
    if (s == null || s.trim() === '') return null;
    if (isNum(s)) return Number(s);
    return evalExpr(s, (n) => (n === 'ix' ? ix : n === 'iy' ? iy : varValues.get(n)));
  }

  /** 繰り返しの各回 [ix, iy] */
  function instances(sh: Shape): [number, number][] {
    const cnt = (v: string | undefined) => (v == null || v.trim() === '' ? 1 : Math.max(0, Math.min(200, Math.round(val(v) ?? 1))));
    const out: [number, number][] = [];
    for (let iy = 0; iy < cnt(sh.ny); iy++) for (let ix = 0; ix < cnt(sh.nx); ix++) out.push([ix, iy]);
    return out;
  }

  type Geom = { x1: number | null; y1: number | null; x2: number | null; y2: number | null; pts: [number | null, number | null][]; label: string };
  /** 繰り返しの1回分の形（平行移動ずみ）。円の半径は移動しない */
  function geom(sh: Shape, ix: number, iy: number): Geom {
    const ox = ix * (val(sh.dx) ?? 0), oy = iy * (val(sh.dy) ?? 0);
    const mv = (v: number | null, o: number) => (v == null ? null : v + o);
    const label = formatLabel(sh.label ?? '', (n) => (n === 'ix' ? ix : n === 'iy' ? iy : varValues.get(n)), (n) => varTexts.get(n));
    return {
      x1: mv(val(sh.x1, ix, iy), ox),
      y1: mv(val(sh.y1, ix, iy), oy),
      x2: sh.kind === 'circle' ? val(sh.x2, ix, iy) : mv(val(sh.x2, ix, iy), ox),
      y2: mv(val(sh.y2, ix, iy), oy),
      pts: splitPts(sh.pts ?? '').map(([a, b]) => [mv(val(a, ix, iy), ox), mv(val(b, ix, iy), oy)]),
      label,
    };
  }

  function splitPts(s: string): [string, string][] {
    return s
      .split(';')
      .filter((p) => p.trim())
      .map((p) => {
        let depth = 0;
        for (let i = 0; i < p.length; i++) {
          if (p[i] === '(') depth++;
          else if (p[i] === ')') depth--;
          else if (p[i] === ',' && depth === 0) return [p.slice(0, i).trim(), p.slice(i + 1).trim()];
        }
        return [p.trim(), ''];
      });
  }
  const joinPts = (pts: [string, string][]) => pts.map(([x, y]) => `${x}, ${y}`).join('; ');

  const fmt = (v: number) => String(Math.round(v * 1000) / 1000);

  // ---------- 画面座標 ----------

  const sx = (x: number) => origin.x + x * ppu;
  const sy = (y: number) => origin.y - y * ppu;

  function world(e: MouseEvent, doSnap = snap): Pt {
    const r = svgEl.getBoundingClientRect();
    let x = (e.clientX - r.left - origin.x) / ppu;
    let y = (origin.y - (e.clientY - r.top)) / ppu;
    if (doSnap && grid > 0) {
      x = Math.round(x / grid) * grid;
      y = Math.round(y / grid) * grid;
    }
    return { x: Number(fmt(x)), y: Number(fmt(y)) };
  }

  // ---------- 取り消し ----------

  function commit(fn: () => void) {
    undoStack.push(JSON.stringify(shapes));
    if (undoStack.length > 200) undoStack.shift();
    redoStack = [];
    fn();
  }
  function undo() {
    const p = undoStack.pop();
    if (!p) return;
    redoStack.push(JSON.stringify(shapes));
    shapes = JSON.parse(p);
    sel = null;
  }
  function redo() {
    const n = redoStack.pop();
    if (!n) return;
    undoStack.push(JSON.stringify(shapes));
    shapes = JSON.parse(n);
    sel = null;
  }

  // ---------- 操作 ----------

  type Drag =
    | { type: 'new'; start: Pt }
    | { type: 'handle'; i: number; h: string; before: string }
    | { type: 'move'; i: number; start: Pt; base: Shape; before: string }
    | { type: 'pan'; sx: number; sy: number; ox: number; oy: number };
  let drag: Drag | null = null;

  /** 図形の点（ハンドル）。h は x1y1 / x2y2 / r / p<番号> */
  function handles(sh: Shape): { h: string; x: number; y: number; locked: boolean }[] {
    const out: { h: string; x: number; y: number; locked: boolean }[] = [];
    const add = (h: string, xs?: string, ys?: string) => {
      const x = val(xs), y = val(ys);
      if (x != null && y != null) out.push({ h, x, y, locked: !isNum(xs) || !isNum(ys) });
    };
    if (sh.kind === 'polygon') splitPts(sh.pts ?? '').forEach(([x, y], j) => add(`p${j}`, x, y));
    else if (sh.kind === 'circle') {
      add('x1y1', sh.x1, sh.y1);
      const cx = val(sh.x1), cy = val(sh.y1), r = val(sh.x2);
      if (cx != null && cy != null && r != null) out.push({ h: 'r', x: cx + r, y: cy, locked: !isNum(sh.x2) });
    } else {
      add('x1y1', sh.x1, sh.y1);
      if (sh.kind !== 'text') add('x2y2', sh.x2, sh.y2);
    }
    return out;
  }

  function lockedShape(sh: Shape): boolean {
    const keys = sh.kind === 'polygon' ? [] : sh.kind === 'text' ? ['x1', 'y1'] : sh.kind === 'circle' ? ['x1', 'y1'] : ['x1', 'y1', 'x2', 'y2'];
    if (keys.some((k) => !isNum((sh as any)[k]))) return true;
    return sh.kind === 'polygon' && splitPts(sh.pts ?? '').some(([x, y]) => !isNum(x) || !isNum(y));
  }

  function onDown(e: MouseEvent) {
    if (e.button === 1 || (e.button === 0 && e.altKey)) {
      drag = { type: 'pan', sx: e.clientX, sy: e.clientY, ox: origin.x, oy: origin.y };
      e.preventDefault();
      return;
    }
    if (e.button !== 0) return;
    const p = world(e);
    if (tool === 'select') {
      sel = null;
      return;
    }
    if (tool === 'polygon') {
      polyPts = [...polyPts, p];
      return;
    }
    if (tool === 'text') {
      commit(() => (shapes = [...shapes, { kind: 'text', x1: fmt(p.x), y1: fmt(p.y), label: '文字' }]));
      sel = shapes.length - 1;
      tool = 'select';
      return;
    }
    drag = { type: 'new', start: p };
    draft = makeShape(tool, p, p);
  }

  function makeShape(t: Tool, a: Pt, b: Pt): Shape {
    if (t === 'circle') return { kind: 'circle', x1: fmt(a.x), y1: fmt(a.y), x2: fmt(Math.hypot(b.x - a.x, b.y - a.y)) };
    const s: Shape = { kind: t, x1: fmt(a.x), y1: fmt(a.y), x2: fmt(b.x), y2: fmt(b.y) };
    if (t === 'dim') s.label = fmt(Math.hypot(b.x - a.x, b.y - a.y));
    return s;
  }

  function onMove(e: MouseEvent) {
    cursor = world(e);
    if (!drag) return;
    if (drag.type === 'pan') {
      origin = { x: drag.ox + e.clientX - drag.sx, y: drag.oy + e.clientY - drag.sy };
      return;
    }
    const p = world(e);
    if (drag.type === 'new') draft = makeShape(tool, drag.start, p);
    else if (drag.type === 'handle') setHandle(drag.i, drag.h, p);
    else if (drag.type === 'move') {
      const d = { x: p.x - drag.start.x, y: p.y - drag.start.y };
      const b = drag.base;
      const mv = (s: string | undefined, dv: number) => (s == null || s === '' ? s : fmt(Number(s) + dv));
      const next: Shape = { ...b, x1: mv(b.x1, d.x), y1: mv(b.y1, d.y) };
      if (b.kind !== 'circle' && b.kind !== 'text') {
        next.x2 = mv(b.x2, d.x);
        next.y2 = mv(b.y2, d.y);
      }
      if (b.kind === 'polygon') next.pts = joinPts(splitPts(b.pts ?? '').map(([x, y]) => [fmt(Number(x) + d.x), fmt(Number(y) + d.y)]));
      shapes[drag.i] = next;
    }
  }

  function onUp() {
    if (drag?.type === 'new' && draft) {
      const d = draft;
      const zero = d.kind === 'circle' ? Number(d.x2) === 0 : d.x1 === d.x2 && d.y1 === d.y2;
      if (!zero) {
        commit(() => (shapes = [...shapes, d]));
        sel = shapes.length - 1;
      }
      draft = null;
    } else if ((drag?.type === 'handle' || drag?.type === 'move') && JSON.stringify(shapes) !== drag.before) {
      undoStack.push(drag.before);
      redoStack = [];
    }
    drag = null;
  }

  function finishPolygon() {
    if (polyPts.length >= 2) {
      const pts = joinPts(polyPts.map((p) => [fmt(p.x), fmt(p.y)]));
      commit(() => (shapes = [...shapes, { kind: 'polygon', pts }]));
      sel = shapes.length - 1;
    }
    polyPts = [];
  }

  function setHandle(i: number, h: string, p: Pt) {
    const sh = { ...shapes[i] };
    if (h === 'x1y1') {
      sh.x1 = fmt(p.x);
      sh.y1 = fmt(p.y);
    } else if (h === 'x2y2') {
      sh.x2 = fmt(p.x);
      sh.y2 = fmt(p.y);
    } else if (h === 'r') {
      sh.x2 = fmt(Math.max(0, Math.hypot(p.x - (val(sh.x1) ?? 0), p.y - (val(sh.y1) ?? 0))));
    } else if (h.startsWith('p')) {
      const j = Number(h.slice(1));
      const pts = splitPts(sh.pts ?? '');
      pts[j] = [fmt(p.x), fmt(p.y)];
      sh.pts = joinPts(pts);
    }
    shapes[i] = sh;
  }

  function onHandleDown(e: MouseEvent, i: number, h: { h: string; locked: boolean }) {
    e.stopPropagation();
    if (e.button !== 0 || tool !== 'select') return;
    sel = i;
    if (h.locked) {
      note = 'この点は変数・式で指定されているため、ドラッグでは動かせません（右の欄で編集してください）';
      return;
    }
    drag = { type: 'handle', i, h: h.h, before: JSON.stringify(shapes) };
  }

  function onShapeDown(e: MouseEvent, i: number) {
    if (tool !== 'select' || e.button !== 0) return;
    e.stopPropagation();
    sel = i;
    note = '';
    if (lockedShape(shapes[i])) {
      note = '変数・式を使った図形は、ドラッグで移動できません（右の欄で編集してください）';
      return;
    }
    drag = { type: 'move', i, start: world(e), base: { ...shapes[i] }, before: JSON.stringify(shapes) };
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const r = svgEl.getBoundingClientRect();
    const mx = e.clientX - r.left, my = e.clientY - r.top;
    const k = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    const next = Math.min(400, Math.max(0.01, ppu * k));
    origin = { x: mx - ((mx - origin.x) * next) / ppu, y: my - ((my - origin.y) * next) / ppu };
    ppu = next;
  }

  function fit() {
    const xs: number[] = [], ys: number[] = [];
    for (const sh of shapes) for (const h of handles(sh)) { xs.push(h.x); ys.push(h.y); }
    if (!xs.length) return;
    const r = svgEl.getBoundingClientRect();
    const w = Math.max(1, Math.max(...xs) - Math.min(...xs)), hgt = Math.max(1, Math.max(...ys) - Math.min(...ys));
    ppu = Math.min(400, Math.max(0.01, Math.min((r.width - 120) / w, (r.height - 120) / hgt)));
    origin = { x: 60 - Math.min(...xs) * ppu, y: r.height - 60 + Math.min(...ys) * ppu };
    // 図の大きさに合ったグリッド（画面上で 15px 以上）
    grid = GRIDS.find((g) => g * ppu >= 15) ?? GRIDS[GRIDS.length - 1];
  }

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    if (t?.closest('input, textarea, select')) return;
    if (e.key === 'Delete' || e.key === 'Backspace') {
      if (sel != null) {
        const i = sel;
        commit(() => (shapes = shapes.filter((_, j) => j !== i)));
        sel = null;
      }
    } else if (e.key === 'Enter' && tool === 'polygon') finishPolygon();
    else if (e.key === 'Escape' && (polyPts.length || draft)) {
      e.preventDefault();
      polyPts = [];
      draft = null;
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
      e.preventDefault();
      e.stopPropagation();
      e.shiftKey ? redo() : undo();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') {
      e.preventDefault();
      e.stopPropagation();
      redo();
    }
  }

  function setField(i: number, k: keyof Shape, v: string) {
    const before = JSON.stringify(shapes);
    shapes[i] = { ...shapes[i], [k]: v };
    undoStack.push(before);
    redoStack = [];
  }

  function reorder(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= shapes.length) return;
    commit(() => {
      const s = [...shapes];
      [s[i], s[j]] = [s[j], s[i]];
      shapes = s;
    });
    sel = j;
  }

  function apply() {
    app.setProp(blockId, 'shapes', JSON.parse(JSON.stringify(shapes)));
    app.dialog = null;
  }

  function cancel() {
    if (JSON.stringify(shapes) !== JSON.stringify(original) && !confirm('描いた内容を破棄して閉じますか？')) return;
    app.dialog = null;
  }

  // 開いたときは図形全体が見えるようにする
  onMount(() => requestAnimationFrame(fit));

  const FILL: Record<string, string> = { gray: '#d2d2d2', dark: '#5a5a5a', black: '#000' };

  // 画面のグリッド線
  const gridLines = $derived.by(() => {
    const out: { x1: number; y1: number; x2: number; y2: number; major: boolean }[] = [];
    if (grid <= 0 || grid * ppu < 6) return out;
    const r = { width: 2000, height: 1400 };
    const x0 = Math.floor(-origin.x / ppu / grid) * grid, x1 = (r.width - origin.x) / ppu;
    const y0 = Math.floor((origin.y - r.height) / ppu / grid) * grid, y1 = origin.y / ppu;
    for (let x = x0; x <= x1; x += grid) out.push({ x1: sx(x), y1: 0, x2: sx(x), y2: r.height, major: Math.abs(x - Math.round(x)) < 1e-9 });
    for (let y = y0; y <= y1; y += grid) out.push({ x1: 0, y1: sy(y), x2: r.width, y2: sy(y), major: Math.abs(y - Math.round(y)) < 1e-9 });
    return out;
  });
</script>

<!-- Esc で多角形の描きかけを消すとき、ダイアログが閉じないよう先に（capture で）受ける -->
<svelte:window onkeydowncapture={onKey} onmouseup={onUp} />

<Modal title="図を描く{block?.props.caption ? `：${block.props.caption}` : ''}" onclose={cancel} width="1180px" height="760px">
  <div class="editor">
    <div class="tools">
      {#each TOOLS as [t, icon, tip]}
        <button class:on={tool === t} title={tip} onclick={() => { tool = t; polyPts = []; note = ''; }}>{icon}</button>
      {/each}
      <hr />
      <button title="元に戻す (Ctrl+Z)" onclick={undo}>↶</button>
      <button title="やり直し (Ctrl+Y)" onclick={redo}>↷</button>
      <button title="全体を表示" onclick={fit}>⤢</button>
    </div>

    <div class="canvas">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <svg bind:this={svgEl} onmousedown={onDown} onmousemove={onMove} onwheel={onWheel}
        ondblclick={() => tool === 'polygon' && finishPolygon()} class:crosshair={tool !== 'select'}>
        {#each gridLines as g}
          <line x1={g.x1} y1={g.y1} x2={g.x2} y2={g.y2} class="grid" class:major={g.major} />
        {/each}
        <line x1={sx(0)} y1="0" x2={sx(0)} y2="2000" class="axis" />
        <line x1="0" y1={sy(0)} x2="3000" y2={sy(0)} class="axis" />

        {#each [...shapes, ...(draft ? [draft] : [])] as sh, i}
          {@const isDraft = draft != null && i === shapes.length}
          {@const fill = FILL[sh.fill ?? ''] ?? 'none'}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <g class="shape" class:sel={sel === i} class:draft={isDraft} onmousedown={(e) => !isDraft && onShapeDown(e, i)}>
            {#each instances(sh) as [ix, iy] (ix + ',' + iy)}
              {@const g = geom(sh, ix, iy)}
              <g class:repeat={ix > 0 || iy > 0}>
                {#if (sh.kind === 'line' || sh.kind === 'arrow' || sh.kind === 'dim') && g.x1 != null && g.y1 != null && g.x2 != null && g.y2 != null}
                  <line x1={sx(g.x1)} y1={sy(g.y1)} x2={sx(g.x2)} y2={sy(g.y2)} class="hit" />
                  <line x1={sx(g.x1)} y1={sy(g.y1)} x2={sx(g.x2)} y2={sy(g.y2)} class="stroke" class:thin={sh.kind === 'dim'}
                    marker-end={sh.kind === 'line' ? undefined : 'url(#arrow)'} marker-start={sh.kind === 'dim' ? 'url(#arrow-s)' : undefined} />
                  {#if sh.kind === 'dim'}
                    <text x={(sx(g.x1) + sx(g.x2)) / 2} y={(sy(g.y1) + sy(g.y2)) / 2 - 6} class="label small">{g.label}</text>
                  {/if}
                {:else if sh.kind === 'rect' && g.x1 != null && g.y1 != null && g.x2 != null && g.y2 != null}
                  <rect x={Math.min(sx(g.x1), sx(g.x2))} y={Math.min(sy(g.y1), sy(g.y2))} width={Math.abs(sx(g.x2) - sx(g.x1))} height={Math.abs(sy(g.y2) - sy(g.y1))}
                    class="stroke" style:fill={fill} class:hollow={fill === 'none'} />
                {:else if sh.kind === 'circle' && g.x1 != null && g.y1 != null && g.x2 != null}
                  <circle cx={sx(g.x1)} cy={sy(g.y1)} r={Math.abs(g.x2) * ppu} class="stroke" style:fill={fill} class:hollow={fill === 'none'} />
                {:else if sh.kind === 'polygon' && g.pts.length >= 2 && g.pts.every(([a, b]) => a != null && b != null)}
                  <polygon points={g.pts.map(([a, b]) => `${sx(a!)},${sy(b!)}`).join(' ')} class="stroke" style:fill={fill} class:hollow={fill === 'none'} />
                {:else if sh.kind === 'text' && g.x1 != null && g.y1 != null}
                  <text x={sx(g.x1)} y={sy(g.y1)} class="label">{g.label || '（文字）'}</text>
                {/if}
              </g>
            {/each}
          </g>
        {/each}

        {#if polyPts.length}
          <polyline points={[...polyPts, ...(cursor ? [cursor] : [])].map((p) => `${sx(p.x)},${sy(p.y)}`).join(' ')} class="stroke draftline" />
        {/if}

        {#if sel != null && shapes[sel] && tool === 'select'}
          {#each handles(shapes[sel]) as h}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <rect x={sx(h.x) - 5} y={sy(h.y) - 5} width="10" height="10" class="handle" class:locked={h.locked}
              onmousedown={(e) => onHandleDown(e, sel!, h)}><title>{h.locked ? '変数・式で指定された点（ロック）' : 'ドラッグで移動'}</title></rect>
          {/each}
        {/if}

        <defs>
          <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" /></marker>
          <marker id="arrow-s" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" /></marker>
        </defs>
      </svg>
      <div class="status small">
        {#if cursor}<span class="mono">x = {fmt(cursor.x)}, y = {fmt(cursor.y)}</span>{/if}
        <span class="muted">ホイール：拡大縮小／中ボタンか Alt+ドラッグ：移動</span>
        {#if note}<span class="note">{note}</span>{/if}
      </div>
    </div>

    <aside>
      <div class="row">
        <label class="inline"><input type="checkbox" bind:checked={snap} />グリッドに吸着</label>
        <select bind:value={grid}>
          {#each GRIDS as g}<option value={g}>{g}</option>{/each}
        </select>
      </div>
      <div class="row small muted">縮尺：1単位 = {block?.props.scale ?? 1} cm（部品の設定で変更）</div>

      <h4>図形（{shapes.length}）</h4>
      <ol class="list">
        {#each shapes as sh, i}
          <li><button class="ghost" class:on={sel === i} onclick={() => { sel = i; tool = 'select'; }}>
            {TOOLS.find((t) => t[0] === sh.kind)?.[1] ?? '?'} {sh.kind}{sh.label ? `「${sh.label}」` : ''}{sh.nx || sh.ny ? ` ×${instances(sh).length}` : ''}{lockedShape(sh) ? ' 🔒' : ''}
          </button></li>
        {/each}
      </ol>

      {#if sel != null && shapes[sel]}
        {@const sh = shapes[sel]}
        {@const i = sel}
        <h4>選択中の図形</h4>
        <p class="small muted">座標には数値のほか、変数名や式（例: L_b / 2）を書けます。</p>
        {#if sh.kind === 'polygon'}
          <label>頂点（x, y; x, y; …）<textarea rows="3" class="mono" value={sh.pts ?? ''} onchange={(e) => setField(i, 'pts', e.currentTarget.value)}></textarea></label>
        {:else}
          <div class="coords">
            <label>x1<input class="mono" value={sh.x1 ?? ''} onchange={(e) => setField(i, 'x1', e.currentTarget.value)} /></label>
            <label>y1<input class="mono" value={sh.y1 ?? ''} onchange={(e) => setField(i, 'y1', e.currentTarget.value)} /></label>
            {#if sh.kind === 'circle'}
              <label>半径<input class="mono" value={sh.x2 ?? ''} onchange={(e) => setField(i, 'x2', e.currentTarget.value)} /></label>
            {:else if sh.kind !== 'text'}
              <label>x2<input class="mono" value={sh.x2 ?? ''} onchange={(e) => setField(i, 'x2', e.currentTarget.value)} /></label>
              <label>y2<input class="mono" value={sh.y2 ?? ''} onchange={(e) => setField(i, 'y2', e.currentTarget.value)} /></label>
            {/if}
          </div>
        {/if}
        <details class="repeat-box" open={!!(sh.nx || sh.ny)}>
          <summary class="small">繰り返し{sh.nx || sh.ny ? `（横 ${sh.nx || 1} × 縦 ${sh.ny || 1}）` : ''}</summary>
          <div class="coords">
            <label>横の回数<input class="mono" value={sh.nx ?? ''} placeholder="1" onchange={(e) => setField(i, 'nx', e.currentTarget.value)} /></label>
            <label>横の間隔<input class="mono" value={sh.dx ?? ''} placeholder="0" onchange={(e) => setField(i, 'dx', e.currentTarget.value)} /></label>
            <label>縦の回数<input class="mono" value={sh.ny ?? ''} placeholder="1" onchange={(e) => setField(i, 'ny', e.currentTarget.value)} /></label>
            <label>縦の間隔<input class="mono" value={sh.dy ?? ''} placeholder="0" onchange={(e) => setField(i, 'dy', e.currentTarget.value)} /></label>
          </div>
          <p class="small muted">回数・間隔に変数を使えます（例: n, s）。座標や文字の式では繰り返しの番号 ix・iy（0始まり）を使えます。文字は {'{{式:桁}}'} で値を表示します（例: {'{{1 - (a + ix * s) / L:3}}'}）。</p>
        </details>
        {#if sh.kind === 'text' || sh.kind === 'dim'}
          <label>文字<input value={sh.label ?? ''} onchange={(e) => setField(i, 'label', e.currentTarget.value)} placeholder={'{{L_b}} で変数の値'} /></label>
        {/if}
        {#if sh.kind === 'rect' || sh.kind === 'circle' || sh.kind === 'polygon'}
          <label>塗り
            <select value={sh.fill ?? ''} onchange={(e) => setField(i, 'fill', e.currentTarget.value)}>
              <option value="">なし</option><option value="gray">薄い灰色</option><option value="dark">濃い灰色</option><option value="black">黒</option>
            </select>
          </label>
        {/if}
        <div class="row">
          <button class="small" onclick={() => reorder(i, -1)} title="背面へ">↑</button>
          <button class="small" onclick={() => reorder(i, 1)} title="前面へ">↓</button>
          <button class="small danger" onclick={() => { commit(() => (shapes = shapes.filter((_, j) => j !== i))); sel = null; }}>削除</button>
        </div>
      {/if}
    </aside>
  </div>

  {#snippet footer()}
    <span class="small muted foot">OK で文書に反映します（文書の「元に戻す」で取り消せます）</span>
    <button onclick={cancel}>キャンセル</button>
    <button class="primary" onclick={apply}>OK</button>
  {/snippet}
</Modal>

<style>
  .editor { display: grid; grid-template-columns: 44px 1fr 270px; height: 100%; min-height: 0; }
  .tools { display: flex; flex-direction: column; gap: 4px; padding: 6px; border-right: 1px solid var(--line); }
  .tools button { padding: 4px 0; font-size: 15px; }
  .tools button.on { background: var(--accent); color: #fff; border-color: var(--accent); }
  .tools hr { width: 100%; border: none; border-top: 1px solid var(--line); }
  .canvas { position: relative; min-height: 0; display: flex; flex-direction: column; }
  svg { flex: 1; width: 100%; background: #fff; user-select: none; }
  svg.crosshair { cursor: crosshair; }
  .grid { stroke: #eef0f3; stroke-width: 1; }
  .grid.major { stroke: #dde1e6; }
  .axis { stroke: #b7c0cc; stroke-width: 1; stroke-dasharray: 4 3; }
  .shape { cursor: move; }
  svg.crosshair .shape { cursor: crosshair; }
  .stroke { stroke: #222; stroke-width: 1.6; vector-effect: non-scaling-stroke; }
  .stroke.thin { stroke-width: 1; }
  .hollow { fill: transparent; }
  .hit { stroke: transparent; stroke-width: 10; }
  .shape.sel .stroke { stroke: #1f5fbf; }
  .shape.draft .stroke { stroke: #1f5fbf; stroke-dasharray: 4 3; }
  .draftline { fill: none; stroke: #1f5fbf; stroke-dasharray: 4 3; }
  .label { font-size: 13px; text-anchor: middle; dominant-baseline: middle; fill: #222; }
  .label.small { font-size: 11px; }
  .handle { fill: #fff; stroke: #1f5fbf; stroke-width: 1.5; cursor: grab; }
  .handle.locked { fill: #e0e0e0; stroke: #888; cursor: not-allowed; }
  marker path { fill: #222; }
  .status { display: flex; gap: 14px; padding: 3px 8px; border-top: 1px solid var(--line); background: var(--panel); }
  .note { color: var(--warn); }
  aside { border-left: 1px solid var(--line); padding: 10px; overflow: auto; display: flex; flex-direction: column; gap: 6px; }
  .row { display: flex; gap: 6px; align-items: center; }
  .inline { display: flex; gap: 4px; align-items: center; font-size: 12px; }
  .inline input { width: auto; }
  h4 { margin: 6px 0 0; }
  .list { list-style: none; padding: 0; margin: 0; max-height: 180px; overflow: auto; }
  .list button { width: 100%; text-align: left; padding: 2px 6px; }
  .list button.on { background: var(--accent-weak); }
  .coords { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 8px; }
  label { font-size: 11.5px; font-weight: 600; }
  .danger { color: var(--error); }
  .repeat :global(.stroke) { opacity: 0.75; }
  .repeat-box { border: 1px solid var(--line); border-radius: 4px; padding: 4px 6px; }
  .repeat-box summary { cursor: pointer; font-weight: 600; }
  .foot { margin-right: auto; }
</style>
