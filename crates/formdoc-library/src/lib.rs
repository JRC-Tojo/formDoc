//! 社内標準ライブラリ一式の埋め込み。
//!
//! Web版（wasm）とデスクトップ版で同一のフォント・Typstパッケージを使い、
//! 環境によって出力が変わらないようにするため、ファイルシステムではなくバイナリに含める。
//! ただしフォントは大きいため、Web版（`embed-fonts` 無効）では一覧だけを持ち、中身は実行時に受け取る。

/// 同梱フォント1つ。
pub struct FontFile {
    /// ファイル名（library/fonts/ からの相対）
    pub file: &'static str,
    /// 中身の SHA-256（16進小文字）。実行時に渡されたフォントが同じ版かの照合に使う
    pub sha256: &'static str,
    /// バイト数
    pub size: usize,
    /// 中身（`embed-fonts` 無効時は None）
    pub data: Option<&'static [u8]>,
}

include!(concat!(env!("OUT_DIR"), "/embedded.rs"));

/// フォント以外の全ファイル（パスは library/ からの相対、区切りは '/'）。
pub fn files() -> impl Iterator<Item = (String, &'static [u8])> {
    FILES.iter().map(|(p, b)| (p.to_string(), *b))
}

/// 1ファイル取得（フォント以外）。
pub fn get(path: &str) -> Option<&'static [u8]> {
    FILES.binary_search_by(|(p, _)| p.cmp(&path)).ok().map(|i| FILES[i].1)
}

/// 同梱フォントの一覧（ファイル名順）。中身を埋め込んでいなくても返す。
pub fn font_files() -> &'static [FontFile] {
    FONTS
}

/// 埋め込み済みのフォントの中身（ファイル名順）。`embed-fonts` 無効時は空。
pub fn fonts() -> impl Iterator<Item = &'static [u8]> {
    FONTS.iter().filter_map(|f| f.data)
}

/// ライブラリの版（library/typst/formdoc の最新版ディレクトリ名）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
