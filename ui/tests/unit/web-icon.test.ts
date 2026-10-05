// Web版のタブにアイコンが出ること（GitHub Pages でアイコンが表示されなかった不具合の再発防止）。
// ビルド結果（dist-web）の index.html がアイコンを指し、配信先（サブパス）でその画像が届くことを確かめる。
// 実行：bun run test:ui（先に build:web が必要。CI ではビルド結果が無ければ失敗させる）
import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const uiDir = join(import.meta.dir, '..', '..');
const dist = join(uiDir, 'dist-web');
/** GitHub Pages の配信先と同じ形のサブパス。絶対パス（/favicon.png など）はここでは届かない */
const SITE = 'https://example.invalid/formDoc/';

/** アイコン用の <link>（icon / apple-touch-icon）の href をすべて取り出す */
function iconHrefs(html: string): string[] {
  return [...html.matchAll(/<link\b[^>]*>/g)]
    .map((m) => m[0])
    .filter((tag) => /\brel="(?:icon|apple-touch-icon)"/.test(tag))
    .map((tag) => /\bhref="([^"]+)"/.exec(tag)?.[1] ?? '');
}

describe('Web版のアイコン', () => {
  test.skipIf(!existsSync(dist) && !process.env.CI)('ビルド結果の index.html が指すアイコンが配信物に含まれる', () => {
    const hrefs = iconHrefs(readFileSync(join(dist, 'index.html'), 'utf8'));
    expect(hrefs.length).toBeGreaterThan(0);
    for (const href of hrefs) {
      if (href.startsWith('data:')) continue; // インライン化された画像はそのまま表示できる
      const url = new URL(href, SITE);
      expect(url.href.startsWith(SITE)).toBe(true);
      expect(existsSync(join(dist, decodeURIComponent(url.pathname.slice('/formDoc/'.length))))).toBe(true);
    }
  });
});
