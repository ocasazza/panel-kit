//! A bounded ring of position frames with keyframe + delta compression.
//!
//! Port of jump-cannon's timeline frame ring (itself a port of its egui
//! graph-renderer `timeline.rs`): full keyframes are stored on a cadence and
//! every other frame is stored as a per-component delta against the previous
//! stored frame, so a deep scrub history costs a fraction of its raw size.
//! Indexing is logical: `get(0)` is always the oldest retained frame.

use std::collections::VecDeque;

/// One stored frame: either a full keyframe or a delta against the
/// immediately-preceding stored frame.
#[derive(Clone, Debug)]
enum Frame {
    /// Full raw positions: `[x0,y0,z0, x1,y1,z1, ...]`, length `3 * n`.
    Key(Vec<f32>),
    /// Per-component delta vs the previous stored frame, length `3 * n`.
    Delta(Vec<f32>),
}

impl Frame {
    fn len(&self) -> usize {
        match self {
            Frame::Key(v) | Frame::Delta(v) => v.len(),
        }
    }
}

/// A bounded ring of position frames with keyframe+delta compression.
/// Indexing is logical: `get(0)` is the oldest retained frame.
#[derive(Clone, Debug)]
pub struct FrameRing {
    frames: VecDeque<Frame>,
    depth: usize,
    keyframe_interval: usize,
    /// Monotonic push count — drives the keyframe cadence so eviction never
    /// shifts the keyframe phase.
    push_count: usize,
    /// Expected component count (`3 * n`); a push with a different length
    /// resets the ring (the graph changed out from under us).
    components: Option<usize>,
}

impl FrameRing {
    /// A ring holding at most `depth` frames, storing a full keyframe every
    /// `keyframe_interval` pushes. Both are floored at 1.
    pub fn new(depth: usize, keyframe_interval: usize) -> Self {
        Self {
            frames: VecDeque::new(),
            depth: depth.max(1),
            keyframe_interval: keyframe_interval.max(1),
            push_count: 0,
            components: None,
        }
    }

    /// Retained frame count.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether no frame is retained.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Maximum retained frames.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Component count of the current capture session, if any.
    pub fn components(&self) -> Option<usize> {
        self.components
    }

    /// Approximate retained byte budget: `Σ frame.len() * 4`.
    pub fn approx_bytes(&self) -> usize {
        self.frames.iter().map(Frame::len).sum::<usize>() * 4
    }

    /// Drop every frame and forget the capture session.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.components = None;
        self.push_count = 0;
    }

    /// Change the maximum retained frames, evicting oldest-first past it.
    pub fn set_depth(&mut self, depth: usize) {
        self.depth = depth.max(1);
        while self.frames.len() > self.depth {
            self.pop_front_preserving();
        }
    }

    /// Push a new position frame: keyframe on the cadence (and always first),
    /// otherwise a delta vs the previous logical frame. Evicts past `depth`.
    /// A push whose length differs from the session's resets the ring first.
    pub fn push(&mut self, positions: &[f32]) {
        if positions.is_empty() {
            return;
        }
        match self.components {
            Some(c) if c != positions.len() => self.clear(),
            _ => {}
        }
        self.components = Some(positions.len());

        let is_keyframe = self.frames.is_empty() || self.push_count % self.keyframe_interval == 0;
        let frame = if is_keyframe {
            Frame::Key(positions.to_vec())
        } else {
            let prev = self
                .reconstruct(self.frames.len() - 1)
                .expect("non-empty ring has a last frame");
            let mut delta = vec![0.0_f32; positions.len()];
            for (delta_i, (prev_i, next_i)) in delta.iter_mut().zip(prev.iter().zip(positions)) {
                *delta_i = next_i - prev_i;
            }
            Frame::Delta(delta)
        };

        self.frames.push_back(frame);
        self.push_count = self.push_count.wrapping_add(1);

        while self.frames.len() > self.depth {
            self.pop_front_preserving();
        }
    }

    /// Reconstruct absolute positions at logical index `idx`; cost is bounded
    /// by `keyframe_interval` (nearest keyframe + folded deltas).
    pub fn get(&self, idx: usize) -> Option<Vec<f32>> {
        self.reconstruct(idx)
    }

    /// Reconstruct the newest retained frame.
    pub fn latest(&self) -> Option<Vec<f32>> {
        if self.frames.is_empty() {
            None
        } else {
            self.reconstruct(self.frames.len() - 1)
        }
    }

    fn reconstruct(&self, idx: usize) -> Option<Vec<f32>> {
        if idx >= self.frames.len() {
            return None;
        }
        let mut key_at = idx;
        loop {
            match &self.frames[key_at] {
                Frame::Key(_) => break,
                Frame::Delta(_) => {
                    if key_at == 0 {
                        // Should never happen: the oldest retained frame is
                        // always promoted to a keyframe on eviction.
                        return None;
                    }
                    key_at -= 1;
                }
            }
        }
        let Frame::Key(base) = &self.frames[key_at] else {
            return None;
        };
        let mut acc = base.clone();
        for j in (key_at + 1)..=idx {
            if let Frame::Delta(d) = &self.frames[j] {
                for (a, dv) in acc.iter_mut().zip(d.iter()) {
                    *a += *dv;
                }
            }
        }
        Some(acc)
    }

    /// Pop the oldest frame, promoting the new front to a keyframe if its
    /// keyframe base just got evicted — keeps `get(0)` always valid.
    fn pop_front_preserving(&mut self) {
        let promote = if self.frames.len() >= 2 {
            match &self.frames[1] {
                Frame::Delta(_) => self.reconstruct(1),
                Frame::Key(_) => None,
            }
        } else {
            None
        };
        self.frames.pop_front();
        if let Some(abs) = promote {
            if let Some(front) = self.frames.front_mut() {
                *front = Frame::Key(abs);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, FrameRing};

    fn frame(seed: f32) -> Vec<f32> {
        vec![seed, seed + 1.0, seed + 2.0, seed + 3.0, seed + 4.0, seed + 5.0]
    }

    #[test]
    fn reconstructs_keyframes_and_deltas_exactly() {
        let mut ring = FrameRing::new(8, 3);
        let frames: Vec<_> = (0..7).map(|i| frame(i as f32 * 10.0)).collect();
        for positions in &frames {
            ring.push(positions);
        }
        for (i, expected) in frames.iter().enumerate() {
            assert_eq!(ring.get(i).as_deref(), Some(expected.as_slice()));
        }
        assert!(matches!(ring.frames[0], Frame::Key(_)));
        assert!(matches!(ring.frames[3], Frame::Key(_)));
        assert!(matches!(ring.frames[6], Frame::Key(_)));
    }

    #[test]
    fn eviction_promotes_the_new_front_to_a_keyframe() {
        let mut ring = FrameRing::new(3, 8);
        let frames: Vec<_> = (0..5).map(|i| frame(i as f32)).collect();
        for positions in &frames {
            ring.push(positions);
        }
        assert_eq!(ring.len(), 3);
        assert!(matches!(ring.frames.front(), Some(Frame::Key(_))));
        assert_eq!(ring.get(0).as_deref(), Some(frames[2].as_slice()));
        assert_eq!(ring.latest().as_deref(), Some(frames[4].as_slice()));
    }

    #[test]
    fn component_count_change_resets_implicitly() {
        let mut ring = FrameRing::new(8, 4);
        ring.push(&frame(1.0));
        ring.push(&frame(2.0));
        let replacement = vec![9.0, 8.0, 7.0];
        ring.push(&replacement);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.components(), Some(3));
        assert_eq!(ring.latest().as_deref(), Some(replacement.as_slice()));
    }

    #[test]
    fn same_arity_graph_requires_explicit_session_reset() {
        let mut ring = FrameRing::new(8, 4);
        ring.push(&frame(1.0));
        ring.push(&frame(2.0));
        // A replacement graph may have the same node count, so component
        // count cannot identify the session boundary.
        ring.clear();
        let replacement = frame(100.0);
        ring.push(&replacement);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.get(0).as_deref(), Some(replacement.as_slice()));
    }

    #[test]
    fn shrinking_depth_preserves_the_newest_frames() {
        let mut ring = FrameRing::new(6, 4);
        let frames: Vec<_> = (0..6).map(|i| frame(i as f32)).collect();
        for positions in &frames {
            ring.push(positions);
        }
        ring.set_depth(2);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.get(0).as_deref(), Some(frames[4].as_slice()));
        assert_eq!(ring.latest().as_deref(), Some(frames[5].as_slice()));
    }
}
