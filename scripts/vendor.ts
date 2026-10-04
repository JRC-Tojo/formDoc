// 外部の同梱物（フォント・Typstパッケージ）を vendor.json に従って library/ 配下に取得する。
// formdoc-library がビルド時に library/ をバイナリへ埋め込むため、初回ビルド前に必要（bun ready が呼ぶ）。
//   bun scripts/vendor.ts          … 未取得・不一致のものだけ取得
//   bun scripts/vendor.ts --force  … すべて取り直す
// 取得物は sha256 で検証する（版が変わると出力PDFが変わるため）。取得は並行して行う。
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { root } from './lib';

type Manifest = {
  files: { path: string; url: string; sha256: string }[];
  packages: { namespace: string; name: string; version: string; sha256: string }[];
};

const sha256 = (buf: Uint8Array) => createHash('sha256').update(buf).digest('hex');

async function download(url: string, expected: string): Promise<Uint8Array> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const buf = new Uint8Array(await res.arrayBuffer());
  const actual = sha256(buf);
  if (actual !== expected) throw new Error(`${url}: sha256 不一致（期待 ${expected}、実際 ${actual}）`);
  return buf;
}

/** tar（ustar / pax）を dest に展開する。外部の tar コマンドに頼らない（Windows で挙動が違うため） */
function untar(tar: Uint8Array, dest: string) {
  const text = (b: Uint8Array) => new TextDecoder().decode(b).replace(/\0.*$/s, '');
  let pos = 0;
  let paxPath: string | null = null;
  while (pos + 512 <= tar.length) {
    const h = tar.subarray(pos, pos + 512);
    if (h.every((b) => b === 0)) break;
    const size = parseInt(text(h.subarray(124, 136)).trim() || '0', 8);
    const type = String.fromCharCode(h[156] || 48);
    const prefix = text(h.subarray(345, 500));
    let name = paxPath ?? (prefix ? `${prefix}/${text(h.subarray(0, 100))}` : text(h.subarray(0, 100)));
    paxPath = null;
    const body = tar.subarray(pos + 512, pos + 512 + size);
    pos += 512 + Math.ceil(size / 512) * 512;
    if (type === 'x') {
      // pax 拡張ヘッダ（"<長さ> path=<名前>\n"）。次の項目の名前を上書きする
      const m = new TextDecoder().decode(body).match(/\d+ path=([^\n]*)\n/);
      if (m) paxPath = m[1];
      continue;
    }
    if (type === 'g') continue;
    name = name.replace(/^\.\//, '');
    if (!name || name.split('/').includes('..')) continue;
    const out = path.join(dest, name);
    if (type === '5') mkdirSync(out, { recursive: true });
    else if (type === '0' || type === '7') {
      mkdirSync(path.dirname(out), { recursive: true });
      writeFileSync(out, body);
    }
  }
}

export async function fetchVendor(force = false) {
  const manifest: Manifest = JSON.parse(readFileSync(path.join(root, 'vendor.json'), 'utf8'));
  const jobs: Promise<void>[] = [];

  for (const f of manifest.files) {
    const dest = path.join(root, f.path);
    if (!force && existsSync(dest) && sha256(readFileSync(dest)) === f.sha256) continue;
    jobs.push(
      download(f.url, f.sha256).then((buf) => {
        mkdirSync(path.dirname(dest), { recursive: true });
        writeFileSync(dest, buf);
        console.log(`取得: ${f.path}`);
      }),
    );
  }

  for (const p of manifest.packages) {
    const rel = `library/vendor/${p.namespace}/${p.name}/${p.version}`;
    const dest = path.join(root, rel);
    if (!force && existsSync(path.join(dest, 'typst.toml'))) continue;
    const url = `https://packages.typst.org/${p.namespace}/${p.name}-${p.version}.tar.gz`;
    jobs.push(
      download(url, p.sha256).then((buf) => {
        rmSync(dest, { recursive: true, force: true });
        mkdirSync(dest, { recursive: true });
        untar(Bun.gunzipSync(buf), dest);
        console.log(`取得: ${rel}`);
      }),
    );
  }

  await Promise.all(jobs);
}

if (import.meta.main) {
  await fetchVendor(process.argv.includes('--force'));
  console.log('library/ の同梱物（フォント・Typstパッケージ）はそろっています');
}
