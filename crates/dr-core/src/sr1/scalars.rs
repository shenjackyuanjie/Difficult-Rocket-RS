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
mod tests;
