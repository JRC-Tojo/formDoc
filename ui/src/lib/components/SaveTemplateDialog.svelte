<script lang="ts">
  // 「テンプレートとして保存」。見出しなら配下の節ごと、それ以外はその部品を保存する。
  // 変数ごとに「入力（受け取る）／公開（外から使える）／内部」を決めておくと、挿入する側は入力をつなぐだけで済む。
  import { app, b64encode } from '../state.svelte';
  import { analyze } from '../vars';
  import type { Block, TemplateFile } from '../types';
  import Modal from './Modal.svelte';
  import Gate from './Gate.svelte';

  let { blockId }: { blockId: string } = $props();

  const comps = $derived(app.catalog?.components ?? {});
  const fragment = $derived<Block[]>(JSON.parse(JSON.stringify(app.fragmentOf(blockId))));
  const usage = $derived(analyze(fragment, comps));
  const head = $derived(fragment[0]);

  let name = $state('');
  let description = $state('');
  let category = $state('');
  let styleOnly = $state(true);
  let dest = $state<'local' | 'publish'>('local');
  let publishFolder = $state(app.settings.publishFolder);
  let saving = $state(false);

  type Role = 'input' | 'export' | 'internal';
  let roles = $state<Record<string, Role>>({});
  let labels = $state<Record<string, string>>({});

  function defBlock(n: string): Block | undefined {
    return fragment.find((b) => b.id === usage.definedBy[n]);
  }

  // 初期値：見出し文を名前に、変数定義は「入力」、計算結果は「公開」
  $effect.pre(() => {
    if (!name) name = String(head?.props.text ?? head?.props.caption ?? comps[head?.kind ?? '']?.label ?? '');
    for (const n of usage.defined) {
      if (roles[n]) continue;
      const b = defBlock(n);
      roles[n] = b?.kind === 'vdef' ? 'input' : 'export';
      labels[n] = String(b?.props.desc ?? '');
    }
    for (const n of usage.external) if (labels[n] === undefined) labels[n] = app.result?.vars.find((v) => v.name === n)?.desc ?? '';
  });

  async function pickPublish() {
    const f = app.platform?.folder;
    const dir = await f?.pick();
    if (dir) {
      publishFolder = dir;
      app.saveSettings({ publishFolder: dir });
    }
  }

  function unitOf(n: string): string {
    return String(defBlock(n)?.props.unit ?? app.result?.vars.find((v) => v.name === n)?.unit ?? '');
  }

  async function save() {
    saving = true;
    const assets: Record<string, string> = {};
    for (const b of fragment) {
      const file = b.kind === 'image' ? String(b.props.file ?? '') : '';
      if (file && app.assets[file]) assets[file] = b64encode(app.assets[file]);
    }
    const t: TemplateFile = {
      format: 'formdoc-template',
      version: 1,
      name: name.trim(),
      description: description.trim(),
      category: category.trim(),
      styles: styleOnly && app.template ? [app.template.id] : [],
      created: new Date().toISOString().slice(0, 10),
      blocks: fragment,
      interface: {
        inputs: [
          ...usage.external.map((n) => ({ name: n, label: labels[n] ?? '', unit: unitOf(n), default: null })),
          ...usage.defined
            .filter((n) => roles[n] === 'input')
            .map((n) => ({ name: n, label: labels[n] ?? '', unit: unitOf(n), default: Number(defBlock(n)?.props.value ?? 0) })),
        ],
        exports: usage.defined.filter((n) => roles[n] === 'export').map((n) => ({ name: n, label: labels[n] ?? '' })),
      },
      assets,
    };
    const ok = await app.saveTemplate(t, dest === 'local' ? null : publishFolder);
    saving = false;
    if (ok) app.dialog = null;
  }

  const canSave = $derived(!!name.trim() && !saving && (dest === 'local' || !!publishFolder));
</script>

<Modal title="テンプレートとして保存" onclose={() => (app.dialog = null)} width="760px">
  <div class="form">
    <div class="row2">
      <label>名前<input type="text" bind:value={name} /></label>
      <label>分類<input type="text" bind:value={category} placeholder="例: 断面、荷重、照査" /></label>
    </div>
    <label>説明<textarea rows="2" bind:value={description}></textarea></label>
    <div class="small muted">保存する部品：{fragment.length} 個（{head?.kind === 'heading' ? '見出しと配下の節' : comps[head?.kind ?? '']?.label}）</div>

    <h4>変数</h4>
    <p class="small muted">
      「入力」は挿入時に値を入れるか既存の変数につなぎます（既定値はいまの値）。「公開」は挿入後に文書で使えます。「内部」は名前が重なれば自動で付け替えます。
    </p>
    <table>
      <thead><tr><th>変数</th><th>扱い</th><th>説明（挿入時に表示）</th></tr></thead>
      <tbody>
        {#each usage.external as n}
          <tr>
            <td class="mono">{n}</td>
            <td><span class="small">入力（必須）</span><div class="small muted">この範囲の外で定義</div></td>
            <td><input type="text" bind:value={labels[n]} /></td>
          </tr>
        {/each}
        {#each usage.defined as n}
          <tr>
            <td class="mono">{n}</td>
            <td>
              <select bind:value={roles[n]}>
                {#if defBlock(n)?.kind === 'vdef'}<option value="input">入力（既定値 {defBlock(n)?.props.value}）</option>{/if}
                <option value="export">公開</option>
                <option value="internal">内部</option>
              </select>
            </td>
            <td><input type="text" bind:value={labels[n]} /></td>
          </tr>
        {:else}
          {#if !usage.external.length}<tr><td colspan="3" class="small muted">変数はありません</td></tr>{/if}
        {/each}
      </tbody>
    </table>

    <h4>保存先</h4>
    <label class="radio"><input type="radio" bind:group={dest} value="local" />このPC（システムフォルダ{app.systemPath ? `：${app.systemPath.templates}` : '：ブラウザ内'}）</label>
    <label class="radio"><input type="radio" bind:group={dest} value="publish" />公開フォルダ（共有フォルダなど。読み込む側はこのフォルダを指定）</label>
    {#if dest === 'publish'}
      <div class="pub">
        {#if app.systemPath}
          <span class="mono small">{publishFolder || '未設定'}</span>
          <Gate cap="localFolder"><button class="small" onclick={pickPublish}>フォルダを選ぶ…</button></Gate>
        {:else}
          <span class="small muted">Web版ではファイル（.fdtpl）をダウンロードします。公開フォルダに置いてください。</span>
        {/if}
      </div>
    {/if}
    <label class="radio"><input type="checkbox" bind:checked={styleOnly} />このスタイル（{app.template?.name}）でだけ使う</label>
  </div>

  {#snippet footer()}
    <button onclick={() => (app.dialog = null)}>キャンセル</button>
    <button class="primary" disabled={!canSave} onclick={save}>保存</button>
  {/snippet}
</Modal>

<style>
  .form { padding: 14px 18px; display: flex; flex-direction: column; gap: 8px; }
  .row2 { display: grid; grid-template-columns: 2fr 1fr; gap: 10px; }
  label { font-size: 12px; font-weight: 600; }
  h4 { margin: 8px 0 0; }
  table { border-collapse: collapse; width: 100%; }
  th { text-align: left; font-size: 11px; color: var(--muted); padding: 2px 6px; }
  td { padding: 3px 6px; border-bottom: 1px solid var(--line); }
  .radio { display: flex; gap: 6px; align-items: center; font-weight: normal; }
  .radio input { width: auto; }
  .pub { display: flex; gap: 8px; align-items: center; padding-left: 22px; }
</style>
