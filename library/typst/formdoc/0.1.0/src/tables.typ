// 表。罫線・文字サイズ・配置は文書テンプレートで固定。

#import "calc.typ": val

/// 縦書きセル（「照査結果」などの縦の見出し）。
#let vt(s) = block(width: 1.2em, {
  // 和文字は字面が cap-height を超えるため、字面全体で行間を測る
  set text(top-edge: "bounds", bottom-edge: "bounds")
  set par(leading: 0.12em, justify: false)
  align(center, s.clusters().join(linebreak()))
})

/// 表のセル値。変数なら書式化された値、数値ならそのまま。
#let cellv(x, unit: false) = if type(x) == dictionary { val(x, unit: unit) } else { [#x] }

/// 番号付きの表。cells は table() にそのまま渡す（table.cell(rowspan:) で結合可）。
#let fd-table(columns: auto, caption: none, header: none, ..cells) = {
  let t = table(
    columns: columns,
    ..if header != none { (table.header(..header),) } else { () },
    ..cells.pos(),
  )
  if caption != none {
    figure(t, caption: caption, kind: table)
  } else {
    align(center, t)
  }
}
