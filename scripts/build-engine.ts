// Web版のエンジン（formdoc-wasm）をビルドし、ui/src/lib/engine/pkg に配置する。
// 同梱物の取得と式エンジンのTypstプラグインのビルドも先に行う（どれも変更がなければすぐ終わる）。
//   bun scripts/build-engine.ts           … 開発用（LTOなし、速い）
//   bun scripts/build-engine.ts --release … 配布用
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { buildPlugin } from './build-plugin';
import { root, run, step } from './lib';
import { fetchVendor } from './vendor';

const PKG = path.join(root, 'ui/src/lib/engine/pkg');

export async function buildEngine(release = false) {
  step('同梱物（フォント・Typstパッケージ）');
  await fetchVendor();
  step('式エンジンのTypstプラグイン');
  buildPlugin();
  step(`Web版エンジン（${release ? '配布用' : '開発用'}）`);
  const profile = release ? 'release' : 'wasm-dev';
  run(['cargo', 'build', '-p', 'formdoc-wasm', '--profile', profile, '--target', 'wasm32-unknown-unknown']);
  const wasm = path.join(root, `target/wasm32-unknown-unknown/${profile}/formdoc_wasm.wasm`);
  // wasm-bindgen は数秒かかるため、前回と同じ入力（版の種類と更新日時）なら省く
  const stamp = path.join(PKG, '.source');
  const source = `${profile} ${statSync(wasm).mtimeMs}`;
  if (existsSync(stamp) && readFileSync(stamp, 'utf8') === source) {
    console.log('engine は最新です');
    return;
  }
  run(['wasm-bindgen', '--target', 'web', '--out-dir', PKG, wasm]);
  writeFileSync(stamp, source);
  console.log('engine -> ui/src/lib/engine/pkg');
}

if (import.meta.main) await buildEngine(process.argv.includes('--release'));
