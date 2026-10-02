<script lang="ts">
  // 片方の版でしか使えない機能を包む。使えない版ではグレーアウトし、理由をツールチップで示す。
  import type { Snippet } from 'svelte';
  import { has, reason, type Capability } from '../platform';

  let { cap, children }: { cap: Capability; children: Snippet } = $props();
  const enabled = $derived(has(cap));
</script>

{#if enabled}
  {@render children()}
{:else}
  <span class="gate" title={reason(cap)} aria-disabled="true">
    {@render children()}
  </span>
{/if}

<style>
  .gate { display: inline-flex; opacity: 0.45; cursor: not-allowed; }
  .gate :global(*) { pointer-events: none; }
</style>
