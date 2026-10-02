// 基準書の引用。略称は data/references.toml に登録されたものに限る。

#let _refs = toml("../data/references.toml")

/// 基準引用:「【鋼標準】第Ⅰ編 4.4.2 より」
/// - abbr: 登録済みの略称
/// - loc: 編・条項（例 "第Ⅰ編 4.4.2"）
/// - suffix: 末尾の語（既定「より」）
#let kijun(abbr, loc, suffix: "より") = {
  assert(abbr in _refs, message: "formdoc: 未登録の基準書です: " + abbr + "（data/references.toml に登録してください）")
  let label = _refs.at(abbr).at("label", default: abbr)
  [【#label】#loc]
  if suffix != none and suffix != "" [ #suffix]
}

/// 登録済み基準書の一覧（GUIの選択肢用）。
#let kijun-list() = _refs
