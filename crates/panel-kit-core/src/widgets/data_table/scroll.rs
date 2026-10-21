//! Scroll offsets of a height-capped scroller, mapped onto a table's entries.
//!
//! Browsers clamp an element's height (Chromium at 2^25 pixels, Firefox near 17.9M), so
//! entries that stand taller than [`MAX_SCROLL`] squeeze their scroll range instead.

use super::state::{MAX_WINDOW, OVERSCAN, WINDOW_COUNT};

/// The tallest scroll height a table lays out, in pixels: below every browser's clamp on
/// an element's height.
pub const MAX_SCROLL: f64 = 15_000_000.0;

/// Where a scroller's offset lands in `entries` of `row` pixels, the scroller showing
/// `visible` pixels of them.
///
/// Up to [`MAX_SCROLL`] pixels an offset is a content offset. Beyond it the offsets between
/// two rows' worth of travel at each end scale onto the whole content, and the ends keep the
/// content's pace so the spacers around any window stay non-negative.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollMap {
    entries: usize,
    row: f64,
    visible: f64,
    /// `entries × row`.
    content: f64,
    /// The laid-out height: the content, capped at [`MAX_SCROLL`].
    height: f64,
    /// Travel at each end that keeps the content's pace.
    edge: f64,
    /// Travel between the two ends.
    span: f64,
    /// Content pixels per scroll pixel across `span`; 1 when nothing scales.
    ratio: f64,
}

/// A finite offset, at least 0.
pub(super) fn clean(pixels: f64) -> f64 {
    if pixels.is_finite() {
        pixels.max(0.0)
    } else {
        0.0
    }
}

impl ScrollMap {
    /// The map for `entries` of `row` pixels in a scroller showing `visible` pixels.
    pub fn new(entries: usize, row: f64, visible: f64) -> ScrollMap {
        let row = if row.is_finite() && row > 0.0 { row } else { 1.0 };
        let content = entries as f64 * row;
        let capped = content > MAX_SCROLL;
        let visible = clean(visible).min(content.min(MAX_SCROLL) / 2.0);
        let edge = 2.0 * row;
        let span = MAX_SCROLL - visible - 2.0 * edge;
        let (height, ratio) = if capped && span > 0.0 {
            (MAX_SCROLL, (content - visible - 2.0 * edge) / span)
        } else {
            (content, 1.0)
        };
        let span = if ratio > 1.0 { span } else { 0.0 };
        ScrollMap {
            entries,
            row,
            visible,
            content,
            height,
            edge,
            span,
            ratio,
        }
    }

    /// The height the entries lay out at, in pixels.
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Whether the entries stand taller than the scroller can, so offsets scale.
    pub fn scaled(&self) -> bool {
        self.ratio > 1.0
    }

    /// Content pixels one scroll pixel covers between the ends; 1 when nothing scales.
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Content pixels the scaling has put above the viewport at scroll offset `top`.
    fn slack(&self, top: f64) -> f64 {
        if !self.scaled() || top <= self.edge {
            0.0
        } else if top >= self.edge + self.span {
            self.content - self.height
        } else {
            (self.ratio - 1.0) * (top - self.edge)
        }
    }

    /// The content offset at the viewport's top edge when the scroller sits at `top`.
    pub fn offset(&self, top: f64) -> f64 {
        let top = clean(top);
        top + self.slack(top)
    }

    /// The entry at the viewport's top edge at scroll offset `top`.
    pub fn first(&self, top: f64) -> usize {
        ((self.offset(top) / self.row) as usize).min(self.entries.saturating_sub(1))
    }

    /// The scroll offset that puts content offset `offset` at the viewport's top edge: the
    /// inverse of [`ScrollMap::offset`], held to the last offset the content can reach.
    pub fn top_at(&self, offset: f64) -> f64 {
        let offset = clean(offset);
        if !self.scaled() {
            return offset;
        }
        let reach = self.content - self.visible;
        let offset = offset.min(reach);
        if offset <= self.edge {
            offset
        } else if offset >= reach - self.edge {
            offset - (self.content - self.height)
        } else {
            self.edge + (offset - self.edge) / self.ratio
        }
    }

    /// The entries `start..end` to mount at scroll offset `top`: the viewport's plus
    /// overscan, held to the rows the spacers around them can make room for.
    pub fn window(&self, top: f64) -> (usize, usize) {
        if self.entries == 0 {
            return (0, 0);
        }
        let top = clean(top);
        let slack = self.slack(top);
        let offset = top + slack;
        let first = ((offset / self.row) as usize).min(self.entries - 1);
        let seen = if self.visible > 0.0 {
            ((offset + self.visible) / self.row).ceil() as usize
        } else {
            first + WINDOW_COUNT - OVERSCAN
        }
        .clamp(first + 1, self.entries);
        let low = ((slack / self.row).ceil() as usize).min(first);
        let high = (((self.height + slack) / self.row) as usize).clamp(seen, self.entries);
        let start = first.saturating_sub(OVERSCAN).max(low);
        let end = (seen + OVERSCAN).min(high);
        (start, end.min(start + MAX_WINDOW))
    }

    /// The heights, in pixels, of the spacers above and below the mounted `start..end` at
    /// scroll offset `top`. With the rows between them they add up to [`ScrollMap::height`].
    pub fn spacers(&self, top: f64, start: usize, end: usize) -> (f64, f64) {
        let start = start.min(self.entries);
        let end = end.clamp(start, self.entries);
        if !self.scaled() {
            return (
                start as f64 * self.row,
                (self.entries - end) as f64 * self.row,
            );
        }
        let above = (start as f64 * self.row - self.slack(clean(top)))
            .floor()
            .max(0.0);
        let below = (self.height - above - (end - start) as f64 * self.row).max(0.0);
        (above, below)
    }

    /// The scroll offset that brings `entry` wholly into view moving the content least,
    /// or `None` when it already is. Whole pixels, rounded the way that keeps the entry in.
    pub fn reveal(&self, top: f64, entry: usize) -> Option<f64> {
        let last = self.entries.checked_sub(1)?;
        let at = entry.min(last) as f64 * self.row;
        let from = self.offset(top);
        let (to, up) = if at < from {
            (at, true)
        } else if at + self.row > from + self.visible {
            (at + self.row - self.visible, false)
        } else {
            return None;
        };
        let aim = self.top_at(to);
        Some(if up { aim.floor() } else { aim.ceil() })
    }

    /// The offset in `to` that keeps the row at the viewport's top edge where `top` has it
    /// here, for when rows change height.
    pub fn rescaled(&self, top: f64, to: &ScrollMap) -> f64 {
        to.top_at(self.offset(top) / self.row * to.row)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// The detail cards the browser run showed clamping: 1,003,805 entries of 64 pixels.
    fn detail() -> ScrollMap {
        ScrollMap::new(1_003_805, 64.0, 600.0)
    }

    /// The last scroll offset: the viewport's bottom edge on the content's end.
    fn reach(map: &ScrollMap) -> f64 {
        map.height() - map.visible
    }

    /// A whole-pixel scroll offset, as a browser reports one: `kind` picks anywhere in the
    /// range (by `fraction`) or `near` pixels from either end, where the layout is tightest.
    fn pick(map: &ScrollMap, kind: u8, fraction: f64, near: u32) -> f64 {
        let reach = reach(map);
        let top = match kind % 3 {
            0 => fraction * reach,
            1 => f64::from(near),
            _ => reach - f64::from(near),
        };
        top.clamp(0.0, reach).round()
    }

    /// The promises the layout at `top` breaks; none when it keeps them all.
    fn faults(map: &ScrollMap, top: f64) -> Vec<String> {
        let (start, end) = map.window(top);
        let (above, below) = map.spacers(top, start, end);
        let rows = (end - start) as f64 * map.row;
        let shown = start as f64 * map.row + (top - above);
        let at_top = start + ((top - above) / map.row) as usize;
        let checks = [
            (
                start < end && end <= map.entries && end - start <= MAX_WINDOW,
                format!("window {start}..{end} of {}", map.entries),
            ),
            (
                above >= 0.0 && below >= 0.0,
                format!("negative spacer {above} / {below}"),
            ),
            (
                (above + rows + below - map.height()).abs() <= 1e-6 * map.height(),
                format!("{above} + {rows} + {below} is not {}", map.height()),
            ),
            (
                (shown - map.offset(top)).abs() <= 1.0 + 1e-6,
                format!("the top edge shows {shown}, not {}", map.offset(top)),
            ),
            (
                at_top.abs_diff(map.first(top)) <= 1,
                format!("entry {at_top} at the top, not {}", map.first(top)),
            ),
            (
                above <= top + 1e-6,
                format!("{above} of spacer above a {top} scroll"),
            ),
            (
                above + rows + 1.0 + 1e-6 >= (top + map.visible).min(map.height()),
                format!("rows stop at {} short of the viewport", above + rows),
            ),
        ];
        checks
            .into_iter()
            .filter(|(ok, _)| !ok)
            .map(|(_, what)| what)
            .collect()
    }

    #[test]
    fn entries_taller_than_the_cap_lay_out_at_the_cap_and_reach_the_last_entry() {
        let map = detail();
        assert!(map.scaled() && map.ratio() > 4.0);
        assert_eq!(map.height(), MAX_SCROLL);

        let top = reach(&map);
        let (start, end) = map.window(top);
        assert_eq!(end, 1_003_805, "the last entry is mounted at the last offset");
        let (above, below) = map.spacers(top, start, end);
        assert_eq!(below, 0.0);
        assert_eq!(above + (end - start) as f64 * 64.0, MAX_SCROLL);
        assert_eq!(
            map.offset(top) + map.visible,
            1_003_805.0 * 64.0,
            "the viewport's bottom edge sits on the content's end"
        );
    }

    #[test]
    fn the_middle_of_the_scroll_range_is_the_middle_of_the_entries() {
        let map = detail();
        let first = map.first(reach(&map) / 2.0);
        assert!(first.abs_diff(1_003_805 / 2) < 100, "middle showed entry {first}");
    }

    /// Where the rows above (or below) the viewport can hold no more than the pixels
    /// scrolled: every whole pixel of the first and last 400.
    #[test]
    fn the_first_and_last_scroll_pixels_lay_out_like_the_middle() {
        for map in [
            detail(),
            ScrollMap::new(1_003_805, 26.0, 629.0),
            ScrollMap::new(5_000_000, 64.0, 100.0),
            ScrollMap::new(240_000, 64.0, 2_000.0),
        ] {
            for step in 0..=400 {
                for top in [f64::from(step), reach(&map) - f64::from(step)] {
                    let broken = faults(&map, top);
                    assert!(
                        broken.is_empty(),
                        "{} entries of {} at {top}: {broken:?}",
                        map.entries,
                        map.row
                    );
                }
            }
        }
    }

    #[test]
    fn a_map_that_fits_keeps_the_plain_spacers_whatever_the_offset() {
        let map = ScrollMap::new(200, 64.0, 600.0);
        assert!(!map.scaled() && map.height() == 12_800.0);
        for top in [0.0, 333.0, 12_200.0] {
            assert_eq!(map.spacers(top, 120, 160), (7_680.0, 2_560.0));
        }
    }

    #[test]
    fn a_fit_to_the_pixel_is_not_scaled() {
        let map = ScrollMap::new(234_375, 64.0, 600.0);
        assert!(!map.scaled() && map.height() == MAX_SCROLL);
        assert!(ScrollMap::new(234_376, 64.0, 600.0).scaled());
    }

    #[test]
    fn nonsense_offsets_and_sizes_stay_inside_the_entries() {
        let map = detail();
        for top in [f64::NAN, f64::INFINITY, -50.0, 1e30] {
            let (start, end) = map.window(top);
            assert!(start < end && end <= 1_003_805, "top {top}: {start}..{end}");
            assert!(map.first(top) < 1_003_805);
        }
        assert_eq!(ScrollMap::new(0, 64.0, 600.0).window(10.0), (0, 0));
        assert_eq!(ScrollMap::new(0, 64.0, 600.0).reveal(0.0, 0), None);
        assert!(!ScrollMap::new(10, f64::NAN, f64::NAN).scaled());
    }

    #[test]
    fn an_unmeasured_viewport_mounts_the_default_window() {
        let map = ScrollMap::new(1_003_805, 64.0, 0.0);
        assert_eq!(map.window(0.0), (0, WINDOW_COUNT));
    }

    #[test]
    fn revealing_an_entry_moves_the_content_the_least() {
        let map = detail();
        let top = 5_000_000.0;
        let first = map.first(top);
        // Fully inside the viewport: nothing to do.
        assert_eq!(map.reveal(top, first + 2), None);
        // Below it: its bottom edge lands on the viewport's.
        let down = map.reveal(top, first + 300).unwrap();
        assert!(down > top);
        let shown = map.offset(down);
        let bottom = (first + 301) as f64 * 64.0;
        assert!(shown + 600.0 >= bottom && shown + 600.0 < bottom + map.ratio() + 1.0);
        // Above it: its top edge lands on the viewport's.
        let up = map.reveal(top, first - 300).unwrap();
        assert!(up < top);
        let at = (first - 300) as f64 * 64.0;
        assert!(map.offset(up) <= at && map.offset(up) > at - map.ratio() - 1.0);
        // The ends.
        assert_eq!(map.reveal(top, 1_003_804), Some(reach(&map).ceil()));
        assert_eq!(map.reveal(top, 0), Some(0.0));
    }

    #[test]
    fn changing_row_height_keeps_the_first_row_in_the_viewport_top() {
        let rows = ScrollMap::new(1_003_805, 26.0, 600.0);
        let detail = detail();
        let top = 9_000_000.0;
        let kept = rows.rescaled(top, &detail);
        let before = rows.offset(top) / 26.0;
        let after = detail.offset(kept) / 64.0;
        assert!((before - after).abs() < 1.0, "{before} entries became {after}");
        let back = detail.rescaled(kept, &rows);
        assert!((back - top).abs() < 2.0 * detail.ratio());
    }

    proptest! {
        /// Whatever the entries, row, viewport and offset, anywhere or at either end of
        /// the range: the window is non-empty, the spacers are non-negative and fill the
        /// laid-out height with the rows, and the viewport's top edge shows the content
        /// the map promises, with rows under all of the viewport.
        #[test]
        fn the_layout_keeps_its_promises_at_any_offset(
            entries in 1usize..5_000_000,
            row in 20u32..=64,
            visible in 100u32..=2_000,
            kind in 0u8..3,
            fraction in 0.0f64..=1.0,
            near in 0u32..=400,
        ) {
            let map = ScrollMap::new(entries, f64::from(row), f64::from(visible));
            let top = pick(&map, kind, fraction, near);
            let broken = faults(&map, top);
            prop_assert!(broken.is_empty(), "at {top}: {broken:?}");
        }

        /// The last offset shows the last entry at the viewport's bottom edge; the first
        /// shows the first entry at its top.
        #[test]
        fn the_ends_of_the_scroll_range_show_the_ends_of_the_entries(
            entries in 1usize..5_000_000,
            row in 20u32..=64,
            visible in 100u32..=2_000,
        ) {
            let row = f64::from(row);
            let map = ScrollMap::new(entries, row, f64::from(visible));
            let top = reach(&map);
            let (start, end) = map.window(top);
            let (above, below) = map.spacers(top, start, end);
            prop_assert_eq!(end, entries);
            prop_assert_eq!(below, 0.0);
            prop_assert!(above + (end - start) as f64 * row == map.height());
            prop_assert!((map.offset(top) + map.visible - entries as f64 * row).abs() <= 1e-6);

            let (start, _) = map.window(0.0);
            prop_assert_eq!(start, 0);
            prop_assert_eq!(map.spacers(0.0, 0, 1).0, 0.0);
            prop_assert_eq!(map.first(0.0), 0);
        }

        /// Entries that fit keep the plain spacers `start × row` and `(entries − end) × row`,
        /// whatever the offset.
        #[test]
        fn entries_that_fit_get_the_plain_spacers(
            entries in 1usize..230_000,
            row in 20u32..=64,
            visible in 100u32..=2_000,
            fraction in 0.0f64..=1.0,
            start in 0usize..230_000,
            count in 1usize..300,
        ) {
            let row = f64::from(row);
            let map = ScrollMap::new(entries, row, f64::from(visible));
            prop_assert!(!map.scaled());
            let start = start.min(entries);
            let end = (start + count).min(entries);
            let top = pick(&map, 0, fraction, 0);
            prop_assert_eq!(
                map.spacers(top, start, end),
                (start as f64 * row, (entries - end) as f64 * row)
            );
            prop_assert_eq!(map.offset(top), top);
        }

        /// Scrolling one pixel never moves the content backwards or by more than the ratio,
        /// and `top_at` undoes `offset`.
        #[test]
        fn offsets_rise_steadily_and_invert(
            entries in 1usize..5_000_000,
            row in 20u32..=64,
            visible in 100u32..=2_000,
            kind in 0u8..3,
            fraction in 0.0f64..=1.0,
            near in 0u32..=400,
        ) {
            let map = ScrollMap::new(entries, f64::from(row), f64::from(visible));
            let top = pick(&map, kind, fraction, near);
            let step = map.offset(top + 1.0) - map.offset(top);
            prop_assert!(step >= 1.0 - 1e-6 && step <= map.ratio() + 1e-6, "step {step}");
            prop_assert!(
                (map.top_at(map.offset(top)) - top).abs() <= 1e-3,
                "{top} came back as {}", map.top_at(map.offset(top))
            );
        }

        /// After `reveal`, the entry is wholly inside the viewport; when it was already,
        /// there is nothing to do.
        #[test]
        fn revealing_leaves_the_entry_in_view(
            entries in 1usize..5_000_000,
            row in 20u32..=64,
            visible in 100u32..=2_000,
            kind in 0u8..3,
            fraction in 0.0f64..=1.0,
            near in 0u32..=400,
            entry in 0usize..5_000_000,
        ) {
            let row = f64::from(row);
            let map = ScrollMap::new(entries, row, f64::from(visible));
            let entry = entry % entries;
            let top = pick(&map, kind, fraction, near);
            let inside = |top: f64| {
                let from = map.offset(top);
                entry as f64 * row >= from - 1e-6
                    && (entry + 1) as f64 * row <= from + map.visible + 1e-6
            };
            match map.reveal(top, entry) {
                None => prop_assert!(inside(top)),
                Some(to) => {
                    prop_assert!(!inside(top));
                    prop_assert!(inside(to), "entry {entry} still out at {to}");
                    prop_assert_eq!(to, to.round());
                }
            }
        }
    }
}
