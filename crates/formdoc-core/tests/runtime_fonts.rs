//! Web版と同じ「フォントを埋め込まず、実行時に渡す」構成の結合テスト。
//! フォントの一覧はプロセスで1つなので、別のテストファイル（別プロセス）にしている。
//! 実行：cargo test -p formdoc-core --no-default-features --test runtime_fonts（bun run test に含まれる）
#![cfg(not(feature = "embed-fonts"))]

use formdoc_core::{Document, Session, api, template::builtin_style, world};

fn bundled_fonts() -> Vec<Vec<u8>> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../library/fonts");
    api::font_files().iter().map(|f| std::fs::read(format!("{dir}/{}", f.file)).unwrap()).collect()
}

#[test]
fn fonts_passed_at_runtime_typeset_the_sample() {
    // 数が合わないフォントは受け付けない（番号がずれるとPDFが変わるため）
    assert!(world::install_fonts(Vec::new()).is_err());
    world::install_fonts(bundled_fonts()).unwrap();
    let families = api::catalog().unwrap().fonts;
    for need in ["Noto Serif JP", "New Computer Modern Math"] {
        assert!(families.iter().any(|f| f == need), "{need} が無い: {families:?}");
    }
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/keisansho-gui/document.json")).unwrap();
    let doc: Document = serde_json::from_str(&text).unwrap();
    let mut s = Session::new();
    s.set_style(&builtin_style("keisansho").unwrap()).unwrap();
    let r = s.update_document(doc, &[]);
    assert!(r.exportable);
    assert!(s.pdf().unwrap().starts_with(b"%PDF"));
}
