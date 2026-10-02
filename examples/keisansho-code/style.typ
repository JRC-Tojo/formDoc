// formDoc スタイル：計算書
//
// 1つのスタイル＝この1ファイル。システムフォルダの styles/ に置くと formDoc の「スタイル」に並ぶ。
// 新しい型（要領書・作業計画書など）を作るときは、このファイルを複製して info と style を書き換える。
//
// - info  : 型の規則と、文書情報（表紙など）の入力欄。formDoc はここを読んで入力画面を作る
// - style : 体裁。info.fields の key と同じ名前の引数で値を受け取る
//
// 見出し:  §4．主桁の設計 / 4.1 作用 / (1) 死荷重 / 1) 車両横荷重
// ページ番号: 上部中央（表紙は数えない）

#let info = (
  id: "keisansho",
  name: "計算書",
  description: "構造計算書。変数・計算行・照査・記号説明・パラメトリック図を使う。",
  max-heading-level: 4,
  // 計算に使う値：表示桁で丸めた値を使う（読者が電卓で検算できるように）
  rounding: "display",

  // 文書情報の入力欄。type: text / multiline / int / bool / date / select（options）
  // date 型は datetime として style に渡され、PDFの作成日時にも使われる
  fields: (
    (key: "title", label: "表題", type: "text", required: true, default: "計算書"),
    (key: "project", label: "業務名", type: "text"),
    (key: "author", label: "作成者（部署）", type: "text"),
    (key: "date", label: "作成日", type: "date",
      help: "2026-10-02 の形式。PDFの作成日時にも使われます（同じ文書なら同じPDFになるよう、出力した日時は使いません）"),
    (key: "chapter-start", label: "最初の章番号（§）", type: "int", default: 1, help: "分冊で章番号を続ける場合に指定"),
    (key: "cover", label: "表紙を付ける", type: "bool", default: true),
  ),

  // 執筆者が使える部品（並び順が「部品を追加」の順）
  blocks: (
    "heading", "paragraph", "kijun", "vdef", "calc", "sum", "check", "where",
    "table", "fig-shapes", "image", "pagebreak", "typst",
    // 旧部品（テンプレートに置き換え済み。既存の文書のために残す）
    "fig-beam", "fig-isection",
  ),

  // 新規作成時の骨組み
  skeleton: (
    (kind: "heading", level: 1, text: "設計条件"),
    (kind: "heading", level: 1, text: "設計計算"),
    (kind: "heading", level: 1, text: "設計結果一覧"),
  ),

  // 単位ごとの既定の表示桁（部品側で桁数を空欄にしたときに使う）
  digits: (
    "m": 3, "mm": 0, "kN": 2, "kN/m": 2, "kN*m": 2, "kN·m": 2, "kN/m2": 2,
    "N/mm2": 1, "km/h": 0, "Hz": 3, "": 3,
  ),

  lint: (
    // 句読点（計算書は「，．」）
    punctuation: (comma: "，", period: "．"),
    // 全角英数字・半角カナを禁止
    fullwidth-alnum: true,
    halfwidth-kana: true,
    // 表記ゆれ（左を右に統一）
    replace: (
      (from: "行なう", to: "行う"),
      (from: "ﾓｰﾒﾝﾄ", to: "モーメント"),
      (from: "曲げﾓｰﾒﾝﾄ", to: "曲げモーメント"),
      (from: "下さい", to: "ください"),
      (from: "出来る", to: "できる"),
      (from: "但し", to: "ただし"),
      (from: "及び", to: "および"),
      (from: "又は", to: "または"),
    ),
    // 単位表記の統一
    unit: (
      (from: "KN", to: "kN"),
      (from: "kn", to: "kN"),
      (from: "N/mm^2", to: "N/mm2"),
      (from: "KM/H", to: "km/h"),
    ),
  ),
)

#let fonts-serif = ("Noto Serif JP",)
#let font-math = "New Computer Modern Math"

#let _heading-number(..n) = {
  let n = n.pos()
  if n.len() == 1 { "§" + str(n.at(0)) + "．" }
  else if n.len() == 2 { str(n.at(0)) + "." + str(n.at(1)) }
  else if n.len() == 3 { "(" + str(n.at(2)) + ")" }
  else { str(n.at(-1)) + ")" }
}

#let _date-text(d) = if type(d) == datetime { d.display("[year]年[month padding:none]月") } else { d }

#let style(title: "計算書", project: none, author: none, date: none, chapter-start: 1, cover: true, doc) = {
  set document(title: title, author: if author != none { author } else { () })
  set page(
    paper: "a4",
    margin: (top: 22mm, bottom: 20mm, left: 22mm, right: 20mm),
    header: context {
      if counter(page).get().first() > (if cover { 1 } else { 0 }) {
        align(center, text(size: 9pt, str(counter(page).get().first() - (if cover { 1 } else { 0 }))))
      }
    },
  )
  set text(font: fonts-serif, size: 10.5pt, lang: "ja", region: "jp")
  set par(leading: 0.95em, spacing: 1.1em, justify: true)
  show math.equation: set text(font: (font-math, ..fonts-serif))

  set heading(numbering: _heading-number)
  show heading: set text(weight: "regular")
  show heading.where(level: 1): it => {
    pagebreak(weak: true)
    block(above: 0em, below: 1.4em, text(size: 12.5pt, [#counter(heading).display(it.numbering)#h(0.6em)#it.body]))
  }
  show heading.where(level: 2): it => block(above: 1.6em, below: 1em, pad(left: 1em,
    text(size: 11pt, [#counter(heading).display(it.numbering)#h(1em)#it.body])))
  show heading.where(level: 3): it => block(above: 1.4em, below: 0.9em, pad(left: 1.5em,
    [#counter(heading).display(it.numbering)#h(0.8em)#it.body]))
  show heading: it => if it.level >= 4 {
    block(above: 1.2em, below: 0.8em, pad(left: 2em, [#counter(heading).display(it.numbering)#h(0.3em)#it.body]))
  } else { it }

  // 本文の句読点は「，．」に統一（Lintでも検出するが、組版上も統一する）
  show "、": "，"
  show "。": "．"

  set table(stroke: 0.5pt, inset: (x: 0.5em, y: 0.4em), align: center + horizon)
  show table: set text(size: 9pt)
  show figure.where(kind: table): set figure.caption(position: top)
  set figure(numbering: n => str(counter(heading).get().first()) + "." + str(n))
  set figure.caption(separator: [: ])

  if cover {
    page(header: none, {
      v(1fr)
      align(center, {
        if project != none { text(size: 14pt, project); v(1.5em) }
        text(size: 24pt, tracking: 0.3em, title)
      })
      v(1fr)
      align(center, {
        if date != none { text(size: 12pt, _date-text(date)); v(0.6em) }
        if author != none { text(size: 12pt, author) }
      })
      v(3cm)
    })
  }
  counter(heading).update(chapter-start - 1)
  doc
}
