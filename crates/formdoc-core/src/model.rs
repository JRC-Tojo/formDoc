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
    /// スタイルの id（スタイルの中身は保存ファイルに同梱する）
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

/// 文書情報（表紙など）。キーはスタイルの `info.fields` の key（= style 関数の引数名）。
/// 旧形式の `chapter_start` は読み込み時に `chapter-start` に読み替える。
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(transparent)]
pub struct Meta(pub Map<String, Value>);

impl<'de> Deserialize<'de> for Meta {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut m = Map::<String, Value>::deserialize(d)?;
        if let Some(v) = m.remove("chapter_start") {
            m.entry("chapter-start").or_insert(v);
        }
        Ok(Meta(m))
    }
}

impl Meta {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key).filter(|v| !v.is_null())
    }

    pub fn str(&self, key: &str) -> &str {
        self.get(key).and_then(Value::as_str).unwrap_or("")
    }

    pub fn set(&mut self, key: &str, v: impl Into<Value>) {
        self.0.insert(key.to_string(), v.into());
    }

    /// 日付（"2026-10-02" 形式）を年月日にする。
    pub fn ymd_of(&self, key: &str) -> Option<(i32, u8, u8)> {
        parse_ymd(self.str(key))
    }

    /// PDFの作成日時に使う日付（"date" 欄）。
    pub fn ymd(&self) -> Option<(i32, u8, u8)> {
        self.ymd_of("date")
    }
}

pub fn parse_ymd(s: &str) -> Option<(i32, u8, u8)> {
    let mut it = s.split(['-', '/', '.']);
    let y = it.next()?.trim().parse().ok()?;
    let m = it.next()?.trim().parse().ok()?;
    let d = it.next()?.trim().parse().ok()?;
    Some((y, m, d))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub props: Map<String, Value>,
    /// 子要素（"group" ＝ 挿入したテンプレートのまとまり）。見出しの配下は並び順から決まるため子要素にはしない
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Block>,
}

/// 子要素も含め、文書の並び順（深さ優先）にすべての部品を並べる。
pub fn walk_blocks<'a>(blocks: &'a [Block], out: &mut Vec<&'a Block>) {
    for b in blocks {
        out.push(b);
        walk_blocks(&b.children, out);
    }
}

impl Document {
    pub fn all_blocks(&self) -> Vec<&Block> {
        let mut out = Vec::new();
        walk_blocks(&self.blocks, &mut out);
        out
    }
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
