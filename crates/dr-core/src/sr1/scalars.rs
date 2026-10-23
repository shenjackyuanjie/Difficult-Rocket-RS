//! SR1 PC 的标量读取与写出语义。
//!
//! 读取对应 MSVCR120 的十进制前缀扫描，不复用严格编辑模型的 Rust 整串解析合同。
//! CRT 版本、宿主架构及未验证范围见 `docs/sr1-evidence/scalars.md`。

use std::f64::consts::{FRAC_PI_2, PI};

/// 按 PC `%d` 读取有符号十进制前缀；累积及取负采用已核实的 32 位回绕。
pub fn parse_i32(text: &str) -> Option<i32> {
    let text = trim_scan_space(c_string(text));
    let bytes = text.as_bytes();
    let negative = bytes.first() == Some(&b'-');
    let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let start = index;
    let mut value = 0_u32;
    while let Some(&digit @ b'0'..=b'9') = bytes.get(index) {
        value = value.wrapping_mul(10).wrapping_add(u32::from(digit - b'0'));
        index += 1;
    }
    if index == start {
        return None;
    }
    let value = value as i32;
    Some(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}

/// 读取 `%f` 使用的十进制前缀，直接舍入为 float32，不经过 float64。
///
/// `nan`/`inf` 不属于此版 CRT 的输入词法；十进制指数溢出仍可产生无穷大。
/// 极端十进制舍入不声称与所有 CRT 输入逐位等价。
pub fn parse_f32(text: &str) -> Option<f32> {
    decimal_prefix(text)?.parse().ok()
}

/// 读取 `%lf` 使用的十进制前缀，保留 float64 精度。
pub fn parse_f64(text: &str) -> Option<f64> {
    decimal_prefix(text)?.parse().ok()
}

/// 先按 `%d` 扫描，成功时非零为真；否则只接受完整的小写 `true` 或 `false`。
pub fn parse_bool(text: &str) -> Option<bool> {
    let text = c_string(text);
    parse_i32(text).map(|value| value != 0).or(match text {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    })
}

/// 缺失或解析失败时保留调用者给出的初值。
pub fn read_i32(text: Option<&str>, default: i32) -> i32 {
    text.and_then(parse_i32).unwrap_or(default)
}

/// 缺失或解析失败时保留 float32 初值。
pub fn read_f32(text: Option<&str>, default: f32) -> f32 {
    text.and_then(parse_f32).unwrap_or(default)
}

/// 缺失或解析失败时保留 float64 初值。
pub fn read_f64(text: Option<&str>, default: f64) -> f64 {
    text.and_then(parse_f64).unwrap_or(default)
}

/// 缺失或解析失败时保留布尔初值。
pub fn read_bool(text: Option<&str>, default: bool) -> bool {
    text.and_then(parse_bool).unwrap_or(default)
}

/// 将实际 float32 值提升为 float64，再按 PC 的六位小数格式写出。
pub fn write_f32(value: f32) -> String {
    write_f64(f64::from(value))
}

/// 按已验证的 MSVCR120 `%f` 规则写出，保留负零。
///
/// 有限数采用离线差分验证的两阶段十进制模型：17 位有效数的精确半数朝零，
/// 再取六位小数、精确半数远离零。它与普通 Rust `format!("{value:.6}")` 不同。
/// 不模拟游戏 200 字节临时缓冲区的截断；全域及跨架构证明不在此入口合同内。
pub fn write_f64(value: f64) -> String {
    let bits = value.to_bits();
    let magnitude = bits & 0x7fff_ffff_ffff_ffff;
    let sign = if bits >> 63 != 0 { "-" } else { "" };
    if magnitude >= 0x7ff0_0000_0000_0000 {
        let name = if magnitude == 0x7ff0_0000_0000_0000 {
            "1.#INF00"
        } else if bits == 0xfff8_0000_0000_0000 {
            "1.#IND00"
        } else if magnitude & 0x0008_0000_0000_0000 != 0 {
            "1.#QNAN0"
        } else {
            "1.#SNAN0"
        };
        return format!("{sign}{name}");
    }

    // f64 的最小二进制指数为 -1074；此精度足以完整展开每个有限 f64 的十进制。
    // 先取精确展开，避免以另一次浮点乘法或 Rust 的 ties-even 代替 CRT 舍入。
    let exact = format!("{:.1074e}", value.abs());
    let (mantissa, exponent) = exact.split_once('e').expect("Rust 科学计数格式包含 e");
    let mut exponent: i32 = exponent.parse().expect("Rust 科学计数指数为整数");
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let round_17 = digits[17] > b'5'
        || (digits[17] == b'5' && digits[18..].iter().any(|&digit| digit != b'0'));
    digits.truncate(17);
    if round_17 {
        increment_decimal(&mut digits);
        if digits.len() > 17 {
            digits.truncate(17);
            exponent += 1;
        }
    }

    let keep = exponent + 7;
    if keep <= 0 {
        return format!(
            "{sign}0.00000{}",
            if keep == 0 && digits[0] >= b'5' {
                '1'
            } else {
                '0'
            }
        );
    }
    let keep = keep as usize;
    let round_6 = digits.get(keep).is_some_and(|&digit| digit >= b'5');
    digits.resize(keep, b'0');
    if round_6 {
        increment_decimal(&mut digits);
    }
    while digits.len() < 7 {
        digits.insert(0, b'0');
    }
    let point = digits.len() - 6;
    let mut output = String::with_capacity(sign.len() + digits.len() + 1);
    output.push_str(sign);
    output.extend(digits[..point].iter().map(|&digit| char::from(digit)));
    output.push('.');
    output.extend(digits[point..].iter().map(|&digit| char::from(digit)));
    output
}

/// 按本体存档的 `%d` 路径写出数值布尔：真为 `1`，假为 `0`。
pub fn write_bool(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

/// PC 的编辑器朝向映射不是对 4 取模：除 0/1/2 外，全部映射到 3π/2。
pub fn editor_angle_radians(angle: i32) -> f64 {
    match angle {
        0 => 0.0,
        1 => FRAC_PI_2,
        2 => PI,
        _ => PI + FRAC_PI_2,
    }
}

fn c_string(text: &str) -> &str {
    text.split_once('\0').map_or(text, |(prefix, _)| prefix)
}

fn trim_scan_space(text: &str) -> &str {
    text.trim_start_matches([' ', '\t', '\n', '\r', '\u{000b}', '\u{000c}'])
}

fn decimal_prefix(text: &str) -> Option<&str> {
    let text = trim_scan_space(c_string(text));
    let bytes = text.as_bytes();
    let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    let mut digits = index - integer_start;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        digits += index - fraction_start;
    }
    if digits == 0 {
        return None;
    }
    let mantissa_end = index;
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            index = mantissa_end;
        }
    }
    Some(&text[..index])
}

fn increment_decimal(digits: &mut Vec<u8>) {
    for digit in digits.iter_mut().rev() {
        if *digit != b'9' {
            *digit += 1;
            return;
        }
        *digit = b'0';
    }
    digits.insert(0, b'1');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_invalid_values_preserve_initial_bits() {
        assert_eq!(read_i32(None, 37), 37);
        assert_eq!(read_i32(Some("oops"), -12), -12);
        assert!(read_bool(None, true));
        assert!(read_bool(Some("TRUE"), true));
        assert!(!read_bool(Some("invalid"), false));
        let f32_default = f32::from_bits(0xffc0_0123);
        let f64_default = f64::from_bits(0xfff8_0000_0000_0123);
        for text in [None, Some("invalid"), Some("")] {
            assert_eq!(read_f32(text, f32_default).to_bits(), f32_default.to_bits());
            assert_eq!(read_f64(text, f64_default).to_bits(), f64_default.to_bits());
        }
        assert_eq!(read_f32(None, -0.0).to_bits(), (-0.0_f32).to_bits());
        assert_eq!(read_f64(None, -0.0).to_bits(), (-0.0_f64).to_bits());
    }

    #[test]
    fn crt_integer_prefix_and_wrapping_golden() {
        // 本机签名有效的 MSVCR120 12.00.40664.0 x64 实测，另有游戏 x86 累积路径佐证。
        for (text, expected) in [
            ("  -12tail", -12),
            ("+7.5", 7),
            ("0x10", 0),
            ("010", 10),
            ("1e2", 1),
            ("2147483647", i32::MAX),
            ("2147483648", i32::MIN),
            ("-2147483649", i32::MAX),
            ("4294967296", 0),
            ("999999999999999999999999999999999999", -1),
            ("\t\u{000b}\u{000c}\r\n-7tail", -7),
        ] {
            assert_eq!(parse_i32(text), Some(expected), "{text:?}");
        }
        for text in ["", "+", "-", "  ", "oops", "\u{00a0}1"] {
            assert_eq!(parse_i32(text), None, "{text:?}");
        }
    }

    #[test]
    fn bool_scans_integer_before_exact_lowercase_words() {
        for (text, expected) in [
            ("true", true),
            ("false", false),
            ("1", true),
            ("0", false),
            ("-2tail", true),
            ("  +0false", false),
            ("0.9", false),
            ("2147483648", true),
            ("4294967296", false),
            ("true\0ignored", true),
        ] {
            assert_eq!(parse_bool(text), Some(expected), "{text:?}");
        }
        for text in ["TRUE", "False", " true", "false ", "trueSuffix", "yes", ""] {
            assert_eq!(parse_bool(text), None, "{text:?}");
        }
        assert_eq!(write_bool(true), "1");
        assert_eq!(write_bool(false), "0");
    }

    #[test]
    fn crt_float_prefix_and_invalid_exponent_golden() {
        for (text, expected) in [
            ("1.25foo", 1.25),
            (".5x", 0.5),
            ("1.x", 1.0),
            ("1e2tail", 100.0),
            ("1e", 1.0),
            ("1e+", 1.0),
            ("1e-foo", 1.0),
            ("1.#INF", 1.0),
            ("-1.#IND", -1.0),
            ("0x1p2", 0.0),
            ("1,25", 1.0),
        ] {
            assert_eq!(parse_f64(text), Some(expected), "{text:?}");
            assert_eq!(parse_f32(text), Some(expected as f32), "{text:?}");
        }
        for text in [
            "",
            "e10",
            "+.",
            ".",
            "nan",
            "NaN",
            "inf",
            "infinity",
            "\u{00a0}1",
        ] {
            assert_eq!(parse_f32(text), None, "{text:?}");
            assert_eq!(parse_f64(text), None, "{text:?}");
        }
    }

    #[test]
    fn float_widths_overflow_underflow_and_negative_zero_golden() {
        assert_eq!(parse_f32("0.1").unwrap().to_bits(), 0x3dcc_cccd);
        assert_eq!(parse_f64("0.1").unwrap().to_bits(), 0x3fb9_9999_9999_999a);
        assert_eq!(parse_f32("16777217"), Some(16777216.0));
        assert_eq!(parse_f64("16777217"), Some(16777217.0));
        assert_eq!(parse_f32("+1.5e-2end").unwrap().to_bits(), 0x3c75_c28f);
        assert_eq!(
            parse_f64("+1.5e-2end").unwrap().to_bits(),
            0x3f8e_b851_eb85_1eb8
        );
        assert_eq!(
            parse_f32("3.4028235677973366e38").unwrap().to_bits(),
            0x7f7f_ffff
        );
        assert_eq!(parse_f32("1e309"), Some(f32::INFINITY));
        assert_eq!(parse_f64("1e309"), Some(f64::INFINITY));
        assert_eq!(parse_f32("-1e9999"), Some(f32::NEG_INFINITY));
        assert_eq!(parse_f64("-1e9999"), Some(f64::NEG_INFINITY));
        assert_eq!(parse_f32("1e-9999"), Some(0.0));
        assert_eq!(parse_f64("1e-9999"), Some(0.0));
        assert_eq!(parse_f32("-0").unwrap().to_bits(), 0x8000_0000);
        assert_eq!(parse_f64("-0").unwrap().to_bits(), 0x8000_0000_0000_0000);
        assert_eq!(parse_i32("12\0junk"), Some(12));
        assert_eq!(parse_f64("1.5\0e2"), Some(1.5));
    }

    #[test]
    fn crt_fixed_six_and_float32_promotion_golden() {
        for (value, expected) in [
            (0.0, "0.000000"),
            (-0.0, "-0.000000"),
            (1.0, "1.000000"),
            (-12.5, "-12.500000"),
            (1.0000005, "1.000001"),
            (0.0000005, "0.000000"),
            (0.0000015, "0.000002"),
            (1.2345678, "1.234568"),
            (249740264909.23462, "249740264909.234620"),
            (f64::from_bits(0x4258_43ae_c04d_7300), "416858964277.796870"),
            (
                f64::from_bits(0xc258_43ae_c04d_7300),
                "-416858964277.796870",
            ),
        ] {
            assert_eq!(write_f64(value), expected, "{value:?}");
        }
        assert_eq!(write_f32(-0.0), "-0.000000");
        assert_eq!(write_f32(0.1), "0.100000");
        assert_eq!(write_f32(1.2345678), "1.234568");
    }

    #[test]
    fn crt_midpoints_and_adjacent_ulps_golden() {
        for (bits, expected) in [
            (0x3ff0_1fff_ffff_ffff, "1.007812"),
            (0x3ff0_2000_0000_0000, "1.007813"),
            (0x3ff0_2000_0000_0001, "1.007813"),
            (0xbff0_2000_0000_0001, "-1.007813"),
            (0xbff0_2000_0000_0000, "-1.007813"),
            (0xbff0_1fff_ffff_ffff, "-1.007812"),
            (0x3f7f_ffff_ffff_ffff, "0.007812"),
            (0x3f80_0000_0000_0000, "0.007813"),
            (0x3f80_0000_0000_0001, "0.007813"),
            (0xbf80_0000_0000_0001, "-0.007813"),
            (0xbf80_0000_0000_0000, "-0.007813"),
            (0xbf7f_ffff_ffff_ffff, "-0.007812"),
            (0x402f_fbff_ffff_ffff, "15.992187"),
            (0x402f_fc00_0000_0000, "15.992188"),
            (0x402f_fc00_0000_0001, "15.992188"),
            (0xc02f_fc00_0000_0001, "-15.992188"),
            (0xc02f_fc00_0000_0000, "-15.992188"),
            (0xc02f_fbff_ffff_ffff, "-15.992187"),
            (0x4258_43ae_c04d_72ff, "416858964277.796810"),
            (0x4258_43ae_c04d_7300, "416858964277.796870"),
            (0x4258_43ae_c04d_7301, "416858964277.796940"),
            (0xc258_43ae_c04d_7301, "-416858964277.796940"),
            (0xc258_43ae_c04d_7300, "-416858964277.796870"),
            (0xc258_43ae_c04d_72ff, "-416858964277.796810"),
        ] {
            assert_eq!(write_f64(f64::from_bits(bits)), expected, "{bits:016x}");
        }
    }

    #[test]
    fn crt_nonfinite_output_bit_patterns_golden() {
        for (bits, expected) in [
            (0x7ff0_0000_0000_0000, "1.#INF00"),
            (0xfff0_0000_0000_0000, "-1.#INF00"),
            (0x7ff8_0000_0000_0000, "1.#QNAN0"),
            (0xfff8_0000_0000_0000, "-1.#IND00"),
            (0x7ff8_0000_0000_0001, "1.#QNAN0"),
            (0xfff8_0000_0000_0001, "-1.#QNAN0"),
            (0x7ff0_0000_0000_0001, "1.#SNAN0"),
            (0xfff0_0000_0000_0001, "-1.#SNAN0"),
        ] {
            assert_eq!(write_f64(f64::from_bits(bits)), expected, "{bits:016x}");
        }
        // CRT 自身的非有限输出也不按特殊字面量读回：%lf 只读到数值前缀 1。
        assert_eq!(parse_f64(&write_f64(f64::INFINITY)), Some(1.0));
    }

    #[test]
    fn editor_angles_use_fallback_instead_of_modulo() {
        assert_eq!(editor_angle_radians(0).to_bits(), 0);
        assert_eq!(editor_angle_radians(1), FRAC_PI_2);
        assert_eq!(editor_angle_radians(2), PI);
        for angle in [3, 4, -1, i32::MIN, i32::MAX] {
            assert_eq!(editor_angle_radians(angle), PI + FRAC_PI_2);
        }
    }
}
