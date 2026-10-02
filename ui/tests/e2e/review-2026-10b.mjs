// 2回目の試用レビュー対応のブラウザ確認。Web版 dev サーバ（npm run dev）に対して実行する。
// 準備: chrome --headless=new --remote-debugging-port=9222 --user-data-dir=<tmp> を起動し、playwright-core を入れた場所で
//   node ui/tests/e2e/review-2026-10b.mjs <スクリーンショットの出力先>
import { chromium } from 'playwright-core';

const out = process.argv[2] ?? '.';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
const ctx = await browser.newContext(); // 毎回まっさらな保存領域
const page = await ctx.newPage();
await page.setViewportSize({ width: 1600, height: 1000 });
const logs = [];
page.on('console', (m) => m.type() === 'error' && logs.push(m.text()));
page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
let ng = 0;
const ok = (cond, msg) => {
  if (!cond) ng++;
  console.log(`${cond ? 'OK ' : 'NG '} ${msg}`);
};
const st = (fn, arg) => page.evaluate(fn, arg);
const errs = () => st(() => (__formdoc.result?.issues ?? []).filter((i) => i.severity === 'error').map((i) => `${i.block_id}:${i.message}`));
const settle = () => page.waitForTimeout(1200);

await page.goto('http://localhost:5173/');
await page.waitForFunction(() => globalThis.__formdoc && __formdoc.loading === '', null, { timeout: 120000 });
await page.locator('button.style', { hasText: '計算書' }).click();
await page.waitForFunction(() => __formdoc.pageHashes.length > 0, null, { timeout: 60000 });

// 1. テンプレートを挿入すると、一覧に1つのまとまりとして出る
async function insertTemplate(name, fill) {
  await page.getByRole('button', { name: '＋ 部品を追加…' }).click();
  await page.locator('.modal .item', { hasText: name }).click();
  await page.locator('.modal aside button.primary').click();
  if (fill) await fill();
  await page.locator('.modal footer button.primary', { hasText: '挿入' }).click();
  await settle();
}
// §1 を選んでから挿入
await page.locator('.outline .row', { hasText: '設計条件' }).click();
await insertTemplate('I形断面');
await insertTemplate('I形断面');
ok((await page.locator('.outline .item.group').count()) === 2, 'テンプレートは一覧で1つのまとまり（2つ挿入 → 2行）');
const names = await st(() => __formdoc.result.vars.map((v) => v.name));
ok(names.includes('A') && names.includes('A_2'), '公開する変数は A と A_2: ' + names.join(','));
ok((await errs()).length === 0, '内部の変数（H など）が重なってもエラーにならない: ' + (await errs()).join(' / '));
await page.screenshot({ path: `${out}/b1-groups.png` });

// 2. 折りたたみ
const before = await page.locator('.outline .item').count();
await page.locator('.outline .item.group .fold').first().click();
const folded = await page.locator('.outline .item').count();
ok(folded < before, `まとまりを折りたためる（${before} → ${folded} 行）`);
await page.locator('.outline .item.heading .fold').first().click();
ok((await page.locator('.outline .item').count()) < folded, '見出しの節も折りたためる');
await page.screenshot({ path: `${out}/b2-folded.png` });
await page.locator('.outline .item.heading .fold').first().click(); // 開き直す

// 3. ドラッグで並べ替え：§3 を §2 の上へ（見出しは節ごと動く）
const order = () => st(() => __formdoc.doc.blocks.filter((b) => b.kind === 'heading').map((b) => b.props.text));
const h3 = page.locator('.outline .row', { hasText: '設計結果一覧' });
const h2 = page.locator('.outline .row', { hasText: '設計計算' });
const a = await h3.boundingBox();
const b = await h2.boundingBox();
await page.mouse.move(a.x + 40, a.y + a.height / 2);
await page.mouse.down();
await page.mouse.move(a.x + 40, a.y - 10, { steps: 4 });
await page.mouse.move(b.x + 40, b.y + 3, { steps: 6 });
await page.screenshot({ path: `${out}/b3-drag.png` });
await page.mouse.up();
await settle();
ok(JSON.stringify(await order()) === JSON.stringify(['設計条件', '設計結果一覧', '設計計算']), 'ドラッグで見出しを並べ替え: ' + (await order()).join(','));

// 4. 変数の有効範囲：まとまりの中の H は外から使えない。グローバル変数なら別の節から使える
await st(() => {
  const d = __formdoc.doc;
  const sec = d.blocks.findIndex((b) => b.props.text === '設計計算');
  d.blocks.splice(sec + 1, 0,
    { id: 'tc1', kind: 'calc', props: { name: 'q1', expr: 'H * 2' } },
    { id: 'tc2', kind: 'calc', props: { name: 'q2', expr: 'A + G' } });
  const top = d.blocks.findIndex((b) => b.props.text === '設計条件');
  d.blocks.splice(top + 1, 0, { id: 'tv1', kind: 'vdef', props: { name: 'G', value: 3, global: true } });
  __formdoc.edit(() => {});
});
await settle();
const e = await errs();
ok(e.length === 2 && e.every((x) => x.startsWith('tc1') || x.startsWith('tc2')), '別の節のローカル変数（H、§1 の A）は使えない: ' + e.join(' / '));
ok(e.some((x) => x.includes('グローバル変数として定義')), 'エラーに「グローバル変数として定義」の案内');
ok(!e.some((x) => x.includes('変数 G')), 'グローバル変数 G は別の節から使える');
await page.locator('.outline .row', { hasText: 'q1' }).first().click();
ok(!(await page.locator('.vars .var .name', { hasText: /^H$/ }).count()), '変数一覧は、選択中の部品で使える変数だけ（H は出ない）');
await page.screenshot({ path: `${out}/b4-scope.png` });
await st(() => __formdoc.edit((d) => (d.blocks = d.blocks.filter((b) => !['tc1', 'tc2'].includes(b.id)))));
await settle();

// 5. コードモード：同じ文書。書き換えると「部品で作成」に反映される
await page.getByRole('tab', { name: 'コードで作成（Typst）' }).click();
await page.waitForSelector('.cm-editor');
const code = await st(() => __formdoc.codeText);
ok(code.includes('// @group') && code.includes('// @block'), 'コードモードは同じ文書（目印つき）');
const pagesBefore = await st(() => __formdoc.pageHashes.length);
ok(pagesBefore > 0, 'プレビューはそのまま');
// 1つ目のテンプレートの H を 800 にする
await st(() => {
  const t = __formdoc.codeText;
  const i = t.indexOf('vdef("H", 700.0');
  __formdoc.setCode(t.slice(0, i) + 'vdef("H", 800.0' + t.slice(i + 'vdef("H", 700.0'.length));
});
await page.waitForTimeout(2000);
await page.screenshot({ path: `${out}/b5-code.png` });
await page.getByRole('tab', { name: '部品で作成' }).click();
await settle();
const vA = await st(() => __formdoc.result.vars.find((v) => v.name === 'A').value);
const vA2 = await st(() => __formdoc.result.vars.find((v) => v.name === 'A_2').value);
ok(vA > vA2, `コードの書き換え（H=800）が文書に反映（A=${vA} > A_2=${vA2}）`);
ok((await st(() => JSON.stringify(__formdoc.doc).includes('"kind":"typst"'))), '書き換えた部品は Typstコード部品になる');
ok((await errs()).length === 0, 'コード編集後もエラー0件: ' + (await errs()).join(' / '));
ok(!(await page.getByRole('button', { name: 'Typstに変換' }).count()), '「Typstに変換」ボタンは無い');

// 6. 単純梁テンプレート：荷重3個にすると図と影響線が変わる
await insertTemplate('単純梁', async () => {
  const row = page.locator('.modal tr', { hasText: '荷重の数' });
  await row.locator('input[value="value"]').check();
  await row.locator('input.mono').fill('3');
});
const fig = await st(() => {
  const all = [];
  const walk = (bs) => bs.forEach((b) => (all.push(b), walk(b.children ?? [])));
  walk(__formdoc.doc.blocks);
  const f = all.filter((b) => b.kind === 'fig-shapes').at(-1);
  return __formdoc.result.blocks[f.id].shapes;
});
ok(fig?.[3]?.length === 3, '荷重の矢印が3回繰り返される');
ok(fig?.[12]?.map((v) => v.label).join(',') === '0.625,0.275,-0.075', '影響線縦距を自動計算: ' + fig?.[12]?.map((v) => v.label).join(','));
await page.screenshot({ path: `${out}/b6-beam.png` });

// 7. 図形エディタで繰り返しが見える
const figId = await st(() => {
  const all = [];
  const walk = (bs) => bs.forEach((b) => (all.push(b), walk(b.children ?? [])));
  walk(__formdoc.doc.blocks);
  return all.filter((b) => b.kind === 'fig-shapes').at(-1).id;
});
await st((id) => (__formdoc.dialog = { kind: 'shapes', blockId: id }), figId);
await page.waitForSelector('.editor svg');
await page.waitForTimeout(500);
ok((await page.locator('.editor g.repeat').count()) >= 6, '図形エディタに繰り返しの2回目以降が描かれる');
await page.screenshot({ path: `${out}/b7-editor.png` });
await page.keyboard.press('Escape');

console.log('console errors:', logs.length ? logs.join('\n') : 'なし');
console.log(ng ? `NG ${ng} 件` : 'すべてOK');
await page.close();
await browser.close();
