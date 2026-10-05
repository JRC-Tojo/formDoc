<script lang="ts">
  // 「部品を追加」ダイアログ。部品と部品テンプレートを同じ一覧から選んで、選択中の部品の下に入れる。
  // 部品テンプレートは1つのまとまり（group）として入る。中の変数はその中だけで使えるため、挿入する側は
  // 「入力」の変数をつなぎ、「公開」の変数の名前を決めるだけでよい。
  import { app } from '../state.svelte';
  import { analyze, renameBlocks, uniqueName, type RenameMap } from '../vars';
  import { visibleVars } from '../tree';
  import type { Block, TemplateEntry } from '../types';
  import Modal from './Modal.svelte';

  type Item =
    | { type: 'component'; kind: string; label: string; icon: string; help: string; group: string }
    | { type: 'template'; entry: TemplateEntry; label: string; icon: string; help: string; group: string };

  let query = $state('');
  let group = $state<string>('すべて');
  let picked = $state<Item | null>(null);
  /** 部品テンプレートの挿入設定（選んだら表示） */
  let binding = $state<null | { entry: TemplateEntry; inputs: Bind[]; exports: { name: string; label: string; to: string }[] }>(null);
  type Bind = { name: string; label: string; unit?: string; hasDefault: boolean; mode: 'var' | 'value'; varName: string; value: string };

  const comps = $derived(app.catalog?.components ?? {});
  const styleId = $derived(app.template?.id ?? '');

  const items = $derived.by<Item[]>(() => {
    const out: Item[] = [];
    for (const kind of app.template?.blocks ?? []) {
      const c = comps[kind];
      if (!c || c.deprecated) continue;
      out.push({ type: 'component', kind, label: c.label, icon: c.icon ?? '•', help: c.help ?? '', group: `部品：${c.category ?? 'その他'}` });
    }
    for (const entry of app.templates) {
      const f = entry.file;
      if (f.styles?.length && !f.styles.includes(styleId)) continue;
      out.push({ type: 'template', entry, label: f.name, icon: '❖', help: f.description, group: `部品テンプレート：${entry.source}` });
    }
    return out;
  });
  const groups = $derived(['すべて', ...new Set(items.map((i) => i.group))]);
  const shown = $derived(
    items.filter((i) => (group === 'すべて' || i.group === group) && (!query || (i.label + i.help).toLowerCase().includes(query.toLowerCase()))),
  );

  /** 挿入位置で使える変数（同じ節・部品テンプレートの中のローカル変数と、グローバル変数） */
  const before = $derived(
    [...new Set(visibleVars(app.doc?.blocks ?? [], app.result?.vars ?? [], app.selectedId, 'after').map((v) => v.name))],
  );
  /** 公開する変数と重なってはいけない名前（挿入位置で使える変数） */
  const taken = $derived(new Set(before));

  function close() {
    app.dialog = null;
  }

  function choose(it: Item) {
    if (it.type === 'component') {
      app.addBlock(it.kind);
      if (app.dialog?.kind === 'insert') close();
      return;
    }
    const f = it.entry.file;
    const inputs: Bind[] = f.interface.inputs.map((x) => {
      const same = before.includes(x.name);
      return {
        name: x.name, label: x.label, unit: x.unit, hasDefault: x.default != null,
        mode: same ? 'var' : 'value', varName: same ? x.name : (before[0] ?? ''), value: x.default != null ? String(x.default) : '',
      };
    });
    const used = new Set(taken);
    const exports = f.interface.exports.map((x) => {
      const to = uniqueName(x.name, used);
      used.add(to);
      return { ...x, to };
    });
    if (!inputs.length && !exports.length) return insert({ entry: it.entry, inputs, exports });
    binding = { entry: it.entry, inputs, exports };
  }

  const VALID = /^[A-Za-z][A-Za-z0-9_]*$/;
  const bindErrors = $derived.by(() => {
    if (!binding) return [];
    const errs: string[] = [];
    for (const b of binding.inputs) {
      if (b.mode === 'var' && !b.varName) errs.push(`「${b.label || b.name}」につなぐ変数を選んでください`);
      if (b.mode === 'value' && !Number.isFinite(Number(b.value)) ) errs.push(`「${b.label || b.name}」の値を数値で入力してください`);
      if (b.mode === 'value' && b.value.trim() === '') errs.push(`「${b.label || b.name}」の値を入力してください`);
    }
    const names = binding.exports.map((e) => e.to);
    for (const e of binding.exports) {
      if (!VALID.test(e.to)) errs.push(`公開する変数名「${e.to}」は使えません`);
      else if (taken.has(e.to)) errs.push(`変数名「${e.to}」は文書内で既に使われています`);
      else if (names.filter((n) => n === e.to).length > 1) errs.push(`変数名「${e.to}」が重複しています`);
    }
    return errs;
  });

  /** 変数のつなぎ替えを適用して、まとまり（group）として挿入する */
  async function insert(bnd: NonNullable<typeof binding>) {
    const f = bnd.entry.file;
    let blocks: Block[] = JSON.parse(JSON.stringify(f.blocks));
    const usage = analyze(blocks, comps);
    const map: RenameMap = {};
    for (const e of bnd.exports) map[e.name] = e.to;
    const prepend: Block[] = [];
    for (const b of bnd.inputs) {
      const defBlock = usage.definedBy[b.name];
      if (b.mode === 'var') {
        if (b.varName !== b.name) map[b.name] = b.varName;
        // 部品テンプレート内の既定値の定義は、つないだ変数で置き換えるので除く
        if (defBlock) blocks = blocks.filter((x) => x.id !== defBlock || x.kind !== 'vdef');
      } else if (defBlock) {
        const vb = blocks.find((x) => x.id === defBlock);
        if (vb) vb.props.value = Number(b.value);
      } else {
        prepend.push({ id: 'in-' + b.name, kind: 'vdef', props: { name: b.name, value: Number(b.value), unit: b.unit ?? '', desc: b.label, show: false } });
      }
    }
    blocks = renameBlocks([...prepend, ...blocks], comps, map);
    await app.insertTemplate(f.name, blocks, bnd.exports.map((e) => e.to), f.assets ?? {});
    app.flash(`部品テンプレート「${f.name}」を挿入しました`);
    close();
  }
</script>

<Modal title={binding ? `部品テンプレートの挿入：${binding.entry.file.name}` : '部品を追加'} onclose={close} width="900px" height="620px">
  {#if !binding}
    <div class="layout">
      <nav>
        {#each groups as g}
          <button class="ghost" class:on={group === g} onclick={() => (group = g)}>{g}</button>
        {/each}
        <div class="navfoot">
          <button class="small" onclick={() => app.addTemplateFolder()} title="フォルダ内の部品テンプレート（.fdtpl）を一覧に加えます">＋ 部品テンプレートのフォルダを追加…</button>
        </div>
      </nav>
      <section class="list">
        <input type="search" placeholder="検索（名前・説明）" bind:value={query} />
        <div class="small muted note">選択中の部品の下に追加します。ダブルクリックですぐに追加できます。</div>
        <div class="grid">
          {#each shown as it}
            <button class="item" class:on={picked === it} onclick={() => (picked = it)} ondblclick={() => choose(it)}>
              <span class="icon">{it.icon}</span>
              <span class="label">{it.label}</span>
              {#if it.type === 'template'}<span class="tag small">部品テンプレート</span>{/if}
            </button>
          {:else}
            <div class="muted small">該当するものがありません</div>
          {/each}
        </div>
      </section>
      <aside>
        {#if picked}
          <div class="ptitle"><span class="icon">{picked.icon}</span>{picked.label}</div>
          <p class="small">{picked.help || '（説明なし）'}</p>
          {#if picked.type === 'template'}
            {@const f = picked.entry.file}
            <div class="small muted">読み込み元：{picked.entry.source}{#if picked.entry.path}<br /><span class="mono">{picked.entry.path}</span>{/if}</div>
            <div class="small muted">部品 {f.blocks.length} 個</div>
            {#if f.interface.inputs.length}
              <div class="sub">入力する変数</div>
              <ul class="small">{#each f.interface.inputs as x}<li><span class="mono">{x.name}</span> {x.label}{x.default != null ? `（既定 ${x.default}）` : '（必須）'}</li>{/each}</ul>
            {/if}
            {#if f.interface.exports.length}
              <div class="sub">公開される変数</div>
              <ul class="small">{#each f.interface.exports as x}<li><span class="mono">{x.name}</span> {x.label}</li>{/each}</ul>
            {/if}
          {/if}
          <button class="primary" onclick={() => choose(picked!)}>{picked.type === 'template' ? '挿入…' : '追加'}</button>
        {:else}
          <p class="muted small">左の一覧から選んでください。</p>
        {/if}
      </aside>
    </div>
  {:else}
    <div class="bind">
      {#if binding.inputs.length}
        <h4>入力する変数</h4>
        <p class="small muted">部品テンプレートが受け取る値です。文書の既存の変数につなぐか、値を入れてください。</p>
        <table>
          <tbody>
            {#each binding.inputs as b}
              <tr>
                <td><span class="mono">{b.name}</span><div class="small muted">{b.label}{b.unit ? `（${b.unit}）` : ''}</div></td>
                <td>
                  <label class="radio"><input type="radio" bind:group={b.mode} value="var" disabled={!before.length} />既存の変数</label>
                  <label class="radio"><input type="radio" bind:group={b.mode} value="value" />値を入れる</label>
                </td>
                <td>
                  {#if b.mode === 'var'}
                    <select bind:value={b.varName}>{#each before as v}<option value={v}>{v}</option>{/each}</select>
                  {:else}
                    <input type="text" class="mono" bind:value={b.value} placeholder={b.hasDefault ? '' : '必須'} />
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#if binding.exports.length}
        <h4>公開される変数</h4>
        <p class="small muted">挿入後、文書の続きで使える変数です。名前が重なる場合は変えてください。</p>
        <table>
          <tbody>
            {#each binding.exports as e}
              <tr>
                <td><span class="mono">{e.name}</span><div class="small muted">{e.label}</div></td>
                <td>→</td>
                <td><input type="text" class="mono" bind:value={e.to} /></td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
      <p class="small muted">部品テンプレートは1つのまとまりとして入ります。内部の変数はその中だけで使えるため、文書の変数と名前が重なっても問題ありません。</p>
      {#each bindErrors as e}<div class="err small">{e}</div>{/each}
    </div>
  {/if}

  {#snippet footer()}
    {#if binding}
      <button onclick={() => (binding = null)}>戻る</button>
      <button class="primary" disabled={bindErrors.length > 0} onclick={() => insert(binding!)}>挿入</button>
    {:else}
      <button onclick={close}>閉じる</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .layout { display: grid; grid-template-columns: 200px 1fr 240px; height: 100%; min-height: 0; }
  nav { border-right: 1px solid var(--line); padding: 8px; display: flex; flex-direction: column; gap: 2px; overflow: auto; }
  nav button { text-align: left; white-space: normal; }
  nav button.on { background: var(--accent-weak); color: var(--accent); font-weight: 600; }
  .navfoot { margin-top: auto; padding-top: 8px; }
  .navfoot button { white-space: normal; text-align: left; width: 100%; }
  .list { padding: 10px; overflow: auto; min-height: 0; }
  .note { margin: 4px 0 8px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 6px; }
  .item { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 10px 6px; white-space: normal; }
  .item.on { border-color: var(--accent); background: var(--accent-weak); }
  .item .icon { font-size: 16px; color: var(--muted); }
  .item .label { text-align: center; }
  .tag { color: var(--accent); }
  aside { border-left: 1px solid var(--line); padding: 12px; overflow: auto; display: flex; flex-direction: column; gap: 6px; }
  .ptitle { font-weight: 700; display: flex; gap: 6px; }
  .sub { font-weight: 600; font-size: 12px; margin-top: 6px; }
  aside ul { margin: 2px 0; padding-left: 1.2em; }
  aside .primary { margin-top: auto; align-self: flex-end; }
  .bind { padding: 14px 18px; }
  h4 { margin: 8px 0 2px; }
  table { border-collapse: collapse; width: 100%; margin: 6px 0 10px; }
  td { padding: 4px 6px; border-bottom: 1px solid var(--line); vertical-align: middle; }
  .radio { display: inline-flex; gap: 3px; align-items: center; margin-right: 10px; }
  .radio input { width: auto; }
  .err { color: var(--error); }
</style>
