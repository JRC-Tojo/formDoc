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
    /// 使える範囲の持ち主（見出し・テンプレートのブロックID）。None は文書全体
    pub scope: Option<String>,
    /// 「グローバル変数として定義」
    pub global: bool,
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
    /// 汎用図形の評価値。図形ごとに、繰り返しの各回（ix が先、iy が後）の座標と文字。
    /// コード生成と描画エディタの両方がこれを使う（式の評価を1か所にするため）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<Vec<ShapeValues>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct ShapeValues {
    pub x1: Option<f64>,
    pub y1: Option<f64>,
    pub x2: Option<f64>,
    pub y2: Option<f64>,
    /// 多角形の頂点
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pts: Vec<(Option<f64>, Option<f64>)>,
    /// 文字（{{変数}}・{{式}}・{{式:桁}} を値にしたもの）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// 図形1つあたりの繰り返し回数の上限（縦横それぞれ）
pub const MAX_REPEAT: usize = 200;

/// 多角形の頂点リスト「x, y; x, y; …」を (x式, y式) に分ける。式の中のカンマ（関数の引数）は括弧の深さで区別する。
pub fn split_points(s: &str) -> Vec<(String, String)> {
    s.split(';')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let mut depth = 0i32;
            for (i, c) in p.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    ',' if depth == 0 => return (p[..i].trim().to_string(), p[i + 1..].trim().to_string()),
                    _ => {}
                }
            }
            (p.trim().to_string(), String::new())
        })
        .collect()
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
        // 図形の繰り返しの番号
        && name != "ix"
        && name != "iy"
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

/// 変数の有効範囲。文書全体（根）・見出しの節・テンプレート（group）ごとに1つ。
/// 変数は、定義した範囲の中で、定義より後ろからだけ使える（「グローバル変数として定義」なら文書全体）。
struct Frame {
    /// 範囲の持ち主のブロックID（根は None）
    owner: Option<String>,
    /// 見出しの階層（group と根は None）
    level: Option<i64>,
    /// この範囲で定義した変数
    vars: Vec<String>,
}

struct Ctx<'a> {
    t: &'a Template,
    /// いま見えている変数
    scope: Scope,
    /// いま見えている変数 → 定義したブロック
    visible_by: HashMap<String, String>,
    frames: Vec<Frame>,
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
        if let Some(prev) = self.visible_by.get(name) {
            if prev != &b.id {
                self.r.issue(b, field, Severity::Error, "var-dup",
                    format!("変数「{name}」は、この位置から使える変数として既に定義されています。別の名前にしてください"));
                return false;
            }
        }
        let global = b.bool("global", false);
        let target = if global { 0 } else { self.frames.len() - 1 };
        self.frames[target].vars.push(name.to_string());
        self.visible_by.insert(name.to_string(), b.id.clone());
        self.r.vars.push(VarInfo {
            scope: self.frames[target].owner.clone(),
            global,
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
                    format!("{{{{{name}}}}} の変数「{name}」は、この位置より前で定義されていないか、使える範囲（同じ見出し・テンプレートの中）の外です{}", self.scope_hint(&name)));
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
                let hint = self.scope_hint(src);
                self.r.issue(b, field, Severity::Error, "calc", format!("{e}{hint}"));
                None
            }
        }
    }

    /// 式の中に「定義はあるが、この位置からは使えない」変数があれば、その案内を返す。
    fn scope_hint(&self, expr: &str) -> String {
        let names = formdoc_expr::parse(expr).map(|e| e.vars()).unwrap_or_default();
        let out: Vec<String> = names
            .iter()
            .filter(|n| !self.scope.contains_key(*n) && self.r.vars.iter().any(|v| &v.name == *n))
            .map(|n| format!("「{n}」は別の節・テンプレートのローカル変数です（使うには、定義側で「グローバル変数として定義」にチェック）"))
            .collect();
        if out.is_empty() { String::new() } else { format!("。{}", out.join("。")) }
    }

    /// 範囲を閉じ、その中で定義した変数を見えなくする。
    fn pop_frame(&mut self) {
        if self.frames.len() <= 1 {
            return;
        }
        let f = self.frames.pop().unwrap();
        for n in f.vars {
            self.scope.remove(&n);
            self.visible_by.remove(&n);
        }
    }

    /// 部品の並び（文書、またはテンプレートの中身）を上から評価する。
    fn walk(&mut self, blocks: &[Block]) {
        let base = self.frames.len();
        for b in blocks {
            if b.kind == "heading" {
                // 同じか上の階層の見出しが来たら、前の節を閉じる
                let level = b.int("level").unwrap_or(2);
                while self.frames.len() > base && self.frames.last().and_then(|f| f.level).is_some_and(|l| l >= level) {
                    self.pop_frame();
                }
                self.block(b);
                self.frames.push(Frame { owner: Some(b.id.clone()), level: Some(level), vars: vec![] });
            } else if b.kind == "group" {
                self.group(b);
            } else {
                self.block(b);
            }
        }
        while self.frames.len() > base {
            self.pop_frame();
        }
    }

    /// テンプレートのまとまり。中の変数は外から見えない。props.exports の変数だけを外（親の範囲）に公開する。
    fn group(&mut self, b: &Block) {
        self.r.blocks.insert(b.id.clone(), BlockResult { status: "ok", ..Default::default() });
        self.frames.push(Frame { owner: Some(b.id.clone()), level: None, vars: vec![] });
        let base = self.frames.len();
        self.walk(&b.children);
        debug_assert_eq!(self.frames.len(), base);
        let exports: Vec<String> = b.arr("exports").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        let found: Vec<(String, Option<VarValue>)> = exports.iter().map(|n| (n.clone(), self.scope.get(n).cloned())).collect();
        self.pop_frame();
        for (name, v) in found {
            let Some(v) = v else {
                self.r.issue(b, "exports", Severity::Error, "export-undef", format!("公開する変数「{name}」がテンプレートの中で定義されていません"));
                continue;
            };
            if self.visible_by.contains_key(&name) {
                self.r.issue(b, "exports", Severity::Error, "var-dup",
                    format!("公開する変数「{name}」は、この位置から使える変数として既に定義されています。名前を変えてください"));
                continue;
            }
            let top = self.frames.len() - 1;
            self.frames[top].vars.push(name.clone());
            self.visible_by.insert(name.clone(), b.id.clone());
            self.scope.insert(name.clone(), v);
            let owner = self.frames[top].owner.clone();
            if let Some(info) = self.r.vars.iter_mut().rev().find(|x| x.name == name && !x.global) {
                info.scope = owner;
            }
        }
    }

    /// value_of と同じだが、エラーを報告しない（繰り返しの2回目以降）。
    fn value_quiet(&self, src: &str) -> Option<f64> {
        if src.trim().is_empty() {
            return None;
        }
        let req = CalcRequest { expr: src.into(), scope: self.scope.clone(), rounding: self.t.rounding, ..Default::default() };
        formdoc_expr::calc(&req).ok().map(|o| o.value)
    }

    /// 図形の文字の {{…}} を値にする。変数名ならその変数の表示（桁は変数の設定）、
    /// 式なら評価して {{式:桁}} の桁（省略時は 3 桁）で表示する。
    fn shape_label(&mut self, b: &Block, i: usize, label: &str, report: bool) -> String {
        let mut out = String::new();
        let mut rest = label;
        while let Some(s) = rest.find("{{") {
            let after = &rest[s + 2..];
            let Some(e) = after.find("}}") else { break };
            out.push_str(&rest[..s]);
            let inner = after[..e].trim();
            let (expr, digits) = match inner.rsplit_once(':') {
                Some((x, d)) if d.trim().parse::<u8>().is_ok() => (x.trim(), d.trim().parse::<u8>().ok()),
                _ => (inner, None),
            };
            let is_var = valid_var_name(expr) && self.scope.contains_key(expr);
            if is_var && digits.is_none() {
                let text = self.r.vars.iter().rev().find(|v| v.name == expr).map(|v| v.text.clone());
                out.push_str(&text.unwrap_or_default());
            } else {
                let v = if report { self.value_of(b, &format!("shapes.{i}.label"), expr) } else { self.value_quiet(expr) };
                if let Some(v) = v {
                    out.push_str(&formdoc_expr::format_number(v, formdoc_expr::NumFormat { digits: Some(digits.unwrap_or(3)), group: true }));
                }
            }
            rest = &after[e + 2..];
        }
        out.push_str(rest);
        out
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
                let hint = self.scope_hint(expr);
                self.r.issue(b, field, Severity::Error, "calc", format!("{e}{hint}"));
                None
            }
        }
    }

    fn block(&mut self, b: &Block) {
        let mut res = BlockResult { status: "ok", ..Default::default() };
        let errors_before = self.r.issues.iter().filter(|i| i.severity == Severity::Error).count();
        if b.kind != "group" && !self.t.blocks.iter().any(|k| k == &b.kind) {
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
                    Err(e) => {
                        let hint = self.scope_hint(b.str("expr"));
                        self.r.issue(b, "expr", Severity::Error, "calc", format!("{e}{hint}"))
                    }
                }
            }
            "where" => {
                let mut vars: Vec<String> = b.arr("vars").iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
                if vars.is_empty() {
                    vars = self.last_vars.clone();
                }
                for v in &vars {
                    match self.scope.get(v) {
                        None => {
                            let hint = self.scope_hint(v);
                            self.r.issue(b, "vars", Severity::Error, "var-undef", format!("変数「{v}」はこの位置より前で定義されていないか、使える範囲の外です{hint}"))
                        }
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
            "fig-shapes" => {
                for (i, sh) in b.arr("shapes").iter().enumerate() {
                    let get = |k: &str| sh.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    // 繰り返し（回数・間隔）。回数は 0〜200 に丸める
                    let count = |cx: &mut Self, key: &str| -> usize {
                        let src = get(key);
                        if src.trim().is_empty() {
                            return 1;
                        }
                        match cx.value_of(b, &format!("shapes.{i}.{key}"), &src) {
                            Some(v) if v >= 0.0 => (v.round() as usize).min(MAX_REPEAT),
                            Some(_) => {
                                cx.r.issue(b, &format!("shapes.{i}.{key}"), Severity::Error, "shape-repeat", "繰り返し回数は 0 以上にしてください");
                                1
                            }
                            None => 1,
                        }
                    };
                    let nx = count(self, "nx");
                    let ny = count(self, "ny");
                    let dx = if get("dx").trim().is_empty() { 0.0 } else { self.value_of(b, &format!("shapes.{i}.dx"), &get("dx")).unwrap_or(0.0) };
                    let dy = if get("dy").trim().is_empty() { 0.0 } else { self.value_of(b, &format!("shapes.{i}.dy"), &get("dy")).unwrap_or(0.0) };
                    let kind = get("kind");
                    let kind = if kind.is_empty() { "line".to_string() } else { kind };
                    let pts = split_points(&get("pts"));
                    if kind == "polygon" && pts.len() < 2 {
                        self.r.issue(b, &format!("shapes.{i}.pts"), Severity::Error, "required", "多角形の頂点を2点以上入力してください（x, y; x, y; …）");
                    }
                    let mut instances = Vec::new();
                    for iy in 0..ny {
                        for ix in 0..nx {
                            // 繰り返しの番号（0始まり）を ix, iy として式で使えるようにする
                            self.scope.insert("ix".into(), VarValue { value: ix as f64, digits: Some(0), ..Default::default() });
                            self.scope.insert("iy".into(), VarValue { value: iy as f64, digits: Some(0), ..Default::default() });
                            // エラーは最初の1つだけ報告する（同じ式の誤りが繰り返しの数だけ並ばないように）
                            let first = ix == 0 && iy == 0;
                            let ev = |cx: &mut Self, key: &str, src: &str| -> Option<f64> {
                                if first { cx.value_of(b, &format!("shapes.{i}.{key}"), src) } else { cx.value_quiet(src) }
                            };
                            let (ox, oy) = (ix as f64 * dx, iy as f64 * dy);
                            let mut out = ShapeValues::default();
                            if kind == "polygon" {
                                for (x, y) in &pts {
                                    let xv = ev(self, "pts", x).map(|v| v + ox);
                                    let yv = ev(self, "pts", y).map(|v| v + oy);
                                    out.pts.push((xv, yv));
                                }
                            } else {
                                out.x1 = ev(self, "x1", &get("x1")).map(|v| v + ox);
                                out.y1 = ev(self, "y1", &get("y1")).map(|v| v + oy);
                                match kind.as_str() {
                                    "text" => {}
                                    // 半径は平行移動しない
                                    "circle" => out.x2 = ev(self, "x2", &get("x2")),
                                    _ => {
                                        out.x2 = ev(self, "x2", &get("x2")).map(|v| v + ox);
                                        out.y2 = ev(self, "y2", &get("y2")).map(|v| v + oy);
                                    }
                                }
                            }
                            let label = get("label");
                            if !label.is_empty() {
                                out.label = Some(self.shape_label(b, i, &label, first));
                            }
                            instances.push(out);
                        }
                    }
                    self.scope.remove("ix");
                    self.scope.remove("iy");
                    res.shapes.push(instances);
                }
            }
            "image" => {
                if b.str("file").is_empty() {
                    self.r.issue(b, "file", Severity::Error, "required", "画像ファイルを選んでください");
                }
            }
            "pagebreak" => {}
            "typst" => {
                // コードの中の vdef / vcalc を変数として定義する（後ろの部品から使えるように）
                for d in crate::code::code_defs(b.str("code")) {
                    match d {
                        crate::code::CodeDef::Value { name, value, unit, digits, desc } => {
                            let digits = digits.or_else(|| self.t.default_digits(&unit));
                            self.define(b, "code", &name, VarValue { value, digits, unit, display: None, desc });
                        }
                        crate::code::CodeDef::Calc { name, expr, unit, digits, desc } => {
                            let digits = digits.or_else(|| self.t.default_digits(&unit));
                            if let Some(o) = self.calc(b, "code", &expr, &unit, digits) {
                                self.define(b, "code", &name, VarValue { value: o.value, digits, unit, display: None, desc });
                            }
                        }
                    }
                }
            }
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
    let mut cx = Ctx {
        t,
        scope: Scope::new(),
        visible_by: HashMap::new(),
        frames: vec![Frame { owner: None, level: None, vars: vec![] }],
        last_vars: Vec::new(),
        r: Report::default(),
    };
    let mut ids = std::collections::HashSet::new();
    for b in doc.all_blocks() {
        if !ids.insert(b.id.clone()) {
            cx.r.issue(b, "", Severity::Error, "block-id", "ブロックIDが重複しています（内部エラー）");
        }
    }
    cx.walk(&doc.blocks);
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
