//! 章構成の拘束（TODO 3 / Issue #7）の結合テスト。
//! 文書テンプレートの章定義に対し、章を消す・入れ替える・見出し文を変える・構造形式を変える、などの操作で
//! 何が検出されるかを、検出の種類（code）と重さで確かめる。

use formdoc_core::api::UpdateResult;
use formdoc_core::template::{builtin_style, parse_style};
use formdoc_core::{Document, Session};
use serde_json::json;

fn session_with(style: &str) -> Session {
    let mut s = Session::new();
    s.set_style(style).unwrap();
    s
}

fn keisansho() -> Session {
    session_with(&builtin_style("keisansho").unwrap())
}

fn sample() -> Document {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/keisansho-gui/document.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// 章構成の検出だけを (code, 重さ) で取り出す。
fn chapter_issues(r: &UpdateResult) -> Vec<(String, String)> {
    r.issues
        .iter()
        .filter(|i| i.code.starts_with("chapter-"))
        .map(|i| (i.code.clone(), format!("{:?}", i.severity).to_lowercase()))
        .collect()
}

fn has(r: &UpdateResult, code: &str, severity: &str) -> bool {
    chapter_issues(r).iter().any(|(c, s)| c == code && s == severity)
}

/// 見出し文が `text` の見出しから、次の同じか上の階層の見出しの手前までを取り除く。
fn remove_section(doc: &mut Document, text: &str) {
    let i = doc.blocks.iter().position(|b| b.kind == "heading" && b.str("text") == text).unwrap();
    let level = doc.blocks[i].int("level").unwrap();
    let end = (i + 1..doc.blocks.len())
        .find(|&j| doc.blocks[j].kind == "heading" && doc.blocks[j].int("level").unwrap() <= level)
        .unwrap_or(doc.blocks.len());
    doc.blocks.drain(i..end);
}

fn heading_texts(doc: &Document, level: i64) -> Vec<String> {
    doc.blocks.iter().filter(|b| b.kind == "heading" && b.int("level") == Some(level)).map(|b| b.str("text").to_string()).collect()
}

#[test]
fn sample_follows_the_chapter_rules() {
    let r = keisansho().update_document(sample(), &[]);
    assert!(chapter_issues(&r).is_empty(), "{:?}", chapter_issues(&r));
}

#[test]
fn missing_required_chapter_is_an_error() {
    let mut doc = sample();
    remove_section(&mut doc, "設計概要");
    let r = keisansho().update_document(doc, &[]);
    assert!(has(&r, "chapter-missing", "error"), "{:?}", chapter_issues(&r));
    assert!(!r.exportable);
}

#[test]
fn chapters_in_wrong_order_are_detected() {
    let mut doc = sample();
    // §2 設計条件（と節）を §1 設計概要の前へ移す
    let start = doc.blocks.iter().position(|b| b.str("text") == "設計条件").unwrap();
    let end = doc.blocks.iter().position(|b| b.str("text") == "設計結果").unwrap();
    let moved: Vec<_> = doc.blocks.drain(start..end).collect();
    doc.blocks.splice(0..0, moved);
    let r = keisansho().update_document(doc, &[]);
    assert!(has(&r, "chapter-order", "error"), "{:?}", chapter_issues(&r));
}

#[test]
fn changed_fixed_title_is_a_warning_with_a_fix() {
    let mut doc = sample();
    let h = doc.blocks.iter_mut().find(|b| b.str("text") == "設計概要").unwrap();
    h.props.insert("text".into(), "概要".into());
    let r = keisansho().update_document(doc, &[]);
    let issue = r.issues.iter().find(|i| i.code == "chapter-title").expect("見出し文の変更を検出する");
    assert_eq!(format!("{:?}", issue.severity), "Warning");
    assert_eq!(issue.fix.as_ref().map(|f| f.to.as_str()), Some("設計概要"));
    assert!(r.exportable, "警告だけなら出力できる");
}

#[test]
fn repeatable_chapter_may_appear_many_times_but_others_may_not() {
    let mut doc = sample();
    // 部材ごとの設計の章（見出し文は自由）を足す
    doc.blocks.push(serde_json::from_value(json!({"id": "x1", "kind": "heading", "props": {"level": 1, "text": "横桁の設計", "chapter": "buzai"}})).unwrap());
    // 設計概要をもう1つ足す
    doc.blocks.push(serde_json::from_value(json!({"id": "x2", "kind": "heading", "props": {"level": 1, "text": "設計概要", "chapter": "gaiyou"}})).unwrap());
    let r = keisansho().update_document(doc, &[]);
    let issues = chapter_issues(&r);
    assert!(r.issues.iter().any(|i| i.code == "chapter-duplicate" && i.block_id.as_deref() == Some("x2")), "{issues:?}");
    assert!(!r.issues.iter().any(|i| i.code == "chapter-duplicate" && i.block_id.as_deref() == Some("x1")), "{issues:?}");
}

#[test]
fn heading_outside_the_definition_is_extra() {
    let mut doc = sample();
    doc.blocks.push(serde_json::from_value(json!({"id": "x1", "kind": "heading", "props": {"level": 1, "text": "地盤条件"}})).unwrap());
    let r = keisansho().update_document(doc, &[]);
    assert!(r.issues.iter().any(|i| i.code == "chapter-extra" && i.block_id.as_deref() == Some("x1")), "{:?}", chapter_issues(&r));
}

#[test]
fn variant_decides_which_chapters_are_needed() {
    let s = keisansho();
    // 新規作成：構造形式の既定値（上路桁）の章
    let doc = s.new_document().unwrap();
    let sections = heading_texts(&doc, 2);
    assert!(sections.contains(&"検討箇所".to_string()) && !sections.contains(&"グルーピング".to_string()), "{sections:?}");
    assert!(chapter_issues(&keisansho().update_document(doc.clone(), &[])).is_empty());

    // 工事桁に変える：検討箇所は不要、グルーピングが必要
    let mut doc = doc;
    doc.meta.set("variant", "工事桁");
    let r = keisansho().update_document(doc.clone(), &[]);
    assert!(has(&r, "chapter-missing", "error") && has(&r, "chapter-extra", "error"), "{:?}", chapter_issues(&r));

    // 足りない章を追加すると、決められた順序の位置に入る（不要な章は残るので、利用者が消す）
    let mut done = s.complete_chapters(&doc).unwrap();
    let sections = heading_texts(&done, 2);
    let pos = |t: &str| sections.iter().position(|x| x == t).unwrap();
    assert!(pos("共通仕様") < pos("グルーピング") && pos("グルーピング") < pos("施工計画"), "{sections:?}");
    remove_section(&mut done, "検討箇所");
    let r = keisansho().update_document(done, &[]);
    assert!(chapter_issues(&r).is_empty(), "{:?}", chapter_issues(&r));
}

/// 拘束の強さと検出の重さを確かめるための小さな文書テンプレート。
fn custom_style(structure: &str, chapters: &str) -> String {
    format!(
        r#"#let info = (
  id: "t", name: "試験", blocks: ("heading", "paragraph", "pagebreak"),
  structure: {structure},
  chapters: {chapters},
)
#let style(..args, doc) = doc
"#
    )
}

fn doc_of(blocks: serde_json::Value) -> Document {
    serde_json::from_value(json!({"template": "t", "blocks": blocks})).unwrap()
}

#[test]
fn locked_chapter_fixes_its_content() {
    let style = custom_style(
        r#"(level: "locked")"#,
        r#"((id: "a", title: "承認事項", content: ((kind: "paragraph", text: "以下のとおり承認を求める。"),)),)"#,
    );
    let s = session_with(&style);
    // 新規作成は決められた中身で始まり、検出はない
    let doc = s.new_document().unwrap();
    assert_eq!(doc.blocks.iter().map(|b| b.kind.as_str()).collect::<Vec<_>>(), ["heading", "paragraph"]);
    assert!(chapter_issues(&session_with(&style).update_document(doc.clone(), &[])).is_empty());
    // 部品を足すと検出する
    let mut added = doc;
    added.blocks.push(serde_json::from_value(json!({"id": "z", "kind": "paragraph", "props": {"text": "追記"}})).unwrap());
    let r = session_with(&style).update_document(added, &[]);
    assert!(has(&r, "chapter-locked", "error"), "{:?}", chapter_issues(&r));
}

#[test]
fn basic_level_does_not_check_chapters() {
    let style = custom_style(r#"(level: "basic")"#, r#"((id: "a", title: "第1章"),)"#);
    let r = session_with(&style).update_document(doc_of(json!([{"id": "h", "kind": "heading", "props": {"level": 1, "text": "自由な章"}}])), &[]);
    assert!(chapter_issues(&r).is_empty(), "{:?}", chapter_issues(&r));
}

#[test]
fn rule_levels_can_be_changed_per_template_and_per_chapter() {
    // 文書全体では「必須の章がない」を警告に、章 b だけは検出しない
    let style = custom_style(
        r#"(rules: (chapter-missing: "warning"))"#,
        r#"((id: "a", title: "第1章"), (id: "b", title: "第2章", rules: (chapter-missing: "off")))"#,
    );
    let r = session_with(&style).update_document(doc_of(json!([])), &[]);
    let issues = chapter_issues(&r);
    assert_eq!(issues, [("chapter-missing".to_string(), "warning".to_string())], "{issues:?}");
}

#[test]
fn chapter_may_limit_the_blocks() {
    let style = custom_style("(:)", r#"((id: "a", title: "第1章", allowed-blocks: ("heading", "paragraph")),)"#);
    let doc = doc_of(json!([
        {"id": "h", "kind": "heading", "props": {"level": 1, "text": "第1章", "chapter": "a"}},
        {"id": "p", "kind": "pagebreak", "props": {}},
    ]));
    let r = session_with(&style).update_document(doc, &[]);
    assert!(r.issues.iter().any(|i| i.code == "chapter-block" && i.block_id.as_deref() == Some("p")), "{:?}", chapter_issues(&r));
}

#[test]
fn mistakes_in_chapter_definitions_are_reported() {
    // 章の id の重複、未知の検出名、文書テンプレートで使えない部品、選択肢にない構造形式
    for chapters in [
        r#"((id: "a", title: "1"), (id: "a", title: "2"))"#,
        r#"((id: "a", title: "1", rules: (no-such-rule: "error")),)"#,
        r#"((id: "a", title: "1", allowed-blocks: ("calc",)),)"#,
        r#"((id: "a", title: "1", variants: ("トラス",)),)"#,
    ] {
        assert!(parse_style(&custom_style("(:)", chapters)).is_err(), "{chapters}");
    }
}
