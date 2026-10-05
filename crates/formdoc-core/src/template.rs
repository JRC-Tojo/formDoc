//! 文書テンプレート（文書の型）と部品定義の読み込み。
//!
//! 文書テンプレートは1ファイルのTypst（library/styles/*.typ、またはシステムフォルダの styles/*.typ）。
//! ファイル内の `info` 辞書が型の規則と文書情報の入力欄、`style` 関数が体裁を持つ。
//! `info` は実際に Typst で評価して取り出す（定義元を1か所にするため、別のパーサーは持たない）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::world::FormdocWorld;

/// 文書テンプレートの規則（`info` の中身）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Template {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "four")]
    pub max_heading_level: u8,
    #[serde(default)]
    pub rounding: formdoc_expr::Rounding,
    pub blocks: Vec<String>,
    /// 文書情報（表紙など）の入力欄。key は style 関数の引数名
    #[serde(default)]
    pub fields: Vec<MetaField>,
    /// 章構成の拘束（既定の強さと、検出ごとの重さ）
    #[serde(default)]
    pub structure: Structure,
    /// 章の定義（並び順が文書での順序）。新規作成時の骨組みもここから作る
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    #[serde(default)]
    pub digits: BTreeMap<String, u8>,
    #[serde(default)]
    pub lint: LintRules,
}

fn four() -> u8 {
    4
}

/// 章構成の拘束の強さ（Issue #7）。文書全体の既定を `structure.level` に、章ごとの上書きを `chapters[].level` に書く。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strictness {
    /// 最も厳しい：章立てに加え、章の中身（部品の並び）も文書テンプレートの `content` のとおりに固定する
    Locked,
    /// 章立てを拘束する（必須・順序・見出し文・余分な章）。中身は `allowed-blocks` の範囲で自由
    #[default]
    Chapters,
    /// 章立ては拘束しない（体裁だけを文書テンプレートで固定する）
    Basic,
}

/// 検出の重さ。`off` は検出しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleLevel {
    Error,
    Warning,
    Info,
    Off,
}

/// 章構成の検出の種類と、既定の重さ。文書テンプレートの `structure.rules`・`chapters[].rules` で上書きできる。
pub const CHAPTER_RULES: &[(&str, RuleLevel)] = &[
    ("chapter-missing", RuleLevel::Error),   // 必須の章がない
    ("chapter-order", RuleLevel::Error),     // 章の順序が違う
    ("chapter-title", RuleLevel::Warning),   // 見出し文の変更が禁止されている章で、見出し文が違う
    ("chapter-extra", RuleLevel::Error),     // 定義にない章（この構造形式では不要な章を含む）
    ("chapter-duplicate", RuleLevel::Error), // 繰り返せない章が2回以上ある
    ("chapter-block", RuleLevel::Error),     // 章で使えない部品
    ("chapter-locked", RuleLevel::Error),    // 中身を固定した章（locked）の部品の並びが違う
];

/// 文書全体の章構成の拘束。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Structure {
    #[serde(default)]
    pub level: Strictness,
    /// 検出の種類（`CHAPTER_RULES` の名前）→ 重さ。書かなかった種類は既定の重さ
    #[serde(default)]
    pub rules: BTreeMap<String, RuleLevel>,
}

/// 章（または節）の定義。`sections` に1つ下の階層の見出しを入れ子で定義する。
///
/// 読み込み時に `level`・`rules`・`allowed-blocks` は親から引き継いだ値で埋める（[`parse_style`]）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Chapter {
    /// 識別子（文書テンプレート内で一意）。見出し部品の `props.chapter` に保存する
    pub id: String,
    /// 既定の見出し文
    pub title: String,
    #[serde(default = "yes")]
    pub required: bool,
    /// 見出し文の変更を禁止する
    #[serde(default = "yes")]
    pub fixed_title: bool,
    /// 同じ章を2回以上置ける（「主桁の設計」「横桁の設計」のような同じ形の章）
    #[serde(default)]
    pub repeatable: bool,
    /// 対象の構造形式（文書情報の `variant` 欄の値）。空なら全形式
    #[serde(default)]
    pub variants: Vec<String>,
    /// 執筆ガイド（GUIに表示）
    #[serde(default)]
    pub guide: String,
    /// この章で使える部品。省略時は親の章（最上位では文書テンプレートの `blocks`）と同じ
    #[serde(default)]
    pub allowed_blocks: Option<Vec<String>>,
    /// 拘束の強さ。省略時は親の章（最上位では `structure.level`）と同じ
    #[serde(default)]
    pub level: Option<Strictness>,
    /// 検出の重さの上書き。親の章の設定に重ねる
    #[serde(default)]
    pub rules: BTreeMap<String, RuleLevel>,
    /// 見出しの直後に置く部品（新規作成・章の追加時）。`locked` の章ではこの並びに固定する
    #[serde(default)]
    pub content: Vec<serde_json::Map<String, Value>>,
    /// 1つ下の階層の見出しの定義
    #[serde(default)]
    pub sections: Vec<Chapter>,
}

fn yes() -> bool {
    true
}

impl Chapter {
    /// 構造形式 `variant` の文書で使う章か。
    pub fn applies_to(&self, variant: &str) -> bool {
        self.variants.is_empty() || self.variants.iter().any(|v| v == variant)
    }

    /// 拘束の強さ（読み込み後は必ず埋まっている）。
    pub fn strictness(&self) -> Strictness {
        self.level.unwrap_or_default()
    }

    /// 検出の重さ（読み込み後は親から引き継いだ値も含む）。
    pub fn rule(&self, code: &str) -> RuleLevel {
        rule_of(&self.rules, code)
    }
}

/// 検出の重さを引く。書かれていなければ既定の重さ。
pub fn rule_of(rules: &BTreeMap<String, RuleLevel>, code: &str) -> RuleLevel {
    rules
        .get(code)
        .copied()
        .or_else(|| CHAPTER_RULES.iter().find(|(c, _)| *c == code).map(|(_, l)| *l))
        .unwrap_or(RuleLevel::Error)
}

/// 構造形式を入力する文書情報の欄の key。章の `variants` はこの欄の値と比べる。
pub const VARIANT_KEY: &str = "variant";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaField {
    pub key: String,
    pub label: String,
    /// text / multiline / int / bool / date / select
    #[serde(rename = "type", default = "text_type")]
    pub kind: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

fn text_type() -> String {
    "text".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LintRules {
    #[serde(default)]
    pub punctuation: Option<Punctuation>,
    #[serde(default)]
    pub fullwidth_alnum: bool,
    #[serde(default)]
    pub halfwidth_kana: bool,
    #[serde(default)]
    pub replace: Vec<Replace>,
    #[serde(default)]
    pub unit: Vec<Replace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Punctuation {
    pub comma: String,
    pub period: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replace {
    pub from: String,
    pub to: String,
}

impl Template {
    /// 単位から既定の表示桁を引く。
    pub fn default_digits(&self, unit: &str) -> Option<u8> {
        let norm = unit.replace(['·', '・', '.'], "*");
        self.digits.get(unit).or_else(|| self.digits.get(&norm)).copied()
    }

    pub fn field(&self, key: &str) -> Option<&MetaField> {
        self.fields.iter().find(|f| f.key == key)
    }

    /// 章・節の定義を id で探す（入れ子も含む）。
    pub fn chapter(&self, id: &str) -> Option<&Chapter> {
        fn find<'a>(cs: &'a [Chapter], id: &str) -> Option<&'a Chapter> {
            cs.iter().find_map(|c| if c.id == id { Some(c) } else { find(&c.sections, id) })
        }
        find(&self.chapters, id)
    }

    /// 章の定義を読み込んだあとの整え：親から拘束の強さ・検出の重さ・使える部品を引き継ぎ、定義の誤りを調べる。
    fn resolve_chapters(&mut self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        let variants: Vec<String> = self.field(VARIANT_KEY).map(|f| f.options.clone()).unwrap_or_default();
        fn walk(
            cs: &mut [Chapter],
            level: Strictness,
            rules: &BTreeMap<String, RuleLevel>,
            allowed: &[String],
            all_blocks: &[String],
            variants: &[String],
            seen: &mut std::collections::HashSet<String>,
        ) -> Result<(), String> {
            for c in cs {
                if c.id.trim().is_empty() || !seen.insert(c.id.clone()) {
                    return Err(format!("文書テンプレートの章の id「{}」が空か重複しています", c.id));
                }
                for k in c.rules.keys() {
                    if !CHAPTER_RULES.iter().any(|(r, _)| r == k) {
                        return Err(format!("章「{}」の rules に未知の検出「{k}」があります", c.title));
                    }
                }
                if let Some(v) = c.variants.iter().find(|v| !variants.contains(v)) {
                    return Err(format!("章「{}」の構造形式「{v}」が、文書情報の欄「{VARIANT_KEY}」の選択肢にありません", c.title));
                }
                let lv = *c.level.get_or_insert(level);
                let mut merged = rules.clone();
                merged.extend(std::mem::take(&mut c.rules));
                c.rules = merged;
                let al = c.allowed_blocks.get_or_insert_with(|| allowed.to_vec()).clone();
                if let Some(k) = al.iter().find(|k| !all_blocks.contains(k)) {
                    return Err(format!("章「{}」の allowed-blocks の部品「{k}」は文書テンプレートの blocks にありません", c.title));
                }
                walk(&mut c.sections, lv, &c.rules.clone(), &al, all_blocks, variants, seen)?;
            }
            Ok(())
        }
        for k in self.structure.rules.keys() {
            if !CHAPTER_RULES.iter().any(|(r, _)| r == k) {
                return Err(format!("structure.rules に未知の検出「{k}」があります"));
            }
        }
        let blocks = self.blocks.clone();
        walk(&mut self.chapters, self.structure.level, &self.structure.rules, &blocks, &blocks, &variants, &mut seen)
    }
}

/// 文書テンプレートのTypstソースを評価し、`info` を取り出す。`style` 関数が無い場合もエラーにする。
pub fn parse_style(source: &str) -> Result<Template, String> {
    let mut world = FormdocWorld::new();
    world.set_files([
        ("/main.typ".to_string(), b"#import \"/style.typ\": info, style\n#metadata(info) <fd-style-info>\n".to_vec()),
        ("/style.typ".to_string(), source.as_bytes().to_vec()),
    ]);
    let c = crate::compile::compile(&world);
    let doc = c.document.ok_or_else(|| {
        let errs: Vec<String> = c
            .diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .map(|d| format!("{}{}", d.line.map(|l| format!("{l}行目: ")).unwrap_or_default(), d.message))
            .collect();
        format!("文書テンプレートを読み込めません: {}", errs.join(" / "))
    })?;
    let label = typst::foundations::Label::new(typst::utils::PicoStr::intern("fd-style-info")).unwrap();
    use typst::introspection::Introspector;
    let content = doc.introspector().query_label(label).map_err(|e| e.to_string())?;
    let value = content.get_by_name("value").map_err(|_| "文書テンプレートの info を取り出せません".to_string())?;
    let json = serde_json::to_value(&value).map_err(|e| e.to_string())?;
    let mut t: Template = serde_json::from_value(json).map_err(|e| format!("文書テンプレートの info に誤りがあります: {e}"))?;
    if t.id.trim().is_empty() {
        return Err("文書テンプレートの info.id が空です".into());
    }
    t.resolve_chapters()?;
    Ok(t)
}

/// 同梱文書テンプレート（library/styles/*.typ）。(ファイル名の拡張子なし, ソース)
pub fn builtin_styles() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = formdoc_library::files()
        .filter_map(|(p, b)| {
            let id = p.strip_prefix("styles/")?.strip_suffix(".typ")?.to_string();
            Some((id, String::from_utf8_lossy(b).into_owned()))
        })
        .collect();
    v.sort();
    v
}

/// 同梱文書テンプレートを id で取得する（テスト・CLI用）。
pub fn builtin_style(id: &str) -> Result<String, String> {
    builtin_styles().into_iter().find(|(i, _)| i == id).map(|(_, s)| s).ok_or_else(|| format!("文書テンプレート {id} が見つかりません"))
}

/// 同梱部品テンプレート（library/snippets/*.fdtpl。JSON）。
pub fn builtin_snippets() -> Vec<Value> {
    let mut v: Vec<(String, Value)> = formdoc_library::files()
        .filter(|(p, _)| p.starts_with("snippets/") && p.ends_with(".fdtpl"))
        .filter_map(|(p, b)| Some((p, serde_json::from_slice(b).ok()?)))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.into_iter().map(|(_, j)| j).collect()
}

/// 部品定義（components.toml）をJSONとして返す（GUIのフォーム生成用）。
pub fn components_json() -> Result<Value, String> {
    let bytes = formdoc_library::get("components/components.toml").ok_or("部品定義が見つかりません")?;
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// 基準書レジストリ。
pub fn references_json() -> Result<Value, String> {
    let bytes = formdoc_library::get("typst/formdoc/0.1.0/data/references.toml").ok_or("基準書レジストリが見つかりません")?;
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    serde_json::to_value(v).map_err(|e| e.to_string())
}
