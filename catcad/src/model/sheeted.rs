//! A plane as the drawing lays one out.

use silverpoint::Plane;

use crate::timeline::FeatureId;
use crate::timeline::feature::World;

/// A plane as the drawing lays one out: which step it is, where it lies, and
/// which of the three the world comes with it is.
///
/// Its own type rather than three values, because a writer reads all of them
/// about one plane and a caller handed them apart is a caller free to draw one
/// plane in another's colours. The `world` is `None` for a datum somebody put
/// there, which has neither a hue of its own nor a name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sheeted {
    pub(crate) at: FeatureId,
    pub(crate) plane: Plane,
    pub(crate) world: Option<World>,
    /// Whether it has an offset to restate, which is what makes its square a
    /// *handle* rather than only a symbol — see
    /// [`Timeline::movable`](crate::timeline::Timeline::movable).
    pub(crate) movable: bool,
}
