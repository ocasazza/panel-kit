//! Form value parsing, clamping, and slider mapping for the shared form rows.
//!
//! Ported from jump-cannon's layout-panel form rows (`panels/layout.rs`):
//! every numeric entry is parsed then clamped into its declared range, and
//! logarithmic sliders map onto a 0–1000 position so a range input can cover
//! orders of magnitude.

/// Parse a text entry as `f64`, clamped into `[min, max]`.
pub fn parse_clamped_f64(raw: &str, min: f64, max: f64) -> Option<f64> {
    raw.trim().parse::<f64>().ok().map(|v| v.clamp(min, max))
}

/// Parse a text entry as `i64`, clamped into `[min, max]`.
pub fn parse_clamped_i64(raw: &str, min: i64, max: i64) -> Option<i64> {
    raw.trim().parse::<i64>().ok().map(|v| v.clamp(min, max))
}

/// Format an `f64` the way the egui `DragValue` did: `f32` precision, no
/// trailing decimals (`1.5` → `"1.5"`, `2.0` → `"2"`).
pub fn fmt_f(v: f64) -> String {
    format!("{}", v as f32)
}

/// Slider position (`0..=1000`) for a log-mapped slider over `[min, max]`.
/// `min` must be positive; the value is floored at `min` first.
pub fn log_position(value: f64, min: f64, max: f64) -> f64 {
    1000.0 * (value.max(min) / min).ln() / (max / min).ln()
}

/// The value of a log-mapped slider at `position` (`0..=1000`).
pub fn log_value(position: f64, min: f64, max: f64) -> f64 {
    min * (max / min).powf(position / 1000.0)
}

/// The `(min, max, step)` a range input paints with: linear sliders span the
/// declared range in 1000 steps, log sliders span the 0–1000 position space.
pub fn slider_range(min: f64, max: f64, log: bool) -> (f64, f64, f64) {
    if log {
        (0.0, 1000.0, 1.0)
    } else {
        (min, max, ((max - min) / 1000.0).max(1e-6))
    }
}

#[cfg(test)]
mod tests {
    use super::{fmt_f, log_position, log_value, parse_clamped_f64, parse_clamped_i64, slider_range};

    #[test]
    fn int_entries_clamp_into_range() {
        assert_eq!(parse_clamped_i64("300", 1, 8), Some(8));
        assert_eq!(parse_clamped_i64("-5", 1, 8), Some(1));
        assert_eq!(parse_clamped_i64("4", 1, 8), Some(4));
        assert_eq!(parse_clamped_i64("abc", 1, 8), None);
        assert_eq!(parse_clamped_i64(" 7 ", 1, 8), Some(7));
    }

    #[test]
    fn float_entries_clamp_into_range() {
        assert_eq!(parse_clamped_f64("42", 0.1, 10.0), Some(10.0));
        assert_eq!(parse_clamped_f64("2.5", 0.1, 10.0), Some(2.5));
        assert_eq!(parse_clamped_f64("", 0.1, 10.0), None);
    }

    #[test]
    fn float_formatting_matches_the_drag_value() {
        assert_eq!(fmt_f(2.0), "2");
        assert_eq!(fmt_f(1.5), "1.5");
        assert_eq!(fmt_f(0.25), "0.25");
    }

    #[test]
    fn log_slider_round_trips() {
        for value in [0.05_f64, 1.0, 7.5, 250.0, 1000.0] {
            let pos = log_position(value, 0.01, 1000.0);
            assert!((0.0..=1000.0).contains(&pos));
            let back = log_value(pos, 0.01, 1000.0);
            assert!((back - value).abs() < value.abs() * 1e-3 + 1e-6, "{value} -> {pos} -> {back}");
        }
        // Values below the minimum floor at the bottom of the range.
        assert_eq!(log_position(0.001, 0.01, 1000.0), 0.0);
    }

    #[test]
    fn slider_ranges_by_mapping() {
        assert_eq!(slider_range(0.1, 100.0, true), (0.0, 1000.0, 1.0));
        let (min, max, step) = slider_range(1.0, 9.0, false);
        assert_eq!((min, max), (1.0, 9.0));
        assert!((step - 0.008).abs() < 1e-9);
    }
}
