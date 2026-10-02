<script lang="ts">
  // システム設定。デスクトップ版はシステムフォルダの settings.json、Web版はブラウザ内に保存する。
  import { app } from '../state.svelte';
  import { has } from '../platform';
  import Modal from './Modal.svelte';
  import Gate from './Gate.svelte';

  const s = $derived(app.settings);

  async function removeFolder(f: string) {
    await app.saveSettings({ templateFolders: s.templateFolders.filter((x) => x !== f) });
    await app.loadTemplates();
  }

  async function pickPublish() {
    const dir = await app.platform?.folder?.pick();
    if (dir) app.saveSettings({ publishFolder: dir });
  }

  function open(path: string) {
    app.platform?.system.openPath?.(path);
  }
</script>

<Modal title="設定" onclose={() => (app.dialog = null)} width="640px">
  <div class="form">
    <section>
      <h4>表示</h4>
      <div class="row">
        <span class="k">テーマ</span>
        <select value={s.theme} onchange={(e) => app.saveSettings({ theme: e.currentTarget.value as any })}>
          <option value="system">OSの設定に合わせる</option>
          <option value="light">ライト</option>
          <option value="dark">ダーク</option>
        </select>
      </div>
      <div class="row">
        <span class="k">文字の大きさ</span>
        <input type="range" min="80" max="150" step="10" value={s.fontScale} oninput={(e) => app.saveSettings({ fontScale: Number(e.currentTarget.value) })} />
        <span class="v mono">{s.fontScale}%</span>
      </div>
      <p class="small muted">プレビューの文書（PDF）の見た目は変わりません。</p>
    </section>

    <section>
      <h4>最近使ったファイル</h4>
      <div class="row">
        <span class="k">保存する件数</span>
        <input type="number" min="0" max="50" value={s.recentMax} onchange={(e) => app.saveSettings({ recentMax: Math.max(0, Math.min(50, Number(e.currentTarget.value) || 0)) })} />
        <Gate cap="recentFiles"><button class="small" onclick={() => app.saveSettings({ recent: [] })} disabled={!s.recent.length}>一覧を消去</button></Gate>
      </div>
      {#if !has('recentFiles')}<p class="small muted">Web版ではファイルをパスで開き直せないため使えません。</p>{/if}
    </section>

    <section>
      <h4>テンプレート</h4>
      <div class="small muted">読み込むフォルダ（このPCのシステムフォルダの分は常に読み込みます）</div>
      <ul>
        {#each s.templateFolders as f}
          <li><span class="mono small">{f}</span><button class="ghost small" onclick={() => removeFolder(f)}>外す</button></li>
        {:else}
          <li class="small muted">なし</li>
        {/each}
      </ul>
      <button class="small" onclick={() => app.addTemplateFolder()}>＋ フォルダを追加…</button>
      <div class="row">
        <span class="k">既定の公開先</span>
        <span class="mono small path">{s.publishFolder || '未設定'}</span>
        <Gate cap="localFolder"><button class="small" onclick={pickPublish}>選ぶ…</button></Gate>
      </div>
    </section>

    <section>
      <h4>システムフォルダ</h4>
      {#if app.systemPath}
        <p class="small">設定・スタイル（styles）・テンプレート（templates）・VSCode用の作業フォルダ（projects）を置く場所です。</p>
        <div class="row"><span class="mono small path">{app.systemPath.root}</span><button class="small" onclick={() => open(app.systemPath!.root)}>開く</button></div>
        <div class="row"><span class="k">スタイル</span><span class="mono small path">{app.systemPath.styles}</span><button class="small" onclick={() => open(app.systemPath!.styles)}>開く</button></div>
        <p class="small muted">styles に Typst ファイル（1スタイル＝1ファイル）を置くと、文書情報のスタイルに並びます（再読み込みで反映）。</p>
      {:else}
        <p class="small muted">Web版では設定・スタイル・テンプレートをブラウザ内に保存します。</p>
      {/if}
      <button class="small" onclick={async () => { await app.loadStyles(); await app.loadTemplates(); app.flash('スタイルとテンプレートを読み込み直しました'); }}>スタイル・テンプレートを再読み込み</button>
    </section>
  </div>
  {#snippet footer()}
    <button class="primary" onclick={() => (app.dialog = null)}>閉じる</button>
  {/snippet}
</Modal>

<style>
  .form { padding: 8px 18px 14px; }
  section { padding: 8px 0; border-bottom: 1px solid var(--line); }
  section:last-child { border-bottom: none; }
  h4 { margin: 4px 0 6px; }
  .row { display: flex; align-items: center; gap: 10px; margin: 4px 0; }
  .k { min-width: 8em; font-weight: 600; font-size: 12px; }
  .row select, .row input[type='number'] { width: 14em; }
  .row input[type='range'] { width: 14em; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  ul { list-style: none; padding: 0; margin: 4px 0; }
  li { display: flex; align-items: center; gap: 8px; }
</style>
