//! 社内標準ライブラリ一式の埋め込み。
//!
//! Web版（wasm）とデスクトップ版で同一のフォント・Typstパッケージを使い、
//! 環境によって出力が変わらないようにするため、ファイルシステムではなくバイナリに含める。

use include_dir::{Dir, File, include_dir};

static LIBRARY: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../library");

fn walk<'a>(d: &'a Dir<'a>, out: &mut Vec<&'a File<'a>>) {
    out.extend(d.files());
    for sub in d.dirs() {
        walk(sub, out);
    }
}

/// ライブラリ内の全ファイル（パスは library/ からの相対、区切りは '/'）。
pub fn files() -> impl Iterator<Item = (String, &'static [u8])> {
    let mut v = Vec::new();
    walk(&LIBRARY, &mut v);
    v.into_iter().map(|f| (f.path().to_string_lossy().replace('\\', "/"), f.contents()))
}

/// 1ファイル取得。
pub fn get(path: &str) -> Option<&'static [u8]> {
    LIBRARY.get_file(path).map(|f| f.contents())
}

/// 同梱フォント（library/fonts/*.otf|ttf）。
pub fn fonts() -> impl Iterator<Item = &'static [u8]> {
    LIBRARY
        .get_dir("fonts")
        .into_iter()
        .flat_map(|d| d.files())
        .filter(|f| matches!(f.path().extension().and_then(|e| e.to_str()), Some("otf" | "ttf" | "ttc")))
        .map(|f| f.contents())
}

/// ライブラリの版（library/typst/formdoc の最新版ディレクトリ名）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
