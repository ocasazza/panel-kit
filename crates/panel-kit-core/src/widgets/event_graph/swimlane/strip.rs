//! Events with no work item: one density strip per kind, each closed to a bar
//! of cells or open to a row of dots.

use serde::Serialize;

use super::super::model::{Event, EventKind};
use super::dot::{cluster, Dot};
use super::metrics::{ROW, STRIP_PX};
use super::num::round_half_up;

/// Which strip rows are expanded to dots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LooseOpen<'a> {
    /// Every row closed.
    #[default]
    None,
    /// Every row open.
    All,
    /// Just these kinds open.
    Kinds(&'a [EventKind]),
}

impl LooseOpen<'_> {
    /// Whether the row of `kind` is open.
    pub fn is_open(&self, kind: EventKind) -> bool {
        match self {
            LooseOpen::None => false,
            LooseOpen::All => true,
            LooseOpen::Kinds(kinds) => kinds.contains(&kind),
        }
    }
}

impl From<bool> for LooseOpen<'_> {
    /// `true` opens every row, `false` none.
    fn from(open: bool) -> Self {
        if open {
            LooseOpen::All
        } else {
            LooseOpen::None
        }
    }
}

/// One density cell of a closed strip row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cell {
    /// Left edge, a multiple of 4.
    pub x: i64,
    /// Width, always 4.
    pub w: u32,
    /// Events whose pixel falls in the cell.
    pub count: usize,
    /// Fill opacity, 2 decimals: the densest cell is 1.0.
    pub alpha: f64,
}

/// The strip row of one kind of loose event, open or not.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StripRow {
    /// The kind every event in the row has.
    pub kind: EventKind,
    /// Events in the row.
    pub count: usize,
    /// Density cells, left to right.
    pub cells: Vec<Cell>,
    /// Whether the row is expanded to dots.
    pub open: bool,
    /// Top of the row.
    pub y0: f64,
}

impl StripRow {
    /// Button caption: `no item: branch head`.
    pub fn label(&self) -> String {
        format!("no item: {}", self.kind.label())
    }

    /// Tooltip: `10 commit events with no work item`.
    pub fn title(&self) -> String {
        format!(
            "{} {} events with no work item",
            self.count,
            self.kind.label()
        )
    }
}

/// An open strip row: its events as dots on one line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LooseRow {
    /// The kind every event in the row has.
    pub kind: EventKind,
    /// Events in the row.
    pub count: usize,
    /// Centre line of the row.
    pub y: f64,
    /// The events, bursts merged.
    pub dots: Vec<Dot>,
}

/// Strip rows (one per kind present, sorted by kind name, open or not) and the
/// open rows' dots. `x` maps an event's instant to its pixel.
pub fn strip(
    events: &[&Event],
    x: impl Fn(&Event) -> f64,
    open: LooseOpen<'_>,
) -> (Vec<StripRow>, Vec<LooseRow>) {
    let mut by_kind: [Vec<(&Event, f64)>; EventKind::ALL.len()] = Default::default();
    for &event in events {
        by_kind[event.kind as usize].push((event, x(event)));
    }

    let mut rows = Vec::new();
    let mut loose = Vec::new();
    for kind in EventKind::BY_NAME {
        let group = &mut by_kind[kind as usize];
        if group.is_empty() {
            continue;
        }
        let y0 = rows.len() as f64 * ROW;
        let is_open = open.is_open(kind);
        rows.push(StripRow {
            kind,
            count: group.len(),
            cells: cells(group.iter().map(|(_, x)| *x)),
            open: is_open,
            y0,
        });
        if is_open {
            loose.push(LooseRow {
                kind,
                count: group.len(),
                y: y0 + ROW / 2.0,
                dots: cluster(group.drain(..)),
            });
        }
    }
    (rows, loose)
}

/// One cell per 4px, darker where more events fall.
fn cells(xs: impl Iterator<Item = f64>) -> Vec<Cell> {
    let mut starts: Vec<i64> = xs
        .map(|x| (x / STRIP_PX).trunc() as i64 * STRIP_PX as i64)
        .collect();
    starts.sort_unstable();

    let mut counted: Vec<(i64, usize)> = Vec::new();
    for start in starts {
        match counted.last_mut() {
            Some((last, n)) if *last == start => *n += 1,
            _ => counted.push((start, 1)),
        }
    }

    let peak = counted.iter().map(|(_, n)| *n).max().unwrap_or(1);
    counted
        .into_iter()
        .map(|(x, count)| Cell {
            x,
            w: STRIP_PX as u32,
            count,
            alpha: round_half_up(0.25 + 0.75 * count as f64 / peak as f64, 2),
        })
        .collect()
}
