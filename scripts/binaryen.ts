// wasm-opt（binaryen）を用意する。Web版エンジンの配布用ビルドで wasm を小さくするために使う。
// 版を固定し、取得物は sha256 で検証して target/tools/ に置く（初回のみ取得）。
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { root } from './lib';
import { untar } from './vendor';

const VERSION = 'version_133';
/** 配布物の sha256（https://github.com/WebAssembly/binaryen/releases の *.tar.gz.sha256） */
const SHA256: Record<string, string> = {
  'x86_64-windows': '17a2cbeac6b5693c5fbafab3838d3c65fd9c1eb38b05f5baec6c657e8c84995b',
  'x86_64-linux': '2dc9c7813f5375db93d96ead4b78222fcc3e2677bbb832297af4797782a37489',
  'x86_64-macos': '13a9b90be775c6389ce3d1f879cb8627bea56708ba8c122983941d53a8199b95',
  'arm64-macos': 'ad66da82ac13f163e424b1643f16c6dfcccc98b5966296b43e52d3cab04f84a8',
};

/** この環境の配布物の名前（binaryen の命名） */
function platform(): string {
  const arch = process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const os = { win32: 'windows', darwin: 'macos', linux: 'linux' }[process.platform as string];
  const key = `${arch}-${os}`;
  if (!os || !SHA256[key]) throw new Error(`binaryen の配布物がこの環境（${process.platform} ${process.arch}）に対応していません`);
  return key;
}

/** wasm-opt の実行ファイルのパスを返す。無ければ取得する */
export async function wasmOpt(): Promise<string> {
  const key = platform();
  const dir = path.join(root, 'target/tools', `binaryen-${VERSION}-${key}`);
  const exe = path.join(dir, `binaryen-${VERSION}`, 'bin', process.platform === 'win32' ? 'wasm-opt.exe' : 'wasm-opt');
  if (existsSync(exe)) return exe;
  const url = `https://github.com/WebAssembly/binaryen/releases/download/${VERSION}/binaryen-${VERSION}-${key}.tar.gz`;
  console.log(`wasm-opt を取得します: ${url}`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const buf = new Uint8Array(await res.arrayBuffer());
  const actual = createHash('sha256').update(buf).digest('hex');
  if (actual !== SHA256[key]) throw new Error(`${url}: sha256 不一致（期待 ${SHA256[key]}、実際 ${actual}）`);
  mkdirSync(dir, { recursive: true });
  untar(Bun.gunzipSync(buf), dir);
  if (process.platform !== 'win32') Bun.spawnSync(['chmod', '+x', exe]);
  return exe;
}
