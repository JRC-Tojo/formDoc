// 計算書スタイル。執筆者は変更できない（テンプレート開発モードでのみ編集）。
// 見出し:  §4．主桁の設計 / 4.1 作用 / (1) 死荷重 / 1) 車両横荷重
// ページ番号: 上部中央

#let fonts-serif = ("Noto Serif JP",)
#let fonts-sans = ("Noto Sans JP",)
#let font-math = "New Computer Modern Math"

#let _heading-number(..n) = {
  let n = n.pos()
  if n.len() == 1 { "§" + str(n.at(0)) + "．" }
  else if n.len() == 2 { str(n.at(0)) + "." + str(n.at(1)) }
  else if n.len() == 3 { "(" + str(n.at(2)) + ")" }
  else { str(n.at(-1)) + ")" }
}

/// 計算書テンプレート。
/// - title: 表題（表紙・PDFメタデータ）
/// - project: 業務名
/// - author: 作成者
/// - date: 作成日（文字列）
/// - chapter-start: 最初の § 番号（分冊時に続き番号にする）
/// - cover: 表紙を付けるか
#let keisansho(title: "計算書", project: none, author: none, date: none, chapter-start: 1, cover: true, doc) = {
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
        if date != none { text(size: 12pt, date); v(0.6em) }
        if author != none { text(size: 12pt, author) }
      })
      v(3cm)
    })
  }
  counter(heading).update(chapter-start - 1)
  doc
}

/// 本文段落（見出しに合わせて字下げ）。
#let para(body) = pad(left: 2em, par(first-line-indent: (amount: 1em, all: true), body))

/// 箇条書き「・」
#let bullets(..items) = pad(left: 2em, list(marker: [・], indent: 0em, body-indent: 0.2em, ..items))
