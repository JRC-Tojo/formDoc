// 開発環境の準備をまとめて行う（クローン直後・pull 後に実行。済んでいる手順はすぐ終わる）。
//   bun ready            … Web版・デスクトップ版の両方を起動できるようにする
//   bun ready --desktop  … デスクトップ版だけ（Web版エンジンの wasm を作らない）
//   bun ready --release  … Web版エンジンを配布用（LTOあり、遅い）で作る（CI 用）
//   bun ready --force    … 同梱物を取り直す
// 終わったら bun run dev（Web版）または bun run tauri dev（デスクトップ版）。
//
// 行うこと
//   1. ツールの確認：Rust（1.85 以上）、wasm32 ターゲット（無ければ追加）、wasm-bindgen-cli（Cargo.lock と同じ版。無ければ導入）
//   2. 依存パッケージ（bun install）と、同梱物（フォント・Typstパッケージ）の取得を並行して行う
//   3. 式エンジンのTypstプラグインのビルド
//   4. Web版エンジン（wasm）のビルド（--desktop のときは省く）
import { buildEngine } from './build-engine';
import { buildPlugin } from './build-plugin';
import { lockedVersion, output, root, run, step } from './lib';
import { fetchVendor } from './vendor';

async function main() {
  const desktopOnly = process.argv.includes('--desktop');
  const force = process.argv.includes('--force');
  const release = process.argv.includes('--release');
  const started = performance.now();

  step('ツールの確認');
  const rustc = output(['rustc', '--version']);
  if (!rustc) throw new Error('Rust が見つかりません。https://rustup.rs から導入してください。');
  const [major, minor] = (rustc.match(/(\d+)\.(\d+)/) ?? []).slice(1).map(Number);
  if (major === 1 && minor < 85) {
    throw new Error(`${rustc} は古すぎます（1.85 以上が必要）。rustup update を実行するか、PATH の先頭の Rust を確認してください。`);
  }
  console.log(rustc);

  const targets = output(['rustup', 'target', 'list', '--installed']);
  if (targets === null) console.log('rustup が見つかりません。wasm32-unknown-unknown ターゲットは導入済みとみなします');
  else if (!targets.split(/\s+/).includes('wasm32-unknown-unknown')) run(['rustup', 'target', 'add', 'wasm32-unknown-unknown']);

  if (!desktopOnly) {
    const want = lockedVersion('wasm-bindgen');
    const have = output(['wasm-bindgen', '--version'])?.split(' ')[1];
    if (have !== want) {
      console.log(`wasm-bindgen-cli ${want} を導入します（現在: ${have ?? 'なし'}。初回のみ数分かかります）`);
      if (output(['cargo', 'binstall', '--version'])) run(['cargo', 'binstall', '-y', `wasm-bindgen-cli@${want}`]);
      else run(['cargo', 'install', 'wasm-bindgen-cli', '--version', want, '--locked']);
    }
  }

  step('依存パッケージと同梱物の取得');
  const install = Bun.spawn(['bun', 'install'], { cwd: root, stdio: ['inherit', 'inherit', 'inherit'] });
  await fetchVendor(force);
  if ((await install.exited) !== 0) throw new Error('bun install に失敗しました');

  if (desktopOnly) {
    step('式エンジンのTypstプラグイン');
    buildPlugin();
  } else {
    await buildEngine(release);
  }

  const sec = Math.round((performance.now() - started) / 1000);
  console.log(`\n準備できました（${sec} 秒）。`);
  console.log(desktopOnly ? '  bun run tauri dev   … デスクトップ版' : '  bun run dev         … Web版（http://localhost:5173）\n  bun run tauri dev   … デスクトップ版');
}

main().catch((e) => {
  console.error(`\n✖ ${e?.message ?? e}`);
  process.exit(1);
});
