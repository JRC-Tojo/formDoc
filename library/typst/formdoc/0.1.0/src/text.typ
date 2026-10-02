// 本文の部品。文書全体の体裁（書体・余白・見出し・表紙）はスタイル（styles/*.typ）が持つ。

/// 本文段落（見出しに合わせて字下げ）。
#let para(body) = pad(left: 2em, par(first-line-indent: (amount: 1em, all: true), body))

/// 箇条書き「・」
#let bullets(..items) = pad(left: 2em, list(marker: [・], indent: 0em, body-indent: 0.2em, ..items))
