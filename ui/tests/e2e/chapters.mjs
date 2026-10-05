// 章構成の拘束（Issue #7）のブラウザ確認。Web版 dev サーバ（bun run dev）に対して実行する。
// 準備: chrome --headless=new --remote-debugging-port=9222 --user-data-dir=<tmp> を起動し、playwright-core を入れた場所で
//   bun ui/tests/e2e/chapters.mjs <スクリーンショットの出力先> [URL]
import { chromium } from 'playwright-core';

const out = process.argv[2] ?? '.';
const url = process.argv[3] ?? 'http://localhost:5173/';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
const ctx = await browser.newContext(); // 毎回まっさらな保存領域
const page = await ctx.newPage();
await page.setViewportSize({ width: 1600, height: 1000 });
const logs = [];
page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
let ng = 0;
const ok = (cond, msg) => {
  if (!cond) ng++;
  console.log(`${cond ? 'OK ' : 'NG '} ${msg}`);
};
const st = (fn, arg) => page.evaluate(fn, arg);
const chapterIssues = () => st(() => (__formdoc.result?.issues ?? []).filter((i) => i.code.startsWith('chapter-')).map((i) => `${i.code}:${i.message}`));
const headings = () => st(() => __formdoc.doc.blocks.filter((b) => b.kind === 'heading').map((b) => b.props.text));
const settle = () => page.waitForTimeout(1500);
const row = (text) => page.locator('.outline .row', { hasText: text }).first();

await page.goto(url);
await page.waitForFunction(() => globalThis.__formdoc && __formdoc.loading === '', null, { timeout: 180000 });
await page.locator('button.style', { hasText: '計算書' }).click();
await page.waitForFunction(() => __formdoc.result && __formdoc.pageHashes.length > 0, null, { timeout: 60000 });
await settle();

// 1. 新規作成：構造形式（上路桁）の必須の章が並び、決められた章に 🔒
const h1 = await headings();
ok(h1.includes('設計概要') && h1.includes('検討箇所') && !h1.includes('グルーピング'), '上路桁の章が並ぶ: ' + h1.join(' / '));
ok((await row('設計概要').locator('.lock').count()) === 1, '設計概要に 🔒');
await page.screenshot({ path: `${out}/c1-new.png` });

// 2. 🔒 の章は削除できない
await row('設計概要').click();
await page.locator('.outline .foot button', { hasText: '削除' }).click();
await settle();
ok((await headings()).includes('設計概要'), '🔒 の章は削除できない');
ok((await page.locator('.inspector .guide').count()) === 1, '章の見出しを選ぶと執筆ガイドが出る');
await page.screenshot({ path: `${out}/c2-guide.png` });

// 3. 構造形式を工事桁にする → グルーピングが決められた位置に入り、不要になった検討箇所は検証パネルに出る
await page.locator('.outline .doc, .outline button', { hasText: '計算書' }).first().click();
await page.locator('.inspector select#variant').selectOption('工事桁');
await page.waitForTimeout(2500);
const h3 = await headings();
const pos = (t) => h3.indexOf(t);
ok(pos('共通仕様') < pos('グルーピング') && pos('グルーピング') < pos('施工計画'), 'グルーピングが決められた位置に入る: ' + h3.join(' / '));
const issues3 = await chapterIssues();
ok(issues3.some((i) => i.startsWith('chapter-extra') && i.includes('検討箇所')), '不要になった検討箇所を検出: ' + issues3.join(' / '));
await page.screenshot({ path: `${out}/c3-variant.png` });

// 4. 不要になった章は削除できる → 章構成の検出が無くなる（見出し文の書き換えを促す注意を除く）
await row('検討箇所').click();
await page.locator('.outline .foot button', { hasText: '削除' }).click();
await settle();
const left = (await chapterIssues()).filter((i) => !i.startsWith('chapter-title'));
ok(left.length === 0, '不要な章を消すと検出が無くなる: ' + left.join(' / '));

// 5. 取り消し1回で、構造形式の変更と章の追加がまとめて戻る（4 の削除を先に戻す）
await page.keyboard.press('Control+z');
await page.keyboard.press('Control+z');
await settle();
const back = await st(() => ({ v: __formdoc.doc.meta.variant, h: __formdoc.doc.blocks.filter((b) => b.kind === 'heading').map((b) => b.props.text) }));
ok(back.v === '上路桁' && !back.h.includes('グルーピング'), `取り消しで構造形式と章が戻る: ${back.v} / ${back.h.join(' / ')}`);

ok(logs.length === 0, 'ページのエラーなし ' + logs.join(' / '));
console.log(ng === 0 ? 'すべてOK' : `NG ${ng} 件`);
await browser.close();
process.exit(ng ? 1 : 0);
