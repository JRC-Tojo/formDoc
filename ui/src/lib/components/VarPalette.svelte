<script lang="ts">
  // 変数の一覧。選択中の部品で使える変数（同じ節・部品テンプレートの中のローカル変数とグローバル変数）を並べる。
  // クリックで最後に触った入力欄へ差し込む（本文は {{名前}}、式は 名前）。
  import { app } from '../state.svelte';
  import { insertVar } from '../insert';
  import { visibleVars } from '../tree';

  let filter = $state('');
  let all = $state(false);
  const usable = $derived(
    all || !app.selectedId ? (app.result?.vars ?? []) : visibleVars(app.doc?.blocks ?? [], app.result?.vars ?? [], app.selectedId, 'at'),
  );
  const vars = $derived(usable.filter((v) => !filter || v.name.toLowerCase().includes(filter.toLowerCase()) || v.desc.includes(filter)));

  function pick(name: string, blockId: string) {
    if (!insertVar(name)) app.selectedId = blockId;
  }
</script>

<div class="palette">
  <div class="head">
    <span class="title">変数</span>
    <input type="search" placeholder="絞り込み" bind:value={filter} />
    <label class="all small" title="チェックを外すと、選択中の部品で使える変数だけを表示します"><input type="checkbox" bind:checked={all} />すべて</label>
  </div>
  <div class="list">
    {#each vars as v (v.block_id + v.name)}
      <button class="var" onclick={() => pick(v.name, v.block_id)} title="{v.desc}（{v.scope == null ? 'グローバル' : 'ローカル'}。クリックで入力欄に差し込み）">
        <span class="name mono">{v.name}{#if v.scope == null}<span class="glob" title="グローバル変数（文書全体で使える）">🌐</span>{/if}</span>
        <span class="val mono">{v.text}<span class="unit">{v.unit}</span></span>
        <span class="desc small muted">{v.desc}</span>
      </button>
    {:else}
      <div class="small muted empty">{all || !app.selectedId ? '変数定義・計算の部品を追加すると、ここに一覧されます。' : 'この位置で使える変数はありません。'}</div>
    {/each}
  </div>
</div>

<style>
  .palette { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .head { display: flex; gap: 8px; align-items: center; padding: 6px 10px; border-bottom: 1px solid var(--line); }
  .title { font-weight: 700; white-space: nowrap; }
  .head input { padding: 2px 6px; }
  .list { overflow: auto; flex: 1; padding: 4px; }
  .var {
    display: grid; grid-template-columns: auto 1fr; column-gap: 8px; width: 100%; text-align: left;
    border: none; background: none; padding: 3px 6px; border-radius: 4px;
  }
  .var:hover { background: var(--accent-weak); }
  .name { font-weight: 600; color: var(--accent); }
  .val { text-align: right; }
  .unit { color: var(--muted); margin-left: 3px; font-size: 11px; }
  .desc { grid-column: 1 / -1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty { padding: 8px; }
  .all { display: flex; gap: 3px; align-items: center; white-space: nowrap; }
  .all input { width: auto; }
  .glob { font-size: 10px; margin-left: 2px; }
</style>
