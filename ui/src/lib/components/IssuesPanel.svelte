<script lang="ts">
  // 検証パネル：計算エラー・照査NG・表記Lint・組版エラーをまとめて表示する。
  import { app } from '../state.svelte';
  import type { Issue } from '../types';

  let show = $state<Record<string, boolean>>({ error: true, warning: true, info: false });
  const issues = $derived(app.result?.issues ?? []);
  const counts = $derived({
    error: issues.filter((i) => i.severity === 'error').length,
    warning: issues.filter((i) => i.severity === 'warning').length,
    info: issues.filter((i) => i.severity === 'info').length,
  });
  const visible = $derived(issues.filter((i) => show[i.severity]));
  const fixableCodes = $derived([...new Set(issues.filter((i) => i.fix).map((i) => i.code))]);

  const labels: Record<string, string> = {
    'lint-punctuation': '句読点',
    'lint-fullwidth': '全角英数字',
    'lint-wording': '表記ゆれ',
    'lint-unit': '単位表記',
  };

  function go(i: Issue) {
    if (i.block_id) app.selectedId = i.block_id;
    else if (i.field?.startsWith('meta.') || i.code === 'style') app.selectedId = null;
  }

  function blockLabel(i: Issue): string {
    if (!i.block_id) return '文書情報';
    const b = app.doc?.blocks.find((x) => x.id === i.block_id);
    return b ? (app.catalog?.components[b.kind]?.label ?? b.kind) : '';
  }
</script>

<div class="issues">
  <div class="bar">
    <span class="title">検証</span>
    <label class="tg error"><input type="checkbox" bind:checked={show.error} />エラー {counts.error}</label>
    <label class="tg warning"><input type="checkbox" bind:checked={show.warning} />注意 {counts.warning}</label>
    <label class="tg info"><input type="checkbox" bind:checked={show.info} />情報 {counts.info}</label>
    <span class="spacer"></span>
    {#each fixableCodes as code}
      <button class="small" onclick={() => app.applyAllFixes(code)} title="文書全体の同じ種類の指摘をまとめて直します">{labels[code] ?? code}をすべて修正</button>
    {/each}
    {#if app.result && !app.result.exportable}
      <span class="block-note small">エラーを解消するまでPDFは出力できません</span>
    {/if}
  </div>
  <ul>
    {#each visible as i}
      <li class={i.severity}>
        <button class="msg" onclick={() => go(i)}>
          <span class="sev">{i.severity === 'error' ? '✕' : i.severity === 'warning' ? '!' : 'i'}</span>
          <span class="where small muted">{blockLabel(i)}</span>
          <span>{i.message}</span>
        </button>
        {#if i.fix && i.block_id}
          <button class="fix small" onclick={() => app.applyFix(i)}>「{i.fix.from}」→「{i.fix.to}」</button>
        {/if}
      </li>
    {:else}
      <li class="none small muted">{issues.length ? '表示する項目はありません' : '問題は見つかりませんでした'}</li>
    {/each}
  </ul>
</div>

<style>
  .issues { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .bar { display: flex; gap: 10px; align-items: center; padding: 4px 10px; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .title { font-weight: 700; }
  .tg { display: inline-flex; gap: 3px; align-items: center; font-size: 12px; }
  .tg input { width: auto; }
  .tg.error { color: var(--error); }
  .tg.warning { color: var(--warn); }
  .spacer { flex: 1; }
  .block-note { color: var(--error); }
  ul { list-style: none; margin: 0; padding: 2px 0; overflow: auto; flex: 1; }
  li { display: flex; align-items: center; gap: 6px; padding: 0 10px; }
  li:hover { background: var(--panel-2); }
  .msg { flex: 1; display: flex; gap: 8px; align-items: baseline; border: none; background: none; padding: 3px 0; text-align: left; }
  .sev { font-weight: 700; width: 1em; text-align: center; }
  li.error .sev { color: var(--error); }
  li.warning .sev { color: var(--warn); }
  li.info .sev { color: var(--muted); }
  .where { min-width: 6em; }
  .fix { padding: 1px 6px; }
  .none { padding: 6px 10px; }
</style>
