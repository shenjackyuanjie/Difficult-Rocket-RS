use crate::sr1::scalars::*;

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
