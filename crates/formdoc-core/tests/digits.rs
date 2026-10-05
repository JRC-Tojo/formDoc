//! 数値の3桁区切り（文書テンプレートの lint.digit-grouping）の結合テスト。
//! 有効にすると、計算結果として出る数字も本文・表の数字も 4桁から区切る。無効なら計算結果は 5桁以上だけ区切り、本文は検査しない。

use formdoc_core::api::UpdateResult;
use formdoc_core::{Document, FormdocWorld, Session, compile};
use serde_json::json;

fn style(grouping: bool) -> String {
    format!(
        r#"#let info = (
  id: "t", name: "試験", blocks: ("heading", "paragraph", "vdef", "calc", "check", "table"),
  lint: (digit-grouping: {grouping}),
)
#let style(..args, doc) = doc
"#
    )
}

fn run(grouping: bool) -> (Session, UpdateResult) {
    let mut s = Session::new();
    s.set_style(&style(grouping)).unwrap();
    let doc: Document = serde_json::from_value(json!({"template": "t", "blocks": [
        {"id": "v", "kind": "vdef", "props": {"name": "A_n", "value": 9428, "unit": "mm2", "digits": 0}},
        {"id": "c", "kind": "calc", "props": {"name": "N", "expr": "A_n * 100", "unit": "N", "digits": 0}},
        {"id": "k", "kind": "check", "props": {"expr": "A_n <= 9500", "digits": 0}},
        {"id": "p", "kind": "paragraph", "props": {"text": "支間長 8000 mm，断面積 {{A_n}} とする．"}},
    ]}))
    .unwrap();
    let r = s.update_document(doc, &[]);
    (s, r)
}

fn text(r: &UpdateResult, name: &str) -> String {
    r.vars.iter().find(|v| v.name == name).unwrap().text.clone()
}

/// コードモードのコードを組版し、PDF の文字に `want` が含まれるか
fn pdf_text_contains(s: &Session, style_src: &str, want: &str) -> bool {
    let mut w = FormdocWorld::new();
    w.set_files([("/main.typ".to_string(), s.code().unwrap().into_bytes()), ("/style.typ".to_string(), style_src.as_bytes().to_vec())]);
    let c = compile(&w);
    let doc = c.document.expect("コードモードのコードが組版できない");
    // ページの文字を集める（PDF ではなく組版結果から取り出す）
    let mut all = String::new();
    for p in doc.pages() {
        collect(&p.frame, &mut all);
    }
    all.replace(['\u{200b}', ' '], "").contains(want)
}

fn collect(frame: &typst::layout::Frame, out: &mut String) {
    use typst::layout::FrameItem;
    for (_, item) in frame.items() {
        match item {
            FrameItem::Text(t) => out.push_str(t.text.as_str()),
            FrameItem::Group(g) => collect(&g.frame, out),
            _ => {}
        }
    }
}

#[test]
fn grouping_on_groups_from_four_digits() {
    let (s, r) = run(true);
    assert_eq!(text(&r, "A_n"), "9,428");
    assert_eq!(text(&r, "N"), "942,800");
    // 本文の区切りの無い数字を検出し，直し方を示す
    let fix = r.issues.iter().find(|i| i.code == "lint-digit-grouping").and_then(|i| i.fix.clone()).expect("8000 を検出する");
    assert_eq!((fix.from.as_str(), fix.to.as_str()), ("8000", "8,000"));
    // コードモード（Typst）で組版しても同じ表記になる（照査に書いた制限値 9500 も区切る）
    assert!(pdf_text_contains(&s, &style(true), "9,428"));
    assert!(pdf_text_contains(&s, &style(true), "9,500"));
}

#[test]
fn grouping_off_keeps_four_digit_numbers() {
    let (s, r) = run(false);
    assert_eq!(text(&r, "A_n"), "9428");
    assert_eq!(text(&r, "N"), "942,800");
    assert!(r.issues.iter().all(|i| i.code != "lint-digit-grouping"));
    assert!(pdf_text_contains(&s, &style(false), "9428"));
    assert!(pdf_text_contains(&s, &style(false), "9500"));
}
