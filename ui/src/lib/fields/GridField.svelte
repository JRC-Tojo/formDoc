<script lang="ts">
  // 表の編集。セルを選んで結合（行・列）、縦書き、配置を設定する。罫線や文字サイズはテンプレートで固定。
  import { track } from '../insert';

  type Cell = { text: string; rowspan?: number; colspan?: number; vertical?: boolean; align?: string };
  type Grid = { header_rows: number; rows: Cell[][]; widths?: string[] };

  let { grid, onchange }: { grid: Grid | undefined; onchange: (g: Grid) => void } = $props();

  const g = $derived<Grid>(grid ?? { header_rows: 1, rows: [[{ text: '' }]] });
  const ncols = $derived(Math.max(1, ...g.rows.map((r) => r.length)));
  let sel = $state<[number, number] | null>(null);

  /** 結合で覆われるセル */
  const covered = $derived.by(() => {
    const cov = g.rows.map(() => Array(ncols).fill(false));
    g.rows.forEach((row, ri) =>
      row.forEach((c, ci) => {
        if (cov[ri][ci]) return;
        for (let r = ri; r < Math.min(g.rows.length, ri + (c.rowspan ?? 1)); r++)
          for (let k = ci; k < Math.min(ncols, ci + (c.colspan ?? 1)); k++) if (r !== ri || k !== ci) cov[r][k] = true;
      }),
    );
    return cov;
  });

  function update(fn: (rows: Cell[][]) => void, header = g.header_rows) {
    const rows: Cell[][] = JSON.parse(JSON.stringify(g.rows));
    for (const r of rows) while (r.length < ncols) r.push({ text: '' });
    fn(rows);
    onchange({ ...g, header_rows: header, rows });
  }

  const cur = $derived(sel ? g.rows[sel[0]]?.[sel[1]] : null);

  function setCell(patch: Partial<Cell>) {
    if (!sel) return;
    const [r, c] = sel;
    update((rows) => Object.assign(rows[r][c], patch));
  }
</script>

<div class="grid-editor">
  <div class="tools">
    <button onclick={() => update((rows) => rows.push(Array.from({ length: ncols }, () => ({ text: '' }))))}>＋行</button>
    <button onclick={() => update((rows) => rows.forEach((r) => r.push({ text: '' })))}>＋列</button>
    <button disabled={!sel || g.rows.length <= 1} onclick={() => { update((rows) => rows.splice(sel![0], 1)); sel = null; }}>行削除</button>
    <button disabled={!sel || ncols <= 1} onclick={() => { update((rows) => rows.forEach((r) => r.splice(sel![1], 1))); sel = null; }}>列削除</button>
    <label class="hdr small">見出し行
      <input type="number" min="0" max={g.rows.length} value={g.header_rows} oninput={(e) => update(() => {}, Number(e.currentTarget.value))} />
    </label>
  </div>

  <div class="scroll">
    <table>
      <tbody>
        {#each g.rows as row, ri}
          <tr class:header={ri < g.header_rows}>
            {#each Array(ncols) as _, ci}
              {#if !covered[ri][ci]}
                {@const c = row[ci] ?? { text: '' }}
                <td rowspan={c.rowspan ?? 1} colspan={c.colspan ?? 1} class:sel={sel?.[0] === ri && sel?.[1] === ci} class:vertical={c.vertical}>
                  <input type="text" value={c.text} use:track data-insert="ref" onfocus={() => (sel = [ri, ci])}
                    oninput={(e) => update((rows) => (rows[ri][ci].text = e.currentTarget.value))} />
                </td>
              {/if}
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if cur && sel}
    <div class="cell-tools small">
      <span class="muted">セル({sel[0] + 1},{sel[1] + 1})</span>
      <span>縦結合
        <button onclick={() => setCell({ rowspan: Math.max(1, (cur.rowspan ?? 1) - 1) })}>−</button>{cur.rowspan ?? 1}<button
          onclick={() => setCell({ rowspan: Math.min(g.rows.length - sel![0], (cur.rowspan ?? 1) + 1) })}>＋</button>
      </span>
      <span>横結合
        <button onclick={() => setCell({ colspan: Math.max(1, (cur.colspan ?? 1) - 1) })}>−</button>{cur.colspan ?? 1}<button
          onclick={() => setCell({ colspan: Math.min(ncols - sel![1], (cur.colspan ?? 1) + 1) })}>＋</button>
      </span>
      <label class="inline"><input type="checkbox" checked={cur.vertical ?? false} onchange={(e) => setCell({ vertical: e.currentTarget.checked })} />縦書き</label>
      <select value={cur.align ?? ''} onchange={(e) => setCell({ align: e.currentTarget.value || undefined })}>
        <option value="">中央</option><option value="left">左</option><option value="right">右</option>
      </select>
    </div>
  {/if}
  <div class="small muted">セルに {'{{変数名}}'} と書くと計算値（単位なし）が入ります。</div>
</div>

<style>
  .tools, .cell-tools { display: flex; gap: 4px; align-items: center; flex-wrap: wrap; margin-bottom: 6px; }
  .cell-tools { background: var(--panel-2); padding: 4px 6px; border-radius: 4px; gap: 10px; }
  .cell-tools button { padding: 0 6px; margin: 0 3px; }
  .cell-tools select { width: auto; }
  .hdr { display: inline-flex; gap: 4px; align-items: center; margin-left: auto; }
  .hdr input { width: 4em; }
  .inline { display: inline-flex; gap: 3px; align-items: center; }
  .inline input { width: auto; }
  .scroll { overflow: auto; max-height: 320px; margin-bottom: 6px; }
  table { border-collapse: collapse; }
  td { border: 1px solid var(--line-strong); padding: 0; min-width: 5em; }
  td input { border: none; border-radius: 0; padding: 3px 4px; }
  tr.header td { background: var(--panel-2); }
  tr.header td input { background: transparent; font-weight: 600; }
  td.sel { outline: 2px solid var(--accent); outline-offset: -2px; }
  td.vertical input { writing-mode: vertical-rl; min-height: 4em; }
</style>
