<script lang="ts">
  // 定義済み変数の一覧。クリックで最後に触った入力欄へ差し込む（本文は {{名前}}、式は 名前）。
  import { app } from '../state.svelte';
  import { insertVar } from '../insert';

  let filter = $state('');
  const vars = $derived(
    (app.result?.vars ?? []).filter((v) => !filter || v.name.toLowerCase().includes(filter.toLowerCase()) || v.desc.includes(filter)),
  );

  function pick(name: string, blockId: string) {
    if (!insertVar(name)) app.selectedId = blockId;
  }
</script>

<div class="palette">
  <div class="head">
    <span class="title">変数</span>
    <input type="search" placeholder="絞り込み" bind:value={filter} />
  </div>
  <div class="list">
    {#each vars as v (v.name)}
      <button class="var" onclick={() => pick(v.name, v.block_id)} title="{v.desc}（クリックで入力欄に差し込み）">
        <span class="name mono">{v.name}</span>
        <span class="val mono">{v.text}<span class="unit">{v.unit}</span></span>
        <span class="desc small muted">{v.desc}</span>
      </button>
    {:else}
      <div class="small muted empty">変数定義・計算の部品を追加すると、ここに一覧されます。</div>
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
</style>
