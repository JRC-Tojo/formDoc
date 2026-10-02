//! 式の評価。三角関数は度（°）で受け取る（土木・建築の計算書の慣習に合わせ、書き手による解釈のぶれをなくす）。

use crate::ast::{BinOp, Expr};
use crate::format::round_half_up;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct EvalError {
    pub message: String,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EvalError {}

fn err<T>(message: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError { message: message.into() })
}

/// 式から参照される変数の値と表示情報。
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct VarValue {
    pub value: f64,
    /// 表示する小数桁数。None なら有効数字で自動。
    #[serde(default)]
    pub digits: Option<u8>,
    /// 単位（例: "kN/m", "N/mm2"）。
    #[serde(default)]
    pub unit: String,
    /// Typst数式での表示（例: "(b/t)_0"）。None なら変数名から生成。
    #[serde(default)]
    pub display: Option<String>,
    /// 記号説明（「ここに，」に使う）。
    #[serde(default)]
    pub desc: String,
}

impl VarValue {
    /// 丸め方針に従って、後続の計算で使う値。
    pub fn effective(&self, policy: Rounding) -> f64 {
        match (policy, self.digits) {
            (Rounding::Display, Some(d)) => round_half_up(self.value, d),
            _ => self.value,
        }
    }
}

/// 後続の計算で、前段の値を表示桁で丸めてから使うか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rounding {
    /// 表示どおりの値で計算する（読者が電卓で検算できる。計算書の既定）。
    #[default]
    Display,
    /// 内部の全精度で計算する。
    Full,
}

pub type Scope = HashMap<String, VarValue>;

pub(crate) fn is_constant(name: &str) -> bool {
    matches!(name, "pi" | "π")
}

const FUNCS: &[(&str, usize, usize)] = &[
    // (名前, 最小引数, 最大引数)
    ("sqrt", 1, 1),
    ("root", 2, 2),
    ("abs", 1, 1),
    ("min", 1, usize::MAX),
    ("max", 1, usize::MAX),
    ("sin", 1, 1),
    ("cos", 1, 1),
    ("tan", 1, 1),
    ("log", 1, 1),
    ("ln", 1, 1),
    ("exp", 1, 1),
    ("round", 1, 2),
    ("floor", 1, 1),
    ("ceil", 1, 1),
];

pub fn is_function(name: &str) -> bool {
    FUNCS.iter().any(|(n, ..)| *n == name)
}

pub fn eval(e: &Expr, scope: &Scope, policy: Rounding) -> Result<f64, EvalError> {
    let v = match e {
        Expr::Num(v, _) => *v,
        Expr::Var(n) if is_constant(n) => std::f64::consts::PI,
        Expr::Var(n) => match scope.get(n) {
            Some(v) => v.effective(policy),
            None => return err(format!("変数 {n} が定義されていません")),
        },
        Expr::Neg(x) => -eval(x, scope, policy)?,
        Expr::Paren(x) => eval(x, scope, policy)?,
        Expr::Bin(op, l, r) => {
            let (a, b) = (eval(l, scope, policy)?, eval(r, scope, policy)?);
            match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => {
                    if b == 0.0 {
                        return err("0で割っています");
                    }
                    a / b
                }
                BinOp::Pow => a.powf(b),
            }
        }
        Expr::Call(name, args) => {
            let Some(&(_, lo, hi)) = FUNCS.iter().find(|(n, ..)| n == name) else {
                return err(format!("関数 {name} は使用できません"));
            };
            if args.len() < lo || args.len() > hi {
                return err(format!("関数 {name} の引数の数が正しくありません"));
            }
            let a: Vec<f64> = args.iter().map(|x| eval(x, scope, policy)).collect::<Result<_, _>>()?;
            match name.as_str() {
                "sqrt" => {
                    if a[0] < 0.0 {
                        return err("負の数の平方根は計算できません");
                    }
                    a[0].sqrt()
                }
                "root" => {
                    let (n, x) = (a[0], a[1]);
                    if n == 0.0 {
                        return err("0乗根は計算できません");
                    }
                    if x < 0.0 {
                        // 奇数乗根のみ負数を許可
                        if n.fract() == 0.0 && (n as i64) % 2 != 0 {
                            -(-x).powf(1.0 / n)
                        } else {
                            return err("負の数の偶数乗根は計算できません");
                        }
                    } else {
                        x.powf(1.0 / n)
                    }
                }
                "abs" => a[0].abs(),
                "min" => a.iter().copied().fold(f64::INFINITY, f64::min),
                "max" => a.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                "sin" => a[0].to_radians().sin(),
                "cos" => a[0].to_radians().cos(),
                "tan" => a[0].to_radians().tan(),
                "log" | "ln" => {
                    if a[0] <= 0.0 {
                        return err("0以下の数の対数は計算できません");
                    }
                    if name == "log" { a[0].log10() } else { a[0].ln() }
                }
                "exp" => a[0].exp(),
                "round" => round_half_up(a[0], a.get(1).copied().unwrap_or(0.0).max(0.0) as u8),
                "floor" => a[0].floor(),
                "ceil" => a[0].ceil(),
                _ => unreachable!(),
            }
        }
        Expr::Cmp(..) => return err("比較式は数値として評価できません"),
    };
    if !v.is_finite() {
        return err("計算結果が有限の数になりません");
    }
    Ok(v)
}
