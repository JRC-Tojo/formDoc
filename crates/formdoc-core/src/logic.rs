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
//!   vars:   [ #{ name, value, unit, digits, desc, display }, … ],   // 定義する変数（digits・display は省略可。digits は整数）
//!   values: #{ … },                                                   // 描画関数に名前付き引数で渡す値（キーは英字で始まる英数字・_・-）
//!   issues: [ #{ field, severity: "error"|"warning"|"info", message }, … ],
//! }
//! ```
//!
//! サンドボックス：ファイル・時刻・乱数・モジュールの読み込み・eval は使えない（同じ入力なら同じ結果になるように）。
//! 無限ループや過大な処理は操作数・呼び出しの深さ・文字列や配列の大きさの上限で止め、エラーとして報告する。

use std::collections::HashMap;
use std::sync::OnceLock;

use rhai::packages::{
    ArithmeticPackage, BasicArrayPackage, BasicMapPackage, BasicMathPackage, BasicStringPackage, LogicPackage, MoreStringPackage, Package,
};
use rhai::{AST, Dynamic, Engine, EvalAltResult, Scope};
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
    /// 小数桁数（整数）。読み込み後に検査する
    #[serde(default)]
    pub digits: Option<f64>,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub display: Option<String>,
}

impl LogicVar {
    /// 小数桁数（0〜10 の整数。検査済み）。
    pub fn digits(&self) -> Option<u8> {
        self.digits.map(|d| d as u8)
    }
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
    /// スクリプトのファイル名（library/components/ からの相対）
    pub file: String,
    /// 構文解析したスクリプト。ファイルが無い・構文に誤りがあるときはその理由
    pub ast: Result<AST, String>,
    /// 描画する Typst 関数（@local/formdoc の関数名）。空なら変数の定義だけ
    pub render: String,
}

/// 計算ロジックを持つ部品の一覧（部品の種類 → 計算ロジック）。部品定義から1回だけ読み、構文解析しておく。
pub fn component_logics() -> &'static HashMap<String, ComponentLogic> {
    static LOGIC: OnceLock<HashMap<String, ComponentLogic>> = OnceLock::new();
    LOGIC.get_or_init(|| {
        let mut out = HashMap::new();
        let Ok(Value::Object(defs)) = crate::template::components_json() else { return out };
        for (kind, def) in defs {
            let Some(file) = def.get("logic").and_then(Value::as_str) else { continue };
            let ast = match formdoc_library::get(&format!("components/{file}")) {
                Some(bytes) => engine().compile(String::from_utf8_lossy(bytes)).map_err(|e| format!("計算ロジック {file} の構文エラー: {e}")),
                None => Err(format!("計算ロジックのファイル components/{file} がありません")),
            };
            let render = def.get("render").and_then(Value::as_str).unwrap_or("").to_string();
            out.insert(kind, ComponentLogic { file: file.into(), ast, render });
        }
        out
    })
}

/// 部品の種類の計算ロジック（無ければ None）。
pub fn component_logic(kind: &str) -> Option<&'static ComponentLogic> {
    component_logics().get(kind)
}

/// 制限つきの Rhai エンジン（プロセスで1つ）。計算・文字列・配列・Map・数学関数だけを使える。
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
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
        // 文字列をスクリプトとして実行する eval は、何を実行するか読み取れなくなるので使わせない
        e.disable_symbol("eval");
        e.set_max_operations(MAX_OPERATIONS);
        // Web版（wasm）はスタックが小さい（既定 1MB）ため、再帰の深さを抑える
        if cfg!(target_arch = "wasm32") {
            e.set_max_call_levels(16);
            e.set_max_expr_depths(32, 16);
        } else {
            e.set_max_call_levels(32);
            e.set_max_expr_depths(64, 32);
        }
        e.set_max_string_size(10_000);
        e.set_max_array_size(10_000);
        e.set_max_map_size(10_000);
        // print / debug の出力は捨てる（標準出力を汚さない）
        e.on_print(|_| {});
        e.on_debug(|_, _, _| {});
        e
    })
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
        EvalAltResult::ErrorDataTooLarge(..) => "計算ロジックの文字列・配列などが大きすぎるため止めました".into(),
        other => format!("計算ロジックのエラー: {other}"),
    }
}

/// 描画関数の名前付き引数に使えるキーか（英字で始まり、英数字・_・- だけ）。
fn valid_key(k: &str) -> bool {
    k.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 戻り値を検査する（値が数でない・小数桁数が整数でない・描画に渡せないキー）。
fn validate(out: &LogicOutput) -> Result<(), String> {
    if let Some(v) = out.vars.iter().find(|v| !v.value.is_finite()) {
        return Err(format!("計算ロジックの変数「{}」の値が数値になりません（0 での割り算など）", v.name));
    }
    if let Some(v) = out.vars.iter().find(|v| v.digits.is_some_and(|d| d.fract() != 0.0 || !(0.0..=10.0).contains(&d))) {
        return Err(format!("計算ロジックの変数「{}」の小数桁数は 0〜10 の整数にしてください", v.name));
    }
    if let Some(k) = out.values.keys().find(|k| !valid_key(k)) {
        return Err(format!("計算ロジックの戻り値の形が違います（values のキー「{k}」は英字で始まる英数字・_・- にしてください）"));
    }
    Ok(())
}

/// 構文解析済みのスクリプトを実行する。`vars` はこの位置から使える変数。
pub fn run_ast(ast: &AST, props: &Map<String, Value>, vars: &formdoc_expr::Scope) -> Result<LogicOutput, String> {
    let props = rhai::serde::to_dynamic(floats(&Value::Object(props.clone()))).map_err(|err| message(&err))?;
    // 変数は名前順に渡す（HashMap の順序に左右されないように）
    let mut names: Vec<&String> = vars.keys().collect();
    names.sort();
    let mut vmap = Map::new();
    for name in names {
        let v = &vars[name];
        vmap.insert(
            name.clone(),
            serde_json::json!({ "value": v.value, "unit": v.unit, "digits": v.digits.map(f64::from), "desc": v.desc }),
        );
    }
    let vars = rhai::serde::to_dynamic(Value::Object(vmap)).map_err(|err| message(&err))?;
    let mut scope = Scope::new();
    scope.push_constant("props", props);
    scope.push_constant("vars", vars);
    let out: Dynamic = engine().eval_ast_with_scope(&mut scope, ast).map_err(|err| message(&err))?;
    let json: Value = rhai::serde::from_dynamic(&out).map_err(|err| message(&err))?;
    let out: LogicOutput = serde_json::from_value(json).map_err(|err| format!("計算ロジックの戻り値の形が違います: {err}"))?;
    validate(&out)?;
    Ok(out)
}

/// スクリプトの文字列を構文解析して実行する（試験・部品定義の確認用）。
pub fn run(script: &str, props: &Map<String, Value>, vars: &formdoc_expr::Scope) -> Result<LogicOutput, String> {
    let ast = engine().compile(script).map_err(|err| format!("計算ロジックの構文エラー: {err}"))?;
    run_ast(&ast, props, vars)
}
