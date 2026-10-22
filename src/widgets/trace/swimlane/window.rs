//! The mounted slice of a long column of fixed-pitch rows: which rows a scroller
//! shows, and when the slice has to move. Pure; the scroller is read in `dom`.

/// Rows mounted beyond each edge of the viewport: a row of a dense graph is
/// dozens of nodes, so the margin is a few rows, not a screenful.
pub(super) const OVERSCAN: usize = 6;

/// Height assumed for a scroller nothing has measured yet: the first screen.
pub(super) const DEFAULT_VIEWPORT: f64 = 336.0;

/// Tallest viewport a window is cut for, so a host that lets the scroller
/// grow with its content still mounts a bounded number of rows.
pub(super) const MAX_VIEWPORT: f64 = 2_400.0;

/// A layout of this many items or fewer is mounted whole: the capped view is
/// never partial, and a screenful costs less than the bookkeeping.
pub(super) const WHOLE: usize = 60;

/// The rows on screen: `top` px scrolled, `viewport` px tall, rows `pitch` px
/// apart, clamped to `rows`. Half-open.
fn visible(top: f64, viewport: f64, pitch: f64, rows: usize) -> (usize, usize) {
    if rows == 0 || pitch.is_nan() || pitch <= 0.0 {
        return (0, 0);
    }
    let top = top.max(0.0);
    let first = ((top / pitch).floor() as usize).min(rows);
    let last = (((top + viewport.max(0.0)) / pitch).ceil() as usize).min(rows);
    (first, last.max(first))
}

/// The rows to mount for a viewport `viewport` px tall scrolled `top` px: the
/// rows on screen and `overscan` more on each side, as `start..end` of `rows`.
pub(super) fn y_window(
    top: f64,
    viewport: f64,
    pitch: f64,
    rows: usize,
    overscan: usize,
) -> (usize, usize) {
    let (first, last) = visible(top, viewport, pitch, rows);
    (
        first.saturating_sub(overscan),
        last.saturating_add(overscan).min(rows),
    )
}

/// Whether every row on screen is inside the `mounted` slice, so a scroll
/// needs no new one.
pub(super) fn covers(
    mounted: (usize, usize),
    top: f64,
    viewport: f64,
    pitch: f64,
    rows: usize,
) -> bool {
    let (first, last) = visible(top, viewport, pitch, rows);
    first == last || (mounted.0 <= first && last <= mounted.1)
}

/// The pixel rows to mount for a layout of `items` items over `rows` rows of
/// `pitch` px: all of them up to [`WHOLE`] items, else the window around the
/// viewport.
pub(super) fn mounted(
    items: usize,
    top: f64,
    viewport: f64,
    pitch: f64,
    rows: usize,
) -> (usize, usize) {
    if items <= WHOLE {
        (0, rows)
    } else {
        y_window(top, viewport, pitch, rows, OVERSCAN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_the_screen_plus_overscan_clamped_to_the_rows() {
        assert_eq!(y_window(0.0, 140.0, 14.0, 1_000, 24), (0, 34));
        assert_eq!(y_window(1_400.0, 140.0, 14.0, 1_000, 24), (76, 134));
        assert_eq!(y_window(13_900.0, 140.0, 14.0, 1_000, 24), (968, 1_000));
        assert_eq!(y_window(0.0, 140.0, 14.0, 5, 24), (0, 5));
    }

    #[test]
    fn an_empty_column_mounts_nothing_and_a_bad_pitch_the_first_overscan() {
        assert_eq!(y_window(500.0, 100.0, 14.0, 0, 24), (0, 0));
        assert_eq!(y_window(500.0, 100.0, 0.0, 100, 24), (0, 24));
        assert_eq!(y_window(500.0, 100.0, f64::NAN, 100, 24), (0, 24));
        assert_eq!(y_window(-50.0, 28.0, 14.0, 100, 0), (0, 2));
    }

    #[test]
    fn a_layout_up_to_the_whole_limit_is_mounted_whole_and_a_larger_one_windowed() {
        assert_eq!(mounted(WHOLE, 9_000.0, 336.0, 14.0, 500), (0, 500));
        assert_eq!(mounted(WHOLE + 1, 0.0, 336.0, 14.0, 500), (0, 24 + OVERSCAN));
        assert_eq!(
            mounted(WHOLE + 1, 1_400.0, 336.0, 14.0, 500),
            (100 - OVERSCAN, 124 + OVERSCAN)
        );
    }

    #[test]
    fn a_scroll_inside_the_overscan_keeps_the_mounted_slice() {
        let mounted = y_window(1_400.0, 140.0, 14.0, 1_000, 24);
        assert!(covers(mounted, 1_400.0, 140.0, 14.0, 1_000));
        assert!(covers(mounted, 1_400.0 + 24.0 * 14.0, 140.0, 14.0, 1_000));
        assert!(!covers(mounted, 1_400.0 + 25.0 * 14.0, 140.0, 14.0, 1_000));
        assert!(!covers(mounted, 1_400.0 - 25.0 * 14.0, 140.0, 14.0, 1_000));
    }

    #[test]
    fn an_unmeasured_scroller_mounts_a_bounded_first_screen() {
        let (start, end) = y_window(0.0, DEFAULT_VIEWPORT, 14.0, 2_000_000, OVERSCAN);
        assert_eq!((start, end), (0, 24 + OVERSCAN));
        assert_eq!(y_window(0.0, DEFAULT_VIEWPORT, 14.0, 3, OVERSCAN), (0, 3));
    }
}
