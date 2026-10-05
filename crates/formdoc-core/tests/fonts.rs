//! 同梱フォントの一覧と、実行時にフォントを渡す仕組み（Web版）の結合テスト。

use formdoc_core::api;
use sha2::{Digest, Sha256};

#[test]
fn font_list_matches_bundled_files() {
    // 一覧の SHA-256 は library/fonts の中身と一致する（Web版はこれで取得したフォントを照合する）
    let list = api::font_files();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../library/fonts");
    for f in &list {
        let body = std::fs::read(format!("{dir}/{}", f.file)).unwrap();
        assert_eq!(f.size, body.len(), "{}", f.file);
        assert_eq!(f.sha256, format!("{:x}", Sha256::digest(&body)), "{}", f.file);
    }
    // 本文と数式の書体がそろっている
    let families = api::catalog().unwrap().fonts;
    for need in ["Noto Serif JP", "Noto Sans JP", "New Computer Modern Math"] {
        assert!(families.iter().any(|f| f == need), "{need} が無い: {families:?}");
    }
}

#[test]
fn fonts_cannot_be_replaced_after_use() {
    // 一度組版に使ったフォントの一覧は差し替えられない（同じ文書が同じPDFになることを守る）
    api::catalog().unwrap();
    assert!(formdoc_core::world::install_fonts(Vec::new()).is_err());
}
