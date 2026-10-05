// 変数・計算行・照査・記号説明。
// 計算と数式表記の生成はすべて formdoc_expr.wasm（Rustの式エンジン）で行い、
// GUIで作った文書と手書きTypstで結果・表記が一致するようにしている。

#let _p = plugin("../formdoc_expr.wasm")

// 本文側の字下げ（見出しより一段深い位置）
#let body-indent = 2em

#let _call(f, req) = json(f(bytes(json.encode(req))))

#let _math(s) = if s == none or s == "" { [] } else { eval(s, mode: "math") }

/// 数値を社内標準の書式（四捨五入・桁区切り）で文字列にする。
/// - group: 3桁区切りを入れる整数部の桁数（none なら式エンジンの既定。文書テンプレートの設定は fd-config で渡す）
#let fmt(x, digits: none, group: none) = str(_p.format(bytes(json.encode((value: float(x), digits: digits, group: group)))))

// 変数辞書の配列から、式エンジンに渡す scope を作る。
#let _scope(vars) = {
  let s = (:)
  for v in vars {
    assert(type(v) == dictionary and "name" in v, message: "formdoc: 変数には vdef() または vcalc() の戻り値を渡してください")
    s.insert(v.name, (value: v.value, digits: v.digits, unit: v.unit, display: v.display, desc: v.desc))
  }
  s
}

/// 変数を定義する。
/// - name: 式中で使う名前（例 "L_b", "sigma_ck"）
/// - value: 数値
/// - unit: 単位（"kN/m", "N/mm2" …）
/// - digits: 表示する小数桁数
/// - desc: 記号説明（「ここに，」に表示）
/// - display: 数式表記を明示する場合（例 "(b/t)_0"）
#let vdef(name, value, unit: "", digits: none, desc: "", display: none, group: none) = {
  assert(type(value) in (int, float), message: "formdoc: " + name + " の値は数値で指定してください")
  (
    name: name, value: float(value), unit: unit, digits: digits, desc: desc, display: display,
    sym: if display != none { display } else { str(_p.name_math(bytes(name))) },
    text: str(_p.format(bytes(json.encode((value: float(value), digits: digits, group: group))))),
    calc: none,
  )
}

/// 式から変数を計算する。参照する変数を位置引数で渡す。
///   #let M = vcalc("M", "w * L^2 / 8", w, L, unit: "kN*m", digits: 2)
#let vcalc(name, expr, ..vars, unit: "", digits: none, desc: "", display: none, frac: true, units-in-sub: false, rounding: "display", group: none) = {
  let out = _call(_p.calc, (
    expr: expr, scope: _scope(vars.pos()), digits: digits, unit: unit,
    frac: frac, units_in_sub: units-in-sub, rounding: rounding, group: group,
  ))
  (
    name: name, value: out.value, unit: unit, digits: digits, desc: desc, display: display,
    sym: if display != none { display } else { str(_p.name_math(bytes(name))) },
    text: out.text,
    calc: out,
    deps: vars.pos(),
  )
}

/// 本文中に値を差し込む（例: 8.000 m）。
#let val(v, unit: true) = {
  let u = if unit and v.unit != "" { [~] + _math(str(_p.unit_math(bytes(v.unit)))) } else { [] }
  [#v.text#u]
}

/// 変数記号を本文中に表示する。
#let sym(v) = _math(v.sym)

#let _row(label, body) = pad(left: body-indent, grid(
  columns: if label == none { (1fr,) } else { (6em, 1fr) },
  column-gutter: 0.5em,
  ..if label == none { (body,) } else { (label, body) },
))

/// 変数定義行: 「L_b = 8.000 m」
#let def-line(v, label: none) = {
  let u = if v.unit != "" { " thin " + str(_p.unit_math(bytes(v.unit))) } else { "" }
  let num = if v.text.contains(",") { "\"" + v.text + "\"" } else { v.text }
  _row(label, _math(v.sym + " = " + num + u))
}

/// 計算行: 「M = 記号式 = 代入式 = 結果 単位」
/// - show-symbolic / show-substituted で途中式の表示を切り替える
#let calc-line(v, label: none, show-symbolic: true, show-substituted: true) = {
  assert(v.calc != none, message: "formdoc: calc-line には vcalc() の戻り値を渡してください")
  let c = v.calc
  let parts = (v.sym,)
  if show-symbolic and c.symbolic != c.substituted and c.substituted != "" { parts.push(c.symbolic) }
  if show-substituted and c.substituted != "" { parts.push(c.substituted) }
  parts.push(c.result)
  _row(label, _math(parts.join(" = ")))
}

/// 照査行: 「(b/t) = 7.7 ≦ 12.7 ――― OK」
/// 判定は紙面に表示される値どうしで行う。
#let check-line(expr, ..vars, label: none, digits: none, unit: "", lhs: none, rhs: none, rounding: "display", group: none) = {
  let r = _call(_p.check, (expr: expr, scope: _scope(vars.pos()), digits: digits, unit: unit, rounding: rounding, group: group))
  let side(s, override) = {
    let parts = (if override != none { override } else { s.symbolic },)
    if s.substituted != "" { parts.push(s.substituted) }
    let num = if s.text.contains(",") { "\"" + s.text + "\"" } else { s.text }
    if parts.at(0) != num { parts.push(num) }
    parts.join(" = ")
  }
  let u = if r.unit != "" { " thin " + r.unit } else { "" }
  let body = _math(side(r.lhs, lhs) + u + " " + r.shown_symbol + " " + side(r.rhs, rhs) + u)
  let verdict = if r.ok { [OK] } else { text(fill: rgb("#c00000"), weight: "bold")[NG] }
  _row(label, grid(
    columns: (auto, 1fr, auto),
    column-gutter: 0.6em,
    align: horizon,
    body, line(length: 100%, stroke: 0.5pt), verdict,
  ))
}

/// 記号説明: 「ここに，」と各変数の説明を並べる。
#let where-list(..vars, title: "ここに，") = {
  let vs = vars.pos()
  pad(left: body-indent, {
    [#title]
    pad(left: 1em, grid(
      columns: (4em, 1.5em, 1fr),
      row-gutter: 0.6em,
      ..vs.map(v => (
        align(right, _math(v.sym)),
        align(center)[：],
        [#v.desc#if v.unit != "" [（#_math(str(_p.unit_math(bytes(v.unit)))))]],
      )).flatten()
    ))
  })
}

/// 内訳と合計: 各行「名称 代入式 = 値 単位」、最後に下線と合計。
///   #sum-lines(((label: [軌道], v: t1), (label: [防塵板], v: t2)), total: W_d)
#let sum-lines(rows, total: none) = {
  let cells = ()
  for r in rows {
    let c = r.v.calc
    cells.push(r.label)
    cells.push(if c != none and c.substituted != "" { _math(c.substituted) } else { [] })
    cells.push(align(right, _math("= " + if c != none { c.result } else { r.v.text })))
  }
  pad(left: body-indent, {
    grid(columns: (8em, 1fr, auto), column-gutter: 1em, row-gutter: 0.65em, ..cells)
    if total != none {
      v(-0.2em)
      line(length: 100%, stroke: 0.5pt)
      v(-0.4em)
      align(right, _math("Sigma " + total.sym + " = " + total.calc.result))
    }
  })
}

/// 文書テンプレートの表記の設定を反映した関数の組。生成コードの先頭で同じ名前に上書きして使う。
///   #let (vdef, vcalc, check-line, fmt) = fd-config(group: 4)
/// - group: 3桁区切りを入れる整数部の桁数（4 なら 1,234。文書テンプレートの lint.digit-grouping）
#let fd-config(group: none) = (
  vdef: vdef.with(group: group),
  vcalc: vcalc.with(group: group),
  check-line: check-line.with(group: group),
  fmt: fmt.with(group: group),
)
