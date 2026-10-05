//! 部品の計算ロジック（Rhai。TODO 5）の結合テスト。
//! 部品「断面諸量（I形）」で計算書サンプル p.40 と JIS の H形鋼の断面性能を再現できること、変数として後ろの計算で使えること、
//! スクリプトの誤りや無限ループがアプリを止めずにエラーとして報告されることを確かめる。

use formdoc_core::api::UpdateResult;
use formdoc_core::{Document, FormdocWorld, Session, compile, logic};
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

/// 断面諸量の部品。props で寸法を上書きする（既定は計算書サンプル p.40 の H-700×350×12×22、フィレット半径 18 mm）
fn section(id: &str, props: Value) -> Value {
    let mut p = json!({"kind": "圧延H形", "label": "H-700×350×12×22（SM400）", "H": 700, "B": 350, "tw": 12, "tf": 22, "r": 18});
    for (k, v) in props.as_object().unwrap() {
        p[k] = v.clone();
    }
    json!({"id": id, "kind": "section-props", "props": p})
}

fn text(r: &UpdateResult, name: &str) -> String {
    r.vars.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("{name} が無い")).text.clone()
}

fn value(r: &UpdateResult, name: &str) -> f64 {
    r.vars.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("{name} が無い")).value
}

#[test]
fn section_properties_match_the_sample() {
    let (s, r) = run(json!([section("s", json!({}))]));
    assert!(r.exportable, "{:?}", r.issues);
    // サンプル p.40 の値（断面積・断面二次モーメント・断面二次半径・図心から縁まで）と p.46 の腹板の断面積
    assert_eq!(text(&r, "A"), "23,550");
    assert_eq!(text(&r, "I_y"), "2,080,000,000");
    assert_eq!(text(&r, "r_y"), "297");
    assert_eq!(text(&r, "r_z"), "81.7");
    assert_eq!(text(&r, "y_u"), "350");
    assert_eq!(text(&r, "y_l"), "350");
    assert_eq!(value(&r, "A_w"), 7872.0);
    assert!(s.pdf().unwrap().starts_with(b"%PDF"));
}

#[test]
fn section_properties_match_the_jis_table() {
    // JIS G 3192 の H-400×200×8×13（r = 16）：A = 84.12 cm²、Ix = 23,700 cm⁴、ix = 16.8 cm、iy = 4.54 cm
    // （フィレットの寄与が大きく、弱軸の断面二次半径がフィレットの断面二次モーメントの式の誤りで変わる）
    let (_, r) = run(json!([section("s", json!({"H": 400, "B": 200, "tw": 8, "tf": 13, "r": 16}))]));
    assert!(r.exportable, "{:?}", r.issues);
    assert_eq!(value(&r, "A").round(), 8412.0);
    assert_eq!(text(&r, "I_y"), "237,000,000");
    assert_eq!(text(&r, "r_y"), "168");
    assert_eq!(text(&r, "r_z"), "45.4");
}

#[test]
fn defined_variables_can_be_used_by_later_calculations() {
    let (_, r) = run(json!([
        section("s", json!({})),
        {"id": "c", "kind": "calc", "props": {"name": "sigma", "expr": "1000000 / A", "unit": "N/mm2"}},
    ]));
    assert!(r.exportable, "{:?}", r.issues);
    assert_eq!(text(&r, "sigma"), "42.5");
}

#[test]
fn suffix_keeps_two_sections_apart() {
    let (_, r) = run(json!([section("s", json!({})), section("s2", json!({"kind": "溶接I形", "H": 500, "B": 200, "tw": 9, "tf": 16, "suffix": "2"}))]));
    assert!(r.exportable, "{:?}", r.issues);
    assert_ne!(value(&r, "A"), value(&r, "A_2"));
    // 添字なしで2つ置くと、同じ名前の変数になるのでエラー
    let (_, r) = run(json!([section("s", json!({})), section("s2", json!({}))]));
    assert!(r.issues.iter().any(|i| i.code == "var-dup" && i.block_id.as_deref() == Some("s2")), "{:?}", r.issues);
}

#[test]
fn input_mistakes_are_reported_on_the_field() {
    for (props, field) in [(json!({"tf": 0}), "tf"), (json!({"r": -1}), "r"), (json!({"r": 200}), "r"), (json!({"tw": 400, "r": 0}), "tw")] {
        let (_, r) = run(json!([section("s", props.clone())]));
        assert!(r.issues.iter().any(|i| i.block_id.as_deref() == Some("s") && i.field.as_deref() == Some(field)), "{props}: {:?}", r.issues);
        assert!(!r.exportable);
    }
}

/// コードモードで見せるコードが、そのまま Typst で組版できるか
fn code_compiles(s: &Session) -> bool {
    let mut w = FormdocWorld::new();
    w.set_files([("/main.typ".to_string(), s.code().unwrap().into_bytes()), ("/style.typ".to_string(), STYLE.as_bytes().to_vec())]);
    compile(&w).document.is_some()
}

#[test]
fn code_view_compiles_and_keeps_the_component() {
    // コードモードで何も変えなければ、計算ロジックの部品はそのまま戻る
    let (s, r) = run(json!([section("s", json!({}))]));
    assert!(r.exportable);
    assert!(code_compiles(&s));
    assert_eq!(s.apply_code(&s.code().unwrap()).unwrap().doc.blocks[0].kind, "section-props");
    // 入力に誤りがあっても、コードモードのコードは組版できる（その部品は表示しないだけ）
    let (s, _) = run(json!([section("s", json!({"tf": 0}))]));
    assert!(code_compiles(&s));
}

#[test]
fn runaway_scripts_are_stopped_with_an_error() {
    let props = Map::new();
    let vars = Default::default();
    // 無限ループ・深すぎる再帰・巨大な文字列は、固まらずにエラーになる
    for script in ["loop {}", "fn f(x) { f(x + 1) } f(0)", "let s = \"x\"; loop { s += s; }"] {
        assert!(logic::run(script, &props, &vars).is_err(), "{script}");
    }
    // 構文の誤り・戻り値の形の誤り・文字列を実行する eval もエラーになる
    for script in ["let = ;", "42", "eval(\"40 + 2\")"] {
        assert!(logic::run(script, &props, &vars).is_err(), "{script}");
    }
}

#[test]
fn return_values_are_checked() {
    let props = Map::new();
    let vars = Default::default();
    for script in [
        r#"#{ vars: [#{ name: "x", value: 1.0, digits: 1.5 }] }"#,  // 小数桁数が整数でない
        r#"#{ vars: [#{ name: "x", value: 1.0 / 0.0 }] }"#,          // 値が数にならない
        r#"#{ values: #{ "長さ": 1.0 } }"#,                           // 描画関数に渡せないキー
    ] {
        assert!(logic::run(script, &props, &vars).is_err(), "{script}");
    }
}

#[test]
fn same_input_gives_the_same_output() {
    // 変数を渡す順序（HashMap の並び）によらず、結果は同じ
    let script = r#"let names = vars.keys(); #{ values: #{ names: names }, vars: [#{ name: "y", value: vars.a.value + vars.b.value }] }"#;
    let v = |x: f64| formdoc_expr::VarValue { value: x, ..Default::default() };
    let mut s1 = formdoc_expr::Scope::new();
    s1.insert("a".into(), v(1.0));
    s1.insert("b".into(), v(2.0));
    let mut s2 = formdoc_expr::Scope::new();
    s2.insert("b".into(), v(2.0));
    s2.insert("a".into(), v(1.0));
    let props = Map::new();
    assert_eq!(logic::run(script, &props, &s1).unwrap(), logic::run(script, &props, &s2).unwrap());
}

#[test]
fn component_definitions_are_consistent() {
    // 計算ロジックを持つ部品はすべて、スクリプトが読めて構文が正しく、描画関数が社内標準パッケージにある
    let lib = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../library/typst/formdoc/0.1.0/lib.typ")).unwrap();
    let logics = logic::component_logics();
    assert!(!logics.is_empty());
    for (kind, l) in logics {
        assert!(l.ast.is_ok(), "{kind}: {:?}", l.ast.as_ref().err());
        assert!(l.render.is_empty() || lib.contains(&l.render), "{kind}: 描画関数 {} が lib.typ にありません", l.render);
    }
}
