//! デモ文書（examples/keisansho-demo。計算書サンプル p.18〜48 の再現）の回帰テスト。
//! 文書を組版し、サンプルに載っている主な値と照査の結果が同じになることを確かめる。

use formdoc_core::evaluate::Severity;
use formdoc_core::template::builtin_style;
use formdoc_core::{Document, Session};

/// examples/<name>/document.json を読み、文書が指す同梱の文書テンプレートを設定したセッションで組版する。
fn open(name: &str) -> (Session, Document, formdoc_core::api::UpdateResult) {
    let path = format!("{}/../../examples/{name}/document.json", env!("CARGO_MANIFEST_DIR"));
    let doc: Document = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut s = Session::new();
    s.set_style(&builtin_style(&doc.template).unwrap()).unwrap();
    let r = s.update_document(doc.clone(), &[]);
    (s, doc, r)
}

#[test]
fn keisansho_demo_reproduces_the_sample() {
    let (s, doc, r) = open("keisansho-demo");
    let errors: Vec<_> = r.issues.iter().filter(|i| matches!(i.severity, Severity::Error)).collect();
    assert!(errors.is_empty() && r.exportable, "{errors:?}");
    // 照査はすべて満たす（NG は注意として出るので、エラーとは別に確かめる）
    let ng: Vec<_> = r.issues.iter().filter(|i| i.code == "check-ng").collect();
    assert!(ng.is_empty(), "{ng:?}");

    let v = |name: &str| r.vars.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("{name}")).text.clone();
    for (name, want) in [
        // p.21〜25 4.1 作用：死荷重・衝撃係数・車両横荷重の係数・風荷重
        ("W_D", "5.88"), ("i_M", "0.425"), ("i_S", "0.476"), ("beta_LF", "0.078"), ("w_2", "10.85"),
        // p.33〜36 4.3 作用の組合せ：e点の設計作用力と A点のせん断力
        ("Mc2max_e", "705.61"), ("Sc2max_e", "121.31"), ("Nc2max_e", "66.00"), ("Sc2max_A", "402.86"),
        // p.37〜39 4.4 反力（地震時）
        ("R_EQ", "11.69"), ("H_EQ2", "26.19"),
        // p.40〜46 4.5 断面諸量と断面耐力
        ("A", "23,550"), ("I_y", "2,080,000,000"), ("N_ud", "934.1"), ("M_ucd", "972.1"), ("M_utd", "1,176.2"), ("V_yd", "999.4"),
    ] {
        assert_eq!(v(name), want, "{name}");
    }

    // p.47〜48 4.5 (5) 耐荷性の照査：照査の左辺の値（評価結果の要約「左辺 ≦ 右辺 → OK」）
    let check = |expr_part: &str| {
        let b = doc.all_blocks().into_iter().find(|b| b.kind == "check" && b.str("expr").contains(expr_part)).unwrap_or_else(|| panic!("{expr_part}"));
        r.blocks[&b.id].summary.clone()
    };
    for (expr_part, want) in [
        ("M_d / M_ucd)", "0.956"), ("M_d / M_ucld)", "0.714"), ("M_d / M_utld)", "0.686"), ("M_d / M_utd)", "0.805"),
        ("V_d / V_yd <=", "0.146"), ("M_ucld)^2", "0.439"), ("M_utld)^2", "0.406"),
    ] {
        let summary = check(expr_part);
        assert!(summary.starts_with(want) && summary.ends_with("OK"), "{expr_part}: {summary}");
    }
    assert!(s.pdf().unwrap().starts_with(b"%PDF"));
}
