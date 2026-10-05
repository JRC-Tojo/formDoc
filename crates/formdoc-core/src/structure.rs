//! 章構成の拘束（TODO 3 / Issue #7）。
//!
//! 文書テンプレートの `info.chapters`（章・節の定義）に対して、文書の見出しの並びを調べる。
//! - 章の見出しは見出し部品の `props.chapter` に章の id を持つ。見出しの階層 n の章は、定義の入れ子の n 段目に対応する
//! - 拘束の強さ（`locked` / `chapters` / `basic`）と検出ごとの重さは、文書全体の既定と章ごとの上書きで決まる
//!   （読み込み時に親から引き継いだ値で埋めてある。`template::Template::resolve_chapters`）
//!
//! ここでは評価（[`check`]）のほか、新規作成時の骨組み（[`skeleton`]）と、足りない章の追加（[`complete`]）も行う。

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;

use crate::evaluate::{Fix, Issue, Severity};
use crate::model::{Block, Document};
use crate::template::{Chapter, RuleLevel, Strictness, Template, VARIANT_KEY, rule_of};

/// 見出しの階層（見出し以外は None）。
fn heading_level(b: &Block) -> Option<i64> {
    (b.kind == "heading").then(|| b.int("level").unwrap_or(2))
}

/// `blocks[range]` を、階層 `level` の見出しで区切る。戻り値は (見出しの位置, 節の終わり) の並び。
/// 最初の見出しより前の部品（親の章の中身）は含まない。
fn sections(blocks: &[Block], start: usize, end: usize, level: i64) -> Vec<(usize, usize)> {
    let heads: Vec<usize> = (start..end).filter(|&i| heading_level(&blocks[i]) == Some(level)).collect();
    heads.iter().enumerate().map(|(k, &h)| (h, heads.get(k + 1).copied().unwrap_or(end))).collect()
}

/// 見出しが指す章の定義（この構造形式で使う章の中から）。
fn matched<'a>(h: &Block, defs: &'a [Chapter], variant: &str) -> Option<&'a Chapter> {
    let id = h.opt_str("chapter")?;
    defs.iter().find(|d| d.id == id && d.applies_to(variant))
}

fn severity(level: RuleLevel) -> Option<Severity> {
    match level {
        RuleLevel::Error => Some(Severity::Error),
        RuleLevel::Warning => Some(Severity::Warning),
        RuleLevel::Info => Some(Severity::Info),
        RuleLevel::Off => None,
    }
}

/// 調べている範囲の親（最上位では文書全体）。
struct Parent<'a> {
    heading: Option<&'a Block>,
    strictness: Strictness,
    rules: &'a BTreeMap<String, RuleLevel>,
}

struct Checker<'a> {
    t: &'a Template,
    variant: &'a str,
    blocks: &'a [Block],
    issues: Vec<Issue>,
}

impl<'a> Checker<'a> {
    fn push(&mut self, rule: RuleLevel, block: Option<&Block>, field: &str, code: &str, message: String, fix: Option<Fix>) {
        if let Some(severity) = severity(rule) {
            self.issues.push(Issue {
                block_id: block.map(|b| b.id.clone()),
                field: (!field.is_empty()).then(|| field.to_string()),
                severity,
                code: code.into(),
                message,
                fix,
            });
        }
    }

    /// 親の範囲 `start..end` にある階層 `level` の見出しを、章の定義 `defs` と照らし合わせる。
    fn chapters(&mut self, start: usize, end: usize, level: i64, defs: &'a [Chapter], parent: Parent<'a>) {
        if parent.strictness == Strictness::Basic {
            return;
        }
        let mut seen: HashSet<&str> = HashSet::new();
        // 直前までに現れた章の、定義での位置（順序の検出用）
        let mut last: Option<(usize, &Chapter)> = None;
        for (h, sec_end) in sections(self.blocks, start, end, level) {
            let head = &self.blocks[h];
            let Some(def) = matched(head, defs, self.variant) else {
                // 定義のない見出し：節の定義があるとき、または中身を固定した章の中では余分な章
                if !defs.is_empty() || parent.strictness == Strictness::Locked {
                    let msg = match head.opt_str("chapter").and_then(|id| defs.iter().find(|d| d.id == id)) {
                        Some(d) => format!("構造形式「{}」の文書では「{}」の章は使いません", self.variant, d.title),
                        None => format!("「{}」は文書テンプレートで決められた章ではありません", head.str("text")),
                    };
                    self.push(rule_of(parent.rules, "chapter-extra"), Some(head), "", "chapter-extra", msg, None);
                }
                continue;
            };
            if !seen.insert(def.id.as_str()) && !def.repeatable {
                self.push(def.rule("chapter-duplicate"), Some(head), "", "chapter-duplicate", format!("「{}」の章は1つだけにしてください", def.title), None);
            }
            let pos = defs.iter().position(|d| d.id == def.id).unwrap_or(0);
            match last {
                Some((p, prev)) if pos < p => {
                    self.push(def.rule("chapter-order"), Some(head), "", "chapter-order",
                        format!("「{}」の章は「{}」より前に置きます", def.title, prev.title), None);
                }
                _ => last = Some((pos, def)),
            }
            let text = head.str("text").trim();
            if def.fixed_title && text != def.title {
                let fix = (!text.is_empty()).then(|| Fix { from: text.to_string(), to: def.title.clone() });
                self.push(def.rule("chapter-title"), Some(head), "text", "chapter-title",
                    format!("この章の見出し文は「{}」と決められています", def.title), fix);
            }
            let rules = &def.rules;
            self.chapters(h + 1, sec_end, level + 1, &def.sections,
                Parent { heading: Some(head), strictness: def.strictness(), rules });
        }
        for d in defs.iter().filter(|d| d.required && d.applies_to(self.variant) && !seen.contains(d.id.as_str())) {
            let place = match parent.heading {
                Some(h) => format!("「{}」の中に", h.str("text")),
                None => String::new(),
            };
            self.push(d.rule("chapter-missing"), parent.heading, "", "chapter-missing",
                format!("{place}必須の章「{}」がありません", d.title), None);
        }
    }

    /// 章ごとの中身を調べる：使える部品（allowed-blocks）と、中身を固定した章（locked）の部品の並び。
    fn contents(&mut self) {
        // 開いている見出しの階層と、その見出しが指す章の定義（定義のない見出しは None）
        let mut stack: Vec<(i64, Option<&'a Chapter>, &'a Block)> = Vec::new();
        // 中身を固定した章の見出しID → 直下に並んでいる部品の種類
        let mut locked: HashMap<&str, (&'a Chapter, Vec<&str>)> = HashMap::new();
        let blocks = self.blocks;
        for b in blocks {
            if let Some(level) = heading_level(b) {
                while stack.last().is_some_and(|(l, _, _)| *l >= level) {
                    stack.pop();
                }
                let def = b.opt_str("chapter").and_then(|id| self.t.chapter(id)).filter(|d| d.applies_to(self.variant));
                if let Some(d) = def.filter(|d| d.strictness() == Strictness::Locked) {
                    locked.insert(&b.id, (d, Vec::new()));
                }
                stack.push((level, def, b));
                continue;
            }
            // いちばん内側の、定義のある章
            let Some(def) = stack.iter().rev().find_map(|(_, d, _)| *d) else { continue };
            if def.strictness() != Strictness::Basic {
                let allowed = def.allowed_blocks.as_deref().unwrap_or(&[]);
                let mut all = Vec::new();
                crate::model::walk_blocks(std::slice::from_ref(b), &mut all);
                for x in all.into_iter().filter(|x| x.kind != "group" && !allowed.contains(&x.kind)) {
                    self.push(def.rule("chapter-block"), Some(x), "", "chapter-block",
                        format!("「{}」の章では部品「{}」は使えません", def.title, x.kind), None);
                }
            }
            if let Some((_, _, h)) = stack.last() {
                if let Some((_, kinds)) = locked.get_mut(h.id.as_str()) {
                    kinds.push(b.kind.as_str());
                }
            }
        }
        for b in blocks.iter().filter(|b| locked.contains_key(b.id.as_str())) {
            let (def, kinds) = &locked[b.id.as_str()];
            let expected: Vec<&str> = def.content.iter().map(content_kind).collect();
            if *kinds != expected {
                self.push(def.rule("chapter-locked"), Some(b), "", "chapter-locked",
                    format!("「{}」の章の中身は文書テンプレートで決められています（部品の追加・削除・並べ替えはできません）", def.title), None);
            }
        }
    }
}

fn content_kind(m: &serde_json::Map<String, Value>) -> &str {
    m.get("kind").and_then(Value::as_str).unwrap_or("paragraph")
}

/// 文書の構造形式（文書情報の `variant` 欄）。
pub fn variant(doc: &Document) -> &str {
    doc.meta.str(VARIANT_KEY)
}

/// 章構成を調べ、検出を返す。文書テンプレートに章の定義がなければ何もしない。
pub fn check(doc: &Document, t: &Template) -> Vec<Issue> {
    if t.chapters.is_empty() {
        return Vec::new();
    }
    let mut c = Checker { t, variant: variant(doc), blocks: &doc.blocks, issues: Vec::new() };
    c.chapters(0, doc.blocks.len(), 1, &t.chapters,
        Parent { heading: None, strictness: t.structure.level, rules: &t.structure.rules });
    c.contents();
    c.issues
}

/// 章（と必須の節・決められた中身）の部品を作る。`id` は新しいブロックIDを返す。
fn chapter_blocks(def: &Chapter, level: i64, variant: &str, id: &mut dyn FnMut() -> String) -> Vec<Block> {
    let mut props = serde_json::Map::new();
    props.insert("level".into(), level.into());
    props.insert("text".into(), def.title.clone().into());
    props.insert("chapter".into(), def.id.clone().into());
    let mut out = vec![Block { id: id(), kind: "heading".into(), props, children: vec![] }];
    for m in &def.content {
        let mut props = m.clone();
        let kind = content_kind(m).to_string();
        props.remove("kind");
        out.push(Block { id: id(), kind, props, children: vec![] });
    }
    for s in def.sections.iter().filter(|s| s.required && s.applies_to(variant)) {
        out.extend(chapter_blocks(s, level + 1, variant, id));
    }
    out
}

/// 新規作成時の骨組み：構造形式 `variant` で必須の章を、定義の順に並べる。
pub fn skeleton(t: &Template, variant: &str, id: &mut dyn FnMut() -> String) -> Vec<Block> {
    t.chapters.iter().filter(|c| c.required && c.applies_to(variant)).flat_map(|c| chapter_blocks(c, 1, variant, id)).collect()
}

/// 足りない必須の章・節を、定義の順序に合う位置へ追加した文書を返す（構造形式を変えたときなど）。
/// 既にある部品・章は動かさない。追加した部品のIDは "new-1" のように既存と重ならないものにする。
pub fn complete(doc: &Document, t: &Template) -> Document {
    let used: HashSet<String> = doc.all_blocks().iter().map(|b| b.id.clone()).collect();
    let mut n = 0;
    let mut id = move || loop {
        n += 1;
        let s = format!("new-{n}");
        if !used.contains(&s) {
            return s;
        }
    };
    let v = variant(doc).to_string();
    let mut out = doc.clone();
    out.blocks = complete_range(doc.blocks.clone(), 1, &t.chapters, t.structure.level, &v, &mut id);
    out
}

/// `blocks`（親の章の中身）に、階層 `level` の足りない章を入れる。
fn complete_range(blocks: Vec<Block>, level: i64, defs: &[Chapter], strictness: Strictness, variant: &str, id: &mut dyn FnMut() -> String) -> Vec<Block> {
    if defs.is_empty() || strictness == Strictness::Basic {
        return blocks;
    }
    let bounds = sections(&blocks, 0, blocks.len(), level);
    let first = bounds.first().map(|(h, _)| *h).unwrap_or(blocks.len());
    let mut out: Vec<Block> = blocks[..first].to_vec();
    // 既にある章：(定義での位置, 部品の並び)。定義のない章は直前の章と同じ位置として扱う
    let mut secs: Vec<(usize, Vec<Block>)> = Vec::new();
    let mut seen = HashSet::new();
    for (h, end) in bounds {
        let def = matched(&blocks[h], defs, variant);
        let pos = match def {
            Some(d) => defs.iter().position(|x| x.id == d.id).unwrap_or(0),
            None => secs.last().map(|(p, _)| *p).unwrap_or(0),
        };
        let mut sec = vec![blocks[h].clone()];
        match def {
            Some(d) => {
                seen.insert(d.id.clone());
                sec.extend(complete_range(blocks[h + 1..end].to_vec(), level + 1, &d.sections, d.strictness(), variant, id));
            }
            None => sec.extend(blocks[h + 1..end].iter().cloned()),
        }
        secs.push((pos, sec));
    }
    for (i, d) in defs.iter().enumerate() {
        if !d.required || !d.applies_to(variant) || seen.contains(&d.id) {
            continue;
        }
        // 定義で後ろにある最初の章の前に入れる（なければ最後）
        let at = secs.iter().position(|(p, _)| *p > i).unwrap_or(secs.len());
        secs.insert(at, (i, chapter_blocks(d, level, variant, id)));
    }
    out.extend(secs.into_iter().flat_map(|(_, s)| s));
    out
}
