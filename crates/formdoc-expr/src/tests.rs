//! 計算書サンプル（samples/計算書サンプル.pdf）の数値を再現するゴールデンテスト。

use super::*;

fn var(value: f64, digits: Option<u8>) -> VarValue {
    VarValue { value, digits, ..Default::default() }
}

fn scope(items: &[(&str, f64, Option<u8>)]) -> Scope {
    items.iter().map(|(n, v, d)| (n.to_string(), var(*v, *d))).collect()
}

fn calc_text(expr: &str, s: &Scope, digits: u8) -> String {
    calc(&CalcRequest { expr: expr.into(), scope: s.clone(), digits: Some(digits), frac: true, ..Default::default() })
        .unwrap()
        .text
}

#[test]
fn impact_factor_p22() {
    // p.22 衝撃係数（曲げモーメントに対して）
    let s = scope(&[("L_b", 8.0, Some(3)), ("V", 110.0, Some(0))]);
    let ne = calc_text("70 * L_b^(-0.8)", &s, 3);
    assert_eq!(ne, "13.263");
    let mut s = s;
    s.insert("n_e".into(), var(13.263, Some(3)));
    s.insert("K_a".into(), var(2.0, Some(1)));
    assert_eq!(calc_text("K_a * V / (7.2 * n_e * L_b) + 10 / (65 + L_b)", &s, 3), "0.425");
    // せん断力に対して Lb = 4.000
    let s2 = scope(&[("L_b", 4.0, Some(3)), ("n_e", 23.091, Some(3)), ("K_a", 2.0, Some(1)), ("V", 110.0, Some(0))]);
    assert_eq!(calc_text("K_a * V / (7.2 * n_e * L_b) + 10 / (65 + L_b)", &s2, 3), "0.476");
}

#[test]
fn plate_buckling_p40_41() {
    // p.40 上フランジ (b/t)0 = 12.7、p.41 腹板 (Dw/tw)0 = 132.8
    let s = scope(&[
        ("R_cr", 0.7, Some(1)),
        ("k_0", 0.425, Some(3)),
        ("E", 2.0e5, Some(0)),
        ("nu", 0.3, Some(1)),
        ("f_syk", 235.0, Some(0)),
    ]);
    assert_eq!(calc_text("R_cr * sqrt(pi^2 * k_0 / (12 * (1 - nu^2)) * E / f_syk)", &s, 1), "12.7");
    let s = scope(&[("R_cr", 1.0, Some(1)), ("k_b", 23.9, Some(1)), ("E", 2.0e5, Some(0)), ("nu", 0.3, Some(1)), ("f_syk", 245.0, Some(0))]);
    assert_eq!(calc_text("R_cr * sqrt(pi^2 * k_b / (12 * (1 - nu^2)) * E / f_syk)", &s, 1), "132.8");
}

#[test]
fn wind_and_sum_p21_24() {
    let s = scope(&[("w_1", 1.5, Some(1)), ("H", 4.659, Some(3))]);
    assert_eq!(calc_text("w_1 * H", &s, 2), "6.99");
    let s = scope(&[("a", 2.25, Some(2)), ("b", 0.25, Some(2)), ("c", 2.86, Some(2)), ("d", 0.42, Some(2)), ("e", 0.10, Some(2))]);
    assert_eq!(calc_text("a + b + c + d + e", &s, 2), "5.88");
}

#[test]
fn check_shown_values() {
    // (b/t) = 169/22 = 7.68 → 表示 7.7 ≦ 12.7 → OK
    let s = scope(&[("b", 169.0, Some(0)), ("t", 22.0, Some(0)), ("bt_0", 12.7, Some(1))]);
    let r = check(&CheckRequest { expr: "b / t <= bt_0".into(), scope: s, digits: Some(1), ..Default::default() }).unwrap();
    assert!(r.ok);
    assert_eq!(r.lhs.text, "7.7");
    assert_eq!(r.rhs.text, "12.7");
    assert_eq!(r.shown_symbol, "≦");

    let s = scope(&[("i", 0.8, Some(3))]);
    let r = check(&CheckRequest { expr: "i <= 0.7".into(), scope: s, ..Default::default() }).unwrap();
    assert!(!r.ok);
    assert_eq!(r.shown_symbol, ">");
}

#[test]
fn rendering() {
    let s = scope(&[("w", 10.0, Some(1)), ("L", 5.0, Some(3)), ("sigma_ck", 24.0, Some(0))]);
    let o = calc(&CalcRequest { expr: "w * L^2 / 8".into(), scope: s.clone(), digits: Some(2), unit: "kN*m".into(), frac: true, ..Default::default() }).unwrap();
    assert_eq!(o.symbolic, "frac(w dot L^(2), 8)");
    assert_eq!(o.substituted, "frac(10.0 times 5.000^(2), 8)");
    assert_eq!(o.text, "31.25");
    assert_eq!(o.result, "31.25 thin \"kN·m\"");
    assert_eq!(o.vars, vec!["w", "L"]);
    assert_eq!(name_math("sigma_ck"), "sigma_(italic(\"ck\"))");
    assert_eq!(name_math("L_b"), "L_(b)");
    assert_eq!(name_math("EQ"), "italic(\"EQ\")");
}

#[test]
fn rounding_policy() {
    // 表示値で計算（既定）: 1.235→1.24 として 1.24*2 = 2.48
    let s = scope(&[("a", 1.235, Some(2))]);
    assert_eq!(calc_text("a * 2", &s, 3), "2.480");
    let r = calc(&CalcRequest { expr: "a * 2".into(), scope: s, digits: Some(3), rounding: Rounding::Full, ..Default::default() }).unwrap();
    assert_eq!(r.text, "2.470");
}

#[test]
fn errors() {
    assert!(matches!(parse("2 * (3 + 4"), Err(_)));
    assert!(matches!(calc(&CalcRequest { expr: "x + 1".into(), ..Default::default() }), Err(Error::Eval(_))));
    assert!(matches!(calc(&CalcRequest { expr: "1 / 0".into(), ..Default::default() }), Err(Error::Eval(_))));
    assert!(matches!(calc(&CalcRequest { expr: "foo(1)".into(), ..Default::default() }), Err(Error::Eval(_))));
    assert_eq!(parse("-x^2").unwrap(), parse("-(x^2)").unwrap().clone().strip_outer());
}

impl Expr {
    fn strip_outer(self) -> Expr {
        match self {
            Expr::Neg(inner) => match *inner {
                Expr::Paren(x) => Expr::Neg(x),
                other => Expr::Neg(Box::new(other)),
            },
            other => other,
        }
    }
}

#[test]
fn digit_grouping_follows_the_setting() {
    use crate::{DEFAULT_GROUP, NumFormat, format_number, group_literal};
    // 既定は5桁以上だけ区切る。4 にすると 4桁から区切る（文書テンプレートの Lint の digit-grouping）
    let f = |v: f64, group: u8| format_number(v, NumFormat { digits: Some(0), group });
    assert_eq!(f(7872.0, DEFAULT_GROUP), "7872");
    assert_eq!(f(23550.0, DEFAULT_GROUP), "23,550");
    assert_eq!(f(7872.0, 4), "7,872");
    assert_eq!(f(-1234567.0, 4), "-1,234,567");
    assert_eq!(f(999.0, 4), "999");
    assert_eq!(f(1234.0, 0), "1234");
    // 式に書いた数値も同じ規則（小数部は区切らない、数字以外を含むものはそのまま）
    assert_eq!(group_literal("1000000", 4), "1,000,000");
    assert_eq!(group_literal("8000.125", 4), "8,000.125");
    assert_eq!(group_literal("1e6", 4), "1e6");
    // 計算行の代入式・結果にも効く
    let s = scope(&[("A", 9428.0, Some(0))]);
    let req = |group| CalcRequest { expr: "A * 1000".into(), scope: s.clone(), digits: Some(0), group, ..Default::default() };
    let o = crate::calc(&req(Some(4))).unwrap();
    assert_eq!(o.text, "9,428,000");
    assert!(o.substituted.contains("9,428") && o.substituted.contains("1,000"), "{}", o.substituted);
    // 設定が無いときは，これまでどおり式に書いた数値は書いたとおり（5桁以上の結果だけ区切る）
    let legacy = crate::calc(&CalcRequest { expr: "A * 100000".into(), scope: s.clone(), digits: Some(0), ..Default::default() }).unwrap();
    assert!(legacy.substituted.contains("100000") && legacy.text == "942,800,000", "{legacy:?}");
}
