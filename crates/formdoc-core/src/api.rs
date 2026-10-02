//! Web版（formdoc-wasm）とデスクトップ版（src-tauri）が共通で使う窓口。
//! 両者はこのモジュールを薄く包むだけにし、挙動の差が出ないようにする。

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectFile {
    pub path: String,
    /// テキストファイル
    #[serde(default)]
    pub text: Option<String>,
    /// バイナリ（base64は使わず数値配列。サイズが大きい画像は set_asset を使う）
    #[serde(default)]
    pub bytes: Option<Vec<u8>>,
}

enum Mode {
    Gui { doc: Box<Document>, generated: Generated },
    Code,
}

pub struct Session {
    world: FormdocWorld,
    assets: BTreeMap<String, Vec<u8>>,
    last: Option<PagedDocument>,
    mode: Mode,
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
            assets: BTreeMap::new(),
            last: None,
            mode: Mode::Code,
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
        files
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
        let t = match template::load_template(&doc.template) {
            Ok(t) => t,
            Err(e) => {
                return UpdateResult {
                    issues: vec![Issue { block_id: None, field: None, severity: Severity::Error, code: "template".into(), message: e, fix: None }],
                    ..Default::default()
                };
            }
        };
        let report = evaluate::evaluate(&doc, &t);
        let generated = codegen::generate(&doc, &t, &report);
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
        self.mode = Mode::Gui { doc: Box::new(doc), generated };
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

    /// コードモード: プロジェクトのファイル一式（main.typ ほか）で再コンパイルする。
    pub fn update_project(&mut self, files: Vec<(String, Vec<u8>)>, template_id: &str, known: &[String]) -> UpdateResult {
        let main = files.iter().find(|(p, _)| p.trim_start_matches('/') == "main.typ").map(|(_, b)| String::from_utf8_lossy(b).to_string());
        let mut all: Vec<(String, Vec<u8>)> = self.assets.iter().map(|(p, b)| (format!("/{p}"), b.clone())).collect();
        all.extend(files.into_iter().map(|(p, b)| (format!("/{}", p.trim_start_matches('/')), b)));
        self.world.set_today(None);
        self.world.set_files(all);
        let (diagnostics, ms) = self.run();
        let mut issues: Vec<Issue> = diagnostics
            .iter()
            .map(|d| Issue {
                block_id: None,
                field: Some(format!("{}:{}", d.file.clone().unwrap_or_default(), d.line.unwrap_or(0))),
                severity: if d.severity == "error" { Severity::Error } else { Severity::Warning },
                code: "typst".into(),
                message: d.message.clone(),
                fix: None,
            })
            .collect();
        if let (Some(src), Ok(t)) = (main, template::load_template(template_id)) {
            issues.extend(lint::lint_source(&src, &t));
        }
        let exportable = !diagnostics.iter().any(|d| d.severity == "error");
        self.exportable = exportable;
        self.mode = Mode::Code;
        UpdateResult { pages: self.pages(known), issues, diagnostics, exportable, compile_ms: ms, ..Default::default() }
    }

    /// PDFを出力する。エラーが残っている文書は出力しない（品質のばらつきを防ぐ）。
    pub fn pdf(&self) -> Result<Vec<u8>, String> {
        if !self.exportable {
            return Err("エラーが残っているためPDFを出力できません。検証パネルのエラーを解消してください".into());
        }
        let doc = self.last.as_ref().ok_or("文書がまだ組版されていません")?;
        compile::render_pdf(doc, "formdoc", self.date).map_err(|e| e.join("\n"))
    }

    /// GUI文書を、コードモードで編集できるTypstソースとして書き出す。
    pub fn export_typst(&self) -> Option<String> {
        match &self.mode {
            Mode::Gui { generated, doc } => Some(format!(
                "// formDoc から書き出し（テンプレート: {} / ライブラリ {}）\n// このファイルは「VSCodeで開く」などで編集できます。\n{}",
                doc.template,
                formdoc_library::version(),
                generated.source.lines().filter(|l| !l.starts_with("// @block")).collect::<Vec<_>>().join("\n")
            )),
            Mode::Code => None,
        }
    }
}

/// テンプレートから新規文書を作る。
pub fn new_document(template_id: &str) -> Result<Document, String> {
    let t: Template = template::load_template(template_id)?;
    let blocks = t
        .skeleton
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut props = s.clone();
            let kind = props.remove("kind").and_then(|k| k.as_str().map(str::to_string)).unwrap_or_else(|| "paragraph".into());
            Block { id: format!("b{}", i + 1), kind, props }
        })
        .collect();
    Ok(Document {
        schema_version: crate::model::SCHEMA_VERSION,
        library: formdoc_library::version().into(),
        template: t.id.clone(),
        meta: Meta { title: t.name.clone(), chapter_start: 1, cover: true, ..Default::default() },
        blocks,
        assets: vec![],
    })
}

/// GUIの初期化に必要な定義一式。
#[derive(Serialize)]
pub struct Catalog {
    pub library_version: String,
    pub templates: Vec<Template>,
    pub components: serde_json::Value,
    pub references: serde_json::Value,
    pub fonts: Vec<String>,
}

pub fn catalog() -> Result<Catalog, String> {
    Ok(Catalog {
        library_version: formdoc_library::version().into(),
        templates: template::list_templates(),
        components: template::components_json()?,
        references: template::references_json()?,
        fonts: crate::world::font_families(),
    })
}

/// コードモードの新規プロジェクトの main.typ。
pub fn code_template(template_id: &str) -> Result<String, String> {
    let t = template::load_template(template_id)?;
    Ok(format!(
        r#"#import "{pkg}": *
#show: {show}.with(title: "{name}", project: none, author: none, date: none)

= 設計条件

#let L = vdef("L", 5.000, unit: "m", desc: "支間長")
#let w = vdef("w", 10.0, unit: "kN/m", desc: "等分布荷重")
#def-line(L)
#def-line(w)

= 設計計算
== 曲げモーメント #h(1fr) #kijun("鋼標準", "第Ⅰ編 4.5")

#let M = vcalc("M", "w * L^2 / 8", w, L, unit: "kN*m", digits: 2, desc: "最大曲げモーメント")
#calc-line(M)
#where-list(M, w, L)
"#,
        pkg = t.package,
        show = t.show,
        name = t.name
    ))
}
