//! 使い方:
//!   formdoc compile <プロジェクトフォルダ> <出力.pdf> [--svg <出力フォルダ>]
//!   formdoc gui <document.json> <出力.pdf> [--style <style.typ>] [--typst <出力.typ>]
//!   formdoc fonts

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if p.is_dir() {
            collect(root, &p, out)?;
        } else {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push((format!("/{rel}"), std::fs::read(&p)?));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fonts") => {
            for f in formdoc_core::font_families() {
                println!("{f}");
            }
            ExitCode::SUCCESS
        }
        Some("gui") if args.len() >= 3 => {
            // GUI文書（document.json）を評価・組版する。--typst <file> で生成ソースも書き出す
            let text = std::fs::read_to_string(&args[1]).expect("document.json を読めません");
            let doc: formdoc_core::Document = serde_json::from_str(&text).expect("document.json の形式が不正です");
            // 文書テンプレートは --style <file.typ>。省略時は文書の template と同じ id の同梱文書テンプレート
            let style = match args.iter().position(|a| a == "--style") {
                Some(i) => std::fs::read_to_string(&args[i + 1]).expect("文書テンプレートを読めません"),
                None => formdoc_core::template::builtin_style(&doc.template).expect("同梱文書テンプレートがありません"),
            };
            let mut s = formdoc_core::Session::new();
            if let Err(e) = s.set_style(&style) {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
            let r = s.update_document(doc, &[]);
            for i in &r.issues {
                eprintln!("{:?} [{}] {}: {}", i.severity, i.block_id.as_deref().unwrap_or("-"), i.code, i.message);
            }
            eprintln!("{} pages, {} vars, exportable={}, {:.0} ms", r.pages.len(), r.vars.len(), r.exportable, r.compile_ms);
            if let Some(i) = args.iter().position(|a| a == "--typst") {
                std::fs::write(&args[i + 1], s.code().unwrap_or_default()).unwrap();
            }
            match s.pdf() {
                Ok(pdf) => {
                    std::fs::write(&args[2], pdf).unwrap();
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("compile") if args.len() >= 3 => {
            let root = PathBuf::from(&args[1]);
            let mut files = Vec::new();
            if let Err(e) = collect(&root, &root, &mut files) {
                eprintln!("フォルダを読み込めません: {e}");
                return ExitCode::FAILURE;
            }
            let mut world = formdoc_core::FormdocWorld::new();
            world.set_files(files);
            let t = std::time::Instant::now();
            let c = formdoc_core::compile(&world);
            for d in &c.diagnostics {
                eprintln!(
                    "{}: {} ({}:{}:{}){}",
                    d.severity,
                    d.message,
                    d.file.as_deref().unwrap_or("?"),
                    d.line.unwrap_or(0),
                    d.column.unwrap_or(0),
                    d.hints.iter().map(|h| format!("\n  hint: {h}")).collect::<String>()
                );
            }
            let Some(doc) = c.document else { return ExitCode::FAILURE };
            eprintln!("compiled {} pages in {:?}", doc.pages().len(), t.elapsed());
            match formdoc_core::render_pdf(&doc, "formdoc", None) {
                Ok(pdf) => std::fs::write(&args[2], pdf).unwrap(),
                Err(e) => {
                    eprintln!("{e:?}");
                    return ExitCode::FAILURE;
                }
            }
            if let Some(i) = args.iter().position(|a| a == "--svg") {
                let dir = PathBuf::from(&args[i + 1]);
                std::fs::create_dir_all(&dir).unwrap();
                for (n, svg) in formdoc_core::render_svgs(&doc).iter().enumerate() {
                    std::fs::write(dir.join(format!("page-{:02}.svg", n + 1)), svg).unwrap();
                }
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: formdoc compile <dir> <out.pdf> [--svg <dir>] | formdoc fonts");
            ExitCode::FAILURE
        }
    }
}
