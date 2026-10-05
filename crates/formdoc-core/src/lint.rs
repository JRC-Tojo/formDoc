//! 表記Lint。文書テンプレートの規則（info.lint）に従い、文章の表記ぶれを検出する。
//! 検出結果には置換内容（fix）を付け、GUIからワンクリックで直せるようにする。

use crate::evaluate::{Fix, Issue, Severity};
use crate::model::{Block, Document, Grid};
use crate::template::{LintRules, Template};

/// 文章として検査する項目（ブロック種別ごと）。
fn text_fields(b: &Block) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    let mut push = |k: &str, s: &str| {
        if !s.is_empty() {
            v.push((k.to_string(), s.to_string()));
        }
    };
    match b.kind.as_str() {
        "heading" => push("text", b.str("text")),
        "paragraph" => push("text", b.str("text")),
        "kijun" => push("prefix", b.str("prefix")),
        "vdef" | "calc" | "sum" => {
            push("desc", b.str("desc"));
            push("label", b.str("label"));
        }
        "check" => push("label", b.str("label")),
        "fig-shapes" | "image" => push("caption", b.str("caption")),
        "table" => {
            push("caption", b.str("caption"));
            let grid: Grid = b.props.get("data").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            for (ri, row) in grid.rows.iter().enumerate() {
                for (ci, c) in row.iter().enumerate() {
                    push(&format!("data.{ri}.{ci}"), &c.text);
                }
            }
        }
        _ => {}
    }
    if b.kind == "sum" {
        for (i, item) in b.arr("items").iter().enumerate() {
            if let Some(l) = item.get("label").and_then(|x| x.as_str()) {
                if !l.is_empty() {
                    v.push((format!("items.{i}.label"), l.to_string()));
                }
            }
        }
    }
    v
}

fn unit_fields(b: &Block) -> Vec<(String, String)> {
    match b.kind.as_str() {
        "vdef" | "calc" | "sum" | "check" => vec![("unit".into(), b.str("unit").to_string())],
        _ => vec![],
    }
}

fn to_halfwidth(c: char) -> Option<char> {
    match c {
        '０'..='９' | 'Ａ'..='Ｚ' | 'ａ'..='ｚ' => char::from_u32(c as u32 - 0xFEE0),
        _ => None,
    }
}

fn is_halfwidth_kana(c: char) -> bool {
    ('\u{FF66}'..='\u{FF9F}').contains(&c)
}

fn mk(b: &Block, field: &str, code: &str, message: String, from: &str, to: &str) -> Issue {
    Issue {
        block_id: Some(b.id.clone()),
        field: Some(field.to_string()),
        severity: Severity::Warning,
        code: code.into(),
        message,
        fix: Some(Fix { from: from.into(), to: to.into() }),
    }
}

/// {{変数}} の内側は検査しない。
fn strip_refs(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        match rest[i..].find("}}") {
            Some(j) => {
                out.push_str(&" ".repeat(j + 2));
                rest = &rest[i + j + 2..];
            }
            None => {
                rest = &rest[i..];
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn lint_text(b: &Block, field: &str, text: &str, rules: &LintRules, out: &mut Vec<Issue>) {
    let text = strip_refs(text);
    if let Some(p) = &rules.punctuation {
        for (wrong, right) in [("、", p.comma.as_str()), ("，", p.comma.as_str()), ("。", p.period.as_str()), ("．", p.period.as_str())] {
            if wrong != right && text.contains(wrong) {
                out.push(mk(b, field, "lint-punctuation", format!("句読点は「{}{}」を使います（「{wrong}」→「{right}」）", p.comma, p.period), wrong, right));
            }
        }
    }
    if rules.fullwidth_alnum {
        let mut seen = std::collections::BTreeSet::new();
        for c in text.chars() {
            if let Some(h) = to_halfwidth(c) {
                if seen.insert(c) {
                    out.push(mk(b, field, "lint-fullwidth", format!("全角英数字「{c}」は半角「{h}」にします"), &c.to_string(), &h.to_string()));
                }
            }
        }
    }
    if rules.halfwidth_kana && text.chars().any(is_halfwidth_kana) {
        out.push(Issue {
            block_id: Some(b.id.clone()),
            field: Some(field.to_string()),
            severity: Severity::Warning,
            code: "lint-kana".into(),
            message: "半角カナが含まれています。全角カナにしてください".into(),
            fix: None,
        });
    }
    for r in &rules.replace {
        if text.contains(&r.from) {
            out.push(mk(b, field, "lint-wording", format!("表記の統一: 「{}」→「{}」", r.from, r.to), &r.from, &r.to));
        }
    }
}

/// 数の直後にあれば「数ではなく番号・日付の一部」とみなす文字（2026年、第1234号、第1234条）
const NUMBER_SUFFIXES: &[char] = &['年', '号', '条'];

/// 3桁区切りの無い `group` 桁以上の整数部を取り出す（1234、-8000、8000mm、12345.6 の 12345）。
/// 対象にしないもの：年・号など（2026年、第1234号）、日付や番号（2026-10-02、2026.10.05、03-5435-7630）、
/// 記号・規格の一部（SM400、G1234、JIS G 3101、ISO 9001）、小数部（0.12345）、既に区切りのある数（1,234）。
fn ungrouped_numbers(text: &str, group: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let at = |k: isize| if k < 0 { None } else { chars.get(k as usize).copied() };
    let digits_from = |k: usize| chars[k..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let (s, e) = (start as isize, i as isize);
        let prev = at(s - 1);
        let next = at(e);
        // 前：英字・_・.（小数部）・,（区切り済み）が直前なら、記号や数の一部
        let mut joined = prev.is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '.' | ',' | '/' | ':'));
        // 前の「-」は、さらに前が英数字なら番号の区切り（03-5435、A-1234）、そうでなければ負の符号
        joined |= prev == Some('-') && at(s - 2).is_some_and(|c| c.is_ascii_alphanumeric());
        // 前が「英大文字＋空白」なら規格番号（JIS G 3101、ISO 9001）
        joined |= prev == Some(' ') && at(s - 2).is_some_and(|c| c.is_ascii_uppercase());
        // 後ろ：番号の区切り（- / : _）、区切り済みの数（,＋ちょうど3桁）、日付（.数字.）、年・号など
        joined |= next.is_some_and(|c| matches!(c, '-' | '/' | ':' | '_'));
        joined |= chars[i..].iter().find(|c| **c != ' ').is_some_and(|c| NUMBER_SUFFIXES.contains(c));
        joined |= next == Some(',') && digits_from(i + 1) == 3;
        joined |= next == Some('.') && {
            let k = i + 1 + digits_from((i + 1).min(chars.len()));
            k > i + 1 && at(k as isize) == Some('.')
        };
        if i - start >= group && !joined {
            out.push(chars[start..i].iter().collect());
        }
    }
    out
}

/// 数値の3桁区切りの検出（Lint の digit-grouping）。`group` 桁以上の区切りの無い数を指す。
/// 修正（文字列の置換）は、その数字の並びが元の文の別の場所（{{式}} の中・別の数の一部）に現れないときだけ付ける。
fn lint_digit_grouping(b: &Block, field: &str, raw: &str, group: u8, out: &mut Vec<Issue>) {
    let text = strip_refs(raw);
    let found = ungrouped_numbers(&text, group as usize);
    let mut seen = std::collections::BTreeSet::new();
    for n in &found {
        if !seen.insert(n.clone()) {
            continue;
        }
        let to = formdoc_expr::group_literal(n, group);
        let msg = format!("{group}桁以上の数値には3桁区切りを入れます（「{n}」→「{to}」）");
        let safe = raw.matches(n.as_str()).count() == found.iter().filter(|x| *x == n).count();
        let mut issue = mk(b, field, "lint-digit-grouping", msg, n, &to);
        if !safe {
            issue.fix = None;
        }
        out.push(issue);
    }
}

pub fn lint(doc: &Document, t: &Template) -> Vec<Issue> {
    let mut out = Vec::new();
    for b in doc.all_blocks() {
        for (field, text) in text_fields(b) {
            lint_text(b, &field, &text, &t.lint, &mut out);
            // 数値の桁区切りは文章の欄だけを見る（text_fields は Typstコード部品のコードを返さない。
            // コードモードのソースは数値の引数が多いため lint_source でも対象にしない）
            if let Some(group) = t.group_setting() {
                lint_digit_grouping(b, &field, &text, group, &mut out);
            }
        }
        for (field, unit) in unit_fields(b) {
            for r in &t.lint.unit {
                if unit.trim() == r.from {
                    out.push(mk(b, &field, "lint-unit", format!("単位の表記: 「{}」→「{}」", r.from, r.to), &r.from, &r.to));
                }
            }
        }
        if b.kind == "paragraph" {
            let s = b.str("text").trim_end();
            if let Some(p) = &t.lint.punctuation {
                if !s.is_empty() && !s.ends_with(p.period.as_str()) && !s.ends_with("。") && !s.ends_with("}}") && !s.ends_with('：') {
                    out.push(Issue {
                        block_id: Some(b.id.clone()),
                        field: Some("text".into()),
                        severity: Severity::Info,
                        code: "lint-period".into(),
                        message: format!("文末に「{}」がありません", p.period),
                        fix: None,
                    });
                }
            }
        }
    }
    out
}

/// コードモード（生Typst）のソースに対する表記Lint。文字列・コメント・コードは区別せず行単位で検査する。
pub fn lint_source(source: &str, t: &Template) -> Vec<Issue> {
    let dummy = Block { id: String::new(), kind: "typst".into(), props: Default::default(), children: vec![] };
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("#import") {
            continue;
        }
        let mut issues = Vec::new();
        lint_text(&dummy, &format!("line:{}", i + 1), line, &t.lint, &mut issues);
        for mut is in issues {
            is.block_id = None;
            out.push(is);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::{builtin_style, parse_style};

    fn para(text: &str) -> Document {
        let mut props = serde_json::Map::new();
        props.insert("text".into(), text.into());
        Document {
            schema_version: 1,
            library: String::new(),
            template: "keisansho".into(),
            meta: Default::default(),
            blocks: vec![Block { id: "b1".into(), kind: "paragraph".into(), props, children: vec![] }],
            assets: vec![],
        }
    }

    #[test]
    fn punctuation_and_width() {
        let t = parse_style(&builtin_style("keisansho").unwrap()).unwrap();
        let issues = lint(&para("設計を行なう。荷重は１０ｋＮとする、ただし{{L_b}}。"), &t);
        let codes: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"lint-punctuation"));
        assert!(codes.contains(&"lint-fullwidth"));
        assert!(codes.contains(&"lint-wording"));
        let ok = lint(&para("設計を行う，荷重は10 kNとする．"), &t);
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn digit_grouping_in_text() {
        // 計算書は桁区切りを有効にしている
        let t = parse_style(&builtin_style("keisansho").unwrap()).unwrap();
        assert!(t.lint.digit_grouping);
        let fixes = |text: &str| -> Vec<(String, String)> {
            lint(&para(text), &t)
                .into_iter()
                .filter(|i| i.code == "lint-digit-grouping")
                .map(|i| i.fix.map(|f| (f.from, f.to)).unwrap_or_default())
                .collect()
        };
        assert_eq!(fixes("支間長は 8000 mm，断面積は 23550.5 mm2 とする．"), [
            ("8000".to_string(), "8,000".to_string()),
            ("23550".to_string(), "23,550".to_string()),
        ]);
        // 単位を詰めて書いた数・負の数・カンマで並べた数も検出する
        assert_eq!(fixes("8000mm，-9500 kN，8000, 9000 とする．").len(), 3);
        // 区切り済み・3桁以下・年・号・日付・番号・規格・記号の一部・小数部・変数参照は対象外
        assert!(fixes("1,234 と 999，2026年，2026 年，第1234号，2026-10-02，2026.10.05，03-5435-7630，JIS G 3101，ISO 9001，SM4000，0.12345，{{L_b}}．").is_empty());
        // {{式}} の中に同じ数字があるときは、置換で式を壊さないよう修正を付けない
        let issues = lint(&para("支間 1000 mm，{{A * 1000}}．"), &t);
        let i = issues.iter().find(|i| i.code == "lint-digit-grouping").expect("1000 を検出する");
        assert!(i.fix.is_none());
    }
}
