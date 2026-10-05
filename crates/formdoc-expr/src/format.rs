//! 数値・単位の書式。四捨五入（half-up）を用い、Rust標準の偶数丸めによる表示ぶれを避ける。

/// 小数 `digits` 桁で四捨五入する。2.675 → 2.68（浮動小数の誤差を吸収）。
pub fn round_half_up(v: f64, digits: u8) -> f64 {
    let p = 10f64.powi(digits as i32);
    let s = v.abs() * p;
    // 2進表現の誤差（2.675 = 2.67499999…）を相対1e-12で吸収する
    let r = (s + 0.5 + s.max(1.0) * 1e-12).floor() / p;
    if v < 0.0 { -r } else { r }
}

/// 3桁区切りを入れる整数部の桁数の既定値（5桁以上：12,345。4桁の 1234 は区切らない）。
/// 文書テンプレートの Lint で桁区切りを有効にすると 4 になる（1,234）。
pub const DEFAULT_GROUP: u8 = 5;

/// 数値の表示書式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumFormat {
    /// 小数桁数。None なら有効数字4桁相当で自動決定。
    pub digits: Option<u8>,
    /// 整数部がこの桁数以上のとき3桁区切りを入れる（例: 2,080,000,000）。0 なら区切らない。
    pub group: u8,
}

impl Default for NumFormat {
    fn default() -> Self {
        Self { digits: None, group: DEFAULT_GROUP }
    }
}

/// 整数部の文字列に3桁区切りを入れる（`group` 桁以上のとき。0 なら入れない）。
fn group_int(int: &str, group: u8) -> String {
    if group == 0 || int.len() < group as usize {
        return int.to_string();
    }
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 式に書かれた数値（"1000000"、"8000.5"）に、書式と同じ規則で3桁区切りを入れる。
/// 指数表記など数字と小数点以外を含むものはそのまま返す。
pub fn group_literal(text: &str, group: u8) -> String {
    let (int, frac) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text, None),
    };
    if int.is_empty() || !int.chars().all(|c| c.is_ascii_digit()) || frac.is_some_and(|f| !f.chars().all(|c| c.is_ascii_digit())) {
        return text.to_string();
    }
    let g = group_int(int, group);
    match frac {
        Some(f) => format!("{g}.{f}"),
        None => g,
    }
}

fn auto_digits(v: f64) -> u8 {
    let a = v.abs();
    if a == 0.0 || a >= 1000.0 {
        0
    } else if a >= 100.0 {
        1
    } else if a >= 10.0 {
        2
    } else if a >= 1.0 {
        3
    } else {
        // 有効数字3桁
        let lead = (-a.log10()).floor() as i32;
        (lead + 3).clamp(3, 10) as u8
    }
}

pub fn format_number(v: f64, f: NumFormat) -> String {
    let d = f.digits.unwrap_or_else(|| auto_digits(v));
    let r = round_half_up(v, d);
    let s = format!("{:.*}", d as usize, r.abs());
    let (int, frac) = match s.split_once('.') {
        Some((i, fr)) => (i.to_string(), Some(fr.to_string())),
        None => (s.clone(), None),
    };
    let int = group_int(&int, f.group);
    let mut out = String::new();
    if r < 0.0 {
        out.push('-');
    }
    out.push_str(&int);
    if let Some(fr) = frac {
        out.push('.');
        out.push_str(&fr);
    }
    out
}

/// 単位文字列（"kN/m2", "N/mm^2", "kN*m"）をTypst数式中に置く形にする。
/// 単位は数式記号ではなく本文と同じ書体の文字として扱う（"kN / m" のような間延びを避ける）。
pub fn unit_to_math(unit: &str) -> String {
    format!("\"{}\"", unit_to_text(unit).replace(['"', '\\'], ""))
}

/// 単位文字列を本文用のテキストに変換する（指数を上付き文字に）。"N/mm2" → "N/mm²"
pub fn unit_to_text(unit: &str) -> String {
    let sup = |c: char| match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '-' => '⁻',
        _ => c,
    };
    let mut out = String::new();
    let mut prev_alpha = false;
    let mut in_exp = false;
    for c in unit.chars() {
        if c == '^' {
            in_exp = true;
            continue;
        }
        if (prev_alpha || in_exp) && (c.is_ascii_digit() || (c == '-' && in_exp)) {
            out.push(sup(c));
            in_exp = true;
            continue;
        }
        in_exp = false;
        prev_alpha = c.is_alphabetic();
        out.push(match c {
            '*' | '.' => '·',
            _ => c,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_up() {
        assert_eq!(round_half_up(2.675, 2), 2.68);
        assert_eq!(round_half_up(0.4245, 3), 0.425);
        assert_eq!(round_half_up(-1.25, 1), -1.3);
        assert_eq!(round_half_up(12.65, 1), 12.7);
    }

    #[test]
    fn numbers() {
        let f = |d| NumFormat { digits: Some(d), group: DEFAULT_GROUP };
        assert_eq!(format_number(8.0, f(3)), "8.000");
        assert_eq!(format_number(2_080_000_000.0, f(0)), "2,080,000,000");
        assert_eq!(format_number(7872.0, f(0)), "7872");
        assert_eq!(format_number(23550.0, f(0)), "23,550");
        assert_eq!(format_number(-11.93, f(2)), "-11.93");
        assert_eq!(format_number(0.4245, NumFormat::default()), "0.425");
    }

    #[test]
    fn units() {
        assert_eq!(unit_to_text("N/mm2"), "N/mm²");
        assert_eq!(unit_to_text("kN*m"), "kN·m");
        assert_eq!(unit_to_math("N/mm^2"), "\"N/mm²\"");
        assert_eq!(unit_to_math("kN·m"), "\"kN·m\"");
    }
}
