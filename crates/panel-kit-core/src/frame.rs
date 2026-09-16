//! Borrowed workspace frame projection for renderer-neutral painters.
//!
//! Hosts own a [`crate::reducer::Snapshot`], keep a reusable
//! [`ProjectionBuffer`], and call [`project_into`] for each render. The returned
//! [`ProjectedFrame`] borrows only the scratch buffer so applications can drop
//! the frame before mutating state for the next event.

mod chrome;
mod hit;
mod layout;
mod partial;
mod projection;
mod projection_record;
mod scratch;
mod tiling;

#[cfg(test)]
mod tests;

pub use hit::hit_test;
pub use layout::{
    FrameStatus, PanelChromeProjection, Placement, TileFillOrder, TileGridProjection,
    TileLayoutMetrics,
};
pub use partial::{project_chrome, project_dock_into, project_panel};
pub use projection::project_into;
pub use projection_record::{
    ChromeProjectionInput, DockProjection, PanelProjection, PanelProjectionInput, ProjectedFrame,
    ProjectionInput,
};
pub use scratch::ProjectionBuffer;
