// Web版のタブにアイコンが出ること（GitHub Pages でアイコンが表示されなかった不具合の再発防止）。
// ビルド結果（dist-web）の index.html がアイコンを指し、その画像が出力に含まれていることを確かめる。
// 実行：bun run --cwd ui test（先に build:web が必要。無ければ index.html の指定と元画像だけを確かめる）
import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const uiDir = join(import.meta.dir, '..', '..');

/** HTML から <link rel="icon"> の href をすべて取り出す */
function iconHrefs(html: string): string[] {
  return [...html.matchAll(/<link\b[^>]*>/g)]
    .map((m) => m[0])
    .filter((tag) => /\brel="(?:icon|apple-touch-icon)"/.test(tag))
    .map((tag) => /\bhref="([^"]+)"/.exec(tag)?.[1] ?? '');
}

describe('Web版のアイコン', () => {
  test('ソースの index.html がアイコンを指定し、画像が存在する', () => {
    const html = readFileSync(join(uiDir, 'index.html'), 'utf8');
    const hrefs = iconHrefs(html);
    expect(hrefs.length).toBeGreaterThan(0);
    for (const href of hrefs) expect(existsSync(join(uiDir, href))).toBe(true);
  });

  const dist = join(uiDir, 'dist-web', 'index.html');
  test.skipIf(!existsSync(dist))('ビルド結果の index.html が指すアイコンが出力に含まれる', () => {
    const hrefs = iconHrefs(readFileSync(dist, 'utf8'));
    expect(hrefs.length).toBeGreaterThan(0);
    for (const href of hrefs) {
      // GitHub Pages はサブパス（/formDoc/）で配信するため、絶対パスでは届かない
      expect(href.startsWith('/')).toBe(false);
      expect(existsSync(join(dirname(dist), href))).toBe(true);
    }
  });
});
