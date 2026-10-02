#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

impl CmpOp {
    pub fn holds(self, l: f64, r: f64) -> bool {
        match self {
            CmpOp::Lt => l < r,
            CmpOp::Le => l <= r,
            CmpOp::Gt => l > r,
            CmpOp::Ge => l >= r,
            CmpOp::Eq => l == r,
        }
    }

    /// 和文の計算書で用いる記号（≦ ≧）。
    pub fn symbol(self) -> &'static str {
        match self {
            CmpOp::Lt => "<",
            CmpOp::Le => "≦",
            CmpOp::Gt => ">",
            CmpOp::Ge => "≧",
            CmpOp::Eq => "=",
        }
    }

    /// 不成立時に表示する記号（NG表示用）。
    pub fn negated_symbol(self) -> &'static str {
        match self {
            CmpOp::Lt => "≧",
            CmpOp::Le => ">",
            CmpOp::Gt => "≦",
            CmpOp::Ge => "<",
            CmpOp::Eq => "≠",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// 値と入力時の表記（"8.000" など）。
    Num(f64, String),
    Var(String),
    Neg(Box<Expr>),
    /// 利用者が明示的に書いた括弧（表示でも保持する）。
    Paren(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
}

impl Expr {
    /// 式中で参照している変数名（重複なし、出現順）。関数名・定数は含まない。
    pub fn vars(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_vars(&mut out);
        out
    }

    fn collect_vars(&self, out: &mut Vec<String>) {
        match self {
            Expr::Num(..) => {}
            Expr::Var(n) => {
                if !crate::eval::is_constant(n) && !out.contains(n) {
                    out.push(n.clone());
                }
            }
            Expr::Neg(e) | Expr::Paren(e) => e.collect_vars(out),
            Expr::Bin(_, l, r) | Expr::Cmp(_, l, r) => {
                l.collect_vars(out);
                r.collect_vars(out);
            }
            Expr::Call(_, args) => args.iter().for_each(|a| a.collect_vars(out)),
        }
    }
}
