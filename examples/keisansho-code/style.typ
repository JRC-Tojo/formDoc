// formDoc 文書テンプレート：計算書
//
// 1つの文書テンプレート＝この1ファイル。システムフォルダの styles/ に置くと formDoc の「文書テンプレート」に並ぶ。
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
    // 構造形式。章の構成（chapters の variants）がこれで変わる
    (key: "variant", label: "構造形式", type: "select", required: true, default: "上路桁",
      options: ("上路桁", "工事桁", "トラス", "こ線橋"), help: "構造形式によって必要な章が変わります"),
  ),

  // 執筆者が使える部品（並び順が「部品を追加」の順）
  blocks: (
    "heading", "paragraph", "kijun", "vdef", "calc", "sum", "check", "where",
    "table", "fig-shapes", "image", "pagebreak", "typst",
  ),

  // 章構成の拘束（Issue #7）
  //   level : 文書全体の既定の強さ。locked（章の中身まで固定）/ chapters（章立てを固定）/ basic（体裁だけ固定）
  //   rules : 検出ごとの重さ（error / warning / info / off）。章ごとに rules で上書きできる
  //     chapter-missing（必須の章がない）/ chapter-order（順序違い）/ chapter-title（見出し文の変更）
  //     chapter-extra（決められていない章）/ chapter-duplicate（章の重複）/ chapter-block（章で使えない部品）
  //     chapter-locked（中身を固定した章の部品の並び違い）
  structure: (
    level: "chapters",
    rules: (chapter-title: "warning"),
  ),

  // 章の定義（並び順が文書での順序）。新規作成時は、構造形式に合う必須の章がこの順で入る。
  //   id: 識別子（見出しに保存。文書テンプレート内で一意） / title: 見出し文 / required: 必須（既定 true）
  //   fixed-title: 見出し文の変更を禁止（既定 true） / repeatable: 同じ章を繰り返し置ける（既定 false）
  //   variants: 対象の構造形式（fields の variant 欄の選択肢。省略で全形式） / guide: 執筆ガイド
  //   allowed-blocks: 章で使える部品（省略で親と同じ） / level・rules: 拘束の上書き
  //   content: 見出しの直後に置く部品 / sections: 1つ下の階層の見出しの定義
  chapters: (
    (id: "gaiyou", title: "設計概要",
      guide: "業務の目的、対象構造物、設計の範囲を記載する。"),
    (id: "jouken", title: "設計条件",
      guide: "線区情報・適用基準類と、構造形式ごとに決められた設計条件の各項目を、この順で記載する。",
      sections: (
        (id: "jouken-ippan", title: "一般条件", guide: "線区情報（線名・軌道構造・列車速度）と適用基準類を記載する。"),
        (id: "kentou", title: "検討箇所", variants: ("上路桁", "トラス"), guide: "検討する部材と、部材ごとの照査項目を記載する。"),
        (id: "sayou", title: "作用の種類", guide: "死荷重などの作用の種類と、その算出方法を記載する。"),
        (id: "kumiawase", title: "荷重の組合せ", guide: "照査ごとの荷重の組合せを記載する。"),
        (id: "zairyou", title: "材料特性値", guide: "使用材料と許容応力度（特性値）を記載する。"),
        (id: "kyoutsuu", title: "共通仕様", guide: "軌道構造や取り合い、最大部材寸法などの細かい条件を記載する。設計条件の各項目に当たるものはそちらに書く。"),
        (id: "grouping", title: "グルーピング", variants: ("工事桁",), guide: "部材のグルーピングの考え方を記載する。"),
        (id: "model", title: "モデル化", variants: ("トラス", "こ線橋"), guide: "解析モデル（骨組・支点条件・剛域）を記載する。"),
        (id: "sekou", title: "施工計画", variants: ("上路桁", "工事桁", "トラス"),
          guide: "架設時と完成時で条件が異なる場合は、その違いと内容を記載する。"),
      )),
    (id: "kekka", title: "設計結果",
      guide: "照査結果の一覧を記載する（部材ごとの設計の章で計算した値をまとめる）。"),
    // 部材ごとの設計（主桁・横桁・支承…）。同じ形の章を部材の数だけ置く
    (id: "buzai", title: "（部材名）の設計", fixed-title: false, repeatable: true,
      guide: "部材ごとに、作用・断面力の算出・照査の順で記載する。見出し文は「主桁の設計」のように部材名を入れる。",
      rules: (chapter-extra: "warning")),
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

#let style(title: "計算書", project: none, author: none, date: none, chapter-start: 1, cover: true, variant: none, doc) = {
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
