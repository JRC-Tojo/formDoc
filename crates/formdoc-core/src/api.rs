//! Web版（formdoc-wasm）とデスクトップ版（src-tauri）が共通で使う窓口。
//! 両者はこのモジュールを薄く包むだけにし、挙動の差が出ないようにする。

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;
use typst_layout::PagedDocument;

use crate::codegen::{self, Generated};
use crate::compile::{self, Diagnostic};
use crate::evaluate::{self, Issue, Report, Severity, VarInfo};
use crate::lint;
use crate::model::{Block, Document, Meta};
use crate::template::{self, Template};
use crate::world::FormdocWorld;

#[derive(Debug, Clone, Serialize)]
pub struct PageOut {
    pub hash: String,
    /// 呼び出し側が既に持っているページ（known に含まれる hash）は None
    pub svg: Option<String>,
    pub width_pt: f64,
    pub height_pt: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateResult {
    pub pages: Vec<PageOut>,
    pub issues: Vec<Issue>,
    pub vars: Vec<VarInfo>,
    pub blocks: BTreeMap<String, evaluate::BlockResult>,
    /// Typstコンパイラの診断（GUIモードでは原因ブロックIDを付けて issues にも入れる）
    pub diagnostics: Vec<Diagnostic>,
    /// PDF出力できるか（エラーがあると出力させない）
    pub exportable: bool,
    pub compile_ms: f64,
}

/// 直前に組版した文書と、コードモードで見せるコード（エラーのある部品も赤枠にしない）
struct Last {
    doc: Box<Document>,
    view: Generated,
}

pub struct Session {
    world: FormdocWorld,
    /// GUIモードのスタイル（ソースと、評価した info）
    style: Option<(String, Template)>,
    assets: BTreeMap<String, Vec<u8>>,
    last: Option<PagedDocument>,
    mode: Option<Last>,
    exportable: bool,
    date: Option<(i32, u8, u8)>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            world: FormdocWorld::new(),
            style: None,
            assets: BTreeMap::new(),
            last: None,
            mode: None,
            exportable: false,
            date: None,
        }
    }

    /// 画像などの添付ファイルを登録する（パスは "assets/xxx.png"）。
    pub fn set_asset(&mut self, path: &str, bytes: Vec<u8>) {
        self.assets.insert(path.trim_start_matches('/').to_string(), bytes);
    }

    pub fn remove_asset(&mut self, path: &str) {
        self.assets.remove(path.trim_start_matches('/'));
    }

    fn files_with(&self, main: &str) -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<(String, Vec<u8>)> = self.assets.iter().map(|(p, b)| (format!("/{p}"), b.clone())).collect();
        files.push(("/main.typ".into(), main.as_bytes().to_vec()));
        if let Some((src, _)) = &self.style {
            files.push(("/style.typ".into(), src.as_bytes().to_vec()));
        }
        files
    }

    /// GUIモードのスタイルを設定する（Typstソース）。同じソースなら評価し直さない。
    pub fn set_style(&mut self, source: &str) -> Result<Template, String> {
        if let Some((src, t)) = &self.style {
            if src == source {
                return Ok(t.clone());
            }
        }
        let t = template::parse_style(source)?;
        self.style = Some((source.to_string(), t.clone()));
        Ok(t)
    }

    /// 現在のスタイルで新規文書を作る（文書情報の既定値と骨組み）。
    pub fn new_document(&self) -> Result<Document, String> {
        let (_, t) = self.style.as_ref().ok_or("スタイルが選ばれていません")?;
        Ok(new_document(t))
    }

    fn pages(&self, known: &[String]) -> Vec<PageOut> {
        let Some(doc) = &self.last else { return vec![] };
        let known: HashSet<&str> = known.iter().map(String::as_str).collect();
        let opts = typst_svg::SvgOptions::default();
        doc.pages()
            .iter()
            .map(|p| {
                let hash = format!("{:032x}", typst::utils::hash128(&p.frame));
                let size = p.frame.size();
                PageOut {
                    svg: (!known.contains(hash.as_str())).then(|| typst_svg::svg(p, &opts)),
                    hash,
                    width_pt: size.x.to_pt(),
                    height_pt: size.y.to_pt(),
                }
            })
            .collect()
    }

    fn run(&mut self) -> (Vec<Diagnostic>, f64) {
        let t0 = now_ms();
        let c = compile::compile(&self.world);
        if let Some(d) = c.document {
            self.last = Some(d);
        } else if c.diagnostics.iter().any(|d| d.severity == "error") {
            // エラー時は直前の成功結果を表示し続ける（入力途中でプレビューが消えないように）
        }
        (c.diagnostics, now_ms() - t0)
    }

    /// GUI文書を更新して再計算・再コンパイルする。
    pub fn update_document(&mut self, doc: Document, known: &[String]) -> UpdateResult {
        let Some((_, t)) = self.style.clone() else {
            return UpdateResult {
                issues: vec![Issue { block_id: None, field: None, severity: Severity::Error, code: "style".into(), message: "スタイルを選んでください（文書情報）".into(), fix: None }],
                ..Default::default()
            };
        };
        let mut report = evaluate::evaluate(&doc, &t);
        for f in t.fields.iter().filter(|f| f.required) {
            if doc.meta.get(&f.key).and_then(|v| v.as_str()).is_none_or(|s| s.trim().is_empty()) && f.default.is_none() {
                report.issues.push(Issue {
                    block_id: None,
                    field: Some(format!("meta.{}", f.key)),
                    severity: Severity::Error,
                    code: "required".into(),
                    message: format!("文書情報の「{}」を入力してください", f.label),
                    fix: None,
                });
            }
        }
        let generated = codegen::generate(&doc, &t, &report);
        let view = codegen::generate_with(&doc, &t, &report, false);
        self.date = doc.meta.ymd();
        self.world.set_today(self.date);
        self.world.set_files(self.files_with(&generated.source));
        let (diagnostics, ms) = self.run();
        let Report { vars, blocks, mut issues } = report;
        issues.extend(lint::lint(&doc, &t));
        // Typstのエラーを原因ブロックに結び付ける
        for d in &diagnostics {
            if d.severity != "error" {
                continue;
            }
            let block = d.line.filter(|_| d.file.as_deref() == Some("/main.typ")).and_then(|l| generated.block_at(l)).map(str::to_string);
            issues.push(Issue {
                block_id: block,
                field: None,
                severity: Severity::Error,
                code: "typst".into(),
                message: format!("組版エラー: {}{}", d.message, d.hints.first().map(|h| format!("（{h}）")).unwrap_or_default()),
                fix: None,
            });
        }
        let exportable = !issues.iter().any(|i| i.severity == Severity::Error);
        self.exportable = exportable;
        self.mode = Some(Last { doc: Box::new(doc), view });
        UpdateResult {
            pages: self.pages(known),
            issues,
            vars,
            blocks: blocks.into_iter().collect(),
            diagnostics,
            exportable,
            compile_ms: ms,
        }
    }

    /// PDFを出力する。エラーが残っている文書は出力しない（品質のばらつきを防ぐ）。
    pub fn pdf(&self) -> Result<Vec<u8>, String> {
        if !self.exportable {
            return Err("エラーが残っているためPDFを出力できません。検証パネルのエラーを解消してください".into());
        }
        let doc = self.last.as_ref().ok_or("文書がまだ組版されていません")?;
        compile::render_pdf(doc, "formdoc", self.date).map_err(|e| e.join("\n"))
    }

    /// コードモードで見せるコード（直前に組版した文書を、部品ごとの目印つきの Typst にしたもの）。
    pub fn code(&self) -> Option<String> {
        self.mode.as_ref().map(|m| m.view.source.clone())
    }

    /// コードモードの編集を、直前に組版した文書に戻す（変わった部品は Typstコード部品になる）。
    pub fn apply_code(&self, code: &str) -> Result<crate::code::Applied, String> {
        let m = self.mode.as_ref().ok_or("文書がまだ組版されていません")?;
        let (_, t) = self.style.as_ref().ok_or("スタイルが選ばれていません")?;
        Ok(crate::code::apply(&m.doc, &m.view.codes, &codegen::header(&m.doc, t), code))
    }
}

/// スタイルの既定値と骨組みから新規文書を作る（ブロックIDは b1, b2 …。GUIは付け直す）。
pub fn new_document(t: &Template) -> Document {
    let blocks = t
        .skeleton
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut props = s.clone();
            let kind = props.remove("kind").and_then(|k| k.as_str().map(str::to_string)).unwrap_or_else(|| "paragraph".into());
            Block { id: format!("b{}", i + 1), kind, props, children: vec![] }
        })
        .collect();
    let mut meta = Meta::default();
    for f in &t.fields {
        if let Some(d) = &f.default {
            meta.set(&f.key, d.clone());
        }
    }
    Document {
        schema_version: crate::model::SCHEMA_VERSION,
        library: formdoc_library::version().into(),
        template: t.id.clone(),
        meta,
        blocks,
        assets: vec![],
    }
}

/// 同梱スタイル（ファイル名とソース）。
#[derive(Serialize)]
pub struct StyleSource {
    pub file: String,
    pub source: String,
}

/// GUIの初期化に必要な定義一式。
#[derive(Serialize)]
pub struct Catalog {
    pub library_version: String,
    /// 同梱スタイル。デスクトップ版は起動時にシステムフォルダへ（無いものだけ）コピーする
    pub styles: Vec<StyleSource>,
    /// 同梱テンプレート（.fdtpl の中身）
    pub snippets: Vec<serde_json::Value>,
    pub components: serde_json::Value,
    pub references: serde_json::Value,
    pub fonts: Vec<String>,
}

pub fn catalog() -> Result<Catalog, String> {
    Ok(Catalog {
        library_version: formdoc_library::version().into(),
        styles: template::builtin_styles().into_iter().map(|(id, source)| StyleSource { file: format!("{id}.typ"), source }).collect(),
        snippets: template::builtin_snippets(),
        components: template::components_json()?,
        references: template::references_json()?,
        fonts: crate::world::font_families(),
    })
}

/// スタイルの info だけを読む（スタイル一覧の表示用。セッションの状態は変えない）。
pub fn style_info(source: &str) -> Result<Template, String> {
    template::parse_style(source)
}
