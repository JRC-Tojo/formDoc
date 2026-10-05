//! 章構成の拘束（Issue #7）。
//!
//! 文書テンプレートの `info.chapters`（章・節の定義）に対して、文書の見出しの並びを調べる。
//! - 章の見出しは見出し部品の `props.chapter` に章の id を持つ。見出しの階層 n の章は、定義の入れ子の n 段目に対応する
//! - 拘束の強さ（`locked` / `chapters` / `basic`）は「その範囲の中」に効く：章が有るか・順序・見出し文は**親**の強さ
//!   （最上位は `structure.level`）で、章の中身（節・部品）は**その章**の強さで調べる
//! - 検出ごとの重さは文書全体の既定と章ごとの上書きで決まる（読み込み時に親から引き継いだ値で埋めてある。`template.rs`）
//!
//! 検証（[`analyze`]）は、検出のほかに GUI が操作の前に止めるための情報（[`ChapterInfo`]）も返す。
//! GUI は判定をまねせず、この情報だけを見る（判定を1か所にするため）。
//! ほかに新規作成時の骨組み（[`skeleton`]）と、足りない章の追加（[`complete`]）も行う。

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

use crate::evaluate::{Fix, Issue, Severity};
use crate::model::{Block, Document};
use crate::template::{Chapter, RuleLevel, Strictness, Template, rule_of};

/// GUI が操作の前に止めるための、部品ごとの章の情報（評価結果の `BlockResult.chapter`）。
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ChapterInfo {
    /// 見出しなら、その見出しが指す章（定義の id）。見出し以外は入っている章
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter: Option<String>,
    /// 執筆ガイド（章の定義の guide）
    #[serde(skip_serializing_if = "String::is_empty")]
    pub guide: String,
    /// 削除・複製できない（必須の章で、無くなると「エラー」になる）
    pub no_remove: bool,
    /// 移動・階層の変更ができない（順序違いが「エラー」になる）
    pub no_move: bool,
    /// 見出し文を変えられない（見出し文の変更が「エラー」になる）
    pub fixed_title: bool,
    /// この部品の直後に置ける部品の種類。None なら制限なし（文書テンプレートの blocks の範囲）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insertable: Option<Vec<String>>,
    /// 見出しに割り当てられる章（この位置・階層で使える定義）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ChapterChoice>,
}

/// 見出しに割り当てられる章1つ。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChapterChoice {
    pub id: String,
    pub title: String,
    pub repeatable: bool,
}

/// 検証の結果。
#[derive(Debug, Default)]
pub struct Analysis {
    pub issues: Vec<Issue>,
    /// 部品ID → 章の情報（部品テンプレートの中の部品は、まとまりと同じ）
    pub info: HashMap<String, ChapterInfo>,
}

/// 見出しの階層（見出し以外は None）。
fn heading_level(b: &Block) -> Option<i64> {
    (b.kind == "heading").then(|| b.int("level").unwrap_or(2))
}

/// `blocks[start..end]` を、階層 `level` の見出しで区切る。戻り値は (見出しの位置, 節の終わり) の並び。
/// 最初の見出しより前の部品（親の章の中身）は含まない。
fn sections(blocks: &[Block], start: usize, end: usize, level: i64) -> Vec<(usize, usize)> {
    let heads: Vec<usize> = (start..end).filter(|&i| heading_level(&blocks[i]) == Some(level)).collect();
    heads.iter().enumerate().map(|(k, &h)| (h, heads.get(k + 1).copied().unwrap_or(end))).collect()
}

/// 検出の重さを、表示する重さにする（off は None）。
fn severity(level: RuleLevel) -> Option<Severity> {
    match level {
        RuleLevel::Error => Some(Severity::Error),
        RuleLevel::Warning => Some(Severity::Warning),
        RuleLevel::Info => Some(Severity::Info),
        RuleLevel::Off => None,
    }
}

/// 検出の重さが「エラー」か（GUI で操作を止めるのはエラーのときだけ。注意以下なら検証パネルに任せる）。
fn is_error(rules: &BTreeMap<String, RuleLevel>, code: &str) -> bool {
    rule_of(rules, code) == RuleLevel::Error
}

/// 調べている範囲の親（最上位では文書全体）。
#[derive(Clone, Copy)]
struct Parent<'a> {
    heading: Option<&'a Block>,
    strictness: Strictness,
    rules: &'a BTreeMap<String, RuleLevel>,
    /// この範囲に置ける章の定義（親の章の sections。最上位は chapters）
    defs: &'a [Chapter],
}

/// 章構成の検証の状態。
struct Checker<'a> {
    t: &'a Template,
    variant: &'a str,
    blocks: &'a [Block],
    issues: Vec<Issue>,
    /// 見出しの位置 → 照合できた章の定義
    matched: HashMap<usize, &'a Chapter>,
    /// 見出しの位置 → その見出しを置いた範囲（選べる章・操作の可否の計算用）
    parents: HashMap<usize, Parent<'a>>,
}

impl<'a> Checker<'a> {
    /// 検出を1つ足す。重さは `rules` から引く（検出名を1回だけ書くように）。
    fn push(&mut self, rules: &BTreeMap<String, RuleLevel>, code: &str, block: Option<&Block>, field: &str, message: String, fix: Option<Fix>) {
        if let Some(severity) = severity(rule_of(rules, code)) {
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

    /// 親の範囲 `start..end` にある階層 `level` の見出しを、章の定義と照らし合わせる。
    /// 章が有るか・順序・見出し文は親の強さで調べ、章の中は章自身の強さで再帰的に調べる。
    fn chapters(&mut self, start: usize, end: usize, level: i64, parent: Parent<'a>) {
        let check = parent.strictness.checks_chapters();
        let defs = parent.defs;
        let mut seen: HashSet<&str> = HashSet::new();
        // 照合できた章の (見出しの位置, 定義での位置)。順序の検出用
        let mut order: Vec<(usize, usize)> = Vec::new();
        for (h, sec_end) in sections(self.blocks, start, end, level) {
            let head = &self.blocks[h];
            self.parents.insert(h, parent);
            let def = head.opt_str("chapter").and_then(|id| defs.iter().find(|d| d.id == id && d.applies_to(self.variant)));
            let Some(def) = def else {
                // 定義のない見出し：節の定義があるとき、または中身を固定した章の中では余分な章
                if check && (!defs.is_empty() || parent.strictness.fixes_content()) {
                    let msg = match head.opt_str("chapter").and_then(|id| defs.iter().find(|d| d.id == id)) {
                        Some(d) => format!("構造形式「{}」の文書では「{}」の章は使いません", self.variant, d.title),
                        None => format!("「{}」は文書テンプレートで決められた章ではありません", head.str("text")),
                    };
                    self.push(parent.rules, "chapter-extra", Some(head), "", msg, None);
                }
                continue;
            };
            self.matched.insert(h, def);
            if check {
                if !seen.insert(def.id.as_str()) && !def.repeatable {
                    self.push(&def.rules, "chapter-duplicate", Some(head), "", format!("「{}」の章は1つだけにしてください", def.title), None);
                }
                order.push((h, defs.iter().position(|d| d.id == def.id).unwrap_or(0)));
                let text = head.str("text").trim();
                if def.fixed_title && text != def.title {
                    let fix = (!text.is_empty()).then(|| Fix { from: text.to_string(), to: def.title.clone() });
                    self.push(&def.rules, "chapter-title", Some(head), "text", format!("この章の見出し文は「{}」と決められています", def.title), fix);
                } else if !def.fixed_title && text == def.title {
                    // 見出し文が自由な章（「（部材名）の設計」など）が、既定の見出し文のまま
                    self.push(&def.rules, "chapter-title", Some(head), "text", format!("見出し文「{}」を書き換えてください", def.title), None);
                }
            }
            let inner = Parent { heading: Some(head), strictness: def.strictness(), rules: &def.rules, defs: &def.sections };
            self.chapters(h + 1, sec_end, level + 1, inner);
        }
        if !check {
            return;
        }
        // 順序：決められた順序に沿う最長の並びから外れた章だけを指す（1つずれただけで全部が違反にならないように）
        let keep = longest_ordered(&order.iter().map(|(_, p)| *p).collect::<Vec<_>>());
        let sequence = defs.iter().filter(|d| d.applies_to(self.variant)).map(|d| d.title.as_str()).collect::<Vec<_>>().join(" → ");
        let misplaced: Vec<usize> = order.iter().enumerate().filter(|(k, _)| !keep.contains(k)).map(|(_, (h, _))| *h).collect();
        for h in misplaced {
            let def = self.matched[&h];
            self.push(&def.rules, "chapter-order", Some(&self.blocks[h]), "",
                format!("「{}」の章の位置が決められた順序と違います（{sequence}）", def.title), None);
        }
        for d in defs.iter().filter(|d| d.required && d.applies_to(self.variant) && !seen.contains(d.id.as_str())) {
            let place = parent.heading.map(|h| format!("「{}」の中に", h.str("text"))).unwrap_or_default();
            self.push(&d.rules, "chapter-missing", parent.heading, "", format!("{place}必須の章「{}」がありません", d.title), None);
        }
    }

    /// 章ごとの中身を調べ、部品ごとの章の情報を作る。
    /// - 使える部品（allowed-blocks）、中身を固定した章（locked）の部品の並び、最初の章より前の部品
    /// - 部品テンプレート（group）の中の見出し（章としては数えないので、決められた章とは別のものとして検出する）
    fn contents(&mut self) -> HashMap<String, ChapterInfo> {
        let t = self.t;
        let top = Parent { heading: None, strictness: t.structure.level, rules: &t.structure.rules, defs: &t.chapters };
        let mut info = HashMap::new();
        // 開いている見出しの (階層, 照合できた章)
        let mut stack: Vec<(i64, Option<&'a Chapter>)> = Vec::new();
        // 中身を固定した章の見出しの位置 → 直下に並んでいる部品の種類
        let mut locked: Vec<(usize, &'a Chapter, Vec<&'a str>)> = Vec::new();
        let blocks = self.blocks;
        for (i, b) in blocks.iter().enumerate() {
            if let Some(level) = heading_level(b) {
                while stack.last().is_some_and(|(l, _)| *l >= level) {
                    stack.pop();
                }
                let def = self.matched.get(&i).copied();
                if let Some(d) = def.filter(|d| d.strictness().fixes_content()) {
                    locked.push((i, d, Vec::new()));
                }
                stack.push((level, def));
                info.insert(b.id.clone(), self.heading_info(i, &stack));
                continue;
            }
            // いちばん内側の、照合できた章（無ければ章の外）
            let inner = stack.iter().rev().find_map(|(_, d)| *d);
            match inner {
                Some(def) if def.strictness().checks_chapters() => {
                    let allowed = def.allowed_blocks.as_deref().unwrap_or(&[]);
                    let mut all = Vec::new();
                    crate::model::walk_blocks(std::slice::from_ref(b), &mut all);
                    for x in all.into_iter().filter(|x| x.kind != "group" && x.kind != "heading" && !allowed.contains(&x.kind)) {
                        self.push(&def.rules, "chapter-block", Some(x), "", format!("「{}」の章では部品「{}」は使えません", def.title, x.kind), None);
                    }
                }
                None if stack.is_empty() && !t.chapters.is_empty() && top.strictness.checks_chapters() => {
                    self.push(top.rules, "chapter-block", Some(b), "", "最初の章より前に部品は置けません".into(), None);
                }
                _ => {}
            }
            // 直下の部品（間に定義のない見出しを挟まない）だけを、中身を固定した章の並びとして数える
            if let (Some((_, Some(d))), Some(last)) = (stack.last(), locked.last_mut()) {
                if std::ptr::eq(*d, last.1) {
                    last.2.push(b.kind.as_str());
                }
            }
            // 部品テンプレートの中の見出しは章として数えない
            if b.kind == "group" && !t.chapters.is_empty() && top.strictness.checks_chapters() {
                let mut all = Vec::new();
                crate::model::walk_blocks(&b.children, &mut all);
                for x in all.into_iter().filter(|x| x.kind == "heading") {
                    self.push(top.rules, "chapter-extra", Some(x), "",
                        format!("部品テンプレートの中の見出し「{}」は章として扱えません（まとまりを解除してください）", x.str("text")), None);
                }
            }
            let ci = self.position_info(inner, stack.is_empty() && !t.chapters.is_empty() && top.strictness.checks_chapters());
            let mut all = Vec::new();
            crate::model::walk_blocks(std::slice::from_ref(b), &mut all);
            for x in all {
                info.insert(x.id.clone(), ci.clone());
            }
        }
        for (i, def, kinds) in locked {
            let expected: Vec<&str> = def.content.iter().map(content_kind).collect();
            if kinds != expected {
                self.push(&def.rules, "chapter-locked", Some(&blocks[i]), "",
                    format!("「{}」の章の中身は文書テンプレートで決められています（部品の追加・削除・並べ替えはできません）", def.title), None);
            }
        }
        info
    }

    /// 見出し以外の部品の情報：入っている章と、直後に置ける部品。
    fn position_info(&self, inner: Option<&Chapter>, before_first: bool) -> ChapterInfo {
        let t = self.t;
        let insertable = match inner {
            Some(d) if d.strictness().fixes_content() && is_error(&d.rules, "chapter-locked") => Some(Vec::new()),
            Some(d) if d.strictness().checks_chapters() && is_error(&d.rules, "chapter-block") => {
                let mut v = d.allowed_blocks.clone().unwrap_or_default();
                v.push("heading".into());
                Some(v)
            }
            None if before_first && is_error(&t.structure.rules, "chapter-block") => Some(vec!["heading".into()]),
            _ => None,
        };
        ChapterInfo { chapter: inner.map(|d| d.id.clone()), guide: inner.map(|d| d.guide.clone()).unwrap_or_default(), insertable, ..Default::default() }
    }

    /// 見出しの情報：操作の可否・選べる章・直後に置ける部品。`stack` は、この見出しまでに開いている見出し。
    fn heading_info(&self, i: usize, stack: &[(i64, Option<&'a Chapter>)]) -> ChapterInfo {
        let def = self.matched.get(&i).copied();
        let parent = self.parents.get(&i).copied();
        let mut ci = self.position_info(stack.iter().rev().find_map(|(_, d)| *d), false);
        ci.chapter = def.map(|d| d.id.clone());
        ci.guide = def.map(|d| d.guide.clone()).unwrap_or_default();
        if let (Some(d), Some(p)) = (def, parent) {
            if p.strictness.checks_chapters() {
                ci.no_remove = d.required && !d.repeatable && is_error(&d.rules, "chapter-missing");
                ci.no_move = is_error(&d.rules, "chapter-order");
                ci.fixed_title = d.fixed_title && is_error(&d.rules, "chapter-title");
            }
        }
        if let Some(p) = parent {
            ci.choices = p
                .defs
                .iter()
                .filter(|d| d.applies_to(self.variant))
                .map(|d| ChapterChoice { id: d.id.clone(), title: d.title.clone(), repeatable: d.repeatable })
                .collect();
        }
        ci
    }
}

/// 数の並びのうち、小さい順（同じ値は続いてよい）に沿う最長の部分列の位置。
fn longest_ordered(xs: &[usize]) -> HashSet<usize> {
    let n = xs.len();
    let mut len = vec![1usize; n];
    let mut prev = vec![usize::MAX; n];
    for i in 0..n {
        for j in 0..i {
            if xs[j] <= xs[i] && len[j] + 1 > len[i] {
                len[i] = len[j] + 1;
                prev[i] = j;
            }
        }
    }
    let mut out = HashSet::new();
    let mut k = (0..n).max_by_key(|&i| (len[i], std::cmp::Reverse(i)));
    while let Some(i) = k {
        out.insert(i);
        k = (prev[i] != usize::MAX).then_some(prev[i]);
    }
    out
}

/// 決められた中身（content）の1つの部品の種類。
fn content_kind(m: &serde_json::Map<String, Value>) -> &str {
    m.get("kind").and_then(Value::as_str).unwrap_or("paragraph")
}

/// 文書の構造形式（文書情報の、文書テンプレートが決めた欄。既定は variant）。
pub fn variant<'a>(doc: &'a Document, t: &Template) -> &'a str {
    doc.meta.str(&t.structure.variant_field)
}

/// 章構成を調べ、検出と部品ごとの章の情報を返す。文書テンプレートに章の定義がなければ何もしない。
pub fn analyze(doc: &Document, t: &Template) -> Analysis {
    if t.chapters.is_empty() {
        return Analysis::default();
    }
    let mut c = Checker { t, variant: variant(doc, t), blocks: &doc.blocks, issues: Vec::new(), matched: HashMap::new(), parents: HashMap::new() };
    c.chapters(0, doc.blocks.len(), 1, Parent { heading: None, strictness: t.structure.level, rules: &t.structure.rules, defs: &t.chapters });
    let info = c.contents();
    Analysis { issues: c.issues, info }
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
    for s in def.sections.iter().filter(|s| needed(s, variant)) {
        out.extend(chapter_blocks(s, level + 1, variant, id));
    }
    out
}

/// 骨組み・足りない章の追加で入れる章か（必須で、この構造形式で使い、無いことを検出しない設定ではない）。
fn needed(d: &Chapter, variant: &str) -> bool {
    d.required && d.applies_to(variant) && d.rule("chapter-missing") != RuleLevel::Off
}

/// 新規作成時の骨組み：構造形式 `variant` で必須の章を、定義の順に並べる。
pub fn skeleton(t: &Template, variant: &str, id: &mut dyn FnMut() -> String) -> Vec<Block> {
    t.chapters.iter().filter(|c| needed(c, variant)).flat_map(|c| chapter_blocks(c, 1, variant, id)).collect()
}

/// 足りない必須の章・節を、定義の順序に合う位置へ追加した文書を返す（構造形式を変えたときなど）。
/// 既にある部品・章は動かさない。章の id が無い見出しで、階層と見出し文が定義と同じものは、その章として扱う
/// （章の定義を入れる前に作った文書の移行）。追加した部品のIDは "new-1" のように既存と重ならないものにする。
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
    let v = variant(doc, t).to_string();
    let mut out = doc.clone();
    out.blocks = complete_range(doc.blocks.clone(), 1, &t.chapters, t.structure.level, &v, &mut id);
    out
}

/// `blocks`（親の章の中身）に、階層 `level` の足りない章を入れる。
fn complete_range(mut blocks: Vec<Block>, level: i64, defs: &[Chapter], strictness: Strictness, variant: &str, id: &mut dyn FnMut() -> String) -> Vec<Block> {
    let bounds = sections(&blocks, 0, blocks.len(), level);
    // 章の id の無い見出しで、見出し文が定義と同じものは、その章とみなす
    for &(h, _) in &bounds {
        if blocks[h].opt_str("chapter").is_none() {
            let text = blocks[h].str("text").trim().to_string();
            if let Some(d) = defs.iter().find(|d| d.applies_to(variant) && d.title == text) {
                blocks[h].props.insert("chapter".into(), d.id.clone().into());
            }
        }
    }
    let first = bounds.first().map(|(h, _)| *h).unwrap_or(blocks.len());
    let mut out: Vec<Block> = blocks[..first].to_vec();
    // 既にある章：(定義での位置, 部品の並び)。定義のない章は直前の章と同じ位置として扱う
    let mut secs: Vec<(usize, Vec<Block>)> = Vec::new();
    let mut seen = HashSet::new();
    for (h, end) in bounds {
        let def = blocks[h].opt_str("chapter").and_then(|cid| defs.iter().find(|d| d.id == cid && d.applies_to(variant)));
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
    if strictness.checks_chapters() {
        for (i, d) in defs.iter().enumerate() {
            if !needed(d, variant) || seen.contains(&d.id) {
                continue;
            }
            // 定義で後ろにある最初の章の前に入れる（なければ最後）
            let at = secs.iter().position(|(p, _)| *p > i).unwrap_or(secs.len());
            secs.insert(at, (i, chapter_blocks(d, level, variant, id)));
        }
    }
    out.extend(secs.into_iter().flat_map(|(_, s)| s));
    out
}
