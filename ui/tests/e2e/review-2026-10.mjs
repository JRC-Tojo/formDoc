// 試用レビュー（TODO 16）対応のブラウザ確認。Web版 dev サーバ（bun run dev）に対して実行する。
// 準備: chrome --headless=new --remote-debugging-port=9222 --user-data-dir=<tmp> を起動し、playwright-core を入れた場所で
//   bun ui/tests/e2e/review-2026-10.mjs <スクリーンショットの出力先>
// （TODO 13 で bun run test:e2e に組み込む予定）

const out = process.argv[2] ?? '.';
const browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
// 毎回まっさらな保存領域（下書き・設定なし）で始める
const ctx = await browser.newContext();
const page = await ctx.newPage();
await page.setViewportSize({ width: 1600, height: 1000 });
const logs = [];
page.on('console', (m) => m.type() === 'error' && logs.push(m.text()));
page.on('pageerror', (e) => logs.push('pageerror: ' + e.message));
const ok = (cond, msg) => console.log(`${cond ? 'OK ' : 'NG '} ${msg}`);
const st = (fn) => page.evaluate(fn);

await page.goto('http://localhost:5173/');
await page.waitForFunction(() => globalThis.__formdoc && globalThis.__formdoc.loading === '', null, { timeout: 120000 });
const errs = () => st(() => (__formdoc.result?.issues ?? []).filter((i) => i.severity === 'error').map((i) => i.message));

// 1. スタイルを選ぶと執筆開始
ok(await page.locator('button.style').count() >= 1, 'スタイル一覧が出る');
await page.locator('button.style', { hasText: '計算書' }).click();
await page.waitForFunction(() => __formdoc.pageHashes.length > 0, null, { timeout: 60000 });
ok((await st(() => __formdoc.doc.blocks.length)) === 3, '骨組み3章が入る');
ok((await page.locator('.inspector label', { hasText: '表題' }).count()) > 0, '表紙の欄（表題）が出る');
ok((await errs()).length === 0, 'エラー0件: ' + (await errs()).join(' / '));
await page.screenshot({ path: `${out}/1-style.png` });

// 2. 部品を追加ダイアログ → テンプレート「I形断面」を挿入（2回：内部変数の衝突回避）
for (let n = 1; n <= 2; n++) {
  await page.getByRole('button', { name: '＋ 部品を追加…' }).click();
  await page.locator('.modal .item', { hasText: 'I形断面' }).click();
  if (n === 1) await page.screenshot({ path: `${out}/2-insert.png` });
  await page.locator('.modal aside button.primary').click();
  if (n === 1) await page.screenshot({ path: `${out}/3-bind.png` });
  await page.locator('.modal footer button.primary', { hasText: '挿入' }).click();
  await page.waitForTimeout(1500);
}
const names = await st(() => __formdoc.result.vars.map((v) => v.name));
ok(names.includes('A') && names.includes('A_2'), '2回目の公開変数は A_2: ' + names.join(','));
ok(!names.some((v) => /^(H|B|t_w|t_f)_[0-9]/.test(v)), '2回目は入力が既存の変数（H, B, t_w, t_f）につながる: ' + names.join(','));
ok((await errs()).length === 0, 'テンプレート挿入後エラー0件: ' + (await errs()).join(' / '));
await page.screenshot({ path: `${out}/4-inserted.png` });

// 3. 右クリックメニュー
await page.locator('.outline .row').first().click({ button: 'right' });
ok(await page.locator('.ctx', { hasText: 'テンプレートとして保存' }).isVisible(), '右クリックにテンプレートとして保存');
await page.screenshot({ path: `${out}/5-context.png` });
await page.locator('.ctx button', { hasText: 'テンプレートとして保存' }).click();
await page.waitForTimeout(300);
await page.screenshot({ path: `${out}/6-save-template.png` });
await page.locator('.modal footer button.primary', { hasText: '保存' }).click();
await page.waitForTimeout(800);
ok((await st(() => __formdoc.templates.filter((t) => t.source === 'このPC').length)) === 1, '「このPC」にテンプレートが保存される');

// 4. 汎用図形を描く
await st(() => __formdoc.addBlock('fig-shapes'));
await page.waitForSelector('.editor svg');
await page.locator('.tools button[title="矩形"]').click();
const box = await page.locator('.editor svg').boundingBox();
await page.mouse.move(box.x + 200, box.y + 300);
await page.mouse.down();
await page.mouse.move(box.x + 400, box.y + 200, { steps: 5 });
await page.mouse.up();
await page.locator('.tools button[title="線"]').click();
await page.mouse.move(box.x + 200, box.y + 400);
await page.mouse.down();
await page.mouse.move(box.x + 450, box.y + 400, { steps: 5 });
await page.mouse.up();
await page.screenshot({ path: `${out}/7-draw.png` });
await page.locator('.modal footer button.primary', { hasText: 'OK' }).click();
await page.waitForTimeout(1500);
const shapes = await st(() => __formdoc.selected?.props.shapes);
ok(shapes?.length === 2, '図形が2つ保存される: ' + JSON.stringify(shapes));
ok((await errs()).length === 0, '図形追加後エラー0件: ' + (await errs()).join(' / '));

// 5. I形断面の図を開く → 式の点はロック表示
const isec = await st(() => __formdoc.doc.blocks.find((b) => b.kind === 'fig-shapes' && b.props.caption === 'I形断面').id);
await page.evaluate((id) => (__formdoc.dialog = { kind: 'shapes', blockId: id }), isec);
await page.waitForSelector('.editor svg');
await page.waitForTimeout(400);
await page.locator('.editor .list button').first().click();
ok((await page.locator('.handle.locked').count()) > 0, '式の点はロック表示');
await page.screenshot({ path: `${out}/8-locked.png` });
await page.keyboard.press('Escape');

// 6. 設定：ダーク
await page.getByRole('button', { name: '⚙ 設定' }).click();
await page.locator('.modal select').first().selectOption('dark');
ok((await st(() => document.documentElement.dataset.theme)) === 'dark', 'ダークテーマ');
await page.locator('.modal footer button.primary').click();
await page.screenshot({ path: `${out}/9-dark.png` });

// 7. 印刷用のページだけが印刷対象
await page.emulateMedia({ media: 'print' });
ok(await page.locator('.print-pages').isVisible(), '印刷時は文書ページのみ表示');
ok(!(await page.locator('.app').isVisible()), '印刷時は画面（ツールバー等）を出さない');
await page.emulateMedia({ media: 'screen' });

console.log('console errors:', logs.length ? logs.join('\n') : 'なし');
await page.close();
await browser.close();
