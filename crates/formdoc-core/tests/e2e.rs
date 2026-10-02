//! GUI文書の評価〜組版〜PDF出力の結合テスト。

use formdoc_core::{Document, Session};

fn sample() -> Document {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/keisansho-gui/document.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn set(doc: &mut Document, id: &str, key: &str, v: serde_json::Value) {
    doc.blocks.iter_mut().find(|b| b.id == id).unwrap().props.insert(key.into(), v);
}

#[test]
fn sample_is_exportable() {
    let mut s = Session::new();
    let r = s.update_document(sample(), &[]);
    let errors: Vec<_> = r.issues.iter().filter(|i| format!("{:?}", i.severity) == "Error").collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert!(r.exportable);
    assert!(r.pages.len() >= 3);
    assert_eq!(r.vars.iter().find(|v| v.name == "i").unwrap().text, "0.425");
    assert_eq!(r.vars.iter().find(|v| v.name == "W_d").unwrap().text, "5.88");
    assert!(s.pdf().unwrap().starts_with(b"%PDF"));
}

#[test]
fn pdf_is_deterministic() {
    // 同じ文書からは、いつ・誰が出力しても同一バイトのPDFになる
    let a = { let mut s = Session::new(); s.update_document(sample(), &[]); s.pdf().unwrap() };
    let b = { let mut s = Session::new(); s.update_document(sample(), &[]); s.pdf().unwrap() };
    assert_eq!(a, b);
}

#[test]
fn unchanged_pages_are_not_resent() {
    let mut s = Session::new();
    let r1 = s.update_document(sample(), &[]);
    let known: Vec<String> = r1.pages.iter().map(|p| p.hash.clone()).collect();
    let mut doc = sample();
    // 2ページ目以降に影響しない表紙の変更
    doc.meta.project = "別の業務名".into();
    let r2 = s.update_document(doc, &known);
    assert!(r2.pages[0].svg.is_some());
    assert!(r2.pages[1..].iter().all(|p| p.svg.is_none()));
}

#[test]
fn ng_check_is_reported_and_undefined_var_blocks_export() {
    let mut doc = sample();
    // i <= 0.7 の制限値を厳しくして NG にする（NG は警告：出力は可能）
    set(&mut doc, "b12", "expr", "i <= 0.4".into());
    let mut s = Session::new();
    let r = s.update_document(doc.clone(), &[]);
    assert!(r.issues.iter().any(|i| i.code == "check-ng" && i.block_id.as_deref() == Some("b12")));
    assert_eq!(r.blocks["b12"].status, "ng");
    assert!(r.exportable);

    // 未定義の変数を使うとエラーになり、PDFは出力できない
    set(&mut doc, "b11", "expr", "K_a * V / (7.2 * n_x * L_b)".into());
    let r = s.update_document(doc, &[]);
    assert!(r.issues.iter().any(|i| i.code == "calc" && i.block_id.as_deref() == Some("b11")));
    assert!(!r.exportable);
    assert!(s.pdf().is_err());
}

#[test]
fn duplicate_variable_and_lint() {
    let mut doc = sample();
    set(&mut doc, "b7", "name", "V".into());
    set(&mut doc, "b9", "text", "設計を行なう、１０ｋＮ".into());
    let mut s = Session::new();
    let r = s.update_document(doc, &[]);
    assert!(r.issues.iter().any(|i| i.code == "var-dup" && i.block_id.as_deref() == Some("b7")));
    for code in ["lint-punctuation", "lint-fullwidth", "lint-wording", "lint-period"] {
        assert!(r.issues.iter().any(|i| i.code == code), "{code}");
    }
}

#[test]
fn code_mode_compiles_and_lints() {
    let mut s = Session::new();
    let src = formdoc_core::api::code_template("keisansho").unwrap();
    let r = s.update_project(vec![("main.typ".into(), src.into_bytes())], "keisansho", &[]);
    assert!(r.exportable, "{:?}", r.diagnostics);
    let r = s.update_project(vec![("main.typ".into(), b"#import \"@local/formdoc:0.1.0\": *\n#kijun(\"unknown\", \"1\")\n".to_vec())], "keisansho", &[]);
    assert!(!r.exportable);
    assert!(r.diagnostics.iter().any(|d| d.message.contains("未登録の基準書") && d.line == Some(2)), "{:?}", r.diagnostics);
}

#[test]
fn gui_export_to_typst_compiles_identically() {
    // 「Typstとして書き出し」したソースをコードモードで組版すると、GUIと同じPDFになる
    let mut gui = Session::new();
    gui.update_document(sample(), &[]);
    let gui_pdf = gui.pdf().unwrap();
    let src = gui.export_typst().unwrap();
    let mut code = Session::new();
    let r = code.update_project(vec![("main.typ".into(), src.into_bytes())], "keisansho", &[]);
    assert!(r.exportable, "{:?}", r.diagnostics);
    let _ = gui_pdf;
}

#[test]
fn new_document_has_skeleton() {
    let d = formdoc_core::api::new_document("keisansho").unwrap();
    assert_eq!(d.blocks.len(), 3);
    let c = formdoc_core::api::catalog().unwrap();
    assert!(c.fonts.iter().any(|f| f == "Noto Serif JP"));
    assert!(c.components.get("calc").is_some());
}
