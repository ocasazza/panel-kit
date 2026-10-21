//! Decimal rounding of pixel numbers, exact on the binary value.

/// `x` rounded to `digits` decimals, ties up (away from zero), judged on the
/// exact binary value: the rule of Elixir's `Float.round/2`, which neither
/// `f64::round` after scaling nor `{:.N}` formatting follows. `0.625` rounds
/// to `0.63` at two digits, `1234.55` (stored just under) to `1234.5` at one.
/// `digits` is clamped to 15.
pub fn round_half_up(x: f64, digits: u32) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let digits = digits.min(15);
    let bits = x.abs().to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    // |x| = mantissa * 2^exp
    let (mantissa, exp) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), biased - 1075)
    };
    if exp >= 0 {
        return x;
    }
    let scale = 10u128.pow(digits);
    let scaled = u128::from(mantissa) * scale;
    let shift = exp.unsigned_abs();
    let (whole, up) = if shift >= 128 {
        (0, false)
    } else {
        let rest = scaled & ((1u128 << shift) - 1);
        (scaled >> shift, rest >= (1u128 << (shift - 1)))
    };
    let rounded = (whole + u128::from(up)) as f64 / scale as f64;
    if x < 0.0 {
        -rounded
    } else {
        rounded
    }
}

/// `x` rounded to one decimal and printed with exactly one: `12.0`, `1234.5`.
/// The number form of an SVG path.
pub fn fmt1(x: f64) -> String {
    format!("{:.1}", round_half_up(x, 1))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0.625, 2, 0.63)]
    #[case(0.375, 2, 0.38)]
    #[case(0.125, 2, 0.13)]
    #[case(12.25, 1, 12.3)]
    #[case(12.75, 1, 12.8)]
    #[case(0.34375, 2, 0.34)]
    #[case(1234.55, 1, 1234.5)]
    #[case(0.05, 1, 0.1)]
    #[case(0.25 + 0.75 * 1.0 / 2.0, 2, 0.63)]
    #[case(0.25 + 0.75 * 1.0 / 1.0, 2, 1.0)]
    #[case(7.0, 1, 7.0)]
    #[case(0.0, 1, 0.0)]
    #[case(-12.25, 1, -12.3)]
    #[case(1e300, 1, 1e300)]
    fn rounds_half_up_on_the_binary_value(#[case] x: f64, #[case] digits: u32, #[case] want: f64) {
        assert_eq!(round_half_up(x, digits), want);
    }

    #[rstest]
    #[case(12.0, "12.0")]
    #[case(1234.5, "1234.5")]
    #[case(7.0, "7.0")]
    #[case(12.25, "12.3")]
    #[case(0.0, "0.0")]
    #[case(59.949999999999996, "59.9")]
    #[case(1234.55, "1234.5")]
    fn fmt1_prints_one_decimal(#[case] x: f64, #[case] want: &str) {
        assert_eq!(fmt1(x), want);
    }

    #[test]
    fn non_finite_passes_through() {
        assert!(round_half_up(f64::NAN, 1).is_nan());
        assert_eq!(round_half_up(f64::INFINITY, 1), f64::INFINITY);
    }

    proptest! {
        #[test]
        fn rounding_moves_at_most_half_a_unit(x in 0.0f64..1.0e7, digits in 0u32..4) {
            let unit = 10f64.powi(-(digits as i32));
            prop_assert!((round_half_up(x, digits) - x).abs() <= unit / 2.0 + 1e-6);
        }

        #[test]
        fn rounding_is_idempotent(x in 0.0f64..1.0e6) {
            let once = round_half_up(x, 1);
            prop_assert_eq!(round_half_up(once, 1), once);
        }

        #[test]
        fn fmt1_parses_back_to_the_rounded_value(x in 0.0f64..1.0e6) {
            let text = fmt1(x);
            prop_assert_eq!(text.rsplit('.').next().map(str::len), Some(1));
            prop_assert_eq!(text.parse::<f64>().ok(), Some(round_half_up(x, 1)));
        }
    }
}
