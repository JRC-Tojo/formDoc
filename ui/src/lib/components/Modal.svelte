<script lang="ts">
  // ダイアログの枠。Esc・背景のクリックで閉じる。
  import type { Snippet } from 'svelte';

  let {
    title,
    onclose,
    width = '760px',
    height = 'auto',
    children,
    footer,
  }: { title: string; onclose: () => void; width?: string; height?: string; children: Snippet; footer?: Snippet } = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} style:width style:height>
    <header>
      <span class="title">{title}</span>
      <button class="ghost close" onclick={onclose} title="閉じる (Esc)">✕</button>
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.35); z-index: 100; display: grid; place-items: center; }
  .modal {
    max-width: calc(100vw - 32px); max-height: calc(100vh - 32px); background: var(--panel); color: var(--text);
    border-radius: 8px; box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3); display: flex; flex-direction: column; min-height: 0;
  }
  header { display: flex; align-items: center; padding: 10px 14px; border-bottom: 1px solid var(--line); }
  .title { font-weight: 700; font-size: 14px; flex: 1; }
  .close { padding: 2px 8px; }
  .body { flex: 1; min-height: 0; overflow: auto; }
  footer { display: flex; gap: 8px; justify-content: flex-end; padding: 10px 14px; border-top: 1px solid var(--line); }
</style>
