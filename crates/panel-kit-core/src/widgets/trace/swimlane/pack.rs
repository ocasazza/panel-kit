//! First-fit interval packing in `O(log n)` per interval.
//!
//! Intervals arrive sorted by where they begin. Each takes the first lane
//! whose last occupant ended strictly before it begins; none ⇒ a new lane. For
//! intervals that is optimal: there are exactly as many lanes as the most
//! intervals ever open at once.

/// Lane ends in a min-tree: the leftmost lane that ends before a point is a
/// root-to-leaf descent. Lanes not yet opened hold `+inf` and never match.
#[derive(Clone, Debug)]
pub struct FirstFit {
    tree: Vec<f64>,
    size: usize,
    used: usize,
}

impl FirstFit {
    /// An empty packing, with room for `lanes` lanes before it has to grow.
    pub fn new(lanes: usize) -> Self {
        let size = lanes.max(1).next_power_of_two();
        FirstFit {
            tree: vec![f64::INFINITY; 2 * size],
            size,
            used: 0,
        }
    }

    /// Lanes opened so far.
    pub fn lanes(&self) -> usize {
        self.used
    }

    /// Places an interval `[left, right]`: the first lane with `end < left`,
    /// else a new one. The lane's end becomes `right`. Returns the lane index.
    pub fn place(&mut self, left: f64, right: f64) -> usize {
        let lane = match self.tree.get(1) {
            Some(&min_end) if min_end < left => self.leftmost_ending_before(left),
            _ => {
                if self.used == self.size {
                    self.grow();
                }
                self.used += 1;
                self.used - 1
            }
        };
        self.set(lane, right);
        lane
    }

    fn leftmost_ending_before(&self, left: f64) -> usize {
        let mut node = 1;
        while node < self.size {
            node = match self.tree.get(2 * node) {
                Some(&end) if end < left => 2 * node,
                _ => 2 * node + 1,
            };
        }
        node - self.size
    }

    fn set(&mut self, lane: usize, end: f64) {
        let mut node = self.size + lane;
        if let Some(leaf) = self.tree.get_mut(node) {
            *leaf = end;
        }
        while node > 1 {
            node /= 2;
            let (a, b) = (self.tree[2 * node], self.tree[2 * node + 1]);
            self.tree[node] = a.min(b);
        }
    }

    fn grow(&mut self) {
        let size = self.size * 2;
        let mut tree = vec![f64::INFINITY; 2 * size];
        tree[size..size + self.size].copy_from_slice(&self.tree[self.size..2 * self.size]);
        for node in (1..size).rev() {
            tree[node] = tree[2 * node].min(tree[2 * node + 1]);
        }
        self.tree = tree;
        self.size = size;
    }
}

/// The reference packing: a linear scan for the first lane that ended before
/// `left`. `O(lanes)` per interval; the tests hold [`FirstFit`] to it.
#[cfg(test)]
pub fn first_fit_naive(intervals: &[(f64, f64)]) -> Vec<usize> {
    let mut ends: Vec<f64> = Vec::new();
    intervals
        .iter()
        .map(
            |&(left, right)| match ends.iter().position(|end| *end < left) {
                Some(lane) => {
                    ends[lane] = right;
                    lane
                }
                None => {
                    ends.push(right);
                    ends.len() - 1
                }
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn pack(intervals: &[(f64, f64)]) -> (Vec<usize>, usize) {
        let mut fit = FirstFit::new(0);
        let lanes = intervals.iter().map(|&(l, r)| fit.place(l, r)).collect();
        (lanes, fit.lanes())
    }

    #[test]
    fn disjoint_intervals_share_one_lane() {
        assert_eq!(
            pack(&[(0.0, 5.0), (6.0, 9.0), (10.0, 12.0)]),
            (vec![0, 0, 0], 1)
        );
    }

    #[test]
    fn touching_intervals_do_not_share_a_lane() {
        // `end < left` is strict: an interval that ends where the next begins overlaps it.
        assert_eq!(pack(&[(0.0, 5.0), (5.0, 9.0)]), (vec![0, 1], 2));
    }

    #[test]
    fn the_first_free_lane_wins_not_the_best_fit() {
        let got = pack(&[
            (0.0, 100.0),
            (1.0, 3.0),
            (2.0, 50.0),
            (10.0, 20.0),
            (60.0, 61.0),
        ]);
        assert_eq!(got.0, vec![0, 1, 2, 1, 1]);
        assert_eq!(got.1, 3);
    }

    #[test]
    fn grows_past_its_initial_capacity() {
        let wide: Vec<(f64, f64)> = (0..100).map(|i| (f64::from(i), 1000.0)).collect();
        let (lanes, count) = pack(&wide);
        assert_eq!(count, 100);
        assert_eq!(lanes, (0..100).collect::<Vec<_>>());
    }

    #[test]
    fn empty_packing_has_no_lanes() {
        assert_eq!(pack(&[]), (vec![], 0));
    }

    proptest! {
        #[test]
        fn matches_the_linear_scan(
            raw in proptest::collection::vec((0.0f64..1000.0, 0.0f64..300.0), 0..200)
        ) {
            let mut starts = raw;
            starts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let intervals: Vec<(f64, f64)> = starts.iter().map(|&(l, len)| (l, l + len)).collect();
            let (lanes, count) = pack(&intervals);
            prop_assert_eq!(&lanes, &first_fit_naive(&intervals));
            prop_assert_eq!(count, lanes.iter().max().map_or(0, |m| m + 1));
        }

        #[test]
        fn lanes_used_equal_peak_overlap(
            raw in proptest::collection::vec((0u32..500, 1u32..120), 0..150)
        ) {
            let mut intervals: Vec<(f64, f64)> =
                raw.iter().map(|&(l, len)| (f64::from(l), f64::from(l + len))).collect();
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (_, count) = pack(&intervals);
            let peak = intervals
                .iter()
                .map(|&(point, _)| intervals.iter().filter(|&&(l, r)| l <= point && point <= r).count())
                .max()
                .unwrap_or(0);
            prop_assert_eq!(count, peak);
        }
    }
}
