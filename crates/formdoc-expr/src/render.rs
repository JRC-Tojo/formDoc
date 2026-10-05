//! 構文木をTypst数式マークアップに変換する。
//! 「記号式」（L_b, γ_f …）と「代入式」（8.000, 1.05 …）の2通りを生成する。

use crate::ast::{BinOp, Expr};
use crate::eval::{Rounding, Scope};
use crate::format::{DEFAULT_GROUP, NumFormat, format_number, group_literal, unit_to_math};

const GREEK: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda",
    "mu", "nu", "xi", "omicron", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega",
    "Gamma", "Delta", "Theta", "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi", "Omega",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// 除算を分数で表示する（false なら a/b のスラッシュ表示）。
    pub frac: bool,
    /// 代入式で変数値の後ろに単位を付ける（例: 4.50 kN/m / 2）。
    pub units_in_sub: bool,
    pub rounding: Rounding,
    /// 3桁区切りを入れる整数部の桁数（format::NumFormat::group と同じ）
    pub group: u8,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { frac: true, units_in_sub: false, rounding: Rounding::Display, group: DEFAULT_GROUP }
    }
}

fn is_ident_part(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn piece_math(s: &str) -> String {
    if GREEK.contains(&s) {
        s.to_string()
    } else if s.chars().count() == 1 || s.chars().all(|c| c.is_ascii_digit()) {
        s.to_string()
    } else if is_ident_part(s) {
        format!("italic(\"{s}\")")
    } else {
        format!("\"{s}\"")
    }
}

/// 変数名からTypst数式表記を作る。`sigma_ck` → `sigma_(italic("ck"))`、`L_b` → `L_(b)`。
pub fn name_math(name: &str) -> String {
    match name.split_once('_') {
        Some((base, sub)) if !base.is_empty() && !sub.is_empty() => {
            let sub = sub.replace('_', ",");
            let sub = if sub.contains(',') { format!("\"{sub}\"") } else { piece_math(&sub) };
            format!("{}_({sub})", piece_math(base))
        }
        _ => piece_math(name),
    }
}

/// 変数の表示（display指定があればそれを使う）。
pub fn var_math(name: &str, scope: &Scope) -> String {
    match scope.get(name).and_then(|v| v.display.as_deref()) {
        Some(d) if !d.trim().is_empty() => d.to_string(),
        _ => name_math(name),
    }
}

/// 数値を数式中に置ける形にする（桁区切りのカンマは引数区切りと衝突するため文字列化）。
pub fn number_math(text: &str) -> String {
    if text.contains(',') {
        format!("\"{text}\"")
    } else {
        text.to_string()
    }
}

fn strip_paren(e: &Expr) -> &Expr {
    match e {
        Expr::Paren(inner) => strip_paren(inner),
        _ => e,
    }
}

fn is_numberish(e: &Expr, substituted: bool) -> bool {
    match e {
        Expr::Num(..) => true,
        Expr::Var(_) => substituted,
        Expr::Bin(BinOp::Pow, b, _) => is_numberish(b, substituted),
        _ => false,
    }
}

struct Ctx<'a> {
    scope: &'a Scope,
    opts: RenderOptions,
    substituted: bool,
}

impl Ctx<'_> {
    fn go(&self, e: &Expr) -> String {
        match e {
            Expr::Num(_, text) => number_math(&group_literal(text, self.opts.group)),
            Expr::Var(n) if n == "pi" || n == "π" => "pi".into(),
            Expr::Var(n) => {
                if !self.substituted {
                    return var_math(n, self.scope);
                }
                let Some(v) = self.scope.get(n) else { return var_math(n, self.scope) };
                let text = format_number(v.effective(self.opts.rounding), NumFormat { digits: v.digits, group: self.opts.group });
                let mut s = number_math(&text);
                if self.opts.units_in_sub && !v.unit.is_empty() {
                    s = format!("{s} thin {}", unit_to_math(&v.unit));
                }
                if text.starts_with('-') { format!("({s})") } else { s }
            }
            Expr::Neg(x) => format!("-{}", self.go(x)),
            Expr::Paren(x) => format!("({})", self.go(x)),
            Expr::Bin(op, l, r) => match op {
                BinOp::Add => format!("{} + {}", self.go(l), self.go(r)),
                BinOp::Sub => format!("{} - {}", self.go(l), self.go(r)),
                BinOp::Mul => {
                    let sym = if self.substituted || (is_numberish(l, false) && is_numberish(r, false)) {
                        "times"
                    } else {
                        "dot"
                    };
                    format!("{} {sym} {}", self.go(l), self.go(r))
                }
                BinOp::Div => {
                    if self.opts.frac {
                        format!("frac({}, {})", self.go(strip_paren(l)), self.go(strip_paren(r)))
                    } else {
                        format!("{} slash {}", self.go(l), self.go(r))
                    }
                }
                BinOp::Pow => {
                    let base = self.go(l);
                    // 代入後の値（単位付きや負数）は括弧で囲む
                    let base = if self.substituted && matches!(**l, Expr::Var(_)) && base.contains(' ') {
                        format!("({base})")
                    } else {
                        base
                    };
                    format!("{base}^({})", self.go(strip_paren(r)))
                }
            },
            Expr::Call(name, args) => {
                let a: Vec<String> = args.iter().map(|x| self.go(strip_paren(x))).collect();
                match name.as_str() {
                    "sqrt" => format!("sqrt({})", a[0]),
                    "root" => format!("root({}, {})", a[0], a[1]),
                    "abs" => format!("abs({})", a[0]),
                    "floor" => format!("floor({})", a[0]),
                    "ceil" => format!("ceil({})", a[0]),
                    "min" | "max" | "sin" | "cos" | "tan" | "log" | "ln" | "exp" => {
                        format!("{name}({})", a.join(", "))
                    }
                    _ => format!("\"{name}\"({})", a.join(", ")),
                }
            }
            Expr::Cmp(op, l, r) => format!("{} {} {}", self.go(l), op.symbol(), self.go(r)),
        }
    }
}

/// 記号式（変数名のまま）。
pub fn symbolic(e: &Expr, scope: &Scope, opts: RenderOptions) -> String {
    Ctx { scope, opts, substituted: false }.go(e)
}

/// 代入式（変数を数値に置き換えたもの）。
pub fn substituted(e: &Expr, scope: &Scope, opts: RenderOptions) -> String {
    Ctx { scope, opts, substituted: true }.go(e)
}
