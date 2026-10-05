//! 章構成の拘束（Issue #7）の結合テスト。
//! 章を消す・入れ替える・見出し文を変える・構造形式を変える、などの操作で何が検出されるかを、
//! 検出の種類（code）と重さ、GUI に渡す操作の可否で確かめる。
//! 判定の確認は小さな試験用の文書テンプレートで行い、同梱の計算書は「例の文書と骨組みに検出が無い」ことだけを確かめる
//! （計算書の章の名前や数を見直してもテストが壊れないように）。

use formdoc_core::api::UpdateResult;
use formdoc_core::template::{builtin_style, parse_style};
use formdoc_core::{Document, Session};
use serde_json::{Value, json};

fn session_with(style: &str) -> Session {
    let mut s = Session::new();
    s.set_style(style).unwrap();
    s
}

/// 章構成の検出だけを (code, 重さ, 部品ID) で取り出す。
fn chapter_issues(r: &UpdateResult) -> Vec<(String, String, String)> {
    r.issues
        .iter()
        .filter(|i| i.code.starts_with("chapter-"))
        .map(|i| (i.code.clone(), format!("{:?}", i.severity).to_lowercase(), i.block_id.clone().unwrap_or_default()))
        .collect()
}

fn has(r: &UpdateResult, code: &str, severity: &str, block: &str) -> bool {
    chapter_issues(r).iter().any(|(c, s, b)| c == code && s == severity && b == block)
}

/// 試験用の文書テンプレート。構造形式の欄（variant：甲・乙）を持つ。
fn style(structure: &str, chapters: &str) -> String {
    format!(
        r#"#let info = (
  id: "t", name: "試験", blocks: ("heading", "paragraph", "pagebreak"),
  fields: ((key: "variant", label: "形式", type: "select", options: ("甲", "乙"), default: "甲"),),
  structure: {structure},
  chapters: {chapters},
)
#let style(..args, doc) = doc
"#
    )
}

fn h(id: &str, level: i64, text: &str, chapter: Option<&str>) -> Value {
    let mut props = json!({"level": level, "text": text});
    if let Some(c) = chapter {
        props["chapter"] = c.into();
    }
    json!({"id": id, "kind": "heading", "props": props})
}

fn p(id: &str) -> Value {
    json!({"id": id, "kind": "paragraph", "props": {"text": "本文．"}})
}

fn doc(variant: &str, blocks: Value) -> Document {
    serde_json::from_value(json!({"template": "t", "meta": {"variant": variant}, "blocks": blocks})).unwrap()
}

fn run(style: &str, d: Document) -> UpdateResult {
    session_with(style).update_document(d, &[])
}

/// 3章（第1章・第2章・第3章）の文書テンプレート
fn three() -> String {
    style("(:)", r#"((id: "a", title: "第1章"), (id: "b", title: "第2章"), (id: "c", title: "第3章"))"#)
}

#[test]
fn keisansho_sample_and_skeletons_have_no_chapter_issues() {
    let src = builtin_style("keisansho").unwrap();
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/keisansho-gui/document.json")).unwrap();
    let r = run(&src, serde_json::from_str(&text).unwrap());
    assert!(chapter_issues(&r).is_empty(), "{:?}", chapter_issues(&r));
    // どの構造形式でも、新規作成の骨組みは決められた章のとおり（見出し文の書き換えを促す検出だけが出る）
    let s = session_with(&src);
    let t = parse_style(&src).unwrap();
    let field = t.fields.iter().find(|f| f.key == t.structure.variant_field).unwrap();
    for v in &field.options {
        // その構造形式で、章の無い状態から必須の章をそろえる
        let mut d = s.new_document().unwrap();
        d.blocks.clear();
        d.meta.set(&t.structure.variant_field, v.as_str());
        let d = s.complete_chapters(&d).unwrap();
        let r = run(&src, d);
        let left: Vec<_> = chapter_issues(&r).into_iter().filter(|(c, _, _)| c != "chapter-title").collect();
        assert!(left.is_empty(), "{v}: {left:?}");
    }
}

#[test]
fn missing_required_chapter_is_an_error() {
    let r = run(&three(), doc("甲", json!([h("1", 1, "第1章", Some("a")), h("3", 1, "第3章", Some("c"))])));
    assert!(has(&r, "chapter-missing", "error", ""), "{:?}", chapter_issues(&r));
    assert!(!r.exportable);
}

#[test]
fn only_the_misplaced_chapter_is_reported_for_order() {
    // 第3章を先頭に置く：第1章・第2章の並びは正しいので、指すのは第3章だけ
    let r = run(&three(), doc("甲", json!([h("3", 1, "第3章", Some("c")), h("1", 1, "第1章", Some("a")), h("2", 1, "第2章", Some("b"))])));
    let order: Vec<_> = chapter_issues(&r).into_iter().filter(|(c, _, _)| c == "chapter-order").collect();
    assert_eq!(order, [("chapter-order".to_string(), "error".to_string(), "3".to_string())]);
}

#[test]
fn titles_follow_the_rule_level() {
    let s = style(
        r#"(rules: (chapter-title: "warning"))"#,
        r#"((id: "a", title: "第1章"), (id: "m", title: "（部材名）の設計", fixed-title: false, repeatable: true))"#,
    );
    let r = run(&s, doc("甲", json!([h("1", 1, "第一章", Some("a")), h("2", 1, "（部材名）の設計", Some("m"))])));
    // 決められた見出し文と違う：警告で、決められた見出し文に直す修正が付く
    let issue = r.issues.iter().find(|i| i.code == "chapter-title" && i.block_id.as_deref() == Some("1")).unwrap();
    assert_eq!(issue.fix.as_ref().map(|f| f.to.as_str()), Some("第1章"));
    // 見出し文が自由な章が既定の見出し文のまま：書き換えを促す
    assert!(has(&r, "chapter-title", "warning", "2"), "{:?}", chapter_issues(&r));
    assert!(r.exportable, "警告だけなら出力できる");
    // 警告の設定なら、GUI は見出し文の変更を止めない
    assert!(!r.blocks["1"].chapter.as_ref().unwrap().fixed_title);
}

#[test]
fn repeatable_chapter_may_appear_many_times_but_others_may_not() {
    let s = style("(:)", r#"((id: "a", title: "概要"), (id: "m", title: "（部材名）の設計", fixed-title: false, repeatable: true))"#);
    let r = run(&s, doc("甲", json!([
        h("1", 1, "概要", Some("a")), h("2", 1, "概要", Some("a")),
        h("3", 1, "主桁の設計", Some("m")), h("4", 1, "横桁の設計", Some("m")),
    ])));
    let dup: Vec<_> = chapter_issues(&r).into_iter().filter(|(c, _, _)| c == "chapter-duplicate").map(|(_, _, b)| b).collect();
    assert_eq!(dup, ["2"]);
}

#[test]
fn headings_outside_the_definition_are_extra() {
    let s = style("(:)", r#"((id: "a", title: "第1章", sections: ((id: "a1", title: "条件"),)),)"#);
    let r = run(&s, doc("甲", json!([
        h("1", 1, "第1章", Some("a")), h("2", 2, "地盤条件", None),
        h("3", 1, "付録", None),
    ])));
    assert!(has(&r, "chapter-extra", "error", "2") && has(&r, "chapter-extra", "error", "3"), "{:?}", chapter_issues(&r));
    // 節の不足は、その章の見出しを指す
    assert!(has(&r, "chapter-missing", "error", "1"), "{:?}", chapter_issues(&r));
}

#[test]
fn headings_inside_a_template_group_are_not_chapters() {
    let r = run(&three(), doc("甲", json!([
        h("1", 1, "第1章", Some("a")), h("2", 1, "第2章", Some("b")), h("3", 1, "第3章", Some("c")),
        {"id": "g", "kind": "group", "props": {"title": "部品テンプレート"}, "children": [h("g1", 1, "第1章", Some("a"))]},
    ])));
    assert!(has(&r, "chapter-extra", "error", "g1"), "{:?}", chapter_issues(&r));
}

#[test]
fn blocks_before_the_first_chapter_are_reported() {
    let r = run(&three(), doc("甲", json!([p("x"), h("1", 1, "第1章", Some("a")), h("2", 1, "第2章", Some("b")), h("3", 1, "第3章", Some("c"))])));
    assert!(has(&r, "chapter-block", "error", "x"), "{:?}", chapter_issues(&r));
}

/// 形式によって章が変わる文書テンプレート：乙だけ「乙の章」が要る
fn by_variant() -> String {
    style("(:)", r#"((id: "a", title: "第1章"), (id: "o", title: "乙の章", variants: ("乙",)), (id: "c", title: "最後の章"))"#)
}

#[test]
fn variant_decides_which_chapters_are_needed() {
    let st = by_variant();
    let s = session_with(&st);
    let d = s.new_document().unwrap();
    assert!(chapter_issues(&run(&st, d.clone())).is_empty());
    // 乙にすると章が足りない。足すと決められた位置に入り、もう1回足しても変わらない
    let mut d = d;
    d.meta.set("variant", "乙");
    assert!(chapter_issues(&run(&st, d.clone())).iter().any(|(c, _, _)| c == "chapter-missing"));
    let done = s.complete_chapters(&d).unwrap();
    let titles: Vec<_> = done.blocks.iter().map(|b| b.str("text").to_string()).collect();
    assert_eq!(titles, ["第1章", "乙の章", "最後の章"]);
    assert_eq!(s.complete_chapters(&done).unwrap(), done);
    assert!(chapter_issues(&run(&st, done.clone())).is_empty());
    // 甲に戻すと、乙の章は不要な章になる
    let mut back = done;
    back.meta.set("variant", "甲");
    assert!(chapter_issues(&run(&st, back)).iter().any(|(c, _, _)| c == "chapter-extra"));
}

#[test]
fn old_documents_without_chapter_ids_are_adopted_by_title() {
    // 章の定義を入れる前の文書（見出しに章の id が無い）
    let s = session_with(&three());
    let old = doc("甲", json!([h("1", 1, "第1章", None), p("p"), h("3", 1, "第3章", None)]));
    let done = s.complete_chapters(&old).unwrap();
    let titles: Vec<_> = done.blocks.iter().filter(|b| b.kind == "heading").map(|b| b.str("text").to_string()).collect();
    assert_eq!(titles, ["第1章", "第2章", "第3章"], "同じ見出し文の章を二重に作らない");
    assert!(chapter_issues(&run(&three(), done)).is_empty());
}

#[test]
fn locked_chapter_fixes_its_content_even_in_a_basic_document() {
    // 文書全体は体裁だけ（basic）、承認事項の章だけ中身まで固定（locked）
    let st = style(
        r#"(level: "basic")"#,
        r#"((id: "a", title: "承認事項", level: "locked", content: ((kind: "paragraph", text: "以下のとおり承認を求める。"),)),)"#,
    );
    let ok = doc("甲", json!([h("x", 1, "自由な章", None), h("1", 1, "承認事項", Some("a")), p("p")]));
    let r = run(&st, ok.clone());
    assert!(chapter_issues(&r).is_empty(), "basic では自由な章を置ける: {:?}", chapter_issues(&r));
    // GUI：承認事項の章の中には部品を追加できない
    assert_eq!(r.blocks["p"].chapter.as_ref().unwrap().insertable, Some(vec![]));
    let mut added = ok;
    added.blocks.push(serde_json::from_value(p("q")).unwrap());
    assert!(has(&run(&st, added), "chapter-locked", "error", "1"));
}

#[test]
fn gui_operations_are_stopped_only_for_errors() {
    let st = style(
        r#"(rules: (chapter-missing: "error", chapter-order: "warning"))"#,
        r#"((id: "a", title: "第1章"), (id: "b", title: "第2章", rules: (chapter-missing: "info")))"#,
    );
    let r = run(&st, doc("甲", json!([h("1", 1, "第1章", Some("a")), h("2", 1, "第2章", Some("b"))])));
    let a = r.blocks["1"].chapter.clone().unwrap();
    let b = r.blocks["2"].chapter.clone().unwrap();
    assert!(a.no_remove && !a.no_move, "第1章：消すとエラー・移動は警告");
    assert!(!b.no_remove, "第2章：無くても情報だけ");
    // 見出しに割り当てられる章は、その位置で使える定義
    assert_eq!(a.choices.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn rule_levels_can_be_changed_per_template_and_per_chapter() {
    let st = style(
        r#"(rules: (chapter-missing: "warning"))"#,
        r#"((id: "a", title: "第1章"), (id: "b", title: "第2章", rules: (chapter-missing: "off")))"#,
    );
    let issues = chapter_issues(&run(&st, doc("甲", json!([]))));
    assert_eq!(issues, [("chapter-missing".to_string(), "warning".to_string(), String::new())]);
}

#[test]
fn chapter_may_limit_the_blocks() {
    let st = style("(:)", r#"((id: "a", title: "第1章", allowed-blocks: ("heading", "paragraph")),)"#);
    let r = run(&st, doc("甲", json!([h("1", 1, "第1章", Some("a")), {"id": "x", "kind": "pagebreak", "props": {}}])));
    assert!(has(&r, "chapter-block", "error", "x"), "{:?}", chapter_issues(&r));
    let ins = r.blocks["1"].chapter.clone().unwrap().insertable.unwrap();
    assert!(ins.contains(&"paragraph".to_string()) && !ins.contains(&"pagebreak".to_string()));
}

#[test]
fn mistakes_in_chapter_definitions_are_reported() {
    // 章の id の重複、未知の検出名、文書テンプレートで使えない部品、選択肢にない構造形式、章で使えない部品を中身に指定
    for chapters in [
        r#"((id: "a", title: "1"), (id: "a", title: "2"))"#,
        r#"((id: "a", title: "1", rules: (no-such-rule: "error")),)"#,
        r#"((id: "a", title: "1", allowed-blocks: ("calc",)),)"#,
        r#"((id: "a", title: "1", variants: ("丙",)),)"#,
        r#"((id: "a", title: "1", allowed-blocks: ("heading",), content: ((kind: "paragraph"),)),)"#,
    ] {
        assert!(parse_style(&style("(:)", chapters)).is_err(), "{chapters}");
    }
}
