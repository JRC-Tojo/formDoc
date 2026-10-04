<script lang="ts">
  // 新しい版の通知（Web版・デスクトップ版で共通の画面）。検知と更新の仕組みは platform の updater が持つ。
  // 起動時と一定の間隔（Web：10分、デスクトップ：6時間）、画面に戻ってきたときに確認する。
  import { app } from '../state.svelte';
  import { TARGET } from '../platform';
  import type { UpdateInfo } from '../platform/types';

  let info = $state<UpdateInfo | null>(null);
  /** 「あとで」にした更新の識別子（同じ更新は出さない。さらに新しい版が出たら出す） */
  let dismissed = $state<string | null>(null);
  let busy = $state(false);
  let ratio = $state<number | null>(null);
  let showNotes = $state(false);

  /** 画面に戻ってきたときの確認は、前回から少し空ける */
  const MIN_GAP = 5 * 60 * 1000;

  $effect(() => {
    const updater = app.platform?.updater;
    if (!updater) return;
    let last = 0;
    let alive = true;
    const run = async () => {
      if (busy) return;
      last = Date.now();
      try {
        const r = await updater.check();
        if (alive) info = r;
      } catch {
        /* オフラインなどで確認できなければ、次の機会に確認する */
      }
    };
    run();
    const timer = setInterval(run, updater.interval);
    const onVisible = () => document.visibilityState === 'visible' && Date.now() - last > MIN_GAP && run();
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      alive = false;
      clearInterval(timer);
      document.removeEventListener('visibilitychange', onVisible);
    };
  });

  async function apply() {
    const updater = app.platform?.updater;
    if (!updater || !(await app.prepareUpdate())) return;
    busy = true;
    try {
      await updater.apply((r) => (ratio = r));
    } catch (e: any) {
      busy = false;
      app.updating = false;
      app.flash(`更新できませんでした: ${e?.message ?? e}`, 'error');
    }
  }

  const isNewVersion = $derived(!!info && info.version !== __APP_VERSION__);
</script>

{#if info && info.id !== dismissed}
  <div class="update" role="status">
    <div class="msg">
      <strong>新しい版{isNewVersion ? `（${info.version}）` : ''}があります。</strong>
      <span class="small muted">
        {#if TARGET === 'web'}
          再読み込みすると更新されます。編集中の文書は下書きとして引き継がれます。
        {:else}
          更新するとインストール後に再起動します（現在 {__APP_VERSION__}）。
        {/if}
      </span>
      {#if info.notes}
        <button class="ghost small link" onclick={() => (showNotes = !showNotes)}>変更内容{showNotes ? 'を閉じる' : 'を見る'}</button>
        {#if showNotes}<pre class="notes small">{info.notes}</pre>{/if}
      {/if}
    </div>
    <div class="actions">
      {#if busy}
        <span class="small">{TARGET === 'web' ? '再読み込みしています…' : ratio === null ? 'ダウンロードしています…' : `ダウンロード中 ${Math.round(ratio * 100)}%`}</span>
      {:else}
        <button class="primary" onclick={apply}>{TARGET === 'web' ? '再読み込みして更新' : '更新して再起動'}</button>
        <button class="ghost" onclick={() => (dismissed = info!.id)}>あとで</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .update {
    position: fixed; right: 16px; bottom: 190px; z-index: 140; max-width: 420px;
    display: flex; flex-direction: column; gap: 8px; padding: 12px 14px;
    background: var(--panel); border: 1px solid var(--accent); border-radius: var(--radius);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
  }
  .msg { display: flex; flex-direction: column; gap: 4px; }
  .actions { display: flex; gap: 6px; justify-content: flex-end; align-items: center; }
  .link { align-self: flex-start; padding: 0; color: var(--accent); }
  .notes { max-height: 200px; overflow: auto; white-space: pre-wrap; margin: 0; padding: 6px; background: var(--panel-2); border-radius: var(--radius); }
</style>
