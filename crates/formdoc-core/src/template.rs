//! スタイル（文書の型）と部品定義の読み込み。
//!
//! スタイルは1ファイルのTypst（library/styles/*.typ、またはシステムフォルダの styles/*.typ）。
//! ファイル内の `info` 辞書が型の規則と文書情報の入力欄、`style` 関数が体裁を持つ。
//! `info` は実際に Typst で評価して取り出す（定義元を1か所にするため、別のパーサーは持たない）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::world::FormdocWorld;

/// スタイルの規則（`info` の中身）。
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
    #[serde(default)]
    pub skeleton: Vec<serde_json::Map<String, Value>>,
    #[serde(default)]
    pub digits: BTreeMap<String, u8>,
    #[serde(default)]
    pub lint: LintRules,
}

fn four() -> u8 {
    4
}

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
}

/// スタイルのTypstソースを評価し、`info` を取り出す。`style` 関数が無い場合もエラーにする。
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
        format!("スタイルを読み込めません: {}", errs.join(" / "))
    })?;
    let label = typst::foundations::Label::new(typst::utils::PicoStr::intern("fd-style-info")).unwrap();
    use typst::introspection::Introspector;
    let content = doc.introspector().query_label(label).map_err(|e| e.to_string())?;
    let value = content.get_by_name("value").map_err(|_| "スタイルの info を取り出せません".to_string())?;
    let json = serde_json::to_value(&value).map_err(|e| e.to_string())?;
    let t: Template = serde_json::from_value(json).map_err(|e| format!("スタイルの info に誤りがあります: {e}"))?;
    if t.id.trim().is_empty() {
        return Err("スタイルの info.id が空です".into());
    }
    Ok(t)
}

/// 同梱スタイル（library/styles/*.typ）。(ファイル名の拡張子なし, ソース)
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

/// 同梱スタイルを id で取得する（テスト・CLI用）。
pub fn builtin_style(id: &str) -> Result<String, String> {
    builtin_styles().into_iter().find(|(i, _)| i == id).map(|(_, s)| s).ok_or_else(|| format!("スタイル {id} が見つかりません"))
}

/// 同梱テンプレート（library/snippets/*.fdtpl。JSON）。
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
