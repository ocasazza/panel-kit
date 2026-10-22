//! Trace model: vertical rails, horizontal swimlanes, and the query that scopes them.
//!
//! [`model`] is the one input vocabulary all three share: [`Event`], [`EventKind`],
//! [`Lane`], [`Edge`], [`SessionTree`] and [`Micros`]. [`Tone`] is the one dot
//! colour class and lives with the legend in [`query`].

pub mod facts;
pub mod model;
pub mod query;
pub mod rails;
pub mod sample;
pub mod swimlane;
pub mod time;

pub use model::is_named;
pub use model::kinds_of;
pub use model::Detail;
pub use model::Edge;
pub use model::EdgeKind;
pub use model::Event;
pub use model::EventKind;
pub use model::ItemRef;
pub use model::Lane;
pub use model::LaneKind;
pub use model::Micros;
pub use model::SessionTree;
pub use model::Source;
pub use model::UnknownKind;
pub use query::Tone;
