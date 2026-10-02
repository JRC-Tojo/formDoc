//! GUI文書の評価（リアクティブ計算）。
//!
//! 計算書は上から順に読まれるため、変数は「使う前に定義する」規則にしている。
//! これにより依存関係は文書の並び順そのものになり、循環参照は起こり得ない。
//! 入力のたびに文書全体を先頭から再評価する（数百行でも数ミリ秒）。

use std::collections::HashMap;

use formdoc_expr::{CalcRequest, CheckRequest, Scope, VarValue};
use serde::Serialize;

use crate::model::{Block, Document, Grid};
use crate::template::Template;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Issue {
    pub block_id: Option<String>,
    pub field: Option<String>,
    pub severity: Severity,
    /// 種別（"calc", "check-ng", "lint-punctuation" …）。GUIの絞り込み用。
    pub code: String,
    pub message: String,
    /// ワンクリック修正: field の文字列を置換する内容
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fix {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VarInfo {
    pub name: String,
    pub block_id: String,
    pub value: f64,
    pub text: String,
    pub unit: String,
    pub digits: Option<u8>,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct BlockResult {
    /// "ok" / "error" / "ng"
    pub status: &'static str,
    /// 結果の表示（"= 2.25 kN/m"、"OK" など）
    pub summary: String,
    /// 記号説明ブロックで実際に並べる変数（自動選択の結果を含む）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<String>,
    /// 表示桁（単位既定値を反映済み）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digits: Option<u8>,
    /// sum ブロックの内訳ごとの桁
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub vars: Vec<VarInfo>,
    pub blocks: HashMap<String, BlockResult>,
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn var(&self, name: &str) -> Option<&VarInfo> {
        self.vars.iter().find(|v| v.name == name)
    }

    fn issue(&mut self, b: &Block, field: &str, severity: Severity, code: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            block_id: Some(b.id.clone()),
            field: (!field.is_empty()).then(|| field.to_string()),
            severity,
            code: code.into(),
            message: message.into(),
            fix: None,
        });
    }
}

/// 変数名の規則: 半角英字で始まり、英数字と添字区切り "_" のみ。
pub fn valid_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.ends_with('_')
        && !name.contains("__")
        && !formdoc_expr::eval::is_function(name)
        && name != "pi"
}

/// 本文中の {{name}} 参照を取り出す。
pub fn refs_in_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("{{") {
        let after = &rest[i + 2..];
        let Some(j) = after.find("}}") else { break };
        out.push(after[..j].trim().to_string());
        rest = &after[j + 2..];
    }
    out
}

struct Ctx<'a> {
    t: &'a Template,
    scope: Scope,
    defined_by: HashMap<String, String>,
    last_vars: Vec<String>,
    r: Report,
}

impl Ctx<'_> {
    fn digits_for(&self, b: &Block, unit: &str) -> Option<u8> {
        b.int("digits").map(|d| d.clamp(0, 10) as u8).or_else(|| self.t.default_digits(unit))
    }

    fn define(&mut self, b: &Block, field: &str, name: &str, v: VarValue) -> bool {
        if !valid_var_name(name) {
            self.r.issue(b, field, Severity::Error, "var-name",
                format!("変数名「{name}」は使えません（半角英字で始め、英数字と _ のみ。関数名は不可）"));
            return false;
        }
        if let Some(prev) = self.defined_by.get(name) {
            if prev != &b.id {
                self.r.issue(b, field, Severity::Error, "var-dup",
                    format!("変数「{name}」は既に定義されています。別の名前にしてください"));
                return false;
            }
        }
        self.defined_by.insert(name.to_string(), b.id.clone());
        self.r.vars.push(VarInfo {
            name: name.to_string(),
            block_id: b.id.clone(),
            value: v.value,
            text: formdoc_expr::format_number(v.value, formdoc_expr::NumFormat { digits: v.digits, group: true }),
            unit: v.unit.clone(),
            digits: v.digits,
            desc: v.desc.clone(),
        });
        self.scope.insert(name.to_string(), v);
        true
    }

    fn check_refs(&mut self, b: &Block, field: &str, text: &str) {
        for name in refs_in_text(text) {
            if !self.scope.contains_key(&name) {
                self.r.issue(b, field, Severity::Error, "var-undef",
                    format!("{{{{{name}}}}} の変数「{name}」は、この位置より前で定義されていません"));
            }
        }
    }

    /// 数値・変数名・式のいずれかを数値にする（図の寸法など）。
    fn value_of(&mut self, b: &Block, field: &str, src: &str) -> Option<f64> {
        if src.trim().is_empty() {
            return None;
        }
        let req = CalcRequest { expr: src.into(), scope: self.scope.clone(), rounding: self.t.rounding, ..Default::default() };
        match formdoc_expr::calc(&req) {
            Ok(o) => Some(o.value),
            Err(e) => {
                self.r.issue(b, field, Severity::Error, "calc", e.to_string());
                None
            }
        }
    }

    fn calc(&mut self, b: &Block, field: &str, expr: &str, unit: &str, digits: Option<u8>) -> Option<formdoc_expr::CalcOutput> {
        if expr.trim().is_empty() {
            self.r.issue(b, field, Severity::Error, "required", "式を入力してください");
            return None;
        }
        let req = CalcRequest {
            expr: expr.into(),
            scope: self.scope.clone(),
            digits,
            unit: unit.into(),
            frac: b.bool("frac", true),
            units_in_sub: b.bool("units_in_sub", false),
            rounding: self.t.rounding,
        };
        match formdoc_expr::calc(&req) {
            Ok(o) => Some(o),
            Err(e) => {
                self.r.issue(b, field, Severity::Error, "calc", e.to_string());
                None
            }
        }
    }

    fn block(&mut self, b: &Block) {
        let mut res = BlockResult { status: "ok", ..Default::default() };
        let errors_before = self.r.issues.iter().filter(|i| i.severity == Severity::Error).count();
        if !self.t.blocks.iter().any(|k| k == &b.kind) {
            self.r.issue(b, "", Severity::Error, "block-kind",
                format!("この文書の型（{}）では部品「{}」は使えません", self.t.name, b.kind));
        }
        match b.kind.as_str() {
            "heading" => {
                let level = b.int("level").unwrap_or(2);
                if level < 1 || level > self.t.max_heading_level as i64 {
                    self.r.issue(b, "level", Severity::Error, "heading-level", format!("見出しの階層は1〜{}です", self.t.max_heading_level));
                }
                if b.str("text").trim().is_empty() {
                    self.r.issue(b, "text", Severity::Error, "required", "見出し文を入力してください");
                }
            }
            "paragraph" => {
                let t = b.str("text").to_string();
                self.check_refs(b, "text", &t);
            }
            "kijun" => {}
            "vdef" => {
                let name = b.str("name").trim().to_string();
                let unit = b.str("unit").trim().to_string();
                let digits = self.digits_for(b, &unit);
                res.digits = digits;
                match b.num("value") {
                    Some(value) => {
                        let v = VarValue { value, digits, unit: unit.clone(), display: b.opt_str("display").map(str::to_string), desc: b.str("desc").into() };
                        if self.define(b, "name", &name, v) {
                            res.summary = format!("{name} = {}", self.r.vars.last().map(|v| v.text.clone()).unwrap_or_default());
                            res.value = Some(value);
                        }
                    }
                    None => self.r.issue(b, "value", Severity::Error, "required", "値を数値で入力してください"),
                }
            }
            "calc" => {
                let name = b.str("name").trim().to_string();
                let unit = b.str("unit").trim().to_string();
                let digits = self.digits_for(b, &unit);
                res.digits = digits;
                if let Some(o) = self.calc(b, "expr", b.str("expr"), &unit, digits) {
                    self.last_vars = o.vars.clone();
                    self.last_vars.insert(0, name.clone());
                    res.summary = format!("{name} = {}", o.text);
                    res.value = Some(o.value);
                    let v = VarValue { value: o.value, digits, unit, display: b.opt_str("display").map(str::to_string), desc: b.str("desc").into() };
                    self.define(b, "name", &name, v);
                }
            }
            "sum" => {
                let unit = b.str("unit").trim().to_string();
                let digits = self.digits_for(b, &unit);
                res.digits = digits;
                let mut names = Vec::new();
                for (i, item) in b.arr("items").iter().enumerate() {
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                    let name = if name.is_empty() { format!("{}_{}", b.str("name").trim(), i + 1) } else { name };
                    let expr = item.get("expr").and_then(|v| v.as_str()).unwrap_or("");
                    let field = format!("items.{i}.expr");
                    if let Some(o) = self.calc(b, &field, expr, &unit, digits) {
                        let desc = item.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if self.define(b, &format!("items.{i}.name"), &name, VarValue { value: o.value, digits, unit: unit.clone(), display: None, desc }) {
                            names.push(name);
                        }
                    }
                }
                let total = b.str("name").trim().to_string();
                if names.is_empty() {
                    self.r.issue(b, "items", Severity::Error, "required", "内訳を1行以上入力してください");
                } else if let Some(o) = self.calc(b, "name", &names.join(" + "), &unit, digits) {
                    res.summary = format!("Σ{total} = {}", o.text);
                    res.value = Some(o.value);
                    let v = VarValue { value: o.value, digits, unit, display: None, desc: b.str("desc").into() };
                    self.define(b, "name", &total, v);
                }
                res.vars = names;
            }
            "check" => {
                let req = CheckRequest {
                    expr: b.str("expr").into(),
                    scope: self.scope.clone(),
                    digits: b.int("digits").map(|d| d.clamp(0, 10) as u8),
                    unit: b.str("unit").into(),
                    frac: true,
                    rounding: self.t.rounding,
                };
                match formdoc_expr::check(&req) {
                    Ok(o) => {
                        self.last_vars = o.vars.clone();
                        res.summary = format!("{} {} {} → {}", o.lhs.text, o.shown_symbol, o.rhs.text, if o.ok { "OK" } else { "NG" });
                        if !o.ok {
                            res.status = "ng";
                            self.r.issue(b, "expr", Severity::Warning, "check-ng",
                                format!("照査を満たしていません: {} {} {}", o.lhs.text, o.shown_symbol, o.rhs.text));
                        }
                    }
                    Err(e) => self.r.issue(b, "expr", Severity::Error, "calc", e.to_string()),
                }
            }
            "where" => {
                let mut vars: Vec<String> = b.arr("vars").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
                if vars.is_empty() {
                    vars = self.last_vars.clone();
                }
                for v in &vars {
                    match self.scope.get(v) {
                        None => self.r.issue(b, "vars", Severity::Error, "var-undef", format!("変数「{v}」はこの位置より前で定義されていません")),
                        Some(val) if val.desc.trim().is_empty() => self.r.issue(b, "vars", Severity::Warning, "where-desc",
                            format!("変数「{v}」に説明がありません（「ここに，」が空欄になります）")),
                        _ => {}
                    }
                }
                if vars.is_empty() {
                    self.r.issue(b, "vars", Severity::Warning, "where-empty", "説明する変数がありません");
                }
                res.vars = vars;
            }
            "table" => {
                let grid: Grid = b.props.get("data").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                for (ri, row) in grid.rows.iter().enumerate() {
                    for (ci, c) in row.iter().enumerate() {
                        let t = c.text.clone();
                        self.check_refs(b, &format!("data.{ri}.{ci}"), &t);
                    }
                }
            }
            "fig-beam" => {
                self.value_of(b, "span", b.str("span"));
                for key in ["loads", "eta"] {
                    for part in b.str(key).split([',', '、', '，']).filter(|s| !s.trim().is_empty()) {
                        self.value_of(b, key, part);
                    }
                }
            }
            "fig-isection" => {
                for key in ["H", "B", "tw", "tf"] {
                    if self.value_of(b, key, b.str(key)).is_none() && b.str(key).trim().is_empty() {
                        self.r.issue(b, key, Severity::Error, "required", format!("{key} を入力してください"));
                    }
                }
                let note = b.str("note").to_string();
                self.check_refs(b, "note", &note);
            }
            "fig-shapes" => {
                for (i, sh) in b.arr("shapes").iter().enumerate() {
                    for k in ["x1", "y1", "x2", "y2"] {
                        if let Some(s) = sh.get(k).and_then(|v| v.as_str()) {
                            self.value_of(b, &format!("shapes.{i}.{k}"), s);
                        }
                    }
                }
            }
            "image" => {
                if b.str("file").is_empty() {
                    self.r.issue(b, "file", Severity::Error, "required", "画像ファイルを選んでください");
                }
            }
            "pagebreak" | "typst" => {}
            other => self.r.issue(b, "", Severity::Error, "block-kind", format!("不明な部品です: {other}")),
        }
        let errors_after = self.r.issues.iter().filter(|i| i.severity == Severity::Error && i.block_id.as_deref() == Some(&b.id)).count();
        if errors_after > 0 && self.r.issues.iter().filter(|i| i.severity == Severity::Error).count() > errors_before {
            res.status = "error";
        }
        self.r.blocks.insert(b.id.clone(), res);
    }
}

pub fn evaluate(doc: &Document, t: &Template) -> Report {
    let mut cx = Ctx { t, scope: Scope::new(), defined_by: HashMap::new(), last_vars: Vec::new(), r: Report::default() };
    let mut ids = std::collections::HashSet::new();
    for b in &doc.blocks {
        if !ids.insert(b.id.clone()) {
            cx.r.issue(b, "", Severity::Error, "block-id", "ブロックIDが重複しています（内部エラー）");
        }
        cx.block(b);
    }
    if doc.library != formdoc_library::version() && !doc.library.is_empty() {
        cx.r.issues.push(Issue {
            block_id: None,
            field: None,
            severity: Severity::Warning,
            code: "library-version".into(),
            message: format!(
                "この文書はライブラリ {} で作成されています（現在 {}）。体裁や計算結果が変わっていないか確認してください",
                doc.library,
                formdoc_library::version()
            ),
            fix: None,
        });
    }
    cx.r
}
