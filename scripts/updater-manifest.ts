// デスクトップ版の自動更新が読む latest.json を作る（release-draft.yml から呼ぶ）。
//   bun scripts/updater-manifest.ts <タグ> <出力先>
// `tauri build`（createUpdaterArtifacts を有効にしたもの）が作ったインストーラーと署名（.sig）を使う。
// リリースノートは環境変数 NOTES、リポジトリは GITHUB_REPOSITORY（owner/name）。
// アプリは https://github.com/<repo>/releases/latest/download/latest.json を見るため、下書きの間は届かず、公開すると届く。
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { root } from './lib';

const [tag, out] = process.argv.slice(2);
const repo = process.env.GITHUB_REPOSITORY;
if (!tag || !out || !repo) {
  console.error('使い方: GITHUB_REPOSITORY=owner/name bun scripts/updater-manifest.ts <タグ> <出力先>');
  process.exit(1);
}

const version = readFileSync(path.join(root, 'Cargo.toml'), 'utf8').match(/\[workspace\.package\][^[]*?\nversion = "([^"]+)"/)?.[1];
if (`v${version}` !== tag) throw new Error(`Cargo.toml の版（${version}）がタグ（${tag}）と合いません`);

const dir = path.join(root, 'target/release/bundle/nsis');
const installer = readdirSync(dir).find((f) => f.endsWith('-setup.exe') && f.includes(`_${version}_`));
if (!installer) throw new Error(`${dir} に ${version} のインストーラーがありません`);
const signature = readFileSync(path.join(dir, `${installer}.sig`), 'utf8').trim();
const url = `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(installer)}`;

const manifest = {
  version,
  notes: process.env.NOTES ?? '',
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': { signature, url },
    'windows-x86_64-nsis': { signature, url },
  },
};
writeFileSync(out, JSON.stringify(manifest, null, 2));
console.log(`${out}: ${installer}`);
