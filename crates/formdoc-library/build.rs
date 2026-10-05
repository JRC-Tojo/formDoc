//! library/ を埋め込むための表（`$OUT_DIR/embedded.rs`）を生成する。
//!
//! - フォント以外のファイル：`FILES`（パス順。`get` は二分探索）
//! - フォント（library/fonts/*.otf|ttf|ttc）：`FONTS`。ファイル名・SHA-256・大きさは常に、
//!   中身は `embed-fonts` 機能が有効なときだけ埋め込む。
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn is_font(rel: &str) -> bool {
    rel.starts_with("fonts/") && [".otf", ".ttf", ".ttc"].iter().any(|e| rel.ends_with(e))
}

fn main() {
    // library/ 配下の追加・変更・削除で作り直す（ディレクトリを指定すると Cargo が中身を走査する）
    println!("cargo:rerun-if-changed=../../library");

    // フォント・Typstパッケージ・式エンジンのプラグインは Git に含めず bun ready（scripts/vendor.ts・scripts/build-plugin.ts）で用意する。
    // 欠けたまま埋め込むと実行時に文字化けや import エラーになるため、ビルドを止める。
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library").canonicalize().unwrap();
    for need in ["fonts/NotoSerifJP-Regular.otf", "fonts/NewCMMath-Regular.otf", "vendor/preview", "typst/formdoc/0.1.0/formdoc_expr.wasm"] {
        if !lib.join(need).exists() {
            panic!("library/{need} がありません。先に `bun ready` を実行してください。");
        }
    }
    let embed_fonts = std::env::var_os("CARGO_FEATURE_EMBED_FONTS").is_some();

    let mut paths = Vec::new();
    walk(&lib, &mut paths);
    let mut files = String::from("static FILES: &[(&str, &[u8])] = &[\n");
    let mut fonts = String::from("static FONTS: &[FontFile] = &[\n");
    let mut rels: Vec<(String, PathBuf)> = paths
        .into_iter()
        .map(|p| (p.strip_prefix(&lib).unwrap().to_string_lossy().replace('\\', "/"), p))
        .collect();
    rels.sort_by(|a, b| a.0.cmp(&b.0));
    for (rel, abs) in rels {
        if is_font(&rel) {
            let bytes = std::fs::read(&abs).unwrap();
            let sha = format!("{:x}", Sha256::digest(&bytes));
            let data = if embed_fonts { format!("Some(include_bytes!({abs:?}))") } else { "None".into() };
            let name = &rel["fonts/".len()..];
            writeln!(fonts, "    FontFile {{ file: {name:?}, sha256: {sha:?}, size: {}, data: {data} }},", bytes.len()).unwrap();
        } else {
            writeln!(files, "    ({rel:?}, include_bytes!({abs:?})),").unwrap();
        }
    }
    files.push_str("];\n");
    fonts.push_str("];\n");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("embedded.rs");
    std::fs::write(out, files + &fonts).unwrap();
}
