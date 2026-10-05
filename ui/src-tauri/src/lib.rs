//! デスクトップ版。組版・計算は formdoc-core::api をそのまま呼ぶ（Web版の wasm と同じコード）。
//! デスクトップ専用の機能（ローカルファイルの読み書き、フォルダ監視、VSCode起動、システムフォルダ）だけをここで実装する。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use formdoc_core::api::{self, Session};
use formdoc_core::Document;
use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Emitter, Manager, State};

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
fn style_info(source: String) -> R<formdoc_core::template::Template> {
    api::style_info(&source)
}

#[tauri::command]
fn set_style(state: State<'_, AppState>, source: String) -> R<formdoc_core::template::Template> {
    state.session.lock().unwrap().set_style(&source)
}

#[tauri::command]
fn new_document(state: State<'_, AppState>) -> R<Document> {
    state.session.lock().unwrap().new_document()
}

/// 足りない必須の章を追加した文書を返す。
#[tauri::command]
fn complete_chapters(state: State<'_, AppState>, doc: Document) -> R<Document> {
    state.session.lock().unwrap().complete_chapters(&doc)
}

#[tauri::command]
async fn update_document(state: State<'_, AppState>, doc: Document, known: Vec<String>) -> R<api::UpdateResult> {
    Ok(state.session.lock().unwrap().update_document(doc, &known))
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

/// コードモードで見せるコード（直前に組版した文書の Typst）。
#[tauri::command]
fn code(state: State<'_, AppState>) -> Option<String> {
    state.session.lock().unwrap().code()
}

/// コードモードの編集を文書に戻す。
#[tauri::command]
fn apply_code(state: State<'_, AppState>, code: String) -> R<formdoc_core::code::Applied> {
    state.session.lock().unwrap().apply_code(&code)
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
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} を作れません: {e}", dir.display()))?;
    }
    std::fs::write(&p, text).map_err(|e| format!("{} に書き込めません: {e}", p.display()))
}

/// VSCode の実行ファイルを探す（PATH に code が無くても、既定のインストール先なら起動できるように）。
#[cfg(windows)]
fn find_vscode() -> Option<PathBuf> {
    let mut cands = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        cands.push(PathBuf::from(local).join("Programs").join("Microsoft VS Code").join("Code.exe"));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(pf) = std::env::var(var) {
            cands.push(PathBuf::from(pf).join("Microsoft VS Code").join("Code.exe"));
        }
    }
    cands.into_iter().find(|p| p.exists())
}

#[tauri::command]
fn open_in_vscode(folder: String) -> R<()> {
    let main = PathBuf::from(&folder).join("main.typ");
    #[cfg(windows)]
    let ok = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        match find_vscode() {
            Some(exe) => std::process::Command::new(exe).arg(&folder).arg(&main).spawn().is_ok(),
            // code は code.cmd のため cmd 経由で起動する。空白・日本語を含むパスは引用符で囲む
            None => std::process::Command::new("cmd")
                .raw_arg(format!("/C code \"{}\" \"{}\"", folder, main.display()))
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .is_ok_and(|s| s.success()),
        }
    };
    #[cfg(not(windows))]
    let ok = std::process::Command::new("code").arg(&folder).arg(&main).spawn().is_ok();
    if ok {
        return Ok(());
    }
    // VSCode が無い場合はフォルダを開く
    let _ = open_path(folder);
    Err("VSCode が見つかりませんでした。フォルダを開きました。VSCodeをインストールするか、VSCodeで「シェルコマンド: PATH内に 'code' コマンドをインストール」を実行してください".into())
}

/// フォルダ（またはファイル）を OS の既定のアプリで開く。
#[tauri::command]
fn open_path(path: String) -> R<()> {
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&path).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(&path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(&path).spawn();
    r.map(|_| ()).map_err(|e| format!("{path} を開けません: {e}"))
}

// ---------- システムフォルダ（設定・文書テンプレート・部品テンプレート） ----------

/// システムフォルダ（Windows: %APPDATA%\formDoc）。無ければ作り、同梱文書テンプレートのうち
/// まだ無いものをコピーする（利用者が編集したファイルは上書きしない）。
fn system_root(app: &AppHandle) -> R<PathBuf> {
    let root = app.path().data_dir().map_err(|e| e.to_string())?.join("formDoc");
    for sub in ["styles", "templates", "projects"] {
        std::fs::create_dir_all(root.join(sub)).map_err(|e| format!("{} を作れません: {e}", root.display()))?;
    }
    for st in api::catalog()?.styles {
        let p = root.join("styles").join(&st.file);
        if !p.exists() {
            let _ = std::fs::write(&p, st.source);
        }
    }
    Ok(root)
}

#[derive(Serialize)]
struct SystemInfo {
    root: String,
    styles: String,
    templates: String,
    projects: String,
}

#[tauri::command]
fn system_info(app: AppHandle) -> R<SystemInfo> {
    let root = system_root(&app)?;
    let s = |p: PathBuf| p.to_string_lossy().into_owned();
    Ok(SystemInfo { styles: s(root.join("styles")), templates: s(root.join("templates")), projects: s(root.join("projects")), root: s(root) })
}

#[tauri::command]
fn read_settings(app: AppHandle) -> R<Option<String>> {
    Ok(std::fs::read_to_string(system_root(&app)?.join("settings.json")).ok())
}

#[tauri::command]
fn write_settings(app: AppHandle, text: String) -> R<()> {
    std::fs::write(system_root(&app)?.join("settings.json"), text).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct TextFile {
    path: String,
    /// 読み込んだフォルダ
    folder: String,
    text: String,
}

/// フォルダ直下の指定拡張子のファイルを読む（読めないファイルは飛ばす）。
fn read_dir_ext(dir: &Path, ext: &str, out: &mut Vec<TextFile>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
            if let Ok(text) = std::fs::read_to_string(&p) {
                out.push(TextFile { path: p.to_string_lossy().into_owned(), folder: dir.to_string_lossy().into_owned(), text });
            }
        }
    }
}

#[tauri::command]
fn list_styles(app: AppHandle) -> R<Vec<TextFile>> {
    let mut out = Vec::new();
    read_dir_ext(&system_root(&app)?.join("styles"), "typ", &mut out);
    Ok(out)
}

/// 部品テンプレート（.fdtpl）を、システムフォルダと指定フォルダから読む。
#[tauri::command]
fn list_templates(app: AppHandle, folders: Vec<String>) -> R<Vec<TextFile>> {
    let mut out = Vec::new();
    read_dir_ext(&system_root(&app)?.join("templates"), "fdtpl", &mut out);
    for f in folders {
        read_dir_ext(Path::new(&f), "fdtpl", &mut out);
    }
    Ok(out)
}

/// 起動時に渡されたファイル（.fdoc をダブルクリックして起動した場合）。
#[tauri::command]
fn startup_file() -> Option<String> {
    std::env::args().skip(1).find(|a| {
        let l = a.to_ascii_lowercase();
        (l.ends_with(".fdoc") || l.ends_with(".json")) && Path::new(a).exists()
    })
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
        // 自動更新：GitHub Releases の latest.json を見て、新しい版をダウンロード・インストールし、再起動する
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            catalog,
            style_info,
            set_style,
            new_document,
            complete_chapters,
            update_document,
            set_asset,
            remove_asset,
            pdf,
            code,
            apply_code,
            read_file,
            write_file,
            read_project,
            write_project_file,
            open_in_vscode,
            open_path,
            system_info,
            read_settings,
            write_settings,
            list_styles,
            list_templates,
            startup_file,
            watch_project,
            unwatch_project,
        ])
        .run(tauri::generate_context!())
        .expect("formDoc の起動に失敗しました");
}
