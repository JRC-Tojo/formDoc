// library/ 配下が変わったら再埋め込みする（include_dir は追加ファイルを検知しないため）。
fn main() {
    println!("cargo:rerun-if-changed=../../library");

    // フォント・Typstパッケージ・式エンジンのプラグインは Git に含めず scripts/setup.mjs で用意する。
    // 欠けたまま埋め込むと実行時に文字化けや import エラーになるため、ビルドを止める。
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library");
    for need in ["fonts/NotoSerifJP-Regular.otf", "vendor/preview", "typst/formdoc/0.1.0/formdoc_expr.wasm"] {
        if !lib.join(need).exists() {
            panic!("library/{need} がありません。先に `node scripts/setup.mjs` を実行してください。");
        }
    }
}
