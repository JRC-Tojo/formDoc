<script lang="ts">
  // 中央ペイン：選択中の部品の入力フォーム、または文書情報。
  import { app } from '../state.svelte';
  import Field from '../fields/Field.svelte';
  import type { FieldDef } from '../types';

  const block = $derived(app.selected);
  const def = $derived(block ? app.catalog?.components[block.kind] : null);
  const issues = $derived(block ? app.issuesFor(block.id) : []);
  const result = $derived(block ? app.result?.blocks[block.id] : null);
  const general = $derived(issues.filter((i) => !i.field));

  const metaFields: FieldDef[] = [
    { key: 'title', label: '表題', type: 'text', required: true },
    { key: 'project', label: '業務名', type: 'text' },
    { key: 'author', label: '作成者（部署）', type: 'text' },
    { key: 'date', label: '作成日', type: 'text', help: '2026-10-02 の形式。PDFの作成日時にも使われます（同じ文書なら同じPDFになるよう、出力した日時は使いません）' },
    { key: 'chapter_start', label: '最初の章番号（§）', type: 'int', help: '分冊で章番号を続ける場合に指定' },
    { key: 'cover', label: '表紙を付ける', type: 'bool', default: true },
  ];
</script>

<div class="inspector">
  {#if block && def}
    <header>
      <div class="kind"><span class="icon">{def.icon}</span>{def.label}</div>
      {#if result?.summary}
        <div class="summary mono" class:ng={result.status === 'ng'} class:error={result.status === 'error'}>{result.summary}</div>
      {/if}
      {#if def.help}<p class="small muted">{def.help}</p>{/if}
      {#each general as i}<div class="msg small {i.severity}">{i.message}</div>{/each}
    </header>
    {#each def.fields as f (block.id + f.key)}
      <Field def={f} value={block.props[f.key]} {issues} onchange={(v) => app.setProp(block.id, f.key, v)} />
    {/each}
  {:else if app.doc}
    <header>
      <div class="kind">📄 文書情報</div>
      <p class="small muted">
        文書の型: <b>{app.template?.name}</b>（{app.template?.description}）<br />
        体裁（書体・余白・見出し番号・図表番号）は型で固定されており、ここでは変更できません。
      </p>
    </header>
    {#each metaFields as f}
      <Field def={f} value={(app.doc.meta as any)[f.key]} onchange={(v) => app.setMeta(f.key, v)} />
    {/each}
  {/if}
</div>

<style>
  .inspector { padding: 12px 14px; overflow: auto; height: 100%; }
  header { margin-bottom: 14px; padding-bottom: 10px; border-bottom: 1px solid var(--line); }
  .kind { font-weight: 700; font-size: 14px; display: flex; gap: 6px; align-items: center; }
  .icon { color: var(--muted); font-size: 12px; min-width: 1.8em; text-align: center; }
  .summary { margin-top: 6px; padding: 4px 8px; background: var(--panel-2); border-radius: 4px; font-family: var(--mono); }
  .summary.ng { background: var(--warn-weak); color: var(--warn); }
  .summary.error { background: var(--error-weak); color: var(--error); }
  p { margin: 6px 0 0; }
  .msg { margin-top: 4px; padding: 2px 6px; border-radius: 3px; }
  .msg.error { color: var(--error); background: var(--error-weak); }
  .msg.warning { color: var(--warn); background: var(--warn-weak); }
  .msg.info { color: var(--muted); background: var(--panel-2); }
</style>
