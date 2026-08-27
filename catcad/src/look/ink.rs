//! What a drawing and the solids beside it are painted in.
//!
//! **The drawing's half only.** What the *overlay* is drawn in is
//! [`Chrome`](crate::look::chrome::Chrome), because the two are handed to
//! different renderers: aperture shades with a [`Vec3`] and palantir fills with
//! a [`Color`]. Both are linear RGB, so the four colours that cross between them
//! cross through [`tint`], which reinterprets rather than converts.
//!
//! That the two agree is the point. A sketch the solver has left half free is
//! drawn in [`PARTLY`], and the readout says so in the same amber, because it is
//! the same colour and not a second one chosen to look like it.

use glam::Vec3;
use palantir::Color;
use silverpoint::Freedom;

use crate::timeline::feature::World;

/// The same colour, as palantir takes it.
///
/// A reinterpretation rather than a conversion: [`Color`] holds straight-alpha
/// linear RGB, which is what every triple here already is. Writing one of these
/// through [`Color::rgb`] instead would linearise a number that is linear and
/// come out dark.
pub(crate) const fn tint(shade: Vec3) -> Color {
    Color::linear_rgb(shade.x, shade.y, shade.z)
}

/// What a solid the document has grown is shaded in.
///
/// Warm grey, and the one colour here that is not about state. Everything a
/// *drawing* is painted in says how much freedom the constraints have left it;
/// a solid has no freedom to report — it is what a feature made, and either it
/// is there or its profile was lost — so it reads as material rather than as a
/// thing with something left to decide.
pub(crate) const SOLID: Vec3 = Vec3::new(0.62, 0.60, 0.56);

/// Geometry is coloured by how much freedom its constraints have left it, cool
/// for none and warm for all of it, so a sketch starts hot and cools as it is
/// pinned down — which is the convention every constrained modeller draws on,
/// and reads at a glance as how much work the drawing still needs.
///
/// A point the user pinned by hand keeps its own colour regardless. It is
/// determined, but by a different authority, and the two are worth telling
/// apart: constraints can be argued with by adding more, and `fix` cannot.
pub(crate) const DETERMINED: Vec3 = Vec3::new(0.35, 0.55, 0.80);
pub(crate) const PARTLY: Vec3 = Vec3::new(0.85, 0.74, 0.20);
pub(crate) const FREE: Vec3 = Vec3::new(0.88, 0.50, 0.10);
pub(crate) const PINNED: Vec3 = Vec3::new(0.80, 0.14, 0.05);

/// What geometry with this much freedom left is drawn in.
pub(crate) fn freedom(freedom: Freedom) -> Vec3 {
    match freedom {
        Freedom::Determined => DETERMINED,
        Freedom::Partly => PARTLY,
        Freedom::Free => FREE,
    }
}

/// What a sketch that is not the one open is drawn in.
///
/// One colour rather than the freedom ladder above. How much a sketch you are
/// not in has left to decide is not something you can act on without opening it
/// first, so saying it would be saying something unusable — and a second ladder
/// in the same picture reads as a second *kind* of geometry rather than as the
/// same kind, set aside.
///
/// Dimmer than [`GHOST`], which is the other thing here drawn in no state at
/// all: a rubber band is what you are doing now, and this is what you are not.
pub(crate) const DORMANT: Vec3 = Vec3::new(0.42, 0.45, 0.50);

/// What a face of one is filled with — the same step down from [`FACE`].
pub(crate) const DORMANT_FACE: Vec3 = Vec3::new(0.11, 0.20, 0.29);

/// What a face the drawing encloses is filled with.
///
/// Cool and dim, and deliberately not on the ladder above: a face reports no
/// freedom of its own — it is whatever its boundary shuts in, and the boundary
/// is already painted in what it has left to decide. So it reads as ground for
/// the drawing to sit on rather than as another thing with a state.
///
/// Stated at the strength it has to survive being *seen through*, which is what
/// makes it look so much bluer here than it does on screen: a region is drawn
/// translucent, so what lands is a fraction of this mixed into whatever it
/// covers — the bare background, or a solid standing behind it.
///
/// How much of it lands is `FACE_OPACITY`'s, not this file's. Lower that and
/// this wants restating, because the two are one decision about how a region
/// reads and only look like two.
pub(crate) const FACE: Vec3 = Vec3::new(0.18, 0.32, 0.46);

/// What a shape still being drawn is drawn in — a grey that belongs to none of
/// the states above, because a rubber band has no freedom to report: it is not
/// geometry yet, and the constraints have not been asked about it.
pub(crate) const GHOST: Vec3 = Vec3::new(0.72, 0.74, 0.78);

/// What a plane's outline is drawn in, by which of the three the world comes
/// with it is — and what one somebody put there gets instead.
///
/// **Hued by the axis its normal runs along**: the ground faces +Y, the front
/// +Z and the side +X, so green, blue and red. The convention every gizmo cube
/// uses, which is worth having because it is the one thing about a plane a user
/// already knows. A plane measured off another has no world axis to claim, so it
/// takes a neutral — which also tells a plane the world gave you from one you
/// made, without a label doing it.
///
/// Cool and low, all of them, so a square standing at the origin reads as chrome
/// rather than as something drawn there. They collide with the freedom ladder —
/// [`PINNED`] is a red and [`FREE`] is close to it — and what keeps them apart
/// is shape and weight: these are hairline squares where a pinned point is a
/// small saturated disc.
pub(crate) const SHEET_GROUND: Vec3 = Vec3::new(0.30, 0.46, 0.32);
pub(crate) const SHEET_FRONT: Vec3 = Vec3::new(0.30, 0.40, 0.56);
pub(crate) const SHEET_SIDE: Vec3 = Vec3::new(0.52, 0.34, 0.32);
pub(crate) const SHEET_DATUM: Vec3 = Vec3::new(0.42, 0.46, 0.54);

/// What a plane's outline is drawn in.
pub(crate) fn sheet(world: Option<World>) -> Vec3 {
    match world {
        Some(World::Ground) => SHEET_GROUND,
        Some(World::Front) => SHEET_FRONT,
        Some(World::Side) => SHEET_SIDE,
        None => SHEET_DATUM,
    }
}

/// What a mark is drawn in.
///
/// Grey-violet, which is the one hue the drawing does not already spend:
/// geometry runs blue through yellow to orange for how much freedom is left,
/// red for pinned, and green for what is picked out. A mark is *about* the
/// geometry rather than part of it, and reads as a different kind of thing for
/// being a different kind of colour.
pub(crate) const MARK: Vec3 = Vec3::new(0.62, 0.58, 0.78);

/// What a mark the constraints could do without is drawn in.
///
/// The one thing a drawing can say that a count in the corner cannot: *this*
/// relation is the spare one. Red, because it is the same news as a conflict —
/// and on a sketch whose constraints disagree, it is exactly the mark to delete.
pub(crate) const REDUNDANT: Vec3 = Vec3::new(0.90, 0.30, 0.25);

/// What the thing under the cursor is lit in.
pub(crate) const HOVERED: Vec3 = Vec3::new(1.0, 0.85, 0.25);

/// What something picked out is lit in.
///
/// Green, which is the one hue the drawing does not already use: its own
/// colours run blue through yellow to orange for how much freedom is left, and
/// red for pinned, and a selection that reused any of them would be saying two
/// things in one colour.
pub(crate) const SELECTED: Vec3 = Vec3::new(0.30, 0.95, 0.45);
