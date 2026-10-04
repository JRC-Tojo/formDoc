# formDoc（試作版）

「誰が作っても同じフォーマット・同じ内容になる」技術文書作成アプリ。組版エンジンに Typst を使い、
体裁は文書テンプレート（1文書テンプレート＝1 Typst ファイル）で固定、計算は式エンジンで自動化する。最初の対象は **計算書**。

- **部品で作成**（GUI）：見出し・本文・変数定義・計算・照査・記号説明・表・図などの部品を並べ、値を入れるだけ
- **コードで作成**（Typst）：**同じ文書**を Typst のコードとして見る・編集する（部品ごとに `// @block ID` の目印つき）。書き換えた部品は Typstコード部品になる。デスクトップ版は「VSCodeで開く」で外部エディタでも編集でき、保存すると文書に反映される
- Web版とデスクトップ版を同じソースから出力。片方でしか使えない機能は能力フラグで無効表示

## ぶれを防ぐ仕組み

| 仕組み | 内容 |
|---|---|
| 文書テンプレート | `styles/<id>.typ`（1ファイル）。`info` に文書情報（表紙）の入力欄・章の骨組み・使える部品・単位ごとの表示桁・表記規則、`style` 関数に体裁。同梱は `library/styles/`、利用者のものはシステムフォルダの `styles/`。保存ファイルには文書テンプレートのソースを同梱する |
| 部品テンプレート | 部品の並び（節・図など）を `.fdtpl` に保存して使い回す。挿入すると1つのまとまり（一覧で折りたためる）になる。変数は「入力／公開／内部」を保存時に決め、挿入時は入力をつなぐだけ |
| 変数の有効範囲 | 変数は定義した見出しの節・部品テンプレートのまとまりの中だけで使える（「グローバル変数として定義」なら文書全体）。大きな文書でも名前がぶつかりにくい |
| 汎用図形 | 線・矢印・矩形・円・多角形・寸法線・文字を描画エディタで描く。座標に変数・式、図形ごとに縦横の繰り返し（回数・間隔、番号 `ix`・`iy`）、文字に `{{式:桁}}` |
| 体裁の固定 | 書体・余白・見出し番号（§4．/4.1/(1)/1)）・図表番号・表紙は文書テンプレートで固定。部品の描画関数は `library/typst/formdoc`。執筆者は変更できない |
| 計算の自動化 | 式から「記号式 = 代入式 = 結果」を自動生成。値は表示桁で丸めた値で後続計算（読者が電卓で検算できる） |
| 照査の自動判定 | 紙面に表示される値どうしで OK/NG を判定 |
| 記号説明 | 「ここに，」は直前の計算で使った変数から自動生成。説明漏れは警告 |
| 基準引用 | 登録済みの略称（`data/references.toml`）からのみ選択 |
| 表記Lint | 句読点「，．」、全角英数字、半角カナ、表記ゆれ、単位表記。ワンクリック／一括修正 |
| 出力の再現性 | フォント・パッケージは同梱のみ使用。同じ文書なら Web版・デスクトップ版・誰のPCでも**同一バイトのPDF** |
| 出力の制限 | エラー（未定義変数・計算不能・組版エラー）が残る文書はPDF出力できない |

## 構成

```
crates/
  formdoc-expr/          式エンジン（構文解析・評価・四捨五入・Typst数式生成）
  formdoc-typst-plugin/  式エンジンの Typst プラグイン版（コードモードでも同じ計算・表記にするため）
  formdoc-library/       library/ 一式をバイナリに埋め込む
  formdoc-core/          World・評価・コード生成・Lint・api::Session（Web/デスクトップ共通の窓口）
  formdoc-wasm/          Web版の wasm-bindgen ラッパー
  formdoc-cli/           コマンドライン（検証・CI用）
library/
  typst/formdoc/0.1.0/   社内標準 Typst パッケージ（文書テンプレート・部品の描画関数・formdoc_expr.wasm ※生成物、Git 管理外）
  styles/                同梱文書テンプレート（keisansho.typ ＝ 計算書）
  snippets/              同梱部品テンプレート（.fdtpl：I形断面、単純梁と集中荷重）
  components/            部品定義（GUIの入力フォームはここから自動生成）
  vendor/preview/        同梱 Typst パッケージ（CeTZ ほか）※bun ready で取得、Git 管理外
  fonts/                 同梱フォント（Noto Serif JP / Noto Sans JP、OFL）※bun ready で取得、Git 管理外
ui/                      Vite + Svelte 5（--mode web / desktop）
  src/lib/platform/      能力フラグ（capabilities.ts）と Web / Tauri 実装
  src-tauri/             デスクトップ版（Tauri 2）
examples/                GUI文書（document.json）とコードモードの例
```

## ビルド

前提: [Rust](https://rustup.rs)（1.85 以上）と [Bun](https://bun.sh)。ツールチェーンは bun に統一している（Node.js・npm は不要）。

```bash
# 初回（クローン直後）・pull 後：これだけで起動できる状態になる。済んでいる手順はすぐ終わる
bun ready                   # wasm32 ターゲット・wasm-bindgen-cli（Cargo.lock と同じ版）の導入、bun install、
                            # フォント・Typstパッケージの取得（vendor.json の sha256 で検証）、Typstプラグイン・Web版エンジンのビルド
                            # デスクトップ版だけなら bun ready --desktop（Web版エンジンを作らない）

bun run dev                 # Web版の開発サーバ http://localhost:5173
bun run tauri dev           # デスクトップ版の開発起動

# 式エンジンを変更したら（Typstプラグインを再生成）。bun ready でもよい
bun scripts/build-plugin.ts

# テスト（式エンジンのゴールデンテスト、評価〜PDFの結合テスト）
bun run test                # = cargo test -p formdoc-expr -p formdoc-core
bun run check               # UI の型チェック（svelte-check）

# 配布用
bun run build:web           # Web版の静的ファイルを ui/dist-web に出力（エンジンは配布用でビルドし直す）
bun run tauri build         # デスクトップ版のインストーラ作成

# CLI
cargo run -p formdoc-cli -- gui examples/keisansho-gui/document.json out.pdf --typst out.typ
cargo run -p formdoc-cli -- compile examples/keisansho-code out.pdf
```

## コードモードの書き方（例）

```typst
#import "@local/formdoc:0.1.0": *
#import "style.typ": style          // 文書テンプレートのファイルを同じフォルダに置く
#show: style.with(title: "主桁の設計計算書", chapter-start: 4)

= 主桁の設計
== 作用
=== 衝撃荷重 #h(1fr) #kijun("鋼標準", "第Ⅰ編 4.4.4")

#let Lb = vdef("L_b", 8.000, unit: "m", digits: 3, desc: "影響線の基線の長さ")
#let ne = vcalc("n_e", "70 * L_b^(-0.8)", Lb, unit: "Hz", digits: 3, desc: "基本固有振動数")
#calc-line(ne)
#check-line("n_e <= 20", ne)
#where-list(ne, Lb)
```

「コードで作成」には、編集中の文書がこの形（部品ごとの `// @block ID` の目印つき）で出る。formDoc の外で組版するときは「ファイルに保存…」で main.typ と style.typ を書き出す。

## 能力フラグ（Web / デスクトップ）

| 機能 | Web | デスクトップ |
|---|:-:|:-:|
| 部品で作成・プレビュー・PDF出力・Typst変換 | ○ | ○ |
| コードで作成（内蔵エディタ） | ○ | ○ |
| フォルダを開く・VSCodeで開く・保存監視 | — | ○ |
| 名前を付けて保存（保存先の選択） | —（ダウンロード） | ○ |
| ブラウザ内への下書き自動保存 | ○ | — |
| 最近使ったファイル | — | ○ |
| システムフォルダ（設定・文書テンプレート・部品テンプレート） | ブラウザ内 | `%APPDATA%ormDoc` |
| 部品テンプレートの公開（フォルダへ保存） | —（ダウンロード） | ○ |
| .fdoc のダブルクリックで起動 | — | ○（インストーラー版） |
| 共有フォルダの社内ライブラリ参照 | 準備中 | 準備中 |

`ui/src/lib/platform/capabilities.ts` に追加し、UI では `<Gate cap="…">` で包む。

## 既知の課題（試作版）

詳細な残作業（設計・手順・完了条件）は [TODO.md](TODO.md) を参照。

- Web版の wasm が大きい（約60MB。うち同梱フォント約30MB）。フォントの別配信・サブセット化が必要
- 部品の計算ロジックを文書テンプレート側で追加する仕組み（Rhai）は未実装
- 要領書・作業計画書の型は未作成（部品の追加で対応予定：脚注・箇条書き・組織図など）
- 単位の次元チェック（kN と kN·m の取り違え検出）は未実装
