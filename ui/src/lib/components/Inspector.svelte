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

  // 章構成の情報（エンジンが判定した、執筆ガイド・操作の可否・割り当てられる章）
  const chapter = $derived(block ? app.chapterOf(block.id) : null);
  const choices = $derived(block?.kind === 'heading' ? (chapter?.choices ?? []) : []);
  /** 章を割り当てる（見出し文の変更が禁止された章なら見出し文も合わせる）。取り消しは1回 */
  function assign(id: string) {
    if (!block) return;
    const def = app.template?.chapters && findChapter(app.template.chapters, id);
    app.setProps(block.id, { chapter: id || null, ...(def && def['fixed-title'] ? { text: def.title } : {}) });
  }
  function findChapter(cs: NonNullable<typeof app.template>['chapters'], id: string): (typeof cs)[number] | null {
    for (const c of cs) {
      if (c.id === id) return c;
      const s = findChapter(c.sections ?? [], id);
      if (s) return s;
    }
    return null;
  }
  /** 決められた章の見出しでは、見出し文・階層の欄を読み取り専用にする */
  function fieldDef(f: FieldDef): FieldDef {
    if (block?.kind !== 'heading' || !chapter) return f;
    if (f.key === 'text' && chapter.fixed_title) return { ...f, readonly: true, help: '文書テンプレートで決められた見出し文です' };
    if (f.key === 'level' && chapter.no_move) return { ...f, readonly: true, help: '文書テンプレートで決められた章のため変更できません' };
    return f;
  }

  const metaIssues = $derived(
    (app.result?.issues ?? []).filter((i) => !i.block_id && i.field?.startsWith('meta.')).map((i) => ({ ...i, field: i.field!.slice(5) })),
  );
  const metaFields = $derived<FieldDef[]>(
    (app.template?.fields ?? []).map((f) => ({ key: f.key, label: f.label, type: f.type, required: f.required, help: f.help, options: f.options, default: f.default })),
  );
  const current = $derived(app.style?.source);
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
      {#if block.kind === 'group'}
        <p class="small">中の部品 {block.children?.length ?? 0} 個。中の変数はこのまとまりの中だけで使え、「公開する変数」だけが後ろの部品から使えます。</p>
        <div class="gops">
          <button class="small" onclick={() => (app.collapsed[block.id] = !app.collapsed[block.id])}>{app.collapsed[block.id] ? '一覧で開く' : '一覧で折りたたむ'}</button>
          <button class="small" onclick={() => app.ungroup(block.id)}>まとまりを解除</button>
        </div>
      {/if}
      {#if choices.length}
        <label class="chap small">
          章の種類
          <select value={block.props.chapter ?? ''} disabled={app.isLocked(block)} onchange={(e) => assign((e.currentTarget as HTMLSelectElement).value)}>
            <option value="">（決められた章ではない）</option>
            {#each choices as c (c.id)}<option value={c.id}>{c.title}{c.repeatable ? '（繰り返し可）' : ''}</option>{/each}
          </select>
        </label>
      {/if}
      {#if chapter?.guide}
        <div class="guide small"><b>執筆ガイド</b>　{chapter.guide}</div>
      {/if}
      {#if block.kind === 'fig-shapes'}
        <button class="primary draw" onclick={() => (app.dialog = { kind: 'shapes', blockId: block.id })}>✎ 図を描く…</button>
      {/if}
    </header>
    {#each def.fields as f (block.id + f.key)}
      <Field def={fieldDef(f)} value={block.props[f.key]} {issues} onchange={(v) => app.setProp(block.id, f.key, v)} />
    {/each}
  {:else if app.doc}
    <header>
      <div class="kind">📄 文書情報</div>
      {#if !app.style}
        <p class="start">文書テンプレートを選ぶと執筆を始められます。文書テンプレートは文書全体の体裁（書体・余白・見出し・表紙）と、ここで入力する項目を決めます。</p>
      {/if}
    </header>
    <div class="styles">
      <div class="lbl">文書テンプレート</div>
      {#each app.styles as st (st.path)}
        <button class="style" class:on={st.source === current} disabled={!st.info} onclick={() => app.chooseStyle(st)} title={st.error ?? st.path}>
          <span class="sname">{st.info?.name ?? st.path.split(/[\/]/).pop()}</span>
          <span class="small muted">{st.error ? `読み込めません: ${st.error}` : st.info?.description}</span>
        </button>
      {:else}
        <p class="small muted">文書テンプレートが見つかりません。</p>
      {/each}
      {#if app.style && !app.styles.some((s) => s.source === current)}
        <div class="small muted">この文書は保存時の文書テンプレート「{app.style.info.name}」で組版しています。</div>
      {/if}
      <div class="sfoot small">
        {#if app.systemPath}
          <button class="ghost small" onclick={() => app.platform?.system.openPath?.(app.systemPath!.styles)}>文書テンプレートのフォルダを開く</button>
        {:else}
          <button class="ghost small" onclick={() => app.importStyle()}>文書テンプレート（.typ）を取り込む…</button>
        {/if}
        <button class="ghost small" onclick={() => app.loadStyles()}>再読み込み</button>
      </div>
    </div>
    {#if app.style}
      <p class="small muted">体裁は文書テンプレートで固定されており、ここでは変更できません。</p>
      {#each metaFields as f (app.style.info.id + f.key)}
        <Field def={f} value={app.doc.meta[f.key]} issues={metaIssues} onchange={(v) => app.setMeta(f.key, v)} />
      {/each}
    {/if}
  {/if}
</div>

<style>
  .inspector { padding: 12px 14px; overflow: auto; height: 100%; }
  header { margin-bottom: 14px; padding-bottom: 10px; border-bottom: 1px solid var(--line); }
  .kind { font-weight: 700; font-size: 14px; display: flex; gap: 6px; align-items: center; }
  .chap { display: flex; gap: 8px; align-items: center; margin-top: 6px; }
  .guide { margin-top: 6px; padding: 6px 8px; border-left: 3px solid var(--accent); background: var(--accent-weak); border-radius: 4px; }
  .icon { color: var(--muted); font-size: 12px; min-width: 1.8em; text-align: center; }
  .summary { margin-top: 6px; padding: 4px 8px; background: var(--panel-2); border-radius: 4px; font-family: var(--mono); }
  .summary.ng { background: var(--warn-weak); color: var(--warn); }
  .summary.error { background: var(--error-weak); color: var(--error); }
  p { margin: 6px 0 0; }
  .draw { margin-top: 8px; }
  .gops { display: flex; gap: 6px; margin-top: 6px; }
  .start { padding: 8px 10px; background: var(--accent-weak); border-radius: 4px; }
  .styles { margin-bottom: 14px; display: flex; flex-direction: column; gap: 4px; }
  .lbl { font-weight: 600; font-size: 12px; }
  .style { display: flex; flex-direction: column; align-items: flex-start; text-align: left; white-space: normal; padding: 6px 10px; }
  .style.on { border-color: var(--accent); background: var(--accent-weak); }
  .sname { font-weight: 600; }
  .sfoot { display: flex; gap: 4px; }
  .msg { margin-top: 4px; padding: 2px 6px; border-radius: 3px; }
  .msg.error { color: var(--error); background: var(--error-weak); }
  .msg.warning { color: var(--warn); background: var(--warn-weak); }
  .msg.info { color: var(--muted); background: var(--panel-2); }
</style>
