//! 部品の計算ロジック（Rhai スクリプト）。TODO 5。
//!
//! 部品定義（components.toml）に `logic = "<file>.rhai"`（library/components/ からの相対）を書くと、
//! その部品の評価時にスクリプトを実行し、返した変数を定義する。描画は `render = "<Typst関数名>"`。
//!
//! スクリプトに渡すもの（スコープの変数）
//! - `props`：部品の入力（Map。数値はすべて浮動小数点数）
//! - `vars` ：この位置から使える変数（Map。名前 → `#{ value, unit, digits, desc }`）
//!
//! スクリプトが返すもの（最後の式の値）
//! ```text
//! #{
//!   vars:   [ #{ name, value, unit, digits, desc, display }, … ],   // 定義する変数（digits・display は省略可）
//!   values: #{ … },                                                   // 描画関数に名前付き引数で渡す値
//!   issues: [ #{ field, severity: "error"|"warning"|"info", message }, … ],
//! }
//! ```
//!
//! サンドボックス：ファイル・時刻・乱数・モジュールの読み込みは使えない（同じ入力なら同じ結果になるように）。
//! 無限ループや過大な処理は操作数・呼び出しの深さ・文字列や配列の大きさの上限で止め、エラーとして報告する。

use std::collections::HashMap;
use std::sync::OnceLock;

use rhai::packages::{
    ArithmeticPackage, BasicArrayPackage, BasicMapPackage, BasicMathPackage, BasicStringPackage, LogicPackage, MoreStringPackage, Package,
};
use rhai::{Dynamic, Engine, EvalAltResult, Scope};
use serde::Deserialize;
use serde_json::{Map, Value};

/// 1回の実行で許す操作数（式の評価・関数呼び出しなどの数）。部品の計算には十分大きく、無限ループはすぐ止まる値
pub const MAX_OPERATIONS: u64 = 1_000_000;

/// スクリプトが定義する変数1つ。
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LogicVar {
    pub name: String,
    pub value: f64,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub digits: Option<f64>,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub display: Option<String>,
}

/// スクリプトが報告する問題1つ。
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LogicIssue {
    #[serde(default)]
    pub field: String,
    #[serde(default = "error")]
    pub severity: String,
    pub message: String,
}

fn error() -> String {
    "error".into()
}

/// スクリプトの実行結果。
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct LogicOutput {
    #[serde(default)]
    pub vars: Vec<LogicVar>,
    /// 描画関数に渡す値（JSON の Map）
    #[serde(default)]
    pub values: Map<String, Value>,
    #[serde(default)]
    pub issues: Vec<LogicIssue>,
}

/// 部品定義の計算ロジック（components.toml の logic・render）。
#[derive(Debug, Clone)]
pub struct ComponentLogic {
    /// Rhai スクリプトの中身
    pub script: String,
    /// 描画する Typst 関数（@local/formdoc の関数名）。空なら変数の定義だけ
    pub render: String,
}

/// 部品の種類 → 計算ロジック。部品定義から1回だけ読む。
pub fn component_logic(kind: &str) -> Option<&'static ComponentLogic> {
    static LOGIC: OnceLock<HashMap<String, ComponentLogic>> = OnceLock::new();
    LOGIC
        .get_or_init(|| {
            let mut out = HashMap::new();
            let Ok(Value::Object(defs)) = crate::template::components_json() else { return out };
            for (kind, def) in defs {
                let Some(file) = def.get("logic").and_then(Value::as_str) else { continue };
                let Some(bytes) = formdoc_library::get(&format!("components/{file}")) else { continue };
                let render = def.get("render").and_then(Value::as_str).unwrap_or("").to_string();
                out.insert(kind, ComponentLogic { script: String::from_utf8_lossy(bytes).into_owned(), render });
            }
            out
        })
        .get(kind)
}

/// 制限つきの Rhai エンジン。計算・文字列・配列・Map・数学関数だけを使える。
fn engine() -> Engine {
    let mut e = Engine::new_raw();
    for m in [
        ArithmeticPackage::new().as_shared_module(),
        LogicPackage::new().as_shared_module(),
        BasicMathPackage::new().as_shared_module(),
        BasicStringPackage::new().as_shared_module(),
        MoreStringPackage::new().as_shared_module(),
        BasicArrayPackage::new().as_shared_module(),
        BasicMapPackage::new().as_shared_module(),
    ] {
        e.register_global_module(m);
    }
    e.set_max_operations(MAX_OPERATIONS);
    e.set_max_call_levels(32);
    e.set_max_expr_depths(64, 32);
    e.set_max_string_size(10_000);
    e.set_max_array_size(10_000);
    e.set_max_map_size(10_000);
    // print / debug の出力は捨てる（標準出力を汚さない）
    e.on_print(|_| {});
    e.on_debug(|_, _, _| {});
    e
}

/// JSON の数値をすべて浮動小数点数にする（スクリプトで整数と小数を混ぜて計算できるように）。
fn floats(v: &Value) -> Value {
    match v {
        Value::Number(n) => n.as_f64().map(Value::from).unwrap_or(Value::Null),
        Value::Array(a) => Value::Array(a.iter().map(floats).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), floats(v))).collect()),
        other => other.clone(),
    }
}

/// Rhai のエラーを利用者向けの文にする。
fn message(e: &EvalAltResult) -> String {
    match e {
        EvalAltResult::ErrorTooManyOperations(_) => {
            format!("計算ロジックの処理が多すぎるため止めました（操作数の上限 {MAX_OPERATIONS}。無限ループになっていないか確認してください）")
        }
        EvalAltResult::ErrorStackOverflow(_) => "計算ロジックの呼び出しが深すぎるため止めました（再帰の上限）".into(),
        EvalAltResult::ErrorDataTooLarge(what, _) => format!("計算ロジックの{what}が大きすぎます"),
        other => format!("計算ロジックのエラー: {other}"),
    }
}

/// スクリプトを実行する。`vars` はこの位置から使える変数。
pub fn run(script: &str, props: &Map<String, Value>, vars: &formdoc_expr::Scope) -> Result<LogicOutput, String> {
    let e = engine();
    let ast = e.compile(script).map_err(|err| format!("計算ロジックの構文エラー: {err}"))?;
    let props = rhai::serde::to_dynamic(floats(&Value::Object(props.clone()))).map_err(|err| message(&err))?;
    let mut vmap = Map::new();
    for (name, v) in vars {
        vmap.insert(
            name.clone(),
            serde_json::json!({ "value": v.value, "unit": v.unit, "digits": v.digits.map(f64::from), "desc": v.desc }),
        );
    }
    let vars = rhai::serde::to_dynamic(Value::Object(vmap)).map_err(|err| message(&err))?;
    let mut scope = Scope::new();
    scope.push_constant("props", props);
    scope.push_constant("vars", vars);
    let out: Dynamic = e.eval_ast_with_scope(&mut scope, &ast).map_err(|err| message(&err))?;
    let json: Value = rhai::serde::from_dynamic(&out).map_err(|err| message(&err))?;
    let out: LogicOutput = serde_json::from_value(json).map_err(|err| format!("計算ロジックの戻り値の形が違います: {err}"))?;
    if let Some(v) = out.vars.iter().find(|v| !v.value.is_finite()) {
        return Err(format!("計算ロジックの変数「{}」の値が数値になりません（0 での割り算など）", v.name));
    }
    Ok(out)
}
