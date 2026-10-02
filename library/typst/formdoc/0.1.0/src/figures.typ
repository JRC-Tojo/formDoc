// パラメトリック図。寸法・荷重位置に変数（vdef/vcalc の戻り値）または数値を渡せる。

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

/// 単純梁と集中荷重（影響線縦距つき）。
/// - span: 支間長
/// - loads: 荷重位置（左支点からの距離）の配列
/// - eta: 各荷重位置の影響線縦距（省略可）
/// - width: 図の幅
#let fig-beam(span, loads: (), eta: none, width: 10cm, digits: 3) = {
  let L = _num(span)
  let s = 8.0 / L  // 描画上の支間を8単位に正規化
  cetz.canvas(length: width / 9, {
    import cetz.draw: *
    let xs = loads.map(_num)
    // 全長寸法
    _dim-h(cetz.draw, 0, L * s, 1.6, _txt(span, digits: digits))
    // 区間寸法
    let pts = (0.0,) + xs + (L,)
    for i in range(pts.len() - 1) {
      if pts.at(i + 1) > pts.at(i) {
        _dim-h(cetz.draw, pts.at(i) * s, pts.at(i + 1) * s, 1.15, fmt(pts.at(i + 1) - pts.at(i), digits: digits))
      }
    }
    line((0, 0), (L * s, 0), stroke: 1.2pt)
    _support(cetz.draw, 0, 0)
    _support(cetz.draw, L * s, 0)
    for x in xs {
      line((x * s, 0.8), (x * s, 0.05), mark: (end: "stealth", fill: black, scale: 0.6), stroke: 0.8pt)
    }
    if eta != none {
      // 影響線（左支点で1.0、右支点で0の三角形を想定し、与えられた縦距を描く）
      line((0, -0.4), (0, -1.4), (L * s, -0.4), stroke: (dash: "dotted", thickness: 0.5pt))
      line((0, -0.4), (L * s, -0.4), stroke: 0.3pt)
      for (i, x) in xs.enumerate() {
        let e = _num(eta.at(i))
        line((x * s, -0.4), (x * s, -0.4 - e), stroke: 0.4pt)
        content((x * s, -0.4 - e - 0.25), text(size: 6.5pt, _txt(eta.at(i), digits: 3)))
      }
    }
  })
}

/// I形断面（H形鋼）の寸法図。H×B×tw×tf（mm）
#let fig-isection(H, B, tw, tf, width: 4.5cm) = {
  let (h, b, w, f) = (_num(H), _num(B), _num(tw), _num(tf))
  let s = 4.0 / calc.max(h, b)
  cetz.canvas(length: width / 5.5, {
    import cetz.draw: *
    let (hh, bb, ww, ff) = (h * s, b * s, calc.max(w * s, 0.04), calc.max(f * s, 0.06))
    let x0 = -bb / 2
    rect((x0, hh - ff), (x0 + bb, hh), fill: luma(220), stroke: 0.5pt)
    rect((x0, 0), (x0 + bb, ff), fill: luma(220), stroke: 0.5pt)
    rect((-ww / 2, ff), (ww / 2, hh - ff), fill: luma(220), stroke: 0.5pt)
    _dim-h(cetz.draw, x0, x0 + bb, hh + 0.35, _txt(B, digits: 0))
    _dim-v(cetz.draw, x0 - 0.35, 0, hh, _txt(H, digits: 0))
    content((ww / 2 + 0.15, hh / 2), anchor: "west", text(size: 7pt, [#_txt(tw, digits: 0)]))
    content((x0 + bb + 0.1, hh - ff / 2), anchor: "west", text(size: 7pt, [#_txt(tf, digits: 0)]))
  })
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
