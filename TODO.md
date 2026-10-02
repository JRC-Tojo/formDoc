# formDoc 残作業（TODO）

別セッションで実装を再開するための作業一覧。各項目に「目的・設計・触るファイル・手順・完了条件」を書いてある。
全体像は [README.md](README.md)、当初の計画は `C:\Users\tojo\.claude\plans\code-artifact-rust-svelte-wraped-by-buzzing-dongarra.md` を参照。

---

## 0. 再開時に最初に読むこと

### 前提と決定事項（変更しない）
- 目的は「誰が作っても同じフォーマット・同じ内容になる」文書作成。体裁はテンプレートで固定し、執筆者には触らせない。
- 執筆者／コンポーネント作成者は**役割上の区別にすぎない**。同一人物が兼ねる前提なので、ユーザー管理・権限管理は作らない。
- 構成は Rust + Svelte 5（Tauri 2）。**同じソースから Web版とデスクトップ版を出力**する。片方でしか使えない機能は能力フラグ（`ui/src/lib/platform/capabilities.ts`）で宣言し、UIでは `<Gate cap="…">` で包んで無効表示にする。
- コードモードは**生のTypst**で書く。エディタは自作しない。デスクトップ版は「VSCodeで開く」と保存の監視で対応する。Web版は既製の CodeMirror を最小構成で使う。
- 試作で完全対応する文書は**計算書**（`samples/計算書サンプル.pdf`）。要領書・作業計画書は後から型（テンプレート）と部品を足して対応する。
- `samples/` のPDFはすべて画像PDF（テキスト層なし）。中身を見るときは PyMuPDF でPNGにして読む（`pymupdf` はインストール済み）。

### アーキテクチャの要点
| 層 | 場所 | 役割 |
|---|---|---|
| 式エンジン | `crates/formdoc-expr` | 構文解析・評価・四捨五入（half-up）・Typst数式生成。**計算と数式表記はここだけで行う** |
| Typstプラグイン | `crates/formdoc-typst-plugin` → `library/typst/formdoc/0.1.0/formdoc_expr.wasm` | 同じ式エンジンをコードモードから使う。式エンジンを変更したら `bash scripts/build-plugin.sh` |
| 社内標準パッケージ | `library/typst/formdoc/0.1.0/` | `@local/formdoc`。スタイル（`keisansho`）と部品の描画関数。GUIの生成コードもコードモードもこれを呼ぶ |
| 埋め込み | `crates/formdoc-library` | `library/` 一式を `include_dir` でバイナリに埋め込む（`build.rs` で変更を検知） |
| コア | `crates/formdoc-core` | `world.rs`（Typst World）、`evaluate.rs`（上から順に評価）、`codegen.rs`（GUI文書→Typst）、`lint.rs`、`api.rs`（`Session`。Web/デスクトップ共通の窓口） |
| Web版 | `crates/formdoc-wasm` + `ui/src/lib/engine/worker.ts` | Web Worker 内で wasm を実行 |
| デスクトップ版 | `ui/src-tauri` | `formdoc-core::api` を呼ぶ Tauri コマンドと、デスクトップ専用機能（ファイル・監視・VSCode） |
| UI | `ui/src` | `state.svelte.ts`（状態）、`lib/components/*`、`lib/fields/*`（部品定義から自動生成するフォーム） |

- 変数は「使う前に定義する」規則にしている。依存関係は文書の並び順そのもので、循環参照は起こらない。入力のたびに文書全体を再評価する。
- GUIの生成コードでは、変数は Typst 上で `v-名前` の識別子になる。
- プレビューはページ単位のSVG。ページのハッシュを送り合い、変更のないページは再送しない。

### よく使うコマンド
```bash
cargo test -p formdoc-expr -p formdoc-core          # 単体・結合テスト（現在 19件）
bash scripts/build-plugin.sh                         # 式エンジン変更時
cargo run -p formdoc-cli -- gui examples/keisansho-gui/document.json out.pdf --typst out.typ
cargo run -p formdoc-cli -- compile examples/keisansho-code out.pdf
cd ui && npm run build:wasm && npm run dev           # Web版（http://localhost:5173）
cd ui && npx svelte-check --tsconfig ./tsconfig.json
cd ui && npm run tauri dev                           # デスクトップ版
```

### この環境での注意
- **セッションが途中で切れることがある。** 長いビルド（wasm の release ビルドは約10分、wasm-dev は約5分）は、PowerShell の `Start-Process cmd /C "...  > target\xxx.log"` で独立プロセスとして起動し、ログで結果を確認する。`run_in_background` で起動したプロセスはセッション終了時に止まる。
- ブラウザの自動テスト：Playwright の `launch` だと Chrome/Edge がすぐ落ちる。`chrome.exe --headless=new --remote-debugging-port=9222 --user-data-dir=<tmp>` で起動し、`chromium.connectOverCDP` で接続すれば動く。前回使ったスクリプトは作業用ディレクトリにあり、消えている可能性がある（→ TODO 13 で常設化する）。
- デスクトップ版（WebView2）は `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=…` を付けてもデバッグポートが開かなかった（社内ポリシーと思われる）。操作確認は人手か画面キャプチャで行う。
- 開発ビルドでは `globalThis.__formdoc` に状態（`app`）が公開されている（`state.svelte.ts` の末尾）。自動テスト専用。
- リポジトリはまだ Git 管理になっていない（→ TODO 14）。

---

## 優先度の目安

| 優先 | 項目 |
|---|---|
| 高 | 1 デスクトップ版の実地確認 / 2 wasm の軽量化 / 3 章構成の必須チェック / 14 Git化 |
| 中 | 4 テンプレート開発モード / 5 部品の計算ロジック（Rhai） / 6 単位の次元チェック / 7 要領書の型 / 8 作業計画書の型 / 9 定型文ライブラリ |
| 中 | 10 表・荷重組合せの計算部品 / 11 ライブラリ版管理と共有フォルダ参照 / 12 UIの改善 / 13 自動テストの常設化 |
| 低 | 15 細かな既知の不具合・整理 |

---

## 1. デスクトップ版の実地確認【高】

**目的**: デスクトップ専用機能を、画面から実際に操作して確認する（前回は自動操作できず、起動と再組版しか確認していない）。

**確認手順**（`cd ui && npm run tauri dev`）
1. 「開く…」で `examples/keisansho-gui/document.json` を開く → 3ページ、エラー0件になること。
2. 「名前を付けて保存…」で `.fdoc` を保存し、閉じて「開く…」で開き直す → 内容と添付画像が戻ること。
3. 「保存」（Ctrl+S）で、2回目以降はダイアログなしで同じパスに上書きされること。
4. 「PDF出力」→ 保存ダイアログ → PDFが開けること。CLI `formdoc gui` の出力と SHA-256 が一致すること。
5. 画像部品で PNG / SVG / PDF を選ぶ → プレビューに出ること。PDF の「PDFのページ」欄が効くこと。
6. 「コードで作成」→「フォルダを開く…」で空のフォルダを選ぶ → `main.typ` が作られ、監視中と表示されること。
7. 「VSCodeで開く」→ VSCode が開くこと。`main.typ` を編集・保存 → 1秒以内にプレビューが更新されること。
8. `code` コマンドが PATH にない環境では、エクスプローラーが開いてエラーメッセージが出ること。
9. 構文エラーを入れて保存 → 下部に `/main.typ:行番号` 付きの診断が出ること。

**直す可能性が高い箇所**
- `ui/src-tauri/src/lib.rs` の `watch_project`：保存1回で複数イベントが来る。JS側で 150ms 間引いているが、VSCode の一時ファイル（`.main.typ.swp` など）で無駄な再組版が起きないか確認する。必要ならイベントのパスで `.typ`・画像に絞り込む。
- `open_in_vscode`：`cmd /C code <folder> <main.typ>` は、パスに空白や日本語を含むと失敗するおそれがある。失敗したら `Command::new("cmd").raw_arg(...)` で引用符を付ける。
- `update_document` / `update_project` は `Mutex<Session>` を握ったまま組版する。連打して固まらないか確認する。

**完了条件**: 上の 1〜9 がすべて期待どおりに動くこと。結果は README の「能力フラグ」表の下に追記する。

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
- `template.toml` に章定義を追加する。
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
- 文書メタに `variant`（構造形式）を追加し、新規作成時に選ばせる。`skeleton` は `chapters` から生成する（`skeleton` は廃止）。
- `evaluate.rs` に章チェックを追加する。
  - 必須章がない → エラー（`code = "chapter-missing"`）
  - 章の順序がテンプレートと違う → エラー（`chapter-order`）
  - `fixed-title` の見出し文が変更されている → 警告と修正（fix）
  - 章ごとの `allowed-blocks` 以外の部品 → エラー
- GUI
  - アウトラインで章見出しに 🔒 を付け、削除・移動を禁止（`state.svelte.ts` の `removeBlock` / `moveBlock` で章見出しを判定）。
  - 章見出しを選ぶと、Inspector に `guide`（執筆ガイド）を表示する。
- コードモード：`#chapter("gaiyou")` 関数を `@local/formdoc` に追加し、Typst 側でも順序をチェックする（`state` で直前の章を記録し、`assert` で順序違反を検出）。

**触るファイル**: `library/templates/keisansho/template.toml`、`crates/formdoc-core/src/{template.rs,evaluate.rs,api.rs,model.rs}`、`ui/src/lib/components/{Outline.svelte,Inspector.svelte}`、`ui/src/lib/state.svelte.ts`、`library/typst/formdoc/0.1.0/src/keisansho.typ`

**完了条件**: 必須章を消す・入れ替える・見出し文を変えるとそれぞれ検出されること（`tests/e2e.rs` に追加）。構造形式を切り替えると章構成が変わること。

---

## 4. テンプレート開発モード（M7）【中】

**目的**: 同じ人が「執筆」と「型・部品の作成」を切り替えられるようにする（ユーザー管理はしない。モード切替だけ）。

**設計**
- ツールバーに「テンプレート開発」トグルを追加する。オンにすると左ペインが「型・部品・Typstパッケージ」のファイルツリーになる。
- 編集対象（すべて `library/` 配下のテキスト）
  - `templates/<id>/template.toml`
  - `components/components.toml`（部品定義）
  - `typst/formdoc/<ver>/**/*.typ`（スタイル・描画関数）
  - `typst/formdoc/<ver>/data/references.toml`（基準書レジストリ）
- **ライブラリの上書きレイヤー**
  - `FormdocWorld` と `template.rs` が `formdoc_library::get()` を直接呼んでいる箇所を、`LibrarySource` トレイト（`get(path) -> Option<Bytes>`、`files()`）経由に変える。
  - `Session` に `set_library_overlay(files: Vec<(path, bytes)>)` を追加し、埋め込み版より上書きを優先して読む。
  - 上書きがある間は、検証パネルと PDF に「開発中のライブラリ」と警告を出し、**PDF出力を禁止**する（正式版以外での出力を防ぐ）。
- **テスト入力**：部品ごとにテスト用 props（`components/<id>.test.json`）を用意し、開発モードではその部品だけを描いた1ページを即時プレビューする。
- **保存先**
  - デスクトップ：ローカルの `library/` を直接編集し、変更を監視する（`watch_project` と同じ仕組み）。
  - Web：IndexedDB に上書きを保存し、「ライブラリをZIPで書き出し」で配布物を作る。
- **版の確定**：`typst.toml` の version と `template.toml` を上げ、`library/CHANGELOG.md` に変更内容を書く手順をUIで案内する（自動化は TODO 11）。

**触るファイル**: `crates/formdoc-library/src/lib.rs`、`crates/formdoc-core/src/{world.rs,template.rs,api.rs}`、`crates/formdoc-wasm/src/lib.rs`、`ui/src-tauri/src/lib.rs`、`ui/src/lib/state.svelte.ts`、新規 `ui/src/lib/components/LibraryDev.svelte`

**完了条件**: 開発モードで `keisansho.typ` の見出しの書式を変えるとプレビューに即時反映されること。モードを解除すると元に戻ること。上書き中はPDF出力ができないこと。

---

## 5. 部品の計算ロジック（Rhai）【中】

**目的**: 当初要件 4.2 の「部品定義＝入力スキーマ＋計算ロジック＋Typstスニペット」のうち、計算ロジックをテンプレート側で追加できるようにする。例：荷重組合せの max/min、影響線縦距の自動計算、断面諸量（A, I, y_u, y_l）の算出。

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

## 7. 要領書の型（youryousho）【中】

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
1. `library/typst/formdoc/0.1.0/src/youryousho.typ` を作り、`lib.typ` から公開する（書体・見出し・図表番号・数式番号・脚注・ページ番号）。
2. `library/templates/youryousho/template.toml` を作る（`show = "youryousho"`、章構成は TODO 3 の形式で）。
3. 部品を追加する（`components.toml`、`codegen.rs`、`evaluate.rs` の3か所）。
   - `list`（箇条書き。番号付き／なしを選べる）
   - `footnote` は独立した部品ではなく、本文の記法 `[[脚注:…]]` で書けるようにする（`text_content()` で `footnote[...]` に変換）
   - `equation`（数式＋番号。式は Typst 数式を直接書く。変数参照も可）
   - `table` に「罫線スタイル」を追加（テンプレートの既定に従う。部品側では選ばせない）
   - `table` のセルに画像を置けるようにする（`Cell` に `image: Option<String>`）
4. Lint：要領書の句読点は「、。」（サンプル準拠）。`template.toml` の `[lint]` で切り替えられることを確認する。
5. `examples/youryousho-gui/document.json` を作り、サンプルの p.52〜55、p.60〜63、p.72〜75 相当を再現する。

**完了条件**: 再現したページをサンプルと並べて目視比較し、図表番号・罫線・見出し・脚注の体裁が一致すること。

---

## 8. 作業計画書の型（sagyoukeikaku）【中】

**参照**: `samples/作業計画書サンプル.pdf`（4ページ、p.22〜25）。
- 見出し：`5 業務組織計画` / `5.1 業務担当者` / `(1) 照査時期`（ゴシック）
- **組織図**（図 5.1）：枠付きの箱（役職・所属・氏名）と、上下・分岐の接続線
- 住所・電話の一覧：項目名は左、`：（電話 03-5435-7630）` は右揃え
- 成果品の表（脚注付きの見出しセル）
- 定型文：照査計画の (1) 照査時期 / (2) 照査項目 / (3) 照査方法 は、ほぼ毎回同じ文章

**作業**
1. `sagyoukeikaku.typ` と `templates/sagyoukeikaku/template.toml` を作る。
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
- 旧版での再出力が必要な場合に備え、過去の版のライブラリを `library-archive/<ver>.zip` として残し、TODO 4 の上書きレイヤーで読み込めるようにする。
- **共有フォルダ参照**（能力フラグ `sharedLibraryFolder`、デスクトップのみ）：設定画面で `\\server\formdoc\library` を指定すると、起動時に版を比較し、新しければ上書きレイヤーとして読み込む（読み取り専用）。Web版は配信サーバ上の `library.zip` を取得する方式にする（その場合は Web版でもフラグを有効にする）。

**触るファイル**: `crates/formdoc-library/{build.rs,src/lib.rs}`、`library/library.toml`、`crates/formdoc-core/src/{api.rs,evaluate.rs}`、`ui/src/lib/platform/capabilities.ts`、新規 `ui/src/lib/components/Settings.svelte`

---

## 12. UIの改善【中】

- **プレビューから該当ブロックへ移動**：コード生成時に各ブロックの先頭へ `#metadata("blk:<id>") <fd-blk>` を入れ、`typst::introspection` でページ上の位置を取得する。`UpdateResult` に `anchors: [{block_id, page, y}]` を追加し、プレビューのクリック位置から最も近いブロックを選ぶ。逆に、ブロックを選んだらプレビューをその位置までスクロールする。
- **変数名の一括変更**：変数名を変えると、式・`{{}}` 参照・記号説明の変数リストを文書全体で置き換える（確認ダイアログを出す）。`formdoc-expr` に「式中の変数名置換」（AST を保ったまま文字列を置換）を追加する。
- **式入力の補完**：expr 欄で、変数名の候補（定義済み・この位置より前のもの）と関数名を表示する。未定義の名前は入力中に赤下線を引く。
- **数式のライブプレビュー**：計算部品の Inspector に、生成した Typst 数式を SVG で小さく表示する（`api` に `render_snippet(typst) -> svg` を追加）。
- **アウトライン**：章ごとの折りたたみ、複数選択での移動・削除、キーボード操作（↑↓で選択、Alt+↑↓で移動）。
- 「部品を追加」メニューを、章の `allowed-blocks`（TODO 3）で絞り込む。
- 保存していない変更がある状態で閉じようとしたときの確認（デスクトップ版は Tauri の `onCloseRequested` を使う。現状の `beforeunload` は WebView2 では効かない場合がある）。

---

## 13. 自動テストの常設化【中】

- `ui/tests/e2e/` に Playwright（`playwright-core`）のスクリプトを置く。前回行った確認を常設のテストにする。
  1. サンプルを開く → 3ページ・エラー0件
  2. 計算式を壊す → エラー表示、PDF出力ボタンが無効
  3. 元に戻す → 復帰
  4. 句読点の一括修正
  5. PDF出力（ダウンロード）
  6. コードモードへの切替、Web版で「フォルダを開く」「VSCodeで開く」が無効表示であること
- 起動は `chrome --headless=new --remote-debugging-port=9222` ＋ `connectOverCDP`（この環境の制約。上の「注意」参照）。`npm run test:e2e` で Chrome 起動 → `vite preview` → テスト → 片付けまで行う `ui/scripts/e2e.mjs` を作る。
- **Web版とネイティブのPDF一致テスト**を `scripts/check-determinism.mjs` として常設する（Node 用の wasm バインディングを生成 → 両方で出力 → SHA-256 比較）。
- **サンプル再現の回帰テスト**：`examples/*` のPDFを PNG 化し、前回の画像とピクセル差分で比較する（PyMuPDF を使用。差分しきい値を設定）。

---

## 14. Git化【高】

- リポジトリ化して最初のコミットを作る。`.gitignore` には `target/`、`ui/node_modules/`、`ui/dist-*`、`ui/src/lib/engine/pkg/`、`ui/src-tauri/gen/` を入れる。
- 【済】`library/fonts/` と `library/vendor/` は Git に含めず、`vendor.json`（版と sha256 を固定）に従い `node scripts/setup.mjs` で取得する。`samples/*.pdf` は社内資料のため Git に含めない。
- 【済】`library/typst/formdoc/0.1.0/formdoc_expr.wasm` は生成物として Git に含めない（配布はインストーラーとWeb版のみのため）。`scripts/setup.mjs` が未生成ならビルドする。

---

## 15. 既知の不具合・整理【低】

| 場所 | 内容 | 対応 |
|---|---|---|
| `crates/formdoc-core/src/codegen.rs` | `let _ = refs_in_text;`、`let _ = report;` という不要な行が残っている | 削除し、未使用の引数・import を整理 |
| `crates/formdoc-core/src/api.rs` `Session::run` | エラー時の `else if` 分岐が空 | 削除するか、直前の成功結果を表示中であることを `UpdateResult` に `stale: true` として返し、プレビューに「前回の結果を表示中」と出す |
| コードモードの `vdef` / `vcalc` | `digits` 省略時にテンプレートの単位別既定桁が効かない（GUIだけ効く）ため、同じ値でも表示桁が変わりうる | `keisansho.with(...)` で単位→桁の表を `state` に置き、`vdef` / `vcalc` で `digits: auto` のとき参照する。表は `template.toml` から生成して Typst パッケージに同梱する |
| `api::code_template` / `state.codeTemplateId` | コードモードの型が `keisansho` に固定 | 新規作成時に型を選ばせる。`formdoc.toml`（プロジェクト設定）に型を記録する |
| `fig-beam` の影響線 | 左支点で1、右支点で0の三角形を前提にしている | 縦距の点列を受け取り、折れ線として描く汎用版にする |
| `sum` 部品 | 合計行の書式（Σ記号の位置・下線の長さ）がサンプルと少し違う | サンプル p.21 に合わせて調整する |
| `check-line` | 右辺が式のとき、代入式が長いと1行に収まらない | 長い場合は記号式・代入式・判定の3行に分ける |
| 保存形式 `.fdoc` | JSON に base64 で画像を埋め込んでいるため、大きいCAD画像で重くなる | ZIP（`document.json` ＋ `assets/`）に変える。読込は旧形式（JSON）にも対応する |
| Tauri の `read_project` | バイナリを JSON の数値配列で返していて遅い | `tauri::ipc::Response` か、ファイルごとの `read_file` に分ける |
| `update_document` のロック | `Mutex<Session>` を握ったまま組版するため、連続入力で詰まる可能性 | JS 側の間引き（現状 250ms）で足りているか TODO 1 で確認。足りなければ世代番号で古い要求を捨てる |
| `world.rs` `font_store()` | `OnceLock` のため、フォントを差し替えられない | TODO 2 で作り直す |
| `keisansho.typ` | `show "、": "，"` で本文の句読点を強制置換しているため、コード内の文字列（基準書名など）も置換される | Lint で統一し、組版時の置換はやめるか、`para` 内だけに限定する |
| 見出し 1 の改ページ | `pagebreak(weak: true)` で必ず改ページする | テンプレート設定で切り替えられるようにする（サンプルは章ごとに改ページ） |
| 表紙 | ページ番号の扱い（表紙を数えない）が固定 | テンプレート設定に `page-number-start` を追加 |
