# formDoc 残作業（TODO）

別セッションで実装を再開するための作業一覧。各項目に「目的・設計・触るファイル・手順・完了条件」を書いてある。
全体像は [README.md](README.md)、当初の計画は `C:\Users\tojo\.claude\plans\code-artifact-rust-svelte-wraped-by-buzzing-dongarra.md` を参照。

> **このファイルの決まり**
> - ここに載せるのは**未完了の作業だけ**。実装と確認が終わった項目（または項目内の一部）は**削除する**（「【済】」の印は付けない。経緯は Git の履歴とコミットメッセージで追う）。
> - 項目の番号は固定。削除した番号は再利用しない（他の項目から「TODO 3」のように参照しているため）。
> - 実装したが実機で確認していないものは、TODO 1（実機確認）の手順に残す。

---

## 0. 再開時に最初に読むこと

### 前提と決定事項（変更しない）
- 目的は「誰が作っても同じフォーマット・同じ内容になる」文書作成。体裁はスタイルで固定し、執筆者には触らせない。
- 執筆者／部品・スタイルの作成者は**役割上の区別にすぎない**。同一人物が兼ねる前提なので、ユーザー管理・権限管理・モード切替は作らない。
- 構成は Rust + Svelte 5（Tauri 2）。**同じソースから Web版とデスクトップ版を出力**する。片方でしか使えない機能は能力フラグ（`ui/src/lib/platform/capabilities.ts`）で宣言し、UIでは `<Gate cap="…">` で包んで無効表示にする。
- **文書は1つ**。「部品で作成」と「コードで作成（Typst）」は同じ文書の見え方の違い。コードは部品ごとの目印（`// @block ID`、まとまりは `// @group ID 名前`〜`// @end ID`）つきで表示し、書き換えた部品は Typstコード部品になる。エディタは自作しない（CodeMirror と「VSCodeで開く」）。
- **スタイル**＝1つの Typst ファイル（`info` 辞書＋`style` 関数）。同梱は `library/styles/`、利用者のものはシステムフォルダ（`%APPDATA%\formDoc\styles`）。保存ファイル（.fdoc）にソースを同梱する。
- **テンプレート**＝部品の並びの保存（`.fdtpl`）。挿入すると1つのまとまり（`group`）になる。変数の「入力／公開／内部」は保存時に決める。
- **変数の有効範囲**：定義した見出しの節・テンプレートのまとまりの中だけ（「グローバル変数として定義」なら文書全体）。この位置から使える変数と同じ名前は定義できない。
- 試作で完全対応する文書は**計算書**（`samples/計算書サンプル.pdf`）。要領書・作業計画書は後からスタイルと部品を足して対応する。
- 開発中のため**後方互換は考えない**（古い部品・古い保存形式の読み込みは残さない）。
- `samples/` のPDFはすべて画像PDF（テキスト層なし）。中身を見るときは PyMuPDF でPNGにして読む（`pymupdf` はインストール済み）。

### アーキテクチャの要点
| 層 | 場所 | 役割 |
|---|---|---|
| 式エンジン | `crates/formdoc-expr` | 構文解析・評価・四捨五入（half-up）・Typst数式生成。**計算と数式表記はここだけで行う** |
| Typstプラグイン | `crates/formdoc-typst-plugin` → `library/typst/formdoc/0.1.0/formdoc_expr.wasm` | 同じ式エンジンを Typst から使う。式エンジンを変更したら `bash scripts/build-plugin.sh` |
| 社内標準パッケージ | `library/typst/formdoc/0.1.0/` | `@local/formdoc`。部品の描画関数（計算行・照査・表・図形）。体裁はスタイルが持つ |
| スタイル | `library/styles/*.typ` | `info`（入力欄・使える部品・骨組み・単位別の桁・Lint）と `style` 関数。`template.rs` が Typst で評価して `info` を読む |
| 同梱テンプレート | `library/snippets/*.fdtpl` | I形断面、単純梁と集中荷重（影響線つき） |
| 埋め込み | `crates/formdoc-library` | `library/` 一式を `include_dir` でバイナリに埋め込む（`build.rs` で変更を検知） |
| コア | `crates/formdoc-core` | `world.rs`（Typst World）、`evaluate.rs`（上から順に評価・変数の有効範囲）、`codegen.rs`（文書→Typst）、`code.rs`（コードの編集を文書に戻す）、`lint.rs`、`api.rs`（`Session`。Web/デスクトップ共通の窓口） |
| Web版 | `crates/formdoc-wasm` + `ui/src/lib/engine/worker.ts` | Web Worker 内で wasm を実行 |
| デスクトップ版 | `ui/src-tauri` | `formdoc-core::api` を呼ぶ Tauri コマンドと、デスクトップ専用機能（ファイル・監視・VSCode・システムフォルダ） |
| UI | `ui/src` | `state.svelte.ts`（状態）、`tree.ts`（部品の木・変数が使える範囲）、`vars.ts`（変数の解析・名前の付け替え）、`expr.ts`（図形エディタ用の表示だけの式評価）、`lib/components/*`、`lib/fields/*` |

- 変数は「使う前に定義する」規則。依存関係は文書の並び順そのもので、循環参照は起こらない。入力のたびに文書全体を再評価する。
- 生成コードでは、変数は Typst 上で `v-名前` の識別子になる。Typstコード部品の中の `vdef("名前", 値, …)` / `vcalc("名前", "式", …)` も変数として読む（コードで書き換えても後ろの計算が続くように）。
- 汎用図形の座標・繰り返し・文字は `evaluate.rs` で評価し（`BlockResult.shapes`）、コード生成はその値を使う。
- プレビューはページ単位のSVG。ページのハッシュを送り合い、変更のないページは再送しない。

### よく使うコマンド（ビルドはメモリに注意。下の「この環境での注意」）
```bash
CARGO_BUILD_JOBS=1 cargo test -j 1 -p formdoc-core   # 結合テスト（現在 14件）＋単体
bash scripts/build-plugin.sh                          # 式エンジン変更時
cargo run -j 1 -p formdoc-cli -- gui examples/keisansho-gui/document.json out.pdf --typst out.typ
cargo run -j 1 -p formdoc-cli -- compile examples/keisansho-code out.pdf
cd ui && npm run build:wasm && npm run dev            # Web版（http://localhost:5173）
cd ui && npx svelte-check --tsconfig ./tsconfig.json
cd ui && npm run tauri dev                            # デスクトップ版
```

### この環境での注意
- **ビルドでメモリを使いすぎるとPCが落ちる。** cargo は `-j 1`（軽いときでも `-j 2`）、ビルド・テストは**同時に1本だけ**。release / LTO ビルドは必要なときだけ。
- **セッションが途中で切れることがある。** 長いビルドは PowerShell の `Start-Process cmd /C "...  > target\xxx.log"` で独立プロセスとして起動し、ログで結果を確認する。こまめにコミットする。
- `npm run build:wasm`（`scripts/build-plugin.sh` を呼ぶ）を cmd から起動すると `bash` が WSL を指して失敗する。Git Bash（`C:\Program Files\Git\usr\bin\bash.exe -lc "..."`）から起動する。
- ブラウザの自動テスト：Playwright の `launch` だと Chrome/Edge がすぐ落ちる。`chrome.exe --headless=new --remote-debugging-port=9222 --user-data-dir=<tmp>` で起動し、`chromium.connectOverCDP` で接続、`browser.newContext()` でまっさらな保存領域にする。スクリプトは `ui/tests/e2e/`。
- デスクトップ版（WebView2）は CDP のデバッグポートが開かない（社内ポリシーと思われる）。操作確認は人手か画面キャプチャで行う。
- 開発ビルドでは `globalThis.__formdoc` に状態（`app`）が公開されている（`state.svelte.ts` の末尾）。自動テスト専用。

---

## 優先度の目安

| 優先 | 項目 |
|---|---|
| 高 | 1 デスクトップ版の実機確認 / 2 wasm の軽量化 / 3 章構成の必須チェック |
| 中 | 5 部品の計算ロジック（Rhai） / 6 単位の次元チェック / 7 要領書のスタイル / 8 作業計画書のスタイル / 9 定型文ライブラリ |
| 中 | 10 表・荷重組合せの計算部品 / 11 ライブラリ版管理と共有フォルダ参照 / 12 UIの改善 / 13 自動テストの常設化 |
| 低 | 15 細かな既知の不具合・整理 |

---

## 1. デスクトップ版の実機確認【高】

**目的**: デスクトップ専用機能と、Web版でしか確認していない機能を、デスクトップ版の画面で実際に操作して確認する。

**確認手順**（`cd ui && npm run tauri dev`）
1. 文書情報にスタイル「計算書」が出る → 選ぶと骨組み3章と表紙の欄が出る。
2. 「開く…」で `examples/keisansho-gui/document.json` を開く → 3ページ、エラー0件。
3. 「名前を付けて保存…」で `.fdoc` を保存し、開き直す → 内容・添付画像・スタイルが戻る。2回目以降の Ctrl+S はダイアログなしで上書き。
4. 「PDF出力」→ CLI `formdoc gui` の出力と SHA-256 が一致する。
5. 一覧でドラッグして並べ替えられる（見出しは節ごと動く。テンプレートのまとまりの中・外へも動かせる）。見出し・まとまりを折りたためる。
6. 「＋部品を追加…」→「I形断面」を挿入 → 一覧に1つのまとまりとして出る。2回挿入してもエラーにならない。
7. 部品を右クリック →「テンプレートとして保存…」→ このPC に保存 → 追加ダイアログの「テンプレート：このPC」に出る。
8. 汎用図形の図形エディタ：描く・点を動かす・繰り返し（横の回数・間隔）が画面に出る。
9. 変更後にウィンドウを閉じる → 確認が出る。Ctrl+P → 文書のページだけが、1ページ1枚で印刷プレビューに出る（ページ番号が次の用紙にはみ出さない）。
10. 「コードで作成」→ 同じ文書が出る。値を書き換えると「部品で作成」に戻っても反映されている。
11. 「VSCodeで開く」→ `%APPDATA%\formDoc\projects\<表題>_<日付>` が作られて VSCode が開く。VSCode で保存 → 文書に反映。VSCode が無い環境ではエクスプローラーが開いてメッセージが出る。
12. 設定 → ダーク・文字の大きさ・最近使ったファイルの件数が効き、再起動後も残る。「開く▾」から最近使ったファイルを開ける。
13. インストーラー（`npm run tauri build`、release ビルドで重い）でインストールし、.fdoc をダブルクリック → その文書で起動する。

**直す可能性が高い箇所**
- `watch_project`：保存1回で複数イベントが来る。VSCode の一時ファイルで無駄な再組版が起きないか。必要ならイベントのパスを `main.typ` に絞る。
- `update_document` は `Mutex<Session>` を握ったまま組版する。連続入力で詰まらないか。

**完了条件**: 上の手順がすべて期待どおりに動くこと。動いたものはこの一覧から削除する。

---

## 2. Web版 wasm の軽量化【高】

**現状**: `formdoc_wasm_bg.wasm` が約72MB（gzip後 約37MB）。主な内訳は、同梱フォント約22MB（Noto Serif/Sans JP の Regular/Bold）、typst-assets のフォント（New Computer Modern ほか）、Typst 本体。

**設計**
1. **フォントを wasm から外す**
   - `formdoc-library` に Cargo feature `embed-fonts`（既定で有効）を追加する。`fonts()` は feature が無効なら空を返す。
   - `world.rs` の `font_store()` を「埋め込みフォント＋実行時に渡されたフォント」に変える。`FormdocWorld` に `add_fonts(Vec<Vec<u8>>)` を追加する。OnceLock の作り直しになるため、`Session::new_with_fonts(fonts)` を用意する。
   - `formdoc-wasm` は `embed-fonts` を無効にしてビルドし、`init_fonts(bytes: Vec<u8>)` を公開する。Worker が起動時に `fetch('fonts/NotoSerifJP-Regular.otf')` などで取得して渡す。ブラウザのキャッシュが効く。
   - **フォントは版ごとのハッシュを付けたファイル名で配信**し、違う版のフォントが混ざらないようにする（「同じ文書なら同じPDF」の前提を守るため）。`api::catalog()` が返すフォント一覧に SHA-256 を含め、起動時に照合して不一致なら警告する。
   - typst-assets の `fonts` feature も外し、数式用の New Computer Modern Math だけを `library/fonts/` に置く（Latin Modern ほかは不要）。
2. **フォントのサブセット化**（任意）：JIS第1・第2水準＋記号に絞る。ただし外字・人名で欠字が出るリスクがあるので、欠字の検出（Typst の warning "unknown font" や tofu）を検証パネルに出す仕組みとセットで行う。
3. `wasm-opt -Oz` を `scripts/build-wasm.mjs` の release 時に通す（binaryen が必要）。
4. 配布時は brotli 圧縮済みファイルを置く。

**触るファイル**: `crates/formdoc-library/{Cargo.toml,src/lib.rs}`、`crates/formdoc-core/src/world.rs`、`crates/formdoc-core/src/api.rs`、`crates/formdoc-wasm/src/lib.rs`、`ui/src/lib/engine/worker.ts`、`ui/scripts/build-wasm.mjs`、`ui/vite.config.ts`（`library/fonts` を `public/fonts` にコピー）

**完了条件**
- wasm 本体が 20MB 以下（目標 15MB）。
- 既存の結合テスト `pdf_is_deterministic` と、**Web版とネイティブのPDFのSHA-256一致**が引き続き成り立つ（`target/wasm-node` に Node 用バインディングを作って比較する手順は README に追記する）。
- 初回表示時間を計測して README に記載する。

---

## 3. 章構成の必須チェック【高】

**背景**: ユーザーの「要件整理メモ.md」に、構造形式ごとに章の有無・順序・書き方がばらつき、一目で違いが分からないという指摘がある（設計概要／設計条件／検討箇所／作用の種類／荷重の組合せ／材料特性値／共通仕様／グルーピング／モデル化／施工計画）。現状の `[[skeleton]]` は新規作成時の骨組みにすぎず、後から消したり順番を入れ替えたりできてしまう。

**設計**
- スタイルの `info` に章定義（`chapters`）を追加する（下は TOML 風に書いた中身。実際は Typst の辞書）。
  ```toml
  [[chapters]]
  id = "gaiyou"            # 章の識別子（見出しブロックの props.chapter に保存）
  title = "設計概要"        # 既定の見出し文
  required = true          # 必須
  fixed-title = true       # 見出し文の変更を禁止
  variants = ["上路桁", "工事桁", "トラス", "こ線橋"]   # 対象の構造形式（空なら全形式）
  guide = "業務の目的、対象構造物、設計範囲を記載する"   # 執筆ガイド（GUIに表示）
  allowed-blocks = ["paragraph", "table", "image"]     # 省略時はテンプレート全体の blocks
  ```
- 文書メタに `variant`（構造形式）を追加し、新規作成時に選ばせる。`info.skeleton` は `chapters` から生成する（`skeleton` は廃止）。
- `evaluate.rs` に章チェックを追加する。
  - 必須章がない → エラー（`code = "chapter-missing"`）
  - 章の順序がテンプレートと違う → エラー（`chapter-order`）
  - `fixed-title` の見出し文が変更されている → 警告と修正（fix）
  - 章ごとの `allowed-blocks` 以外の部品 → エラー
- GUI
  - アウトラインで章見出しに 🔒 を付け、削除・移動を禁止（`state.svelte.ts` の `removeBlock` / `moveBlock` で章見出しを判定）。
  - 章見出しを選ぶと、Inspector に `guide`（執筆ガイド）を表示する。
- コードモード：`#chapter("gaiyou")` 関数を `@local/formdoc` に追加し、Typst 側でも順序をチェックする（`state` で直前の章を記録し、`assert` で順序違反を検出）。

**触るファイル**: `library/styles/keisansho.typ`、`crates/formdoc-core/src/{template.rs,evaluate.rs,api.rs,model.rs}`、`ui/src/lib/components/{Outline.svelte,Inspector.svelte}`、`ui/src/lib/state.svelte.ts`、`library/styles/keisansho.typ`

**完了条件**: 必須章を消す・入れ替える・見出し文を変えるとそれぞれ検出されること（`tests/e2e.rs` に追加）。構造形式を切り替えると章構成が変わること。

---

## 5. 部品の計算ロジック（Rhai）【中】

**目的**: 当初要件 4.2 の「部品定義＝入力スキーマ＋計算ロジック＋Typstスニペット」のうち、計算ロジックを部品定義の側で追加できるようにする。例：荷重組合せの max/min、影響線縦距の自動計算、断面諸量（A, I, y_u, y_l）の算出。

**設計**
- 部品定義に `logic = "components/<id>.rhai"` と `render = "<Typst関数名>"` を追加する。
- `formdoc-core` に `rhai`（`sync`、`no_std` は不要。wasm でも動く）を追加し、`evaluate.rs` で該当部品の評価時に実行する。
  - 入力：`props`（Map）と、その時点までの変数（`name → #{value, unit, digits, desc}`）
  - 出力：`#{ vars: [...定義する変数], values: #{...描画に渡す値}, issues: [...] }`
  - サンドボックス：`Engine::new_raw()` にパッケージを最小限だけ登録する。`set_max_operations`、`set_max_call_levels`、`set_max_string_size` で無限ループと過大な処理を防ぐ。ファイル・時刻・乱数は登録しない（決定性のため）。
- コード生成：`#<render>(..values)` を出力し、定義した変数は `vdef` として出力する（コードモードとの一致を保つ）。
- Typst 側にも同じ計算が必要な場合は、Rhai ではなく **Typst 関数側で計算する**か、Rust の式エンジンに関数を追加して両方で使う。どちらにするかは部品ごとに判断し、`components.toml` にコメントで残す。
- 最初の実装例：**断面諸量**（I形断面の A, Iy, y_u, y_l を自動計算し、変数として定義）。サンプル p.40 の値（A=23,550 mm², I=2,080,000,000 mm⁴）を再現するテストを書く。

**触るファイル**: `crates/formdoc-core/{Cargo.toml,src/evaluate.rs,src/codegen.rs,src/template.rs}`、`library/components/`、`crates/formdoc-core/tests/e2e.rs`

**完了条件**: Rhai で無限ループを書いてもアプリが固まらないこと（操作数上限のエラーとして検出）。断面諸量の値がサンプルと一致すること。

---

## 6. 単位の次元チェック【中】

**目的**: kN と kN·m、N/mm² と kN/m² の取り違えを検出する。

**設計**
- `formdoc-expr` に `unit.rs` を追加する。単位文字列を「SI基本次元の指数ベクトル（M, L, T）＋倍率」に変換する。
  - 対応単位：N, kN, MN, kgf（任意）, m, mm, cm, km, s, h, Hz, Pa, kPa, MPa, N/mm2, kN/m2, kN/m3, °, %, 無次元
  - 合成：`kN*m`, `kN/m`, `N/mm^2`, `km/h`
- 式の評価と同時に次元を計算する（`eval` と並行する `dim_of(expr)`）。
  - `+`・`-`・比較：両辺の次元が違えばエラー（`unit-mismatch`）
  - `*`・`/`：次元を合成、`^`：指数が定数のときだけ（それ以外は無次元を要求）
  - `sqrt`：次元の指数を半分にする（奇数ならエラー）、`sin` などの引数：無次元
- 結果の単位と、計算行で指定した `unit` の次元が違えば警告。**倍率の違い**（kN と N）は換算係数を提示する（例：「結果は N 単位です。kN にするには /1000 が必要です」）。
- 数値リテラルは無次元として扱う。物理定数を含む式（例 `70 * L_b^(-0.8)` → Hz）は、計算行に `unit-check = false` を指定して検査を外せるようにする（部品定義に bool を追加）。
- まず警告として導入し、運用で誤検知が少ないことを確かめてからエラーに格上げする。

**触るファイル**: `crates/formdoc-expr/src/{unit.rs,lib.rs,eval.rs,tests.rs}`、`crates/formdoc-core/src/evaluate.rs`、`library/components/components.toml`

**完了条件**: `M = w * L^2 / 8`（w: kN/m, L: m）の結果が kN·m と判定されること。`w + L` が不一致エラーになること。サンプルの各計算で誤検知が出ないこと（`examples/keisansho-gui` で警告0件）。

---

## 7. 要領書のスタイル（youryousho）【中】

**参照**: `samples/要領書サンプル.pdf`（30ページ）。以下の特徴を再現する。
- 見出し：`3.3 構造細目` / `(1) 新設ホーム床版` / `1) 穴あきPC板・PPC板の細目`（§ は付かない。章は「3」）
- 図表番号：**「表 3.7:」「図 3.11:」**（章番号付き、コロン区切り、表は上・図は下にキャプション）
- 表：横罫線中心の booktabs 風（縦罫線なし。計算書の格子罫線とは違う）。セル内に図を入れる表（表 3.13, 3.14）がある
- 数式番号：右端に `(4.2)`
- 脚注：`※9` 形式（ページ下部、`第52回 SMFD 盛土WG…`）
- 箇条書き：`・`、番号付き `1)`
- 本文：見出しより1字下げ、段落頭1字下げ
- 書体：本文明朝、見出しゴシック（計算書は見出しも明朝）
- ページ番号：下部中央（計算書は上部中央）

**作業**
1. `library/styles/youryousho.typ`（1ファイル）を作る。`info`（入力欄・部品・Lint・章構成は TODO 3 の形式）と `style`（書体・見出し・図表番号・数式番号・脚注・ページ番号）を書く。`keisansho.typ` を複製して始める。
3. 部品を追加する（`components.toml`、`codegen.rs`、`evaluate.rs` の3か所）。
   - `list`（箇条書き。番号付き／なしを選べる）
   - `footnote` は独立した部品ではなく、本文の記法 `[[脚注:…]]` で書けるようにする（`text_content()` で `footnote[...]` に変換）
   - `equation`（数式＋番号。式は Typst 数式を直接書く。変数参照も可）
   - `table` に「罫線スタイル」を追加（テンプレートの既定に従う。部品側では選ばせない）
   - `table` のセルに画像を置けるようにする（`Cell` に `image: Option<String>`）
4. Lint：要領書の句読点は「、。」（サンプル準拠）。`info.lint` で切り替えられることを確認する。
5. `examples/youryousho-gui/document.json` を作り、サンプルの p.52〜55、p.60〜63、p.72〜75 相当を再現する。

**完了条件**: 再現したページをサンプルと並べて目視比較し、図表番号・罫線・見出し・脚注の体裁が一致すること。

---

## 8. 作業計画書のスタイル（sagyoukeikaku）【中】

**参照**: `samples/作業計画書サンプル.pdf`（4ページ、p.22〜25）。
- 見出し：`5 業務組織計画` / `5.1 業務担当者` / `(1) 照査時期`（ゴシック）
- **組織図**（図 5.1）：枠付きの箱（役職・所属・氏名）と、上下・分岐の接続線
- 住所・電話の一覧：項目名は左、`：（電話 03-5435-7630）` は右揃え
- 成果品の表（脚注付きの見出しセル）
- 定型文：照査計画の (1) 照査時期 / (2) 照査項目 / (3) 照査方法 は、ほぼ毎回同じ文章

**作業**
1. `library/styles/sagyoukeikaku.typ`（1ファイル）を作る。
2. 部品 `org-chart`（組織図）
   - 入力：ノードの表（id, 親id, 役割, 所属, 役職, 氏名の複数行）
   - 描画：CeTZ の `tree` で自動配置する。手で座標を指定させない（ぶれ防止）。
   - 役割の選択肢（主任技術者／照査技術者／照査者／担当者）はテンプレートで固定する。
3. 部品 `contact-list`（右揃えの項目リスト：左の文字列と右の文字列。区切りは `：`）。
4. 定型文：TODO 9 の仕組みで、照査計画の文章を部品として提供する。
5. `examples/sagyoukeikaku-gui/document.json` でサンプル4ページを再現する。

**完了条件**: サンプルの4ページを再現できること。組織図はノードを追加すると自動でレイアウトし直されること。

---

## 9. 定型文ライブラリ【中】

**目的**: 照査計画・照査方法などの決まった文章を、毎回書き写さずに選べるようにする。言い回しのぶれをなくす。

**設計**
- `library/phrases/<template>.toml`
  ```toml
  [[phrase]]
  id = "shousa-houhou"
  title = "照査方法（社内チェックリスト）"
  chapter = "shousa"                       # 使える章（TODO 3）
  text = "当社作成の「{{checklist}}」により照査を行う．"
  params = [{ key = "checklist", label = "チェックリスト名", default = "土木構造物設計チェックリスト" }]
  editable = false                         # 本文の編集を禁止（パラメータのみ入力）
  ```
- 部品 `phrase`：定型文を選び、パラメータだけを入力する。`editable = false` の文章は変更できない。
- 本文（paragraph）で定型文とほぼ同じ文章を手入力した場合は、Lint で「定型文 ○○ を使えます」と提案する（文字の類似度 0.8 以上を目安に）。

**触るファイル**: `library/phrases/`、`crates/formdoc-core/src/{template.rs,evaluate.rs,codegen.rs,lint.rs}`、`library/components/components.toml`

---

## 10. 表・荷重組合せの計算部品【中】

**背景**: 計算書サンプル p.33 の「作用の組合せおよび設計応答値のまとめ」（単ケース・耐荷性・耐疲労性の max/min）、p.18〜20 の「設計結果一覧表」、p.39 の「反力のまとめ」は、本文で計算した値を集計した表。現状の `table` 部品はセルに `{{変数}}` を書けるだけで、手作業が多い。

**設計**
- 部品 `load-combination`
  - 入力：荷重ケース（記号・名称・各断面力の変数名）と、組合せ式（`1.0D + 1.1L + 1.1I + 1.1C + {LR} + {LF} + {W}`。`{}` は「不利な場合のみ加える」）
  - 計算：max/min を自動で求め、変数として定義する（例 `M_max_c1`）。どの荷重を加えたかも記録する。
  - 描画：サンプル p.33 と同じ形の表
- 部品 `result-summary`（設計結果一覧）：照査部品の結果（左辺値・右辺値・OK/NG）を自動で集めて表にする。本文と一覧表の値の不一致が原理的に起こらなくなる。
- `table` の変数参照セルに書式指定を追加する：`{{M_max:2}}`（桁）、`{{M_max:unit}}`（単位付き）。
- 計算は TODO 5（Rhai）または Rust の組み込み関数で行う。組合せの max/min はテンプレートによらず共通なので、**Rust の組み込み**を推奨。

**完了条件**: サンプル p.33 の A点の表（max 402.86 / min 23.52 など）を、荷重ケースの値から再現できること。

---

## 11. ライブラリ版管理と共有フォルダ参照【中】

**現状**: `formdoc_library::version()` は**クレートのバージョン**を返している（`library/typst/formdoc/<ver>` とは連動していない）。文書には `library: "0.1.0"` を保存し、違えば警告しているだけ。

**設計**
- ライブラリの版を `library/library.toml`（`version`, `released`, `changelog`）で管理する。`formdoc-library` の build.rs で `library/` 全体の SHA-256 を計算し、`LIBRARY_HASH` として埋め込む。
- 文書の保存時に `library: { version, hash }` を記録する。開いたときに hash が違えば、**旧版と新版の両方で評価し、変わった値と体裁の差分を一覧で示す**（値は `Report.vars` を比較、体裁はページのハッシュを比較）。
- 旧版での再出力が必要な場合に備え、過去の版のライブラリを `library-archive/<ver>.zip` として残し、上書きレイヤー（`FormdocWorld` が埋め込み版より優先して読むファイル群）で読み込めるようにする。
- **共有フォルダ参照**（能力フラグ `sharedLibraryFolder`、デスクトップのみ）：設定画面で `\\server\formdoc\library` を指定すると、起動時に版を比較し、新しければ上書きレイヤーとして読み込む（読み取り専用）。Web版は配信サーバ上の `library.zip` を取得する方式にする（その場合は Web版でもフラグを有効にする）。

**触るファイル**: `crates/formdoc-library/{build.rs,src/lib.rs}`、`library/library.toml`、`crates/formdoc-core/src/{api.rs,evaluate.rs}`、`ui/src/lib/platform/capabilities.ts`、新規 `ui/src/lib/components/Settings.svelte`

---

## 12. UIの改善【中】

- **プレビューから該当ブロックへ移動**：コード生成時に各ブロックの先頭へ `#metadata("blk:<id>") <fd-blk>` を入れ、`typst::introspection` でページ上の位置を取得する。`UpdateResult` に `anchors: [{block_id, page, y}]` を追加し、プレビューのクリック位置から最も近いブロックを選ぶ。逆に、ブロックを選んだらプレビューをその位置までスクロールする。
- **変数名の一括変更**：変数名を変えると、式・`{{}}` 参照・記号説明の変数リストを、その変数が使える範囲で置き換える（確認ダイアログを出す）。`ui/src/lib/vars.ts` の `renameBlocks` と `tree.ts` の範囲判定を使う。
- **式入力の補完**：expr 欄で、変数名の候補（この位置で使える変数）と関数名を表示する。未定義の名前は入力中に赤下線を引く。
- **数式のライブプレビュー**：計算部品の Inspector に、生成した Typst 数式を SVG で小さく表示する（`api` に `render_snippet(typst) -> svg` を追加）。
- **一覧**：複数選択での移動・削除、キーボード操作（↑↓で選択、Alt+↑↓で移動）。
- **コードから部品に戻す**：コードモードで書き換えて Typstコード部品になった部品を、元の種類（変数定義・計算など）として読み直せるなら戻す（`code.rs`。`vdef`/`vcalc` だけの部品から始める）。
- 「部品を追加」ダイアログを、章の `allowed-blocks`（TODO 3）で絞り込む。

---

## 13. 自動テストの常設化【中】

- `ui/tests/e2e/review-2026-10.mjs`（スタイル選択・テンプレート挿入・右クリック・図形エディタ・テーマ・印刷）を、`npm run test:e2e` で Chrome 起動 → `vite preview` → テスト → 片付けまで行う `ui/scripts/e2e.mjs` に組み込む。`playwright-core` を devDependencies に入れる。
- 追加するテスト：サンプルを開く → 3ページ・エラー0件／計算式を壊す → PDF出力ボタンが無効／元に戻す → 復帰／句読点の一括修正／コードモードで書き換え → 部品で作成に反映／一覧のドラッグ並べ替え／変数の有効範囲（別の節の変数が使えない・グローバルなら使える）。
- 起動は `chrome --headless=new --remote-debugging-port=9222` ＋ `connectOverCDP`（この環境の制約。上の「注意」参照）。
- **Web版とネイティブのPDF一致テスト**を `scripts/check-determinism.mjs` として常設する（Node 用の wasm バインディングを生成 → 両方で出力 → SHA-256 比較）。
- **サンプル再現の回帰テスト**：`examples/*` のPDFを PNG 化し、前回の画像とピクセル差分で比較する（PyMuPDF を使用。差分しきい値を設定）。

---

## 15. 既知の不具合・整理【低】

| 場所 | 内容 | 対応 |
|---|---|---|
| `crates/formdoc-core/src/api.rs` `Session::run` | エラー時の `else if` 分岐が空 | 削除するか、直前の成功結果を表示中であることを `UpdateResult` に `stale: true` として返し、プレビューに「前回の結果を表示中」と出す |
| Typst の `vdef` / `vcalc` | `digits` 省略時にスタイルの単位別既定桁が効かない（GUIの部品だけ効く）ため、Typstコード部品で書いた変数の表示桁が変わりうる | スタイルの `info.digits` を `state` に置き、`vdef` / `vcalc` で `digits: auto` のとき参照する |
| `sum` 部品 | 合計行の書式（Σ記号の位置・下線の長さ）がサンプルと少し違う | サンプル p.21 に合わせて調整する |
| `check-line` | 右辺が式のとき、代入式が長いと1行に収まらない | 長い場合は記号式・代入式・判定の3行に分ける |
| 保存形式 `.fdoc` | JSON に base64 で画像を埋め込んでいるため、大きいCAD画像で重くなる | ZIP（`document.json` ＋ `assets/`）に変える |
| Tauri の `read_project` | バイナリを JSON の数値配列で返していて遅い | `tauri::ipc::Response` か、ファイルごとの `read_file` に分ける |
| `world.rs` `font_store()` | `OnceLock` のため、フォントを差し替えられない | TODO 2 で作り直す |
| `styles/keisansho.typ` | `show "、": "，"` で本文の句読点を強制置換しているため、コード内の文字列（基準書名など）も置換される | Lint で統一し、組版時の置換はやめるか、`para` 内だけに限定する |
| 見出し 1 の改ページ | `pagebreak(weak: true)` で必ず改ページする | スタイルの `info` で切り替えられるようにする（サンプルは章ごとに改ページ） |
| 表紙 | ページ番号の扱い（表紙を数えない）が固定 | スタイルの `info` に `page-number-start` を追加 |
| 汎用図形の繰り返し | 1つの図形あたり縦横それぞれ200回まで（`MAX_REPEAT`） | 足りなければ上限を見直す |
