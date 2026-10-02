// formDoc 社内標準パッケージ
//   #import "@local/formdoc:0.1.0": *
//   #import "style.typ": style      （スタイルは styles/*.typ。プロジェクトに style.typ として置く）
//   #show: style.with(title: "主桁の設計計算書")

#import "src/text.typ": para, bullets
#import "src/calc.typ": fmt, vdef, vcalc, val, sym, def-line, calc-line, check-line, where-list, sum-lines
#import "src/refs.typ": kijun, kijun-list
#import "src/figures.typ": fig-beam, fig-isection, fig-shapes, fd-figure
#import "src/tables.typ": vt, cellv, fd-table

#let formdoc-version = "0.1.0"
