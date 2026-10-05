//! Typstプラグイン。Typst側からは `plugin("formdoc_expr.wasm")` で読み込み、
//! JSONのバイト列を渡して JSON のバイト列を受け取る。

use formdoc_expr::{CalcRequest, CheckRequest};
use wasm_minimal_protocol::*;

initiate_protocol!();

fn run<Req: serde::de::DeserializeOwned, Out: serde::Serialize>(
    input: &[u8],
    f: impl Fn(&Req) -> Result<Out, formdoc_expr::Error>,
) -> Result<Vec<u8>, String> {
    let req: Req = serde_json::from_slice(input).map_err(|e| format!("formdoc: 引数が不正です: {e}"))?;
    let out = f(&req).map_err(|e| e.to_string())?;
    serde_json::to_vec(&out).map_err(|e| e.to_string())
}

/// 計算行。入力: CalcRequest のJSON、出力: CalcOutput のJSON。
#[wasm_func]
pub fn calc(input: &[u8]) -> Result<Vec<u8>, String> {
    run::<CalcRequest, _>(input, formdoc_expr::calc)
}

/// 照査。入力: CheckRequest のJSON、出力: CheckOutput のJSON。
#[wasm_func]
pub fn check(input: &[u8]) -> Result<Vec<u8>, String> {
    run::<CheckRequest, _>(input, formdoc_expr::check)
}

/// 変数名の数式表記。入力: 変数名（UTF-8）、出力: Typst数式マークアップ。
#[wasm_func]
pub fn name_math(input: &[u8]) -> Result<Vec<u8>, String> {
    let name = std::str::from_utf8(input).map_err(|e| e.to_string())?;
    Ok(formdoc_expr::name_math(name).into_bytes())
}

/// 単位の数式表記。
#[wasm_func]
pub fn unit_math(input: &[u8]) -> Result<Vec<u8>, String> {
    let unit = std::str::from_utf8(input).map_err(|e| e.to_string())?;
    Ok(formdoc_expr::unit_to_math(unit).into_bytes())
}

/// 数値の書式化。入力: {"value": f64, "digits": u8|null, "group": u8|null}（group は3桁区切りを入れる整数部の桁数）
#[wasm_func]
pub fn format(input: &[u8]) -> Result<Vec<u8>, String> {
    let v: serde_json::Value = serde_json::from_slice(input).map_err(|e| e.to_string())?;
    let value = v["value"].as_f64().ok_or("value が数値ではありません")?;
    let digits = v["digits"].as_u64().map(|d| d as u8);
    let group = v["group"].as_u64().map(|g| g as u8).unwrap_or(formdoc_expr::DEFAULT_GROUP);
    Ok(formdoc_expr::format_number(value, formdoc_expr::NumFormat { digits, group }).into_bytes())
}
