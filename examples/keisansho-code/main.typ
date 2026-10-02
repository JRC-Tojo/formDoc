#import "@local/formdoc:0.1.0": *
#import "style.typ": style
#show: style.with(title: "主桁の設計計算書", project: "〇〇駅ホーム改良設計", author: "設計8U", date: "2026年10月", chapter-start: 4)

= 主桁の設計
== 作用
=== 死荷重　D　#h(1fr) #kijun("鋼標準", "第Ⅰ編 4.4.2")

#let t1 = vcalc("w_1", "4.50 / 2", unit: "kN/m", digits: 2, frac: false)
#let t2 = vcalc("w_2", "0.50 / 2", unit: "kN/m", digits: 2, frac: false)
#let t3 = vcalc("w_3", "1.81 * 1.58", unit: "kN/m", digits: 2, frac: false)
#let t4 = vcalc("w_4", "0.006 * 77.0 * 1.2 * 1.5 / 2", unit: "kN/m", digits: 2, frac: false)
#let t5 = vcalc("w_5", "0.2 / 2", unit: "kN/m", digits: 2, frac: false)
#let Wd = vcalc("W_d", "w_1 + w_2 + w_3 + w_4 + w_5", t1, t2, t3, t4, t5, unit: "kN/m", digits: 2)
#sum-lines((
  (label: [軌道], v: t1),
  (label: [脱線防止ガード], v: t2),
  (label: [鋼重], v: t3),
  (label: [防塵板], v: t4),
  (label: [歩行板], v: t5),
), total: Wd)

=== 衝撃荷重　I　#h(1fr) #kijun("鋼標準", "第Ⅰ編 4.4.4")

#let V = vdef("V", 110, unit: "km/h", digits: 0, desc: "当該区間を走行する列車の最高速度")
#let Lb = vdef("L_b", 8.000, unit: "m", digits: 3, desc: "部材に最大活荷重断面力を生じさせる同符号の影響線の基線の長さ")
#let Ka = vdef("K_a", 2.0, digits: 1, desc: "係数で，在来鉄道では 2.0")
#let ne = vcalc("n_e", "70 * L_b^(-0.8)", Lb, digits: 3, unit: "Hz", desc: "載荷時の部材の基本固有振動数")
#let i = vcalc("i", "K_a * V / (7.2 * n_e * L_b) + 10 / (65 + L_b)", Ka, V, ne, Lb, digits: 3, desc: "設計衝撃係数")

#para[曲げモーメントに対して，以下により算出する．]
#calc-line(ne)
#calc-line(i)
#check-line("i <= 0.7", i)
#where-list(i, Ka, V, Lb, ne)

== 耐荷性の照査
=== 断面諸量

#let H = vdef("H", 700, unit: "mm", digits: 0)
#let B = vdef("B", 350, unit: "mm", digits: 0)
#let tw = vdef("t_w", 12, unit: "mm", digits: 0)
#let tf = vdef("t_f", 22, unit: "mm", digits: 0)
#let (h, bb, w, f) = (H.value, B.value, tw.value, tf.value)
#grid(columns: (auto, 1fr), column-gutter: 2em, align: horizon,
  fig-shapes((
    (kind: "rect", from: (-bb / 2, 0), to: (bb / 2, f), fill: luma(210)),
    (kind: "rect", from: (-bb / 2, h - f), to: (bb / 2, h), fill: luma(210)),
    (kind: "rect", from: (-w / 2, f), to: (w / 2, h - f), fill: luma(210)),
    (kind: "dim", from: (-bb / 2, h + 60), to: (bb / 2, h + 60), label: val(B, unit: false)),
  ), scale: 0.006),
  [H−#val(H, unit: false)×#val(B, unit: false)×#val(tw, unit: false)×#val(tf, unit: false)（SM400）])

=== 板要素の耐荷性の照査
#para[軸圧縮力を受ける板要素の照査（片縁支持板）#kijun("鋼標準", "第Ⅱ編 2.2.3.2")]
#let Rcr = vdef("R_cr", 0.7, digits: 1, desc: "限界座屈パラメーター")
#let k0 = vdef("k_0", 0.425, digits: 3, desc: "座屈係数")
#let E = vdef("E", 2.0e5, unit: "N/mm2", digits: 0, desc: "鋼材のヤング係数")
#let nu = vdef("nu", 0.3, digits: 1, desc: "鋼材のポアソン比")
#let fsyk = vdef("f_syk", 235, unit: "N/mm2", digits: 0, desc: "鋼材の降伏強度の特性値")
#let bt0 = vcalc("bt_0", "R_cr * sqrt(pi^2 * k_0 / (12 * (1 - nu^2)) * E / f_syk)", Rcr, k0, E, nu, fsyk, digits: 1, display: "(b/t)_0")
#let b = vdef("b", 169, digits: 0)
#let t = vdef("t", 22, digits: 0)
#calc-line(bt0)
#check-line("b / t <= bt_0", b, t, bt0, digits: 1, lhs: "(b/t)")
#where-list(Rcr, k0, E, nu, fsyk)

=== 影響線
#fd-figure(fig-shapes((
  (kind: "line", from: (0, 0), to: (8, 0)),
  (kind: "polygon", pts: ((0, 0), (-0.15, -0.3), (0.15, -0.3))),
  (kind: "polygon", pts: ((8, 0), (7.85, -0.3), (8.15, -0.3))),
  (kind: "arrow", from: (3.0, 1), to: (3.0, 0.05)),
  (kind: "arrow", from: (5.8, 1), to: (5.8, 0.05)),
  (kind: "dim", from: (0, 1.4), to: (8, 1.4), label: val(Lb, unit: false)),
)), caption: [載荷状態])

#fd-table(columns: 4, caption: [反力のまとめ],
  header: ([設計作用], [記号], [鉛直反力 (kN)], [水平反力 (kN)]),
  [死荷重], [D], cellv(23.52), [—],
  [列車荷重], [L], cellv(199.56), [—],
  table.cell(rowspan: 2)[#vt("風荷重")], [W#sub[1]], [43.40], [13.98],
  [W#sub[2]], [4.48], [6.36],
)
