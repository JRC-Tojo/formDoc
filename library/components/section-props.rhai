// 断面諸量（左右対称のI形断面）の計算ロジック。部品「断面諸量（I形）」（components.toml の section-props）。
// 入力：props.H（高さ）, props.B（フランジ幅）, props.tw（腹板厚）, props.tf（フランジ厚）, props.r（フィレット半径。圧延H形のみ）,
//       props.suffix（変数名の添字）, props.label（断面の名前）。寸法の単位は mm。
// 出力：A（断面積）, A_w（腹板の断面積）, I_y・I_z（断面二次モーメント）, r_y・r_z（断面二次半径）, y_u・y_l（図心から縁まで）
// 断面二次モーメントは有効数字3桁に丸めた値を変数にする（計算書の慣例。後続の計算もこの値で行い、紙面の値と検算が合うようにする）。
// 断面二次半径は丸める前の断面二次モーメントから求める。表示桁は計算書サンプルに合わせた（r_y は整数、r_z は小数1桁）。

let H = props.H ?? 0.0;
let B = props.B ?? 0.0;
let tw = props.tw ?? 0.0;
let tf = props.tf ?? 0.0;
let r = if props.kind == "圧延H形" { props.r ?? 0.0 } else { 0.0 };

let issues = [];
for f in ["H", "B", "tw", "tf"] {
    if (props[f] ?? 0.0) <= 0.0 {
        issues.push(#{ field: f, severity: "error", message: "寸法 " + f + " を正の数で入力してください" });
    }
}
if r < 0.0 {
    issues.push(#{ field: "r", severity: "error", message: "フィレット半径 r は 0 以上にしてください" });
}
if issues.len() > 0 { return #{ issues: issues }; }
// 板の寸法の組合せ（腹板の高さが残らない・フランジから腹板がはみ出す・フィレットが入りきらない）
if 2.0 * (tf + r) >= H {
    issues.push(#{ field: if r > 0.0 { "r" } else { "tf" }, severity: "error", message: "フランジ厚（とフィレット）が高さ H に対して大きすぎます" });
}
if tw + 2.0 * r >= B {
    issues.push(#{ field: if r > 0.0 { "r" } else { "tw" }, severity: "error", message: "腹板厚（とフィレット）がフランジ幅 B に対して大きすぎます" });
}
if issues.len() > 0 { return #{ issues: issues }; }

// 有効数字 n 桁に丸める
fn sig(x, n) {
    if x == 0.0 { return 0.0; }
    let e = (x.abs().log() ).floor() - (n - 1).to_float();
    let p = 10.0 ** e;
    (x / p).round() * p
}

let hw = H - 2.0 * tf;                 // 腹板の高さ
// フィレット（4か所）：面積 (1 - π/4) r²、角からの図心距離 (10 - 3π) / (3 (4 - π)) r
let af = (1.0 - PI() / 4.0) * r * r;
let cf = (10.0 - 3.0 * PI()) / (3.0 * (4.0 - PI())) * r;
// フィレット自身の図心まわり（両軸とも同じ）：角を通る辺まわりの (1 − 5π/16) r⁴ から平行軸の定理で図心へ移す
let If = (1.0 - 5.0 * PI() / 16.0) * r ** 4 - af * cf * cf;

let A = 2.0 * B * tf + hw * tw + 4.0 * af;
let Aw = hw * tw;
let yf = H / 2.0 - tf / 2.0;           // フランジの図心
let yr = H / 2.0 - tf - cf;            // フィレットの図心（強軸方向）
let xr = tw / 2.0 + cf;                // フィレットの図心（弱軸方向）
let Iy = 2.0 * (B * tf ** 3 / 12.0 + B * tf * yf * yf) + tw * hw ** 3 / 12.0 + 4.0 * (If + af * yr * yr);
let Iz = 2.0 * (tf * B ** 3 / 12.0) + hw * tw ** 3 / 12.0 + 4.0 * (If + af * xr * xr);

let sx = (props.suffix ?? "").to_string();
let s = if sx == "" { "" } else { "_" + sx };
#{
    vars: [
        #{ name: "A" + s, value: A, unit: "mm2", digits: 0, desc: "断面積" },
        #{ name: "A_w" + s, value: Aw, unit: "mm2", digits: 0, desc: "腹板の断面積" },
        #{ name: "I_y" + s, value: sig(Iy, 3), unit: "mm4", digits: 0, desc: "断面二次モーメント（強軸）" },
        #{ name: "I_z" + s, value: sig(Iz, 3), unit: "mm4", digits: 0, desc: "断面二次モーメント（弱軸）" },
        #{ name: "r_y" + s, value: (Iy / A).sqrt(), unit: "mm", digits: 0, desc: "断面二次半径（強軸）" },
        #{ name: "r_z" + s, value: (Iz / A).sqrt(), unit: "mm", digits: 1, desc: "断面二次半径（弱軸）" },
        #{ name: "y_u" + s, value: H / 2.0, unit: "mm", digits: 0, desc: "図心から上縁まで" },
        #{ name: "y_l" + s, value: H / 2.0, unit: "mm", digits: 0, desc: "図心から下縁まで" },
    ],
    values: #{ label: props.label ?? "", H: H, B: B, tw: tw, tf: tf },
    issues: [],
}
