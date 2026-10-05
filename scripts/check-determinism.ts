// Web版（wasm）とネイティブ（CLI）で、同じ文書から同じPDFが出ることを確かめる。
// Web版と同じく「フォントを埋め込まない wasm に、同梱フォントを実行時に渡す」手順で組版する。
//   bun scripts/check-determinism.ts [document.json]   … 既定は examples/keisansho-gui/document.json
// 事前に bun ready（wasm-dev の wasm と wasm-bindgen が必要）。CLI は無ければビルドする。
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { root, run, step } from './lib';

const doc = path.resolve(process.argv[2] ?? path.join(root, 'examples/keisansho-gui/document.json'));
const out = path.join(root, 'target/determinism');
mkdirSync(out, { recursive: true });
const sha = (b: Uint8Array) => createHash('sha256').update(b).digest('hex');

step('ネイティブ（CLI）');
run(['cargo', 'build', '-j', '1', '-p', 'formdoc-cli']);
const exe = path.join(root, 'target/debug', process.platform === 'win32' ? 'formdoc.exe' : 'formdoc');
const nativePdf = path.join(out, 'native.pdf');
run([exe, 'gui', doc, nativePdf]);

step('Web版（wasm、フォントは実行時に渡す）');
const wasm = path.join(root, 'target/wasm32-unknown-unknown/wasm-dev/formdoc_wasm.wasm');
if (!existsSync(wasm)) throw new Error('wasm がありません。先に bun ready を実行してください');
const pkg = path.join(root, 'target/wasm-node');
run(['wasm-bindgen', '--target', 'nodejs', '--out-dir', pkg, wasm]);
const m = require(path.join(pkg, 'formdoc_wasm.js'));
const list: { file: string; sha256: string }[] = JSON.parse(m.font_files());
const bodies = list.map((f) => {
  const b = readFileSync(path.join(root, 'library/fonts', f.file));
  if (sha(b) !== f.sha256) throw new Error(`フォント ${f.file} の版がエンジンと一致しません`);
  return b;
});
for (const b of bodies) m.add_font(b);
m.init_fonts();
const docJson = readFileSync(doc, 'utf8');
// CLI と同じく、文書の template に対応する同梱の文書テンプレートを使う
const template: string = JSON.parse(docJson).template;
const style = JSON.parse(m.catalog()).styles.find((s: { file: string }) => s.file === `${template}.typ`);
if (!style) throw new Error(`同梱の文書テンプレート ${template} がありません`);
m.set_style(style.source);
const r = JSON.parse(m.update_document(docJson, '[]'));
if (!r.exportable) {
  const errors = r.issues.filter((i: { severity: string }) => i.severity === 'error').map((i: { message: string }) => i.message);
  throw new Error(`Web版で出力できない文書です: ${errors.join(' / ')}`);
}
const webPdf: Uint8Array = m.pdf();

const a = sha(readFileSync(nativePdf));
const b = sha(webPdf);
console.log(`native ${a}\nweb    ${b}`);
if (a !== b) {
  console.error('✖ Web版とネイティブのPDFが一致しません');
  process.exit(1);
}
console.log('✔ Web版とネイティブのPDFは一致しました');
