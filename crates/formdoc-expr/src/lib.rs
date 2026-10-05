//! formDoc の式エンジン。
//!
//! アプリ内のリアクティブ計算（formdoc-core）と、コードモード用のTypstプラグイン
//! （formdoc-typst-plugin）の両方がこのクレートを使う。GUIで作っても生Typstで書いても
//! 同じ計算・同じ表記になることを保証するため、計算と数式表記の生成はここだけで行う。

pub mod ast;
pub mod eval;
pub mod format;
pub mod parse;
pub mod render;

pub use ast::{CmpOp, Expr};
pub use eval::{EvalError, Rounding, Scope, VarValue, eval};
pub use format::{DEFAULT_GROUP, NumFormat, format_number, group_literal, round_half_up, unit_to_math, unit_to_text};
pub use parse::{ParseError, parse};
pub use render::{RenderOptions, name_math, var_math};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Parse(ParseError),
    Eval(EvalError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "式の書き方に誤りがあります（{e}）"),
            Error::Eval(e) => write!(f, "計算できません（{e}）"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Self {
        Error::Parse(e)
    }
}

impl From<EvalError> for Error {
    fn from(e: EvalError) -> Self {
        Error::Eval(e)
    }
}

/// 計算行の要求。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CalcRequest {
    pub expr: String,
    #[serde(default)]
    pub scope: Scope,
    /// 結果の小数桁数。
    #[serde(default)]
    pub digits: Option<u8>,
    #[serde(default)]
    pub unit: String,
    #[serde(default = "yes")]
    pub frac: bool,
    #[serde(default)]
    pub units_in_sub: bool,
    #[serde(default)]
    pub rounding: Rounding,
    /// 3桁区切りを入れる整数部の桁数（文書テンプレートの設定）。None なら計算結果は DEFAULT_GROUP 桁から区切り、
    /// 式に書いた数値は書いたとおりに表示する。Some(n) なら両方とも n 桁から区切る
    #[serde(default)]
    pub group: Option<u8>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalcOutput {
    /// 全精度の計算結果。
    pub value: f64,
    /// 表示桁で書式化した結果（"2.25"）。
    pub text: String,
    /// 記号式（Typst数式）。
    pub symbolic: String,
    /// 代入式（Typst数式）。式が単一の数値・変数のときは空。
    pub substituted: String,
    /// 結果（Typst数式、単位付き）。
    pub result: String,
    /// 単位（Typst数式）。
    pub unit: String,
    /// 参照した変数。
    pub vars: Vec<String>,
}

fn opts(frac: bool, units_in_sub: bool, rounding: Rounding, group: Option<u8>) -> RenderOptions {
    RenderOptions { frac, units_in_sub, rounding, group: group.unwrap_or(DEFAULT_GROUP), literal_group: group.unwrap_or(0) }
}

pub fn calc(req: &CalcRequest) -> Result<CalcOutput, Error> {
    let e = parse(&req.expr)?;
    if matches!(e, Expr::Cmp(..)) {
        return Err(EvalError { message: "計算行に比較式は書けません（照査を使ってください）".into() }.into());
    }
    let value = eval(&e, &req.scope, req.rounding)?;
    let o = opts(req.frac, req.units_in_sub, req.rounding, req.group);
    let text = format_number(value, NumFormat { digits: req.digits, group: o.group });
    let trivial = matches!(e, Expr::Num(..));
    let unit = if req.unit.is_empty() { String::new() } else { unit_to_math(&req.unit) };
    let num = render::number_math(&text);
    Ok(CalcOutput {
        value,
        symbolic: render::symbolic(&e, &req.scope, o),
        substituted: if trivial { String::new() } else { render::substituted(&e, &req.scope, o) },
        result: if unit.is_empty() { num } else { format!("{num} thin {unit}") },
        unit,
        text,
        vars: e.vars(),
    })
}

/// 照査（比較式）の要求。例: "b_t <= bt_0"
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckRequest {
    pub expr: String,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub digits: Option<u8>,
    #[serde(default)]
    pub unit: String,
    #[serde(default = "yes")]
    pub frac: bool,
    #[serde(default)]
    pub rounding: Rounding,
    /// 3桁区切りを入れる整数部の桁数（CalcRequest::group と同じ）
    #[serde(default)]
    pub group: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckSide {
    pub value: f64,
    pub text: String,
    pub symbolic: String,
    pub substituted: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckOutput {
    pub ok: bool,
    pub op: CmpOp,
    /// 成立時の記号（≦ 等）。
    pub symbol: String,
    /// 実際の大小関係を表す記号（NG時は反転記号）。
    pub shown_symbol: String,
    pub lhs: CheckSide,
    pub rhs: CheckSide,
    pub unit: String,
    pub vars: Vec<String>,
}

pub fn check(req: &CheckRequest) -> Result<CheckOutput, Error> {
    let e = parse(&req.expr)?;
    let Expr::Cmp(op, l, r) = &e else {
        return Err(EvalError { message: "照査式には比較演算子（<= など）が必要です".into() }.into());
    };
    let o = opts(req.frac, false, req.rounding, req.group);
    let side = |x: &Expr| -> Result<CheckSide, Error> {
        let value = eval(x, &req.scope, req.rounding)?;
        // 判定は表示値どうしで行う（紙面上の数値と判定結果を一致させる）
        let digits = req.digits.or_else(|| match x {
            Expr::Var(n) => req.scope.get(n).and_then(|v| v.digits),
            _ => None,
        });
        Ok(CheckSide {
            value,
            // 数値をそのまま書いた側（制限値 0.7 など）は書いたとおりに表示する（3桁区切りの設定があれば区切りだけ入れる）
            text: match x {
                Expr::Num(_, src) => group_literal(src, o.literal_group),
                _ => format_number(value, NumFormat { digits, group: o.group }),
            },
            symbolic: render::symbolic(x, &req.scope, o),
            substituted: if matches!(x, Expr::Num(..) | Expr::Var(_)) {
                String::new()
            } else {
                render::substituted(x, &req.scope, o)
            },
        })
    };
    let (lhs, rhs) = (side(l)?, side(r)?);
    let parse_shown = |s: &CheckSide| s.text.replace(',', "").parse::<f64>().unwrap_or(s.value);
    let ok = op.holds(parse_shown(&lhs), parse_shown(&rhs));
    Ok(CheckOutput {
        ok,
        op: *op,
        symbol: op.symbol().into(),
        shown_symbol: if ok { op.symbol() } else { op.negated_symbol() }.into(),
        lhs,
        rhs,
        unit: if req.unit.is_empty() { String::new() } else { unit_to_math(&req.unit) },
        vars: e.vars(),
    })
}

#[cfg(test)]
mod tests;
