//! GUI文書 → Typstソース。
//!
//! 出力はコードモードで人が書くものと同じ形（@local/formdoc の関数呼び出し）にする。
//! そのため「Typstとして書き出し」でそのままコードモードへ移行できる。
//! 変数は Typst 上では `v-名前` という識別子で定義する（関数名との衝突を避けるため）。

use serde::Serialize;

use serde_json::Value;

use crate::evaluate::{Report, Severity};
use crate::model::{Block, Document, Grid, parse_ymd};
use crate::template::{MetaField, Template};

/// 社内標準パッケージ。
pub const PACKAGE: &str = "@local/formdoc:0.1.0";

/// 文書情報の1項目を style 関数の引数にする。値が無ければ None（スタイル側の既定値を使う）。
fn meta_arg(f: &MetaField, v: Option<&Value>) -> Option<String> {
    let v = v.filter(|v| !v.is_null()).or(f.default.as_ref())?;
    Some(match f.kind.as_str() {
        "int" => match v {
            Value::Number(n) => n.as_f64().map(|x| x.round() as i64).unwrap_or(0).to_string(),
            Value::String(s) => s.trim().parse::<i64>().ok()?.to_string(),
            _ => return None,
        },
        "bool" => v.as_bool().unwrap_or(false).to_string(),
        "date" => {
            let s = v.as_str().unwrap_or("").trim();
            match parse_ymd(s) {
                Some((y, m, d)) => format!("datetime(year: {y}, month: {m}, day: {d})"),
                None if s.is_empty() => "none".into(),
                None => lit(s),
            }
        }
        _ => match v.as_str().map(str::trim) {
            Some("") | None => "none".into(),
            Some(s) => lit(s),
        },
    })
}

fn style_args(doc: &Document, t: &Template) -> Vec<String> {
    t.fields.iter().filter_map(|f| meta_arg(f, doc.meta.get(&f.key)).map(|a| format!("{}: {a}", f.key))).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockSpan {
    pub id: String,
    /// 生成ソース中の行範囲（1始まり、両端含む）
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Generated {
    pub source: String,
    pub spans: Vec<BlockSpan>,
}

impl Generated {
    /// 生成ソースの行番号から、元のブロックIDを引く（コンパイルエラーの表示用）。
    pub fn block_at(&self, line: usize) -> Option<&str> {
        self.spans.iter().find(|s| s.start <= line && line <= s.end).map(|s| s.id.as_str())
    }
}

/// Typst文字列リテラル。
pub fn lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn ident(name: &str) -> String {
    format!("v-{name}")
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{v:.1}") } else { format!("{v}") }
}

fn opt_digits(d: Option<u8>) -> String {
    d.map(|d| d.to_string()).unwrap_or_else(|| "none".into())
}

/// 本文（{{変数}} 参照つき）を Typst コンテンツ式にする。改行は改行、空行は段落区切り。
fn text_content(text: &str, with_unit: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("{{") {
        let after = &rest[i + 2..];
        let Some(j) = after.find("}}") else { break };
        if i > 0 {
            parts.push(lit_lines(&rest[..i]));
        }
        let name = after[..j].trim();
        parts.push(format!("val({}, unit: {})", ident(name), with_unit));
        rest = &after[j + 2..];
    }
    if !rest.is_empty() {
        parts.push(lit_lines(rest));
    }
    if parts.is_empty() { "[]".into() } else { format!("[{}]", parts.iter().map(|p| format!("#{p}")).collect::<String>()) }
}

fn lit_lines(s: &str) -> String {
    if !s.contains('\n') {
        return lit(s);
    }
    let lines: Vec<String> = s.split('\n').map(lit).collect();
    format!("({})", lines.join(" + linebreak() + "))
}

fn label_arg(b: &Block) -> String {
    match b.opt_str("label") {
        Some(l) => format!(", label: {}", text_content(l, true)),
        None => String::new(),
    }
}

fn opt_lit_arg(key: &str, v: Option<&str>) -> String {
    match v {
        Some(s) => format!(", {key}: {}", lit(s)),
        None => String::new(),
    }
}

/// 数値・変数名・式の値を Typst 引数にする（変数ならその変数、式なら評価済みの値）。
fn value_arg(src: &str, report: &Report, rounding: formdoc_expr::Rounding) -> String {
    let s = src.trim();
    if report.var(s).is_some() {
        return ident(s);
    }
    let scope: formdoc_expr::Scope = report
        .vars
        .iter()
        .map(|v| (v.name.clone(), formdoc_expr::VarValue { value: v.value, digits: v.digits, unit: v.unit.clone(), display: None, desc: String::new() }))
        .collect();
    match formdoc_expr::calc(&formdoc_expr::CalcRequest { expr: s.into(), scope, rounding, ..Default::default() }) {
        Ok(o) => num(o.value),
        Err(_) => "0.0".into(),
    }
}

fn expr_vars(expr: &str) -> Vec<String> {
    formdoc_expr::parse(expr).map(|e| e.vars()).unwrap_or_default()
}

fn rounding_str(r: formdoc_expr::Rounding) -> &'static str {
    match r {
        formdoc_expr::Rounding::Display => "display",
        formdoc_expr::Rounding::Full => "full",
    }
}

fn table_code(b: &Block) -> String {
    let grid: Grid = b.props.get("data").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let ncols = grid.rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    // 結合で覆われるセルを飛ばす
    let nrows = grid.rows.len();
    let mut covered = vec![vec![false; ncols]; nrows];
    let mut cells_by_row: Vec<Vec<String>> = vec![Vec::new(); nrows];
    for (ri, row) in grid.rows.iter().enumerate() {
        for (ci, c) in row.iter().enumerate() {
            if covered[ri][ci] {
                continue;
            }
            let rs = c.rowspan.unwrap_or(1).max(1) as usize;
            let cs = c.colspan.unwrap_or(1).max(1) as usize;
            for r in ri..(ri + rs).min(nrows) {
                for cc in ci..(ci + cs).min(ncols) {
                    covered[r][cc] = true;
                }
            }
            let body = if c.vertical {
                format!("[#vt({})]", lit(&c.text))
            } else {
                // 表のセルの変数は値のみ（単位は見出しに書く）
                text_content(&c.text, false)
            };
            let mut args = Vec::new();
            if rs > 1 {
                args.push(format!("rowspan: {rs}"));
            }
            if cs > 1 {
                args.push(format!("colspan: {cs}"));
            }
            if let Some(a) = c.align.as_deref().filter(|a| matches!(*a, "left" | "right" | "center")) {
                args.push(format!("align: {a} + horizon"));
            }
            cells_by_row[ri].push(if args.is_empty() { body } else { format!("table.cell({}){body}", args.join(", ")) });
        }
    }
    let columns = if grid.widths.len() == ncols && grid.widths.iter().all(|w| !w.trim().is_empty()) {
        format!("({},)", grid.widths.join(", "))
    } else {
        ncols.to_string()
    };
    let hr = (grid.header_rows as usize).min(nrows);
    let header: Vec<String> = cells_by_row[..hr].iter().flatten().cloned().collect();
    let body: Vec<String> = cells_by_row[hr..].iter().flatten().cloned().collect();
    let mut s = format!("#fd-table(columns: {columns}{}", match b.opt_str("caption") {
        Some(c) => format!(", caption: {}", text_content(c, true)),
        None => String::new(),
    });
    if !header.is_empty() {
        s.push_str(&format!(",\n  header: ({},)", header.join(", ")));
    }
    for line in body.chunks(ncols.max(1)) {
        s.push_str(&format!(",\n  {}", line.join(", ")));
    }
    s.push_str(",\n)");
    s
}

fn block_code(b: &Block, t: &Template, report: &Report) -> String {
    let rounding = rounding_str(t.rounding);
    let res = report.blocks.get(&b.id);
    let digits = res.and_then(|r| r.digits);
    match b.kind.as_str() {
        "heading" => {
            let level = b.int("level").unwrap_or(2).clamp(1, t.max_heading_level as i64);
            let mut body = text_content(b.str("text"), true);
            if let Some(sym) = b.opt_str("symbol") {
                body = format!("{body} + h(1em) + {}", lit(sym));
            }
            if let Some(k) = b.props.get("kijun").filter(|k| k.get("abbr").and_then(|a| a.as_str()).is_some_and(|a| !a.is_empty())) {
                let abbr = k.get("abbr").and_then(|v| v.as_str()).unwrap_or("");
                let loc = k.get("loc").and_then(|v| v.as_str()).unwrap_or("");
                body = format!("{body} + h(1fr) + kijun({}, {})", lit(abbr), lit(loc));
            }
            format!("#heading(level: {level}, {body})")
        }
        "paragraph" => b
            .str("text")
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .map(|p| format!("#para({})", text_content(p.trim_end(), true)))
            .collect::<Vec<_>>()
            .join("\n"),
        "kijun" => {
            let k = b.props.get("kijun");
            let abbr = k.and_then(|k| k.get("abbr")).and_then(|v| v.as_str()).unwrap_or("");
            let loc = k.and_then(|k| k.get("loc")).and_then(|v| v.as_str()).unwrap_or("");
            format!("#para({} + kijun({}, {}))", text_content(b.str("prefix"), true), lit(abbr), lit(loc))
        }
        "vdef" => {
            let name = b.str("name").trim();
            let mut s = format!(
                "#let {} = vdef({}, {}, unit: {}, digits: {}, desc: {}{})",
                ident(name),
                lit(name),
                num(b.num("value").unwrap_or(0.0)),
                lit(b.str("unit").trim()),
                opt_digits(digits),
                lit(b.str("desc")),
                opt_lit_arg("display", b.opt_str("display")),
            );
            if b.bool("show", true) {
                s.push_str(&format!("\n#def-line({}{})", ident(name), label_arg(b)));
            }
            s
        }
        "calc" => {
            let name = b.str("name").trim();
            let expr = b.str("expr");
            let deps: Vec<String> = expr_vars(expr).iter().map(|v| format!(", {}", ident(v))).collect();
            let mut s = format!(
                "#let {} = vcalc({}, {}{}, unit: {}, digits: {}, desc: {}{}, frac: {}, units-in-sub: {}, rounding: {})",
                ident(name),
                lit(name),
                lit(expr),
                deps.concat(),
                lit(b.str("unit").trim()),
                opt_digits(digits),
                lit(b.str("desc")),
                opt_lit_arg("display", b.opt_str("display")),
                b.bool("frac", true),
                b.bool("units_in_sub", false),
                lit(rounding),
            );
            if b.bool("show", true) {
                s.push_str(&format!(
                    "\n#calc-line({}{}, show-symbolic: {})",
                    ident(name),
                    label_arg(b),
                    b.bool("show_symbolic", true)
                ));
            }
            s
        }
        "sum" => {
            let total = b.str("name").trim();
            let unit = b.str("unit").trim();
            let names = res.map(|r| r.vars.clone()).unwrap_or_default();
            let mut lines = Vec::new();
            let mut rows = Vec::new();
            for (i, item) in b.arr("items").iter().enumerate() {
                let Some(name) = names.get(i) else { continue };
                let expr = item.get("expr").and_then(|v| v.as_str()).unwrap_or("");
                let label = item.get("label").and_then(|v| v.as_str()).unwrap_or("");
                let deps: String = expr_vars(expr).iter().map(|v| format!(", {}", ident(v))).collect();
                lines.push(format!(
                    "#let {} = vcalc({}, {}{deps}, unit: {}, digits: {}, desc: {}, frac: false, rounding: {})",
                    ident(name), lit(name), lit(expr), lit(unit), opt_digits(digits), lit(label), lit(rounding)
                ));
                rows.push(format!("(label: {}, v: {})", text_content(label, true), ident(name)));
            }
            let deps: String = names.iter().map(|n| format!(", {}", ident(n))).collect();
            lines.push(format!(
                "#let {} = vcalc({}, {}{deps}, unit: {}, digits: {}, desc: {}, rounding: {})",
                ident(total), lit(total), lit(&names.join(" + ")), lit(unit), opt_digits(digits), lit(b.str("desc")), lit(rounding)
            ));
            lines.push(format!("#sum-lines(({},), total: {})", rows.join(", "), ident(total)));
            lines.join("\n")
        }
        "check" => {
            let expr = b.str("expr");
            let deps: String = expr_vars(expr).iter().map(|v| format!(", {}", ident(v))).collect();
            format!(
                "#check-line({}{deps}, digits: {}, unit: {}{}{}, rounding: {})",
                lit(expr),
                opt_digits(b.int("digits").map(|d| d.clamp(0, 10) as u8)),
                lit(b.str("unit").trim()),
                opt_lit_arg("lhs", b.opt_str("lhs")),
                label_arg(b),
                lit(rounding),
            )
        }
        "where" => {
            let vars = res.map(|r| r.vars.clone()).unwrap_or_default();
            format!("#where-list({})", vars.iter().map(|v| ident(v)).collect::<Vec<_>>().join(", "))
        }
        "table" => table_code(b),
        "fig-beam" => {
            let list = |key: &str| -> String {
                let items: Vec<String> = b
                    .str(key)
                    .split([',', '、', '，'])
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| value_arg(s, report, t.rounding))
                    .collect();
                if items.is_empty() { "()".into() } else { format!("({},)", items.join(", ")) }
            };
            let eta = if b.str("eta").trim().is_empty() { "none".into() } else { list("eta") };
            format!(
                "#fd-figure(fig-beam({}, loads: {}, eta: {}), caption: {})",
                value_arg(b.str("span"), report, t.rounding),
                list("loads"),
                eta,
                b.opt_str("caption").map(|c| text_content(c, true)).unwrap_or_else(|| "none".into())
            )
        }
        "fig-isection" => {
            let fig = format!(
                "fig-isection({}, {}, {}, {})",
                value_arg(b.str("H"), report, t.rounding),
                value_arg(b.str("B"), report, t.rounding),
                value_arg(b.str("tw"), report, t.rounding),
                value_arg(b.str("tf"), report, t.rounding)
            );
            let body = match b.opt_str("note") {
                Some(n) => format!("pad(left: 2em, grid(columns: (auto, 1fr), column-gutter: 2em, align: horizon, {fig}, {}))", text_content(n, false)),
                None => fig,
            };
            match b.opt_str("caption") {
                Some(c) => format!("#fd-figure({body}, caption: {})", text_content(c, true)),
                None => format!("#{body}"),
            }
        }
        "fig-shapes" => {
            let mut items = Vec::new();
            for sh in b.arr("shapes") {
                let g = |k: &str| sh.get(k).and_then(|v| v.as_str()).unwrap_or("0");
                let kind = sh.get("kind").and_then(|v| v.as_str()).unwrap_or("line");
                let p = |x: &str, y: &str| format!("({}, {})", value_arg(g(x), report, t.rounding), value_arg(g(y), report, t.rounding));
                let label = sh.get("label").and_then(|v| v.as_str()).unwrap_or("");
                let fill = match sh.get("fill").and_then(|v| v.as_str()).unwrap_or("") {
                    "gray" => ", fill: luma(210)",
                    "dark" => ", fill: luma(90)",
                    "black" => ", fill: black",
                    _ => "",
                };
                items.push(match kind {
                    "circle" => format!("(kind: \"circle\", at: {}, r: {}{fill})", p("x1", "y1"), value_arg(g("x2"), report, t.rounding)),
                    "rect" => format!("(kind: \"rect\", from: {}, to: {}{fill})", p("x1", "y1"), p("x2", "y2")),
                    "polygon" => {
                        let pts: Vec<String> = crate::evaluate::split_points(sh.get("pts").and_then(|v| v.as_str()).unwrap_or(""))
                            .iter()
                            .map(|(x, y)| format!("({}, {}),", value_arg(x, report, t.rounding), value_arg(y, report, t.rounding)))
                            .collect();
                        format!("(kind: \"polygon\", pts: ({}){fill})", pts.concat())
                    }
                    "text" => format!("(kind: \"text\", at: {}, body: {})", p("x1", "y1"), text_content(label, true)),
                    "dim" => format!("(kind: \"dim\", from: {}, to: {}, label: {})", p("x1", "y1"), p("x2", "y2"), text_content(label, false)),
                    k => format!("(kind: {}, from: {}, to: {})", lit(k), p("x1", "y1"), p("x2", "y2")),
                });
            }
            let fig = format!("fig-shapes(({}), scale: {})", items.iter().map(|i| format!("{i},")).collect::<String>(), num(b.num("scale").unwrap_or(1.0)));
            match b.opt_str("caption") {
                Some(c) => format!("#fd-figure({fig}, caption: {})", text_content(c, true)),
                None => format!("#align(center, {fig})"),
            }
        }
        "image" => {
            let width = b.int("width").unwrap_or(80).clamp(5, 100);
            let path = format!("/{}", b.str("file").trim_start_matches('/'));
            let page = if path.to_ascii_lowercase().ends_with(".pdf") {
                format!(", page: {}", b.int("page").unwrap_or(1).max(1))
            } else {
                String::new()
            };
            let img = format!("image({}, width: {width}%{page})", lit(&path));
            match b.opt_str("caption") {
                Some(c) => format!("#fd-figure({img}, caption: {})", text_content(c, true)),
                None => format!("#align(center, {img})"),
            }
        }
        "pagebreak" => "#pagebreak()".into(),
        "typst" => b.str("code").to_string(),
        _ => String::new(),
    }
}

fn error_box(b: &Block, report: &Report) -> String {
    let msgs: Vec<String> = report
        .issues
        .iter()
        .filter(|i| i.block_id.as_deref() == Some(&b.id) && i.severity == Severity::Error)
        .map(|i| i.message.clone())
        .collect();
    format!(
        "#block(width: 100%, inset: 6pt, stroke: 1pt + rgb(\"#c00000\"), fill: rgb(\"#fff0f0\"), text(fill: rgb(\"#c00000\"), size: 9pt, {}))",
        lit(&format!("⚠ {}：{}", b.kind, msgs.join(" / ")))
    )
}

/// GUI文書をTypstソースに変換する。
/// エラーのあるブロックは赤枠のメッセージに置き換える（PDF出力は呼び出し側で止める）。
pub fn generate(doc: &Document, t: &Template, report: &Report) -> Generated {
    let mut out = String::new();
    out.push_str(&format!("#import \"{PACKAGE}\": *
"));
    out.push_str("#import \"style.typ\": style
");
    out.push_str(&format!("#show: style.with({})

", style_args(doc, t).join(", ")));
    let mut spans = Vec::new();
    for b in &doc.blocks {
        let has_error = report.blocks.get(&b.id).is_some_and(|r| r.status == "error");
        let code = if has_error { error_box(b, report) } else { block_code(b, t, report) };
        if code.is_empty() {
            continue;
        }
        let start = out.lines().count() + 1;
        out.push_str(&format!("// @block {}\n", b.id));
        out.push_str(&code);
        out.push_str("\n\n");
        spans.push(BlockSpan { id: b.id.clone(), start, end: out.lines().count() });
    }
    Generated { source: out, spans }
}
