//! コードモード：GUI文書をそのままTypstとして見せ、編集を同じ文書に戻す。
//!
//! 表示するコードは、部品ごとに目印の行（`// @block <ID>`、まとまりは `// @group <ID> <名前>` 〜 `// @end <ID>`）で区切る。
//! 編集を戻すときは区切りごとに比べ、生成したコードと同じなら元の部品のまま、変わっていれば
//! 「Typstコード」部品（中身はその区切りのコード）にする。目印の無い行を足すと、新しい Typstコード部品になる。
//! これにより、どちらのモードで編集しても、編集中の文書は常に1つ（document.json）になる。

use std::collections::HashMap;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::model::{Block, Document};

/// コードを文書に戻した結果
#[derive(Debug, Clone, Serialize)]
pub struct Applied {
    pub doc: Document,
    /// 戻せなかった変更（文書情報の行など）の案内
    pub warnings: Vec<String>,
}

enum Line<'a> {
    Block(&'a str),
    Group(&'a str, &'a str),
    End,
    Text(&'a str),
}

fn classify(line: &str) -> Line<'_> {
    let t = line.trim_end();
    if let Some(id) = t.strip_prefix("// @block ") {
        return Line::Block(id.trim());
    }
    if let Some(rest) = t.strip_prefix("// @group ") {
        let (id, title) = rest.split_once(' ').unwrap_or((rest, ""));
        return Line::Group(id.trim(), title.trim());
    }
    if t.starts_with("// @end") {
        return Line::End;
    }
    Line::Text(line)
}

/// 元の文書の部品（ID → 部品）
fn index<'a>(blocks: &'a [Block], out: &mut HashMap<&'a str, &'a Block>) {
    for b in blocks {
        out.insert(&b.id, b);
        index(&b.children, out);
    }
}

struct Parser<'a> {
    orig: HashMap<&'a str, &'a Block>,
    /// 元の文書を生成したときの、部品ごとのコード
    codes: &'a HashMap<String, String>,
    used: std::collections::HashSet<String>,
    seq: usize,
}

impl Parser<'_> {
    fn new_id(&mut self) -> String {
        loop {
            self.seq += 1;
            let id = format!("c{}", self.seq);
            if !self.orig.contains_key(id.as_str()) && !self.used.contains(&id) {
                self.used.insert(id.clone());
                return id;
            }
        }
    }

    /// 区切り1つ分のコードを部品にする
    fn segment(&mut self, id: Option<&str>, lines: &[&str], out: &mut Vec<Block>) {
        let text = lines.join("\n");
        let text = text.trim_matches(|c| c == '\n' || c == '\r').trim_end().to_string();
        let orig = id.and_then(|i| self.orig.get(i).copied()).filter(|b| b.kind != "group");
        if let Some(b) = orig {
            if self.used.insert(b.id.clone()) {
                let orig_code = self.codes.get(&b.id).map(|c| c.trim_end().to_string()).unwrap_or_default();
                if orig_code == text {
                    out.push(b.clone());
                    return;
                }
                // 元のコードの前後に行を足しただけなら、元の部品は残し、足した行を新しい Typstコード部品にする
                if !orig_code.is_empty() {
                    if let Some(after) = text.strip_prefix(&orig_code).filter(|a| a.starts_with('\n')) {
                        out.push(b.clone());
                        self.segment(None, &[after], out);
                        return;
                    }
                    if let Some(before) = text.strip_suffix(&orig_code).filter(|a| a.ends_with('\n')) {
                        self.segment(None, &[before], out);
                        out.push(b.clone());
                        return;
                    }
                }
                if text.trim().is_empty() {
                    return; // 区切りの中身を消した → 部品を削除
                }
                let mut props = Map::new();
                props.insert("code".into(), Value::String(text));
                out.push(Block { id: b.id.clone(), kind: "typst".into(), props, children: vec![] });
                return;
            }
        }
        if text.trim().is_empty() {
            return;
        }
        let mut props = Map::new();
        props.insert("code".into(), Value::String(text));
        let id = self.new_id();
        out.push(Block { id, kind: "typst".into(), props, children: vec![] });
    }
}

/// 表示したコードの編集を文書に戻す。`header` は生成したコードの先頭（読み込みと文書情報）。
pub fn apply(doc: &Document, codes: &HashMap<String, String>, header: &str, code: &str) -> Applied {
    let mut orig = HashMap::new();
    index(&doc.blocks, &mut orig);
    let mut p = Parser { orig, codes, used: Default::default(), seq: 0 };
    let mut warnings = Vec::new();

    let lines: Vec<&str> = code.lines().collect();
    let first = lines.iter().position(|l| !matches!(classify(l), Line::Text(_))).unwrap_or(lines.len());
    if lines[..first].join("\n").trim() != header.trim() {
        warnings.push("先頭の読み込み・文書情報（#show: style.with(…)）の変更は文書に戻せません。文書情報は「部品で作成」の文書情報で変更してください".into());
    }

    // まとまりの入れ子を追いながら、区切りごとに部品にする
    let mut stack: Vec<(Option<Block>, Vec<Block>)> = vec![(None, vec![])];
    let mut cur_id: Option<String> = None;
    let mut buf: Vec<&str> = Vec::new();
    let flush = |p: &mut Parser, id: &Option<String>, buf: &mut Vec<&str>, stack: &mut Vec<(Option<Block>, Vec<Block>)>| {
        let out = &mut stack.last_mut().unwrap().1;
        p.segment(id.as_deref(), buf, out);
        buf.clear();
    };
    for line in &lines[first..] {
        match classify(line) {
            Line::Block(id) => {
                flush(&mut p, &cur_id, &mut buf, &mut stack);
                cur_id = Some(id.to_string());
            }
            Line::Group(id, title) => {
                flush(&mut p, &cur_id, &mut buf, &mut stack);
                cur_id = None;
                let g = match p.orig.get(id).copied().filter(|b| b.kind == "group") {
                    Some(b) if p.used.insert(b.id.clone()) => {
                        let mut g = b.clone();
                        g.props.insert("title".into(), Value::String(title.to_string()));
                        g
                    }
                    _ => {
                        let mut props = Map::new();
                        props.insert("title".into(), Value::String(title.to_string()));
                        props.insert("exports".into(), Value::Array(vec![]));
                        Block { id: p.new_id(), kind: "group".into(), props, children: vec![] }
                    }
                };
                stack.push((Some(g), vec![]));
            }
            Line::End => {
                flush(&mut p, &cur_id, &mut buf, &mut stack);
                cur_id = None;
                if stack.len() > 1 {
                    let (g, children) = stack.pop().unwrap();
                    let mut g = g.unwrap();
                    g.children = children;
                    stack.last_mut().unwrap().1.push(g);
                } else {
                    warnings.push("対応する「// @group」の無い「// @end」があります（無視しました）".into());
                }
            }
            Line::Text(t) => buf.push(t),
        }
    }
    flush(&mut p, &cur_id, &mut buf, &mut stack);
    while stack.len() > 1 {
        warnings.push("「// @group」に対応する「// @end」がありません（末尾で閉じました）".into());
        let (g, children) = stack.pop().unwrap();
        let mut g = g.unwrap();
        g.children = children;
        stack.last_mut().unwrap().1.push(g);
    }
    let mut out = doc.clone();
    out.blocks = stack.pop().unwrap().1;
    Applied { doc: out, warnings }
}

/// Typstコード部品の中の変数定義（`vdef("名前", 値, …)` / `vcalc("名前", "式", …)`）を読む。
/// 後ろの部品からその変数を使えるようにするため（コードモードで値を書き換えても、GUIの部品の計算が続くように）。
#[derive(Debug, Clone, PartialEq)]
pub enum CodeDef {
    Value { name: String, value: f64, unit: String, digits: Option<u8>, desc: String },
    Calc { name: String, expr: String, unit: String, digits: Option<u8>, desc: String },
}

/// 関数呼び出しの引数を、文字列の中のカンマ・括弧を区別して分ける。`src` は "(" の直後から。
fn split_args(src: &str) -> Option<Vec<&str>> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    let mut start = 0;
    let mut out = Vec::new();
    for (i, c) in src.char_indices() {
        if in_str {
            match (esc, c) {
                (true, _) => esc = false,
                (false, '\\') => esc = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ')' => {
                out.push(src[start..i].trim());
                return Some(out);
            }
            ',' if depth == 0 => {
                out.push(src[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\\\"", "\"").replace("\\\\", "\\").replace("\\n", "\n"))
}

pub fn code_defs(code: &str) -> Vec<CodeDef> {
    let mut out = Vec::new();
    for (func, is_calc) in [("vdef(", false), ("vcalc(", true)] {
        let mut rest = code;
        while let Some(i) = rest.find(func) {
            // 識別子の一部（myvdef( など）は除く
            let prev = rest[..i].chars().last();
            let after = &rest[i + func.len()..];
            rest = after;
            if prev.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                continue;
            }
            let Some(args) = split_args(after) else { continue };
            let Some(name) = args.first().and_then(|a| unquote(a)) else { continue };
            let mut named: HashMap<&str, &str> = HashMap::new();
            for a in &args[1..] {
                if let Some((k, v)) = a.split_once(':') {
                    if k.trim().chars().all(|c| c.is_alphanumeric() || c == '-') {
                        named.insert(k.trim(), v.trim());
                    }
                }
            }
            let unit = named.get("unit").and_then(|v| unquote(v)).unwrap_or_default();
            let desc = named.get("desc").and_then(|v| unquote(v)).unwrap_or_default();
            let digits = named.get("digits").and_then(|v| v.parse::<u8>().ok());
            let Some(second) = args.get(1) else { continue };
            if is_calc {
                if let Some(expr) = unquote(second) {
                    out.push(CodeDef::Calc { name, expr, unit, digits, desc });
                }
            } else if let Ok(value) = second.parse::<f64>() {
                out.push(CodeDef::Value { name, value, unit, digits, desc });
            }
        }
    }
    // コード中の順に並べ直す
    out.sort_by_key(|d| {
        let n = match d {
            CodeDef::Value { name, .. } | CodeDef::Calc { name, .. } => name,
        };
        code.find(&format!("\"{n}\"")).unwrap_or(usize::MAX)
    });
    out
}
