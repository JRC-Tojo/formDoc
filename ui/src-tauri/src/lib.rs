//! デスクトップ版。組版・計算は formdoc-core::api をそのまま呼ぶ（Web版の wasm と同じコード）。
//! デスクトップ専用の機能（ローカルファイルの読み書き、フォルダ監視、VSCode起動）だけをここで実装する。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use formdoc_core::api::{self, Session};
use formdoc_core::Document;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
struct AppState {
    session: Mutex<Session>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

type R<T> = Result<T, String>;

fn raw_body<'a>(req: &'a Request<'_>) -> R<(&'a [u8], String)> {
    let InvokeBody::Raw(bytes) = req.body() else { return Err("バイナリ本文が必要です".into()) };
    let path = req
        .headers()
        .get("x-path")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or("x-path ヘッダーが必要です")?;
    Ok((bytes.as_slice(), path))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------- エンジン（Web版の formdoc-wasm と同じ窓口） ----------

#[tauri::command]
fn catalog() -> R<api::Catalog> {
    api::catalog()
}

#[tauri::command]
fn new_document(template: String) -> R<Document> {
    api::new_document(&template)
}

#[tauri::command]
fn code_template(template: String) -> R<String> {
    api::code_template(&template)
}

#[tauri::command]
async fn update_document(state: State<'_, AppState>, doc: Document, known: Vec<String>) -> R<api::UpdateResult> {
    Ok(state.session.lock().unwrap().update_document(doc, &known))
}

#[tauri::command]
async fn update_project(state: State<'_, AppState>, files: Vec<ProjectFileIn>, template: String, known: Vec<String>) -> R<api::UpdateResult> {
    let files = files.into_iter().map(|f| (f.path, f.text.map(String::into_bytes).or(f.bytes).unwrap_or_default())).collect();
    Ok(state.session.lock().unwrap().update_project(files, &template, &known))
}

#[tauri::command]
fn set_asset(state: State<'_, AppState>, request: Request<'_>) -> R<()> {
    let (bytes, path) = raw_body(&request)?;
    state.session.lock().unwrap().set_asset(&path, bytes.to_vec());
    Ok(())
}

#[tauri::command]
fn remove_asset(state: State<'_, AppState>, path: String) {
    state.session.lock().unwrap().remove_asset(&path);
}

#[tauri::command]
async fn pdf(state: State<'_, AppState>) -> R<Response> {
    Ok(Response::new(state.session.lock().unwrap().pdf()?))
}

#[tauri::command]
fn export_typst(state: State<'_, AppState>) -> Option<String> {
    state.session.lock().unwrap().export_typst()
}

// ---------- ファイル（デスクトップのみ） ----------

#[tauri::command]
async fn read_file(path: String) -> R<Response> {
    std::fs::read(&path).map(Response::new).map_err(|e| format!("{path} を読めません: {e}"))
}

#[tauri::command]
fn write_file(request: Request<'_>) -> R<()> {
    let (bytes, path) = raw_body(&request)?;
    std::fs::write(&path, bytes).map_err(|e| format!("{path} に保存できません: {e}"))
}

// ---------- コードモードのプロジェクトフォルダ ----------

#[derive(Deserialize)]
struct ProjectFileIn {
    path: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    bytes: Option<Vec<u8>>,
}

#[derive(Serialize)]
struct ProjectFileOut {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<Vec<u8>>,
}

const TEXT_EXT: &[&str] = &["typ", "toml", "json", "csv", "txt", "yml", "yaml", "bib"];
const BIN_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "pdf", "webp"];

fn walk(root: &Path, dir: &Path, out: &mut Vec<ProjectFileOut>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || matches!(name, "node_modules" | "target") {
            continue;
        }
        if p.is_dir() {
            walk(root, &p, out)?;
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        if TEXT_EXT.contains(&ext.as_str()) {
            out.push(ProjectFileOut { path: rel, text: Some(std::fs::read_to_string(&p)?), bytes: None });
        } else if BIN_EXT.contains(&ext.as_str()) {
            out.push(ProjectFileOut { path: rel, text: None, bytes: Some(std::fs::read(&p)?) });
        }
    }
    Ok(())
}

#[tauri::command]
async fn read_project(folder: String) -> R<Vec<ProjectFileOut>> {
    let root = PathBuf::from(&folder);
    let mut out = Vec::new();
    walk(&root, &root, &mut out).map_err(|e| format!("{folder} を読めません: {e}"))?;
    Ok(out)
}

#[tauri::command]
fn write_project_file(folder: String, path: String, text: String) -> R<()> {
    let p = PathBuf::from(&folder).join(&path);
    if path.contains("..") {
        return Err("プロジェクト外には書き込めません".into());
    }
    std::fs::write(&p, text).map_err(|e| format!("{} に書き込めません: {e}", p.display()))
}

#[tauri::command]
fn open_in_vscode(folder: String) -> R<()> {
    let main = PathBuf::from(&folder).join("main.typ");
    #[cfg(windows)]
    let status = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // code は code.cmd のため cmd 経由で起動する
        std::process::Command::new("cmd")
            .args(["/C", "code", &folder, &main.to_string_lossy()])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
    };
    #[cfg(not(windows))]
    let status = std::process::Command::new("code").arg(&folder).arg(&main).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => {
            // VSCode が無い場合はフォルダを開く
            #[cfg(windows)]
            let _ = std::process::Command::new("explorer").arg(&folder).spawn();
            Err("VSCode（code コマンド）が見つかりませんでした。フォルダを開きました。VSCodeの「シェルコマンド: PATH内に 'code' コマンドをインストール」を確認してください".into())
        }
    }
}

#[tauri::command]
fn watch_project(app: AppHandle, state: State<'_, AppState>, folder: String) -> R<()> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if ev.kind.is_modify() || ev.kind.is_create() || ev.kind.is_remove() {
                let _ = app.emit("project-changed", ());
            }
        }
    })
    .map_err(|e| e.to_string())?;
    watcher.watch(Path::new(&folder), RecursiveMode::Recursive).map_err(|e| e.to_string())?;
    *state.watcher.lock().unwrap() = Some(watcher);
    Ok(())
}

#[tauri::command]
fn unwatch_project(state: State<'_, AppState>) {
    *state.watcher.lock().unwrap() = None;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            catalog,
            new_document,
            code_template,
            update_document,
            update_project,
            set_asset,
            remove_asset,
            pdf,
            export_typst,
            read_file,
            write_file,
            read_project,
            write_project_file,
            open_in_vscode,
            watch_project,
            unwatch_project,
        ])
        .run(tauri::generate_context!())
        .expect("formDoc の起動に失敗しました");
}
