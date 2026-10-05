//! Web版の窓口。formdoc-core::api を JSON 文字列でやり取りするだけの薄い層。
//! デスクトップ版（src-tauri）も同じ api を呼ぶため、両者の挙動は一致する。

use std::cell::RefCell;

use formdoc_core::api::{self, Session};
use wasm_bindgen::prelude::*;

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session::new());
    /// init_fonts までに add_font で受け取ったフォント
    static PENDING_FONTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn err(e: impl ToString) -> JsError {
    JsError::new(&e.to_string())
}

fn known(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

/// 同梱フォントのファイル一覧（JSON 配列。[{file, sha256, size}]）。フォントを読み込む前に呼べる。
#[wasm_bindgen]
pub fn font_files() -> Result<String, JsError> {
    serde_json::to_string(&api::font_files()).map_err(err)
}

/// 取得した同梱フォントを1つ渡す。`font_files` と同じ順に呼ぶ（まとめて渡すとメモリを一時的に倍使うため1つずつ）。
#[wasm_bindgen]
pub fn add_font(data: Vec<u8>) {
    PENDING_FONTS.with(|f| f.borrow_mut().push(data));
}

/// add_font で渡したフォントを確定する。ほかの関数（組版）より前に1回だけ呼ぶ。
#[wasm_bindgen]
pub fn init_fonts() -> Result<(), JsError> {
    let fonts = PENDING_FONTS.with(|f| std::mem::take(&mut *f.borrow_mut()));
    formdoc_core::world::install_fonts(fonts).map_err(err)
}

#[wasm_bindgen]
pub fn catalog() -> Result<String, JsError> {
    serde_json::to_string(&api::catalog().map_err(err)?).map_err(err)
}

/// 文書テンプレートの info を読む（一覧表示用。セッションは変えない）。
#[wasm_bindgen]
pub fn style_info(source: &str) -> Result<String, JsError> {
    serde_json::to_string(&api::style_info(source).map_err(err)?).map_err(err)
}

/// GUIモードの文書テンプレートを設定し、その info を返す。
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

/// 足りない必須の章を追加した文書を返す（JSON）。
#[wasm_bindgen]
pub fn complete_chapters(doc_json: &str) -> Result<String, JsError> {
    let doc = serde_json::from_str(doc_json).map_err(|e| err(format!("文書の形式が不正です: {e}")))?;
    let d = SESSION.with(|s| s.borrow().complete_chapters(&doc)).map_err(err)?;
    serde_json::to_string(&d).map_err(err)
}

/// GUI文書を更新する。known は既に表示済みのページハッシュ（JSON配列）。
#[wasm_bindgen]
pub fn update_document(doc_json: &str, known_json: &str) -> Result<String, JsError> {
    let doc = serde_json::from_str(doc_json).map_err(|e| err(format!("文書の形式が不正です: {e}")))?;
    let r = SESSION.with(|s| s.borrow_mut().update_document(doc, &known(known_json)));
    serde_json::to_string(&r).map_err(err)
}

/// コードモードで見せるコード（直前に組版した文書の Typst）。
#[wasm_bindgen]
pub fn code() -> Option<String> {
    SESSION.with(|s| s.borrow().code())
}

/// コードモードの編集を文書に戻す。{"doc": …, "warnings": […]}
#[wasm_bindgen]
pub fn apply_code(code: &str) -> Result<String, JsError> {
    let a = SESSION.with(|s| s.borrow().apply_code(code)).map_err(err)?;
    serde_json::to_string(&a).map_err(err)
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
