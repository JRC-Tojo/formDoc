// 外部の同梱物（フォント・Typstパッケージ）を vendor.json に従って library/ 配下に取得し、
// 式エンジンのTypstプラグイン（formdoc_expr.wasm）が未生成ならビルドする。
// formdoc-library がビルド時に library/ をバイナリへ埋め込むため、初回ビルド前に実行する。
//   node scripts/setup.mjs          … 未取得・不一致のものだけ取得
//   node scripts/setup.mjs --force  … すべて取り直す
// 取得物は sha256 で検証する（版が変わると出力PDFが変わるため）。
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const force = process.argv.includes('--force');
const manifest = JSON.parse(readFileSync(path.join(root, 'vendor.json'), 'utf8'));

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');

async function download(url, expected) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const buf = Buffer.from(await res.arrayBuffer());
  const actual = sha256(buf);
  if (actual !== expected) throw new Error(`${url}: sha256 不一致（期待 ${expected}、実際 ${actual}）`);
  return buf;
}

for (const f of manifest.files) {
  const dest = path.join(root, f.path);
  if (!force && existsSync(dest) && sha256(readFileSync(dest)) === f.sha256) continue;
  console.log(`取得: ${f.path}`);
  mkdirSync(path.dirname(dest), { recursive: true });
  writeFileSync(dest, await download(f.url, f.sha256));
}

for (const p of manifest.packages) {
  const rel = `library/vendor/${p.namespace}/${p.name}/${p.version}`;
  const dest = path.join(root, rel);
  if (!force && existsSync(path.join(dest, 'typst.toml'))) continue;
  console.log(`取得: ${rel}`);
  const url = `https://packages.typst.org/${p.namespace}/${p.name}-${p.version}.tar.gz`;
  const buf = await download(url, p.sha256);
  rmSync(dest, { recursive: true, force: true });
  mkdirSync(dest, { recursive: true });
  // tar に絶対パスを渡すと Git Bash の tar が "C:" をホスト名と解釈するため、展開先で相対パス指定する。
  writeFileSync(path.join(dest, 'package.tar.gz'), buf);
  try {
    execFileSync('tar', ['-xzf', 'package.tar.gz'], { cwd: dest, stdio: 'inherit' });
  } finally {
    rmSync(path.join(dest, 'package.tar.gz'), { force: true });
  }
}

// 式エンジンのTypstプラグインは生成物のため Git に含めない。未生成ならここでビルドする。
if (force || !existsSync(path.join(root, 'library/typst/formdoc/0.1.0/formdoc_expr.wasm'))) {
  execFileSync('bash', ['scripts/build-plugin.sh'], { cwd: root, stdio: 'inherit' });
}

console.log('library/ の同梱物はそろっています');
