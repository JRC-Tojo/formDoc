//! Web版の窓口。formdoc-core::api を JSON 文字列でやり取りするだけの薄い層。
//! デスクトップ版（src-tauri）も同じ api を呼ぶため、両者の挙動は一致する。

use std::cell::RefCell;

use formdoc_core::api::{self, Session};
use wasm_bindgen::prelude::*;

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
}

fn err(e: impl ToString) -> JsError {
    JsError::new(&e.to_string())
}

fn known(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

#[wasm_bindgen]
pub fn catalog() -> Result<String, JsError> {
    serde_json::to_string(&api::catalog().map_err(err)?).map_err(err)
}

/// スタイルの info を読む（一覧表示用。セッションは変えない）。
#[wasm_bindgen]
pub fn style_info(source: &str) -> Result<String, JsError> {
    serde_json::to_string(&api::style_info(source).map_err(err)?).map_err(err)
}

/// GUIモードのスタイルを設定し、その info を返す。
#[wasm_bindgen]
pub fn set_style(source: &str) -> Result<String, JsError> {
    let t = SESSION.with(|s| s.borrow_mut().set_style(source)).map_err(err)?;
    serde_json::to_string(&t).map_err(err)
}

#[wasm_bindgen]
pub fn new_document() -> Result<String, JsError> {
    let d = SESSION.with(|s| s.borrow().new_document()).map_err(err)?;
    serde_json::to_string(&d).map_err(err)
}

#[wasm_bindgen]
pub fn code_template() -> Result<String, JsError> {
    SESSION.with(|s| s.borrow().code_template()).map_err(err)
}

/// GUI文書を更新する。known は既に表示済みのページハッシュ（JSON配列）。
#[wasm_bindgen]
pub fn update_document(doc_json: &str, known_json: &str) -> Result<String, JsError> {
    let doc = serde_json::from_str(doc_json).map_err(|e| err(format!("文書の形式が不正です: {e}")))?;
    let r = SESSION.with(|s| s.borrow_mut().update_document(doc, &known(known_json)));
    serde_json::to_string(&r).map_err(err)
}

/// コードモード。files_json は [{"path": "main.typ", "text": "..."}]
#[wasm_bindgen]
pub fn update_project(files_json: &str, known_json: &str) -> Result<String, JsError> {
    let files: Vec<api::ProjectFile> = serde_json::from_str(files_json).map_err(err)?;
    let files = files
        .into_iter()
        .map(|f| (f.path, f.text.map(String::into_bytes).or(f.bytes).unwrap_or_default()))
        .collect();
    let r = SESSION.with(|s| s.borrow_mut().update_project(files, &known(known_json)));
    serde_json::to_string(&r).map_err(err)
}

#[wasm_bindgen]
pub fn set_asset(path: &str, bytes: Vec<u8>) {
    SESSION.with(|s| s.borrow_mut().set_asset(path, bytes));
}

#[wasm_bindgen]
pub fn remove_asset(path: &str) {
    SESSION.with(|s| s.borrow_mut().remove_asset(path));
}

#[wasm_bindgen]
pub fn pdf() -> Result<Vec<u8>, JsError> {
    SESSION.with(|s| s.borrow().pdf()).map_err(err)
}

#[wasm_bindgen]
pub fn export_typst() -> Option<String> {
    SESSION.with(|s| s.borrow().export_typst())
}
