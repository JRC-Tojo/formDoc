<script lang="ts">
  // プレビュー。ページごとのSVGを表示する（変更のないページはエンジンから再送されず、保持中のSVGを使う）。
  import { app } from '../state.svelte';

  let zoom = $state(1);
  const hashes = $derived(app.pageHashes);
</script>

<div class="preview">
  <div class="bar">
    <span class="small muted">
      {hashes.length} ページ
      {#if app.result}・{app.result.compile_ms > 0 ? `${Math.round(app.result.compile_ms)} ms` : ''}{/if}
      {#if app.compiling}<span class="spin">更新中…</span>{/if}
    </span>
    <span class="zoom">
      <button class="ghost" onclick={() => (zoom = Math.max(0.4, zoom - 0.1))}>−</button>
      <span class="small">{Math.round(zoom * 100)}%</span>
      <button class="ghost" onclick={() => (zoom = Math.min(2.5, zoom + 0.1))}>＋</button>
    </span>
  </div>
  <div class="pages" style:--zoom={zoom}>
    {#each hashes as h, i (h + i)}
      <div class="page">
        {@html app.svgs.get(h) ?? ''}
      </div>
    {:else}
      <div class="empty muted">{app.compiling ? '組版しています…' : 'プレビューはまだありません'}</div>
    {/each}
  </div>
</div>

<style>
  .preview { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--preview-bg); }
  .bar { display: flex; justify-content: space-between; align-items: center; padding: 4px 10px; background: var(--panel); border-bottom: 1px solid var(--line); }
  .zoom { display: flex; align-items: center; gap: 2px; }
  .zoom button { padding: 0 8px; }
  .spin { margin-left: 8px; color: var(--accent); }
  .pages { overflow: auto; flex: 1; padding: 16px; display: flex; flex-direction: column; align-items: center; gap: 16px; }
  .page { width: calc(560px * var(--zoom)); background: #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25); flex: none; }
  .page :global(svg) { width: 100%; height: auto; display: block; }
  .empty { padding: 40px; }
</style>
