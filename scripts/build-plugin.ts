// 式エンジンのTypstプラグインをビルドし、社内標準パッケージに配置する。
// formdoc-expr を変更したら必ず実行する（GUIとコードモードの計算を一致させるため）。
//   bun scripts/build-plugin.ts
// 中身が変わらなければ配置し直さない（library/ の更新日時が変わると、埋め込み側が全部ビルドし直しになるため）。
import { copyFileSync } from 'node:fs';
import path from 'node:path';
import { root, run, sameFile } from './lib';

export const PLUGIN = path.join(root, 'library/typst/formdoc/0.1.0/formdoc_expr.wasm');

export function buildPlugin() {
  run(['cargo', 'build', '-p', 'formdoc-typst-plugin', '--release', '--target', 'wasm32-unknown-unknown']);
  const built = path.join(root, 'target/wasm32-unknown-unknown/release/formdoc_typst_plugin.wasm');
  if (sameFile(built, PLUGIN)) return;
  copyFileSync(built, PLUGIN);
  console.log('plugin -> library/typst/formdoc/0.1.0/formdoc_expr.wasm');
}

if (import.meta.main) buildPlugin();
