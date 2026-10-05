//! GUI文書の評価〜組版〜PDF出力の結合テスト。

use formdoc_core::template::{builtin_style, parse_style};
use formdoc_core::{Document, Session};

fn style() -> String {
    builtin_style("keisansho").unwrap()
}

/// 計算書の文書テンプレートを設定したセッション。
fn session() -> Session {
    let mut s = Session::new();
    s.set_style(&style()).unwrap();
    s
}

fn sample() -> Document {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/keisansho-gui/document.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// 内容のエラー（章構成の検出を除く）。章構成は tests/structure.rs で確かめるため、
/// ここでは章の無い小さな文書でも変数・計算・図形の振る舞いだけを見られるようにする。
fn is_content_error(i: &formdoc_core::evaluate::Issue) -> bool {
    format!("{:?}", i.severity) == "Error" && !i.code.starts_with("chapter-")
}

fn set(doc: &mut Document, id: &str, key: &str, v: serde_json::Value) {
    doc.blocks.iter_mut().find(|b| b.id == id).unwrap().props.insert(key.into(), v);
}

#[test]
fn sample_is_exportable() {
    let mut s = session();
    let r = s.update_document(sample(), &[]);
    let errors: Vec<_> = r.issues.iter().filter(|i| is_content_error(i)).collect();
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
    let a = { let mut s = session(); s.update_document(sample(), &[]); s.pdf().unwrap() };
    let b = { let mut s = session(); s.update_document(sample(), &[]); s.pdf().unwrap() };
    assert_eq!(a, b);
}

#[test]
fn unchanged_pages_are_not_resent() {
    let mut s = session();
    let r1 = s.update_document(sample(), &[]);
    let known: Vec<String> = r1.pages.iter().map(|p| p.hash.clone()).collect();
    let mut doc = sample();
    // 2ページ目以降に影響しない表紙の変更
    doc.meta.set("project", "別の業務名");
    let r2 = s.update_document(doc, &known);
    assert!(r2.pages[0].svg.is_some());
    assert!(r2.pages[1..].iter().all(|p| p.svg.is_none()));
}

#[test]
fn ng_check_is_reported_and_undefined_var_blocks_export() {
    let mut doc = sample();
    // i <= 0.7 の制限値を厳しくして NG にする（NG は警告：出力は可能）
    set(&mut doc, "b12", "expr", "i <= 0.4".into());
    let mut s = session();
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
    let mut s = session();
    let r = s.update_document(doc, &[]);
    assert!(r.issues.iter().any(|i| i.code == "var-dup" && i.block_id.as_deref() == Some("b7")));
    for code in ["lint-punctuation", "lint-fullwidth", "lint-wording", "lint-period"] {
        assert!(r.issues.iter().any(|i| i.code == code), "{code}");
    }
}

#[test]
fn new_document_has_skeleton() {
    // 骨組みは文書テンプレートの章の定義から作る（中身の確認は tests/structure.rs）
    let d = session().new_document().unwrap();
    assert!(d.blocks.iter().any(|b| b.kind == "heading" && b.str("chapter") == "gaiyou"));
    assert_eq!(d.meta.str("title"), "計算書");
    assert_eq!(d.meta.get("chapter-start").and_then(|v| v.as_i64()), Some(1));
    let c = formdoc_core::api::catalog().unwrap();
    assert!(c.fonts.iter().any(|f| f == "Noto Serif JP"));
    assert!(c.components.get("calc").is_some());
}

#[test]
fn style_info_is_read_from_typst() {
    let t = parse_style(&style()).unwrap();
    assert_eq!(t.id, "keisansho");
    assert_eq!(t.default_digits("kN"), Some(2));
    assert!(t.lint.punctuation.is_some());
    assert!(t.fields.iter().any(|f| f.key == "date" && f.kind == "date"));
    // style 関数の無いファイル、構文エラーはエラーになる
    assert!(parse_style("#let info = (id: \"x\", name: \"x\", blocks: ())").is_err());
    assert!(parse_style("#let info = (").is_err());
}

#[test]
fn old_meta_chapter_start_is_migrated() {
    // 旧形式（chapter_start）で保存された文書
    let mut json: serde_json::Value = serde_json::to_value(sample()).unwrap();
    let meta = json["meta"].as_object_mut().unwrap();
    meta.remove("chapter-start");
    meta.insert("chapter_start".into(), 4.into());
    let doc: Document = serde_json::from_value(json).unwrap();
    assert_eq!(doc.meta.get("chapter-start").and_then(|v| v.as_i64()), Some(4));
    let mut s = session();
    s.update_document(doc, &[]);
    assert!(s.code().unwrap().contains("chapter-start: 4"));
}

#[test]
fn builtin_snippets_compile_without_errors() {
    // 同梱部品テンプレート（I形断面・単純梁）をそのまま文書にして組版できる。汎用図形の評価値も返る
    let snippets = formdoc_core::template::builtin_snippets();
    assert!(snippets.len() >= 2);
    for sn in snippets {
        let mut s = session();
        let mut doc = s.new_document().unwrap();
        doc.blocks = serde_json::from_value(sn["blocks"].clone()).unwrap();
        let r = s.update_document(doc, &[]);
        let errors: Vec<_> = r.issues.iter().filter(|i| is_content_error(i)).collect();
        assert!(errors.is_empty(), "{}: {errors:?}", sn["name"]);
        let shapes = r.blocks.values().find(|b| !b.shapes.is_empty()).expect("図形の評価値");
        assert!(shapes.shapes.iter().flatten().all(|v| v.x1.is_some() || !v.pts.is_empty()));
    }
}

#[test]
fn shape_repeat_and_influence_line() {
    // 単純梁の部品テンプレート：荷重 n 個を間隔 s で繰り返し、影響線縦距を自動計算する
    let sn = formdoc_core::template::builtin_snippets().into_iter().find(|s| s["name"].as_str().unwrap().starts_with("単純梁")).unwrap();
    let mut s = session();
    let mut doc = s.new_document().unwrap();
    doc.blocks = serde_json::from_value(sn["blocks"].clone()).unwrap();
    let r = s.update_document(doc.clone(), &[]);
    let fig = r.blocks.values().find(|b| !b.shapes.is_empty()).unwrap();
    // 荷重の矢印は2回、間隔の寸法線は n-1 = 1回
    assert_eq!(fig.shapes[3].len(), 2);
    assert_eq!(fig.shapes[3][1].x1, Some(5.8));
    assert_eq!(fig.shapes[7].len(), 1);
    // 縦距の文字：サンプル（0.625, 0.275）と一致
    let labels: Vec<_> = fig.shapes[12].iter().map(|v| v.label.clone().unwrap()).collect();
    assert_eq!(labels, ["0.625", "0.275"]);
    assert_eq!(fig.shapes[4][1].label.as_deref(), Some("P2"));
    assert_eq!(r.vars.iter().find(|v| v.name == "eta_sum").unwrap().text, "0.900");
    // 荷重を3個にすると図も縦距も変わる
    doc.blocks.iter_mut().find(|b| b.str("name") == "n").unwrap().props.insert("value".into(), 3.into());
    doc.blocks.iter_mut().find(|b| b.str("name") == "s").unwrap().props.insert("value".into(), 2.0.into());
    let r = s.update_document(doc, &[]);
    let fig = r.blocks.values().find(|b| !b.shapes.is_empty()).unwrap();
    assert_eq!(fig.shapes[3].len(), 3);
    let labels: Vec<_> = fig.shapes[12].iter().map(|v| v.label.clone().unwrap()).collect();
    assert_eq!(labels, ["0.625", "0.375", "0.125"]);
    // 章の無い試験用の文書なので、章構成以外のエラーが無いことを確かめる
    let errs: Vec<_> = r.issues.iter().filter(|i| is_content_error(i)).collect();
    assert!(errs.is_empty(), "{errs:?}");
}

fn blk(id: &str, kind: &str, props: serde_json::Value) -> formdoc_core::model::Block {
    formdoc_core::model::Block { id: id.into(), kind: kind.into(), props: props.as_object().unwrap().clone(), children: vec![] }
}

fn errors(r: &formdoc_core::api::UpdateResult) -> Vec<String> {
    r.issues.iter().filter(|i| is_content_error(i)).map(|i| format!("{}:{}", i.block_id.clone().unwrap_or_default(), i.code)).collect()
}

#[test]
fn local_and_global_variables() {
    use serde_json::json;
    let mut s = session();
    let mut doc = s.new_document().unwrap();
    doc.blocks = vec![
        blk("h1", "heading", json!({"level": 1, "text": "A"})),
        blk("v1", "vdef", json!({"name": "x", "value": 1.0})),
        blk("v2", "vdef", json!({"name": "g", "value": 2.0, "global": true})),
        blk("h2", "heading", json!({"level": 2, "text": "A.1"})),
        blk("c1", "calc", json!({"name": "y", "expr": "x + g"})),       // 親の節の変数は使える
        blk("h3", "heading", json!({"level": 1, "text": "B"})),
        blk("v3", "vdef", json!({"name": "x", "value": 5.0})),           // 別の節なら同じ名前でよい
        blk("c2", "calc", json!({"name": "z", "expr": "x + g"})),       // x は B の x
        blk("c3", "calc", json!({"name": "w", "expr": "y"})),           // y は A.1 のローカル → 使えない
    ];
    let r = s.update_document(doc.clone(), &[]);
    assert_eq!(errors(&r), ["c3:calc"]);
    let msg = &r.issues.iter().find(|i| i.block_id.as_deref() == Some("c3")).unwrap().message;
    assert!(msg.contains("グローバル変数として定義"), "{msg}");
    assert_eq!(r.vars.iter().find(|v| v.name == "z").unwrap().value, 7.0);
    assert_eq!(r.vars.iter().find(|v| v.name == "g").unwrap().scope, None);
    assert_eq!(r.vars.iter().find(|v| v.name == "y").unwrap().scope.as_deref(), Some("h2"));

    // 見えている変数と同じ名前は定義できない
    doc.blocks.truncate(8);
    doc.blocks.push(blk("v4", "vdef", json!({"name": "g", "value": 1.0})));
    let r = s.update_document(doc, &[]);
    assert_eq!(errors(&r), ["v4:var-dup"]);
}

#[test]
fn group_hides_internal_variables_and_exports() {
    use serde_json::json;
    let mut s = session();
    let mut doc = s.new_document().unwrap();
    let mut g = blk("g1", "group", json!({"title": "I形断面", "exports": ["A"]}));
    g.children = vec![
        blk("t1", "vdef", json!({"name": "H", "value": 700.0})),
        blk("t2", "calc", json!({"name": "A", "expr": "2 * H"})),
    ];
    let mut g2 = g.clone();
    g2.id = "g2".into();
    g2.children[0].id = "u1".into();
    g2.children[1].id = "u2".into();
    g2.props.insert("exports".into(), json!(["A_2"]));
    g2.children[1].props.insert("name".into(), json!("A_2"));
    doc.blocks = vec![
        g,
        blk("c1", "calc", json!({"name": "a1", "expr": "A"})),   // 公開した変数は使える
        g2,                                                       // 中の H は前の部品テンプレートと重ならない
        blk("c2", "calc", json!({"name": "a2", "expr": "H"})),   // 内部の変数は使えない
    ];
    let r = s.update_document(doc, &[]);
    assert_eq!(errors(&r), ["c2:calc"]);
    assert_eq!(r.vars.iter().find(|v| v.name == "a1").unwrap().value, 1400.0);
    assert!(s.code().unwrap().contains("// @group g1"));
}

#[test]
fn code_view_round_trips_to_the_same_document() {
    // コードモードで何も変えなければ、文書はそのまま
    let mut s = session();
    let doc = sample();
    s.update_document(doc.clone(), &[]);
    let code = s.code().unwrap();
    assert!(code.contains("// @block b11") && code.contains("// @group g1"));
    let a = s.apply_code(&code).unwrap();
    assert!(a.warnings.is_empty(), "{:?}", a.warnings);
    assert_eq!(a.doc, doc);
}

#[test]
fn code_edit_becomes_typst_block_and_keeps_variables() {
    let mut s = session();
    let doc = sample();
    s.update_document(doc.clone(), &[]);
    let code = s.code().unwrap();
    // 変数定義 V の値をコードで書き換える → その部品は Typstコード部品になり、V は引き続き使える
    let start = code.find("// @block b6\n").unwrap();
    let end = start + code[start..].find("\n\n").unwrap();
    let edited = format!("{}// @block b6\n#let v-V = vdef(\"V\", 120.0, unit: \"km/h\", digits: 0, desc: \"最高速度\"){}", &code[..start], &code[end..]);
    // 目印の無い行を足すと、新しい Typstコード部品になる
    let edited = edited.replace("// @block b9\n", "#v(1em)\n\n// @block b9\n");
    let a = s.apply_code(&edited).unwrap();
    let b6 = a.doc.blocks.iter().find(|b| b.id == "b6").unwrap();
    assert_eq!(b6.kind, "typst");
    assert_eq!(a.doc.blocks.len(), doc.blocks.len() + 1);
    assert!(a.doc.blocks.iter().any(|b| b.kind == "typst" && b.str("code") == "#v(1em)"));
    // 他の部品はそのまま
    assert_eq!(a.doc.blocks.iter().find(|b| b.id == "b11").unwrap(), doc.blocks.iter().find(|b| b.id == "b11").unwrap());
    let r = s.update_document(a.doc, &[]);
    assert!(r.exportable, "{:?}", r.issues.iter().filter(|i| is_content_error(i)).collect::<Vec<_>>());
    // V = 120 で後ろの計算（i）が変わる
    let i = r.vars.iter().find(|v| v.name == "i").unwrap();
    assert_ne!(i.text, "0.425");
    // 先頭の文書情報の行を変えると案内が出る
    let code = s.code().unwrap().replacen("#show: style.with(", "#show: style.with(foo: 1, ", 1);
    assert!(!s.apply_code(&code).unwrap().warnings.is_empty());
}
