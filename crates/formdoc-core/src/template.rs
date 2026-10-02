//! テンプレート（文書の型）と部品定義の読み込み。どちらも埋め込みライブラリ内のTOML。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Template {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub package: String,
    pub show: String,
    #[serde(default = "four")]
    pub max_heading_level: u8,
    #[serde(default)]
    pub rounding: formdoc_expr::Rounding,
    pub blocks: Vec<String>,
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
}

pub fn load_template(id: &str) -> Result<Template, String> {
    let bytes = formdoc_library::get(&format!("templates/{id}/template.toml"))
        .ok_or_else(|| format!("テンプレート {id} が見つかりません"))?;
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    toml::from_str(text).map_err(|e| format!("テンプレート {id} の定義に誤りがあります: {e}"))
}

pub fn list_templates() -> Vec<Template> {
    let mut ids: Vec<String> = formdoc_library::files()
        .filter_map(|(p, _)| p.strip_prefix("templates/")?.strip_suffix("/template.toml").map(str::to_string))
        .collect();
    ids.sort();
    ids.iter().filter_map(|id| load_template(id).ok()).collect()
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
