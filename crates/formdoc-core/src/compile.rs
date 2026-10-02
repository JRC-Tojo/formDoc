//! コンパイルと出力（ページSVG / PDF）。

use serde::Serialize;
use typst::World;
use typst::WorldExt;
use typst::diag::{Severity, SourceDiagnostic};
use typst::foundations::Smart;
use typst::syntax::VirtualRoot;
use typst_layout::PagedDocument;

use crate::world::FormdocWorld;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Diagnostic {
    pub severity: &'static str,
    pub message: String,
    pub hints: Vec<String>,
    /// プロジェクト内のファイルなら "/main.typ" など。パッケージ内なら "@local/formdoc:0.1.0/src/calc.typ"。
    pub file: Option<String>,
    /// 1始まりの行・列
    pub line: Option<usize>,
    pub column: Option<usize>,
}

fn convert(world: &FormdocWorld, d: &SourceDiagnostic) -> Diagnostic {
    let mut file = None;
    let mut line = None;
    let mut column = None;
    // 本体のspanがパッケージ内（=社内ライブラリの assert 等）の場合も、呼び出し元の行を示せるよう
    // トレースの中からプロジェクト内の位置を優先して探す。
    let spans = std::iter::once(d.span).chain(d.trace.iter().map(|t| t.span.into()));
    let mut first = None;
    for span in spans {
        let Some(id) = span.id() else { continue };
        let is_project = matches!(id.get().root(), VirtualRoot::Project);
        if first.is_none() || is_project {
            first = Some((id, span));
        }
        if is_project {
            break;
        }
    }
    if let Some((id, span)) = first {
        let rooted = id.get();
        file = Some(match rooted.root() {
            VirtualRoot::Project => rooted.vpath().get_with_slash().to_string(),
            VirtualRoot::Package(spec) => format!("{spec}{}", rooted.vpath().get_with_slash()),
        });
        if let (Some(range), Ok(src)) = (world.range(span), world.source(id)) {
            if let Some((l, c)) = src.lines().byte_to_line_column(range.start) {
                line = Some(l + 1);
                column = Some(c + 1);
            }
        }
    }
    Diagnostic {
        severity: match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        },
        message: d.message.to_string(),
        hints: d.hints.iter().map(|h| h.v.to_string()).collect(),
        file,
        line,
        column,
    }
}

pub struct Compiled {
    pub document: Option<PagedDocument>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn compile(world: &FormdocWorld) -> Compiled {
    let result = typst::compile::<PagedDocument>(world);
    let mut diagnostics: Vec<Diagnostic> = result.warnings.iter().map(|d| convert(world, d)).collect();
    match result.output {
        Ok(doc) => Compiled { document: Some(doc), diagnostics },
        Err(errs) => {
            diagnostics.splice(0..0, errs.iter().map(|d| convert(world, d)));
            Compiled { document: None, diagnostics }
        }
    }
}

/// 各ページのSVG。
pub fn render_svgs(doc: &PagedDocument) -> Vec<String> {
    let opts = typst_svg::SvgOptions::default();
    doc.pages().iter().map(|p| typst_svg::svg(p, &opts)).collect()
}

/// PDF。再現性のため作成日時は文書の日付（無ければ埋め込まない）、識別子は固定値を使う。
pub fn render_pdf(doc: &PagedDocument, ident: &str, date: Option<(i32, u8, u8)>) -> Result<Vec<u8>, Vec<String>> {
    let opts = typst_pdf::PdfOptions {
        ident: Smart::Custom(ident.to_string()),
        creator: Smart::Custom(Some("formDoc".to_string())),
        timestamp: date
            .and_then(|(y, m, d)| typst::foundations::Datetime::from_ymd(y, m, d))
            .map(typst_pdf::Timestamp::new_utc),
        ..Default::default()
    };
    typst_pdf::pdf(doc, &opts).map_err(|errs| errs.iter().map(|e| e.message.to_string()).collect())
}
