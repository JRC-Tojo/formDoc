// Web版のエンジン（formdoc-wasm）をビルドし、src/lib/engine/pkg に配置する。
//   node scripts/build-wasm.mjs          … 開発用（LTOなし、速い）
//   node scripts/build-wasm.mjs --release … 配布用
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const release = process.argv.includes('--release');
const profile = release ? 'release' : 'wasm-dev';
const run = (cmd) => execSync(cmd, { cwd: root, stdio: 'inherit' });

run('node scripts/setup.mjs');
run('bash scripts/build-plugin.sh');
run(`cargo build -p formdoc-wasm --profile ${profile} --target wasm32-unknown-unknown`);
run(`wasm-bindgen --target web --out-dir ui/src/lib/engine/pkg target/wasm32-unknown-unknown/${profile}/formdoc_wasm.wasm`);
console.log('engine -> ui/src/lib/engine/pkg');
