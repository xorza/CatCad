//! Geometry for CAD. Everything 2D lives under `sketch`: the [`Sketch`]
//! container, the [`Constraint`]s over it, and the [`Solver`] that satisfies
//! them. Three-dimensional work belongs beside it as a sibling module rather
//! than as an extension of it — the two share a crate, not a coordinate space.
//!
//! A [`Sketch`] holds points, segments, and circles together with the
//! [`Constraint`]s relating them, and [`Plane`] carries the whole of one into
//! the world it is modelled in. [`Solver`] moves the unfixed geometry until
//! every constraint is satisfied, reporting how the run went and filling in
//! what the sketch it settled on can still do:
//!
//! ```
//! # use glam::DVec2;
//! # use silverpoint::{Along, Constraint, Dimension, Outcome, Sketch, Solver};
//! let mut sketch = Sketch::default();
//! let origin = sketch.add_point(DVec2::ZERO);
//! let end = sketch.add_point(DVec2::new(1.0, 0.2));
//! sketch.fix(origin);
//! sketch.add_constraint(Constraint::Horizontal { a: origin, b: end });
//! sketch.add_constraint(Constraint::Distance {
//!     a: origin,
//!     b: end,
//!     along: Along::Shortest,
//!     dimension: Dimension::new(5.0),
//! });
//!
//! let mut outcome = Outcome::default();
//! Solver::default().solve(&mut sketch, &mut outcome);
//! assert!(outcome.converged());
//! assert_eq!(outcome.degrees_of_freedom(), 0);
//! assert!((sketch.point(end).position - DVec2::new(5.0, 0.0)).length() < 1e-9);
//! ```
//!
//! Solving is Levenberg-Marquardt over the analytic Jacobian of the constraint
//! residuals, on dense matrices — sketches are small, and the parameter count
//! is twice the point count plus one per circle. Everything is `f64`: the
//! residuals of a nearly-degenerate sketch do not survive `f32`.

// Doc links rot silently: nothing in a build reads them, so a renamed method
// leaves a dead link behind, and a `pub` item can come to point at a private one
// the rendered docs will not show. Denied rather than warned so that `cargo doc`
// refuses them — it is the only step that reads them at all, and it reads only
// what it documents, so `--document-private-items` is what puts the private half
// of the crate under them too.
//
// Here rather than in the manifest because cargo will not take a crate lint
// table beside an inherited one, and the two `[workspace.lints.rust]` entries
// are worth keeping shared.
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(rustdoc::redundant_explicit_links)]

pub(crate) mod arena;
pub(crate) mod loops;
pub(crate) mod math;
pub(crate) mod prism;
pub(crate) mod sketch;

pub use arena::Id;
pub use math::plane::Plane;
pub use math::triangulate::Fill;
pub use prism::Prism;
pub use prism::grown::Grown;
pub use prism::skinner::{Patch, Skinner};
pub use sketch::arrangement::Arrangement;
pub use sketch::arrangement::bound::Bound;
pub use sketch::arrangement::face::Face;
pub use sketch::arrangement::filler::Filler;
pub use sketch::constraint::{Along, Constraint, ConstraintId, Dimension};
pub use sketch::entity::Entity;
pub use sketch::measurement::{Frame, Measurement};
pub use sketch::snapshot::Snapshot;
/// The one call `tests/alloc.rs` makes. The driver itself stays in `src/`,
/// where it can reach what it measures.
#[cfg(feature = "bench")]
pub use sketch::solver::bench::alloc_bench;
pub use sketch::solver::freedom::Freedom;
pub use sketch::solver::outcome::Outcome;
pub use sketch::solver::{Drive, Solver};
pub use sketch::{Circle, CircleId, Point, PointId, Removed, Segment, SegmentId, Sketch};
