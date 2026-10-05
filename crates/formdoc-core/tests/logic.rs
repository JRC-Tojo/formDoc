//! 部品の計算ロジック（Rhai。TODO 5）の結合テスト。
//! 部品「断面諸量（I形）」で計算書サンプル p.40 の値を再現できること、変数として後ろの計算で使えること、
//! スクリプトの誤りや無限ループがアプリを止めずにエラーとして報告されることを確かめる。

use formdoc_core::api::UpdateResult;
use formdoc_core::{Document, Session, logic};
use serde_json::{Map, Value, json};

/// 章の定義の無い試験用の文書テンプレート（計算ロジックの部品と計算行を使える）
const STYLE: &str = r#"#let info = (
  id: "t", name: "試験", blocks: ("heading", "paragraph", "section-props", "calc", "where"),
  digits: ("mm": 0, "mm2": 0, "mm4": 0, "N/mm2": 1),
)
#let style(..args, doc) = doc
"#;

fn run(blocks: Value) -> (Session, UpdateResult) {
    let mut s = Session::new();
    s.set_style(STYLE).unwrap();
    let doc: Document = serde_json::from_value(json!({"template": "t", "blocks": blocks})).unwrap();
    let r = s.update_document(doc, &[]);
    (s, r)
}

/// 計算書サンプル p.40 の主桁：H-700×350×12×22（SM400）、圧延H形のフィレット半径 18 mm
fn h700(props: Value) -> Value {
    let mut p = json!({"kind": "圧延H形", "label": "H-700×350×12×22（SM400）", "H": 700, "B": 350, "tw": 12, "tf": 22, "r": 18});
    for (k, v) in props.as_object().unwrap() {
        p[k] = v.clone();
    }
    json!({"id": "s", "kind": "section-props", "props": p})
}

fn text(r: &UpdateResult, name: &str) -> String {
    r.vars.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("{name} が無い")).text.clone()
}

#[test]
fn section_properties_match_the_sample() {
    let (s, r) = run(json!([h700(json!({}))]));
    assert!(r.exportable, "{:?}", r.issues);
    // サンプル p.40 の値（断面積・断面二次モーメント・断面二次半径・図心から縁まで）と p.46 の腹板の断面積
    assert_eq!(text(&r, "A"), "23,550");
    assert_eq!(text(&r, "I_y"), "2,080,000,000");
    assert_eq!(text(&r, "r_y"), "297");
    assert_eq!(text(&r, "r_z"), "81.7");
    assert_eq!(text(&r, "y_u"), "350");
    assert_eq!(text(&r, "y_l"), "350");
    assert_eq!(text(&r, "A_w").replace(',', ""), "7872"); // 4桁の桁区切りは文書テンプレートの設定による
    assert!(s.pdf().unwrap().starts_with(b"%PDF"));
}

#[test]
fn defined_variables_can_be_used_by_later_calculations() {
    let (_, r) = run(json!([
        h700(json!({})),
        {"id": "c", "kind": "calc", "props": {"name": "sigma", "expr": "1000000 / A", "unit": "N/mm2"}},
    ]));
    assert!(r.exportable, "{:?}", r.issues);
    assert_eq!(text(&r, "sigma"), "42.5");
}

#[test]
fn suffix_keeps_two_sections_apart() {
    let (_, r) = run(json!([h700(json!({})), {"id": "s2", "kind": "section-props", "props": {"kind": "溶接I形", "H": 500, "B": 200, "tw": 9, "tf": 16, "suffix": "2"}}]));
    assert!(r.exportable, "{:?}", r.issues);
    assert!(r.vars.iter().any(|v| v.name == "A_2"));
}

#[test]
fn input_mistakes_are_reported_on_the_field() {
    let (_, r) = run(json!([h700(json!({"tf": 0}))]));
    assert!(r.issues.iter().any(|i| i.block_id.as_deref() == Some("s") && i.field.as_deref() == Some("tf")), "{:?}", r.issues);
    assert!(!r.exportable);
}

#[test]
fn code_view_keeps_the_component() {
    // コードモードで何も変えなければ、計算ロジックの部品はそのまま戻る
    let (s, r) = run(json!([h700(json!({}))]));
    assert!(r.exportable);
    let code = s.code().unwrap();
    assert!(code.contains("section-props("), "{code}");
    let a = s.apply_code(&code).unwrap();
    assert_eq!(a.doc.blocks[0].kind, "section-props");
}

#[test]
fn runaway_scripts_are_stopped_with_an_error() {
    let props = Map::new();
    let vars = Default::default();
    // 無限ループ・深すぎる再帰・巨大な文字列は、固まらずにエラーになる
    for script in ["loop {}", "fn f(x) { f(x + 1) } f(0)", "let s = \"x\"; loop { s += s; }"] {
        let e = logic::run(script, &props, &vars).unwrap_err();
        assert!(e.contains("止めました") || e.contains("大きすぎます"), "{script}: {e}");
    }
    // 構文の誤り・戻り値の形の誤りもエラーとして報告する
    assert!(logic::run("let = ;", &props, &vars).is_err());
    assert!(logic::run("42", &props, &vars).is_err());
}

#[test]
fn scripts_cannot_reach_outside() {
    // ファイル・時刻・モジュールの読み込みは使えない（同じ入力なら同じ結果になるように）
    let props = Map::new();
    let vars = Default::default();
    for script in ["import \"x\" as y; #{}", "timestamp(); #{}", "open_file(\"a\"); #{}"] {
        assert!(logic::run(script, &props, &vars).is_err(), "{script}");
    }
}
