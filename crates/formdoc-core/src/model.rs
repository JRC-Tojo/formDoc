//! GUI文書のデータモデル（document.json）。
//!
//! 文書はブロックの平坦な列。章立ては見出しブロックの階層から導出する（並べ替えが単純になる）。
//! ブロックの中身（props）は部品定義（library/components/components.toml）に従う自由形式のJSON。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    /// 作成に使ったライブラリの版。開いた環境の版と異なれば警告する。
    #[serde(default)]
    pub library: String,
    pub template: String,
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub blocks: Vec<Block>,
    /// 文書に添付したファイル（画像など）のパス一覧。中身は保存形式側で持つ。
    #[serde(default)]
    pub assets: Vec<String>,
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Meta {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub author: String,
    /// "2026-10-02" 形式
    #[serde(default)]
    pub date: String,
    #[serde(default = "one")]
    pub chapter_start: u32,
    #[serde(default = "yes")]
    pub cover: bool,
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

impl Meta {
    pub fn ymd(&self) -> Option<(i32, u8, u8)> {
        let mut it = self.date.split(['-', '/', '.']);
        let y = it.next()?.trim().parse().ok()?;
        let m = it.next()?.trim().parse().ok()?;
        let d = it.next()?.trim().parse().ok()?;
        Some((y, m, d))
    }

    /// 表紙に出す日付（和文表記）。
    pub fn date_text(&self) -> String {
        match self.ymd() {
            Some((y, m, _)) => format!("{y}年{m}月"),
            None => self.date.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub props: Map<String, Value>,
}

impl Block {
    pub fn str(&self, key: &str) -> &str {
        self.props.get(key).and_then(Value::as_str).unwrap_or("")
    }

    pub fn opt_str(&self, key: &str) -> Option<&str> {
        self.props.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty())
    }

    pub fn num(&self, key: &str) -> Option<f64> {
        match self.props.get(key)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        self.num(key).map(|v| v.round() as i64)
    }

    pub fn bool(&self, key: &str, default: bool) -> bool {
        self.props.get(key).and_then(Value::as_bool).unwrap_or(default)
    }

    pub fn arr(&self, key: &str) -> &[Value] {
        self.props.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// 表のセル。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Cell {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub rowspan: Option<u32>,
    #[serde(default)]
    pub colspan: Option<u32>,
    #[serde(default)]
    pub vertical: bool,
    #[serde(default)]
    pub align: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Grid {
    #[serde(default)]
    pub header_rows: u32,
    #[serde(default)]
    pub rows: Vec<Vec<Cell>>,
    /// 列幅（"auto", "1fr", "3em" …）。空なら自動。
    #[serde(default)]
    pub widths: Vec<String>,
}
