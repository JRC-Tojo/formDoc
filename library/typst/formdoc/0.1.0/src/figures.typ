// 図。座標に変数（vdef/vcalc の戻り値）または数値を渡せる。

#import "@preview/cetz:0.5.2"

#import "calc.typ": fmt

#let _num(x) = if type(x) == dictionary { x.value } else { float(x) }
#let _txt(x, digits: 3) = if type(x) == dictionary { x.text } else { fmt(x, digits: digits) }

// 寸法線（水平）。y は描画座標。
#let _dim-h(draw, x1, x2, y, label) = {
  draw.line((x1, y), (x2, y), mark: (start: "straight", end: "straight", scale: 0.5), stroke: 0.4pt)
  draw.line((x1, y - 0.12), (x1, y + 0.12), stroke: 0.3pt)
  draw.line((x2, y - 0.12), (x2, y + 0.12), stroke: 0.3pt)
  draw.content(((x1 + x2) / 2, y + 0.22), text(size: 7pt, label))
}

#let _dim-v(draw, x, y1, y2, label) = {
  draw.line((x, y1), (x, y2), mark: (start: "straight", end: "straight", scale: 0.5), stroke: 0.4pt)
  draw.line((x - 0.12, y1), (x + 0.12, y1), stroke: 0.3pt)
  draw.line((x - 0.12, y2), (x + 0.12, y2), stroke: 0.3pt)
  draw.content((x - 0.3, (y1 + y2) / 2), angle: 90deg, text(size: 7pt, label))
}

#let _support(draw, x, y) = {
  draw.line((x, y), (x - 0.15, y - 0.25), (x + 0.15, y - 0.25), close: true, stroke: 0.5pt)
}

/// 汎用図形。shapes は辞書の配列:
///   (kind: "line", from: (x, y), to: (x, y))
///   (kind: "arrow", from: (x, y), to: (x, y))          … to の側に矢印
///   (kind: "rect", from: (x, y), to: (x, y), fill: luma(210))
///   (kind: "circle", at: (x, y), r: 1, fill: none)
///   (kind: "polygon", pts: ((x, y), (x, y), …), fill: none)
///   (kind: "dim", from: (x, y), to: (x, y), label: "5.000")
///   (kind: "text", at: (x, y), body: [文字])
/// 座標・半径には数値または変数を指定できる。
#let fig-shapes(shapes, unit: 1cm, scale: 1.0) = {
  let k = scale  // cetz.draw の scale 関数と衝突しないよう退避
  let p(pt) = (_num(pt.at(0)) * k, _num(pt.at(1)) * k)
  cetz.canvas(length: unit, {
    import cetz.draw: *
    for sh in shapes {
      let fill = sh.at("fill", default: none)
      if sh.kind == "line" { line(p(sh.from), p(sh.to), stroke: 0.6pt) }
      else if sh.kind == "arrow" { line(p(sh.from), p(sh.to), stroke: 0.6pt, mark: (end: "stealth", fill: black, scale: 0.6)) }
      else if sh.kind == "rect" { rect(p(sh.from), p(sh.to), stroke: 0.6pt, fill: fill) }
      else if sh.kind == "circle" { circle(p(sh.at), radius: _num(sh.r) * k, stroke: 0.6pt, fill: fill) }
      else if sh.kind == "polygon" and sh.pts.len() >= 2 { line(..sh.pts.map(p), close: true, stroke: 0.6pt, fill: fill) }
      else if sh.kind == "dim" {
        let (a, b) = (p(sh.from), p(sh.to))
        line(a, b, mark: (start: "straight", end: "straight", scale: 0.5), stroke: 0.4pt)
        content(((a.at(0) + b.at(0)) / 2, (a.at(1) + b.at(1)) / 2 + 0.2), text(size: 7pt, sh.at("label", default: "")))
      }
      else if sh.kind == "text" { content(p(sh.at), sh.body) }
    }
  })
}

/// 図（番号付きキャプション）
#let fd-figure(body, caption: none) = figure(body, caption: caption, kind: image)
