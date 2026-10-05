// 計算ロジックを持つ部品の描画（TODO 5）。値の計算は formdoc-core の logic.rs（Rhai）で行い、ここでは表示だけをする。
#import "calc.typ": _p, _math, body-indent

#let _unit(v) = if v.unit != "" { _math(str(_p.unit_math(bytes(v.unit)))) } else { [] }

/// 変数の一覧：「説明　記号 = 値 単位」を縦に並べる（断面諸量など）。
///   #props-list((A, I_y, r_y))
#let props-list(vars) = grid(
  columns: (auto, auto, auto, auto, auto),
  column-gutter: 0.8em,
  row-gutter: 0.7em,
  align: (left, right, center, right, left),
  ..vars.map(v => ([#v.desc], _math(v.sym), [=], v.text, _unit(v))).flatten(),
)

/// I形断面の略図（寸法は mm。高さ height の大きさで描く）。
#let i-section-fig(H, B, tw, tf, height: 3.4cm) = {
  let s = height / H
  let pts = (
    (0, 0), (B, 0), (B, tf), ((B + tw) / 2, tf), ((B + tw) / 2, H - tf), (B, H - tf),
    (B, H), (0, H), (0, H - tf), ((B - tw) / 2, H - tf), ((B - tw) / 2, tf), (0, tf),
  )
  let w = B * s
  let num(x) = str(calc.round(x, digits: 1))
  box(width: w + 2.2em, height: height + 1.6em, {
    // 上：フランジ幅、左：高さ、中：腹板厚・フランジ厚
    place(dx: 1.8em, dy: 0pt, box(width: w, align(center, text(size: 8pt, num(B)))))
    place(dx: 0pt, dy: 1.3em, box(height: height, align(horizon, text(size: 8pt, rotate(-90deg, reflow: true, num(H))))))
    place(dx: 1.8em, dy: 1.3em, polygon(fill: luma(230), stroke: 0.6pt, ..pts.map(((x, y)) => (x * s, y * s))))
    place(dx: 1.8em + w / 2 + tw * s, dy: 1.3em + height / 2, text(size: 7pt, num(tw)))
    place(dx: 1.8em + w + 0.2em, dy: 1.3em, text(size: 7pt, num(tf)))
  })
}

/// 部品「断面諸量（I形）」の描画：左に略図、右に断面の名前と断面諸量の一覧。
#let section-props(vars, label: "", H: 0, B: 0, tw: 0, tf: 0) = pad(left: body-indent, grid(
  columns: (auto, 1fr),
  column-gutter: 2.5em,
  align: horizon,
  i-section-fig(H, B, tw, tf),
  {
    if label != "" { [#label]; v(0.6em) }
    props-list(vars)
  },
))
