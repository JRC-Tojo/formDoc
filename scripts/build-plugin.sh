#!/usr/bin/env bash
# 式エンジンのTypstプラグインをビルドし、社内標準パッケージに配置する。
# formdoc-expr を変更したら必ず実行する（GUIとコードモードの計算を一致させるため）。
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p formdoc-typst-plugin --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/formdoc_typst_plugin.wasm library/typst/formdoc/0.1.0/formdoc_expr.wasm
echo "plugin -> library/typst/formdoc/0.1.0/formdoc_expr.wasm"
