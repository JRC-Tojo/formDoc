// アプリの版を上げる（Web版・デスクトップ版で共通の版）。
//   bun scripts/bump-version.ts patch|minor|major   … 0.1.0 → 0.1.1 / 0.2.0 / 1.0.0
//   bun scripts/bump-version.ts 1.2.3               … 版を指定
// 書き換えるもの：Cargo.toml（[workspace.package] version）、Cargo.lock（ワークスペースのクレート）、ui/package.json。
// デスクトップ版（tauri.conf.json）は Cargo.toml の版を使う。ライブラリ（formdoc-library）の版は別管理なので変えない。
// GitHub Actions から呼ばれたときは、新しい版を出力 version に書く。
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { root } from './lib';

const file = (p: string) => path.join(root, p);
const cargoToml = readFileSync(file('Cargo.toml'), 'utf8');
const current = cargoToml.match(/\[workspace\.package\][^[]*?\nversion = "([^"]+)"/)?.[1];
if (!current) throw new Error('Cargo.toml に [workspace.package] version がありません');

const arg = process.argv[2];
const [major, minor, patch] = current.split('.').map(Number);
const next =
  arg === 'major' ? `${major + 1}.0.0`
  : arg === 'minor' ? `${major}.${minor + 1}.0`
  : arg === 'patch' ? `${major}.${minor}.${patch + 1}`
  : /^\d+\.\d+\.\d+$/.test(arg ?? '') ? arg
  : null;
if (!next) {
  console.error('使い方: bun scripts/bump-version.ts patch|minor|major|<X.Y.Z>');
  process.exit(1);
}

writeFileSync(file('Cargo.toml'), cargoToml.replace(/(\[workspace\.package\][^[]*?\nversion = ")[^"]+"/, `$1${next}"`));

// Cargo.lock：版を workspace から受け継ぐクレートだけ（formdoc-library は別管理）
const members = ['formdoc-cli', 'formdoc-core', 'formdoc-expr', 'formdoc-typst-plugin', 'formdoc-wasm', 'formdoc-desktop'];
let lock = readFileSync(file('Cargo.lock'), 'utf8');
for (const name of members) {
  const re = new RegExp(`(\\nname = "${name}"\\r?\\nversion = ")[^"]+"`);
  if (!re.test(lock)) throw new Error(`Cargo.lock に ${name} がありません`);
  lock = lock.replace(re, `$1${next}"`);
}
writeFileSync(file('Cargo.lock'), lock);

const pkgPath = file('ui/package.json');
writeFileSync(pkgPath, readFileSync(pkgPath, 'utf8').replace(/("version": ")[^"]+"/, `$1${next}"`));

console.log(`${current} → ${next}`);
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `version=${next}\n`);
