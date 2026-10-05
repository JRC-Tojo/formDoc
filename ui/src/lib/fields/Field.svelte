<script lang="ts">
  // 部品定義（components.toml）の1項目を、型に応じた入力欄として表示する。
  import { app } from '../state.svelte';
  import { track } from '../insert';
  import type { FieldDef, Issue } from '../types';
  import GridField from './GridField.svelte';
  import RowsField from './RowsField.svelte';

  let { def, value, onchange, issues = [] }: { def: FieldDef; value: any; onchange: (v: any) => void; issues?: Issue[] } = $props();

  const refs = $derived(app.catalog?.references ?? {});
  const vars = $derived(app.result?.vars ?? []);
  const own = $derived(issues.filter((i) => i.field === def.key || i.field?.startsWith(def.key + '.')));

  function num(v: string): number | string | null {
    if (v.trim() === '') return null;
    const n = Number(v.replace(/,/g, ''));
    return Number.isFinite(n) ? n : v;
  }

  async function pickFile() {
    const f = await app.platform!.files.openFile(['png', 'jpg', 'jpeg', 'svg', 'pdf']);
    if (!f) return;
    onchange(await app.addAsset(f.name, f.bytes));
  }
</script>

<div class="field" class:has-error={own.some((i) => i.severity === 'error')}>
  {#if def.type !== 'bool'}
    <label class="lbl" for={def.key}>
      {def.label}{#if def.required}<span class="req">*</span>{/if}
    </label>
  {/if}

  {#if def.type === 'text' || def.type === 'var'}
    <input id={def.key} type="text" value={value ?? ''} use:track data-insert={def.type === 'var' ? 'name' : 'ref'} readonly={def.readonly}
      class:mono={def.type === 'var'} oninput={(e) => onchange(e.currentTarget.value)} spellcheck="false" />
  {:else if def.type === 'expr'}
    <input id={def.key} type="text" class="mono" value={value ?? ''} use:track data-insert="name"
      oninput={(e) => onchange(e.currentTarget.value)} spellcheck="false" placeholder="例: w * L^2 / 8" />
  {:else if def.type === 'multiline'}
    <textarea id={def.key} rows="5" value={value ?? ''} use:track data-insert="ref" oninput={(e) => onchange(e.currentTarget.value)}></textarea>
  {:else if def.type === 'code'}
    <textarea id={def.key} rows="10" class="mono" value={value ?? ''} use:track data-insert="name"
      oninput={(e) => onchange(e.currentTarget.value)} spellcheck="false"></textarea>
  {:else if def.type === 'number'}
    <input id={def.key} type="text" inputmode="decimal" class="mono" value={value ?? ''} oninput={(e) => onchange(num(e.currentTarget.value))} />
  {:else if def.type === 'int'}
    <input id={def.key} type="number" step="1" min="0" class="mono" value={value ?? ''}
      oninput={(e) => onchange(e.currentTarget.value === '' ? null : Math.round(Number(e.currentTarget.value)))} />
  {:else if def.type === 'date'}
    <input id={def.key} type="date" value={value ?? ''} oninput={(e) => onchange(e.currentTarget.value)} />
  {:else if def.type === 'bool'}
    <label class="check"><input type="checkbox" checked={value ?? def.default ?? false} onchange={(e) => onchange(e.currentTarget.checked)} />{def.label}</label>
  {:else if def.type === 'select'}
    <select id={def.key} value={String(value ?? def.default ?? '')} onchange={(e) => onchange(def.key === 'level' ? Number(e.currentTarget.value) : e.currentTarget.value)}>
      {#each def.options ?? [] as o}<option value={o}>{o}</option>{/each}
    </select>
  {:else if def.type === 'kijun'}
    <div class="kijun">
      <select value={value?.abbr ?? ''} onchange={(e) => onchange({ ...(value ?? {}), abbr: e.currentTarget.value })}>
        <option value="">（なし）</option>
        {#each Object.entries(refs) as [abbr, r]}<option value={abbr} title={r.title}>【{r.label ?? abbr}】</option>{/each}
      </select>
      <input type="text" placeholder="第Ⅰ編 4.4.2" value={value?.loc ?? ''} oninput={(e) => onchange({ ...(value ?? {}), loc: e.currentTarget.value })} />
    </div>
  {:else if def.type === 'varlist'}
    <div class="varlist">
      {#each vars as v}
        {@const on = (value ?? []).includes(v.name)}
        <button class="chip" class:on onclick={() => onchange(on ? value.filter((x: string) => x !== v.name) : [...(value ?? []), v.name])} title={v.desc}>{v.name}</button>
      {/each}
    </div>
  {:else if def.type === 'table'}
    <RowsField columns={def.columns ?? []} rows={value ?? []} {onchange} />
  {:else if def.type === 'grid'}
    <GridField grid={value} {onchange} />
  {:else if def.type === 'file'}
    <div class="file">
      <span class="mono small">{value || '未選択'}</span>
      <button onclick={pickFile}>ファイルを選ぶ…</button>
    </div>
    <div class="small muted">PNG / JPG / SVG / PDF（PDFは図としてそのまま挿入）</div>
  {/if}

  {#if def.help}<div class="help small muted">{def.help}</div>{/if}
  {#each own as i}
    <div class="msg small {i.severity}">{i.message}</div>
  {/each}
</div>

<style>
  .field { margin-bottom: 12px; }
  .lbl { font-weight: 600; margin-bottom: 3px; font-size: 12px; }
  .req { color: var(--error); margin-left: 2px; }
  .help { margin-top: 3px; }
  .has-error :is(input, textarea, select) { border-color: var(--error); }
  .msg { margin-top: 3px; padding: 2px 6px; border-radius: 3px; }
  .msg.error { color: var(--error); background: var(--error-weak); }
  .msg.warning { color: var(--warn); background: var(--warn-weak); }
  .msg.info { color: var(--muted); background: var(--panel-2); }
  .check { display: flex; gap: 6px; align-items: center; }
  .check input { width: auto; }
  .kijun { display: grid; grid-template-columns: 9em 1fr; gap: 6px; }
  .varlist { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip { font-family: var(--mono); font-size: 11.5px; padding: 1px 7px; border-radius: 10px; }
  .chip.on { background: var(--accent); color: #fff; border-color: var(--accent); }
  .file { display: flex; align-items: center; gap: 8px; justify-content: space-between; }
</style>
