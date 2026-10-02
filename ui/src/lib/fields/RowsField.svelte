<script lang="ts">
  // 行の繰り返し入力（内訳、図形の一覧など）。
  import { track } from '../insert';
  import type { FieldDef } from '../types';

  let { columns, rows, onchange }: { columns: FieldDef[]; rows: Record<string, any>[]; onchange: (v: any) => void } = $props();

  function set(i: number, key: string, v: any) {
    onchange(rows.map((r, j) => (j === i ? { ...r, [key]: v } : r)));
  }
  function add() {
    onchange([...rows, Object.fromEntries(columns.map((c) => [c.key, c.options?.[0] ?? '']))]);
  }
  function remove(i: number) {
    onchange(rows.filter((_, j) => j !== i));
  }
  function move(i: number, d: number) {
    const r = [...rows];
    const [x] = r.splice(i, 1);
    r.splice(Math.max(0, Math.min(r.length, i + d)), 0, x);
    onchange(r);
  }
</script>

<div class="rows">
  <table>
    <thead>
      <tr>{#each columns as c}<th>{c.label}</th>{/each}<th></th></tr>
    </thead>
    <tbody>
      {#each rows as r, i}
        <tr>
          {#each columns as c}
            <td>
              {#if c.type === 'select'}
                <select value={r[c.key] ?? ''} onchange={(e) => set(i, c.key, e.currentTarget.value)}>
                  {#each c.options ?? [] as o}<option value={o}>{o}</option>{/each}
                </select>
              {:else}
                <input type="text" class:mono={c.type !== 'text'} value={r[c.key] ?? ''} use:track
                  data-insert={c.type === 'text' ? 'ref' : 'name'} oninput={(e) => set(i, c.key, e.currentTarget.value)} spellcheck="false" />
              {/if}
            </td>
          {/each}
          <td class="ops">
            <button class="ghost" title="上へ" onclick={() => move(i, -1)}>↑</button>
            <button class="ghost" title="下へ" onclick={() => move(i, 1)}>↓</button>
            <button class="ghost" title="削除" onclick={() => remove(i)}>✕</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  <button onclick={add}>＋ 行を追加</button>
</div>

<style>
  .rows { overflow-x: auto; }
  table { border-collapse: collapse; width: 100%; margin-bottom: 4px; }
  th { font-size: 11px; color: var(--muted); font-weight: 600; text-align: left; padding: 2px 3px; }
  td { padding: 2px 3px; }
  td input, td select { padding: 2px 4px; min-width: 4em; }
  .ops { white-space: nowrap; }
  .ops button { padding: 0 4px; }
</style>
