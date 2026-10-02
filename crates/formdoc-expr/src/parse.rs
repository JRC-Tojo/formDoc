//! 数式文字列の字句解析・構文解析（Pratt法）。
//!
//! 受け付ける構文:
//! - 数値 `8.000`, `2.0e5`（入力時の表記を保持し、表示に使う）
//! - 変数 `L_b`, `gamma_f`, `sigma_ck`
//! - 演算子 `+ - * / ^`、括弧、関数呼び出し `sqrt(x)`, `root(3, x)`, `min(a, b)` …
//! - 比較 `<= < >= > ==`（照査用。式全体の最上位でのみ使う想定）

use crate::ast::{BinOp, CmpOp, Expr};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    /// 入力中の文字位置（0始まり、char単位）。
    pub pos: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}文字目: {}", self.pos + 1, self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64, String),
    Ident(String),
    Op(char),
    Cmp(CmpOp),
    LParen,
    RParen,
    Comma,
    Eof,
}

fn lex(src: &str) -> Result<Vec<(Tok, usize)>, ParseError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // 指数表記 1.2e5 / 3E-4
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    i = j;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let text: String = chars[start..i].iter().collect();
            let value = text.parse::<f64>().map_err(|_| ParseError {
                message: format!("数値として解釈できません: {text}"),
                pos: start,
            })?;
            out.push((Tok::Num(value, text), start));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push((Tok::Ident(chars[start..i].iter().collect()), start));
            continue;
        }
        let next = chars.get(i + 1).copied();
        let (tok, len) = match (c, next) {
            ('<', Some('=')) => (Tok::Cmp(CmpOp::Le), 2),
            ('>', Some('=')) => (Tok::Cmp(CmpOp::Ge), 2),
            ('=', Some('=')) => (Tok::Cmp(CmpOp::Eq), 2),
            ('<', _) => (Tok::Cmp(CmpOp::Lt), 1),
            ('>', _) => (Tok::Cmp(CmpOp::Gt), 1),
            ('≦' | '≤', _) => (Tok::Cmp(CmpOp::Le), 1),
            ('≧' | '≥', _) => (Tok::Cmp(CmpOp::Ge), 1),
            ('*', Some('*')) => (Tok::Op('^'), 2),
            ('+' | '-' | '*' | '/' | '^', _) => (Tok::Op(c), 1),
            ('×' | '・' | '·', _) => (Tok::Op('*'), 1),
            ('÷', _) => (Tok::Op('/'), 1),
            ('(', _) => (Tok::LParen, 1),
            (')', _) => (Tok::RParen, 1),
            (',', _) => (Tok::Comma, 1),
            _ => {
                return Err(ParseError { message: format!("使用できない文字です: '{c}'"), pos: start });
            }
        };
        out.push((tok, start));
        i += len;
    }
    out.push((Tok::Eof, chars.len()));
    Ok(out)
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].0
    }

    fn pos(&self) -> usize {
        self.toks[self.i].1
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.i].0.clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError { message: message.into(), pos: self.pos() })
    }

    fn expect(&mut self, tok: Tok, what: &str) -> Result<(), ParseError> {
        if *self.peek() == tok {
            self.bump();
            Ok(())
        } else {
            self.err(format!("{what} が必要です"))
        }
    }

    /// 比較は最上位で1回だけ許可する。
    fn top(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.expr(0)?;
        if let Tok::Cmp(op) = *self.peek() {
            self.bump();
            let rhs = self.expr(0)?;
            return Ok(Expr::Cmp(op, Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.prefix()?;
        loop {
            let op = match self.peek() {
                Tok::Op(c) => *c,
                // 暗黙の乗算: `2 L`, `2(a+b)`, `w L` は許可しない（曖昧さを避け、記述を統一するため）。
                _ => break,
            };
            let (l_bp, r_bp, bin) = match op {
                '+' => (1, 2, BinOp::Add),
                '-' => (1, 2, BinOp::Sub),
                '*' => (3, 4, BinOp::Mul),
                '/' => (3, 4, BinOp::Div),
                // べき乗は右結合、単項マイナスより強い: -x^2 = -(x^2)
                '^' => (8, 7, BinOp::Pow),
                _ => unreachable!(),
            };
            if l_bp < min_bp {
                break;
            }
            self.bump();
            let rhs = self.expr(r_bp)?;
            lhs = Expr::Bin(bin, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<Expr, ParseError> {
        let pos = self.pos();
        match self.bump() {
            Tok::Num(v, text) => Ok(Expr::Num(v, text)),
            Tok::Op('-') => Ok(Expr::Neg(Box::new(self.expr(5)?))),
            Tok::Op('+') => self.expr(5),
            Tok::LParen => {
                let inner = self.expr(0)?;
                self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                Ok(Expr::Paren(Box::new(inner)))
            }
            Tok::Ident(name) => {
                if *self.peek() == Tok::LParen {
                    self.bump();
                    let mut args = Vec::new();
                    if *self.peek() != Tok::RParen {
                        loop {
                            args.push(self.expr(0)?);
                            if *self.peek() == Tok::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Tok::Eof => Err(ParseError { message: "式が途中で終わっています".into(), pos }),
            t => Err(ParseError { message: format!("ここに {} は置けません", tok_name(&t)), pos }),
        }
    }
}

fn tok_name(t: &Tok) -> String {
    match t {
        Tok::Num(_, s) => s.clone(),
        Tok::Ident(s) => s.clone(),
        Tok::Op(c) => c.to_string(),
        Tok::Cmp(_) => "比較演算子".into(),
        Tok::LParen => "(".into(),
        Tok::RParen => ")".into(),
        Tok::Comma => ",".into(),
        Tok::Eof => "式の終わり".into(),
    }
}

/// 数式文字列を構文木に変換する。
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let toks = lex(src)?;
    let mut p = Parser { toks, i: 0 };
    let e = p.top()?;
    if *p.peek() != Tok::Eof {
        return p.err(format!("余分な記述があります: {}", tok_name(p.peek())));
    }
    Ok(e)
}
