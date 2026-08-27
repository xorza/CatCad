//! What a drawing and the solids beside it are painted in.

use glam::Vec3;
use palantir::Color;
use silverpoint::Freedom;

use crate::look::palette::Palette;
use crate::timeline::feature::World;

/// Everything that decides how the geometry itself looks.
///
/// **Linear-RGB triples, not [`Color`]**, because that is what aperture shades
/// and strokes with, and almost all of this is drawn by aperture. What palantir
/// wants of it — the handful the corner *reports* in, and the ground the window
/// is cleared to — comes through [`tint`], a reinterpretation rather than a
/// conversion, since both sides hold linear RGB.
///
/// Every field is named for the *role* it plays and never for the colour or the
/// number it happens to hold, so a second preset is a second table rather than a
/// rethink.
#[derive(Debug, Clone)]
pub(crate) struct Drawing {
    /// What everything is drawn against.
    ///
    /// **Here rather than with the chrome**, though the window is cleared to it
    /// too. Only a sliver of the window is ever seen — the viewport fills the
    /// rest — so the two have to agree, and one of them stating it while the
    /// other derives it is the only arrangement in which they cannot drift. The
    /// scene is the half that decides, because this is what a drawing is read
    /// against.
    pub(crate) ground: Vec3,

    /// What a solid the document has grown is shaded in.
    ///
    /// Plain grey, and the one colour here that is not about state. Everything a
    /// *drawing* is painted in says how much freedom the constraints have left
    /// it; a solid has no freedom to report — it is what a feature made, and
    /// either it is there or its profile was lost — so it reads as material
    /// rather than as a thing with something left to decide.
    pub(crate) solid: Vec3,

    /// Geometry is coloured by how much freedom its constraints have left it,
    /// cool for none and warm for all of it, so a sketch starts hot and cools as
    /// it is pinned down — which is the convention every constrained modeller
    /// draws on, and reads at a glance as how much work the drawing still needs.
    ///
    /// A point the user pinned by hand keeps its own colour regardless. It is
    /// determined, but by a different authority, and the two are worth telling
    /// apart: constraints can be argued with by adding more, and `fix` cannot.
    pub(crate) determined: Vec3,
    pub(crate) partly: Vec3,
    pub(crate) free: Vec3,
    pub(crate) pinned: Vec3,

    /// What a sketch that is not the one open is drawn in.
    ///
    /// One colour rather than the freedom ladder above. How much a sketch you
    /// are not in has left to decide is not something you can act on without
    /// opening it first, so saying it would be saying something unusable — and a
    /// second ladder in the same picture reads as a second *kind* of geometry
    /// rather than as the same kind, set aside.
    ///
    /// Dimmer than [`Drawing::ghost`], which is the other thing here drawn in no
    /// state at all: a rubber band is what you are doing now, and this is what
    /// you are not.
    pub(crate) dormant: Vec3,
    /// What a face of one is filled with — the same step down from
    /// [`Drawing::face`].
    pub(crate) dormant_face: Vec3,

    /// What a face the drawing encloses is filled with.
    ///
    /// Cool and dim, and deliberately not on the ladder above: a face reports no
    /// freedom of its own — it is whatever its boundary shuts in, and the
    /// boundary is already painted in what it has left to decide. So it reads as
    /// ground for the drawing to sit on rather than as another thing with a
    /// state.
    ///
    /// Stated at the strength it has to survive being *seen through*, which is
    /// what makes it look so much bluer here than it does on screen: a region is
    /// drawn translucent, so what lands is a fraction of this mixed into
    /// whatever it covers.
    ///
    /// How much of it lands is aperture's `FACE_OPACITY`, not this file's. Lower
    /// that and this wants restating, because the two are one decision about how
    /// a region reads and only look like two.
    pub(crate) face: Vec3,

    /// What a shape still being drawn is drawn in — a grey that belongs to none
    /// of the states above, because a rubber band has no freedom to report: it
    /// is not geometry yet, and the constraints have not been asked about it.
    pub(crate) ghost: Vec3,

    /// What a plane's outline is drawn in, by which of the three the world comes
    /// with it is — and what one somebody put there gets instead.
    ///
    /// **Hued by the axis its normal runs along**: the ground faces +Y, the
    /// front +Z and the side +X, so green, blue and red. The convention every
    /// gizmo cube uses, which is worth having because it is the one thing about
    /// a plane a user already knows. A plane measured off another has no world
    /// axis to claim, so it takes a neutral — which also tells a plane the world
    /// gave you from one you made, without a label doing it.
    ///
    /// Cool and low, all of them, so a square standing at the origin reads as
    /// chrome rather than as something drawn there. They collide with the
    /// freedom ladder — [`Drawing::pinned`] is a red and [`Drawing::free`] is
    /// close to it — and what keeps them apart is shape and weight: these are
    /// hairline squares where a pinned point is a small saturated disc.
    pub(crate) sheet_ground: Vec3,
    pub(crate) sheet_front: Vec3,
    pub(crate) sheet_side: Vec3,
    pub(crate) sheet_datum: Vec3,

    /// What a mark is drawn in.
    ///
    /// Grey-violet, which is the one hue the drawing does not already spend:
    /// geometry runs blue through yellow to orange for how much freedom is left,
    /// red for pinned, and green for what is picked out. A mark is *about* the
    /// geometry rather than part of it, and reads as a different kind of thing
    /// for being a different kind of colour.
    pub(crate) mark: Vec3,
    /// What a mark the constraints could do without is drawn in.
    ///
    /// The one thing a drawing can say that a count in the corner cannot: *this*
    /// relation is the spare one. Red, because it is the same news as a conflict
    /// — and on a sketch whose constraints disagree, it is exactly the mark to
    /// delete.
    pub(crate) redundant: Vec3,

    /// What the arrow carrying a solid's depth is drawn in.
    ///
    /// A control rather than geometry, and grey for it: what it is about is the
    /// solid, so it takes the solid's own family rather than a state colour that
    /// would read as a claim about the drawing.
    pub(crate) depth_arrow: Vec3,

    /// How wide a sketch stroke is drawn, in logical pixels.
    ///
    /// Not aperture's own default, which is narrower: a drawing is read at a
    /// glance against a shaded model behind it and wants a little more weight
    /// than a bare overlay does. Every stroke and every rim is set to this, so
    /// the default is never seen — and the visual suite measures against it,
    /// rather than keeping a second opinion about what it should be.
    pub(crate) edge: f32,
    /// How wide a plane's outline is.
    ///
    /// Under [`Drawing::edge`], so a plane's own edge cannot be taken for
    /// something drawn on it.
    pub(crate) sheet: f32,
    /// How wide a control is.
    ///
    /// A shade heavier than the drawing's own, so a handle reads as something to
    /// take hold of rather than as something drawn.
    pub(crate) gizmo: f32,

    /// Marker diameters. A pinned point reads larger because it is the one the
    /// drawing hangs off.
    pub(crate) fixed_marker: f32,
    pub(crate) free_marker: f32,
}

impl Drawing {
    /// What geometry with this much freedom left is drawn in.
    pub(crate) fn freedom(&self, freedom: Freedom) -> Vec3 {
        match freedom {
            Freedom::Determined => self.determined,
            Freedom::Partly => self.partly,
            Freedom::Free => self.free,
        }
    }

    /// What a plane's outline is drawn in.
    pub(crate) fn sheet_ink(&self, world: Option<World>) -> Vec3 {
        match world {
            Some(World::Ground) => self.sheet_ground,
            Some(World::Front) => self.sheet_front,
            Some(World::Side) => self.sheet_side,
            None => self.sheet_datum,
        }
    }

    /// The drawing this palette paints, at the weights it is drawn with.
    ///
    /// **Colour from the table, weight from here**, by the rule
    /// [`Chrome::from_palette`](crate::look::chrome::Chrome::from_palette)
    /// keeps: how wide a stroke is answers how a drawing reads against a shaded
    /// model, which no palette knows about.
    pub(super) fn from_palette(palette: &Palette) -> Self {
        Self {
            ground: palette.ground.ink(),
            solid: palette.solid.ink(),
            determined: palette.determined.ink(),
            partly: palette.partly.ink(),
            free: palette.free.ink(),
            pinned: palette.pinned.ink(),
            dormant: palette.dormant.ink(),
            dormant_face: palette.dormant_face.ink(),
            face: palette.face.ink(),
            ghost: palette.ghost.ink(),
            sheet_ground: palette.sheet_ground.ink(),
            sheet_front: palette.sheet_front.ink(),
            sheet_side: palette.sheet_side.ink(),
            sheet_datum: palette.sheet_datum.ink(),
            mark: palette.mark.ink(),
            redundant: palette.redundant.ink(),
            depth_arrow: palette.depth_arrow.ink(),
            edge: 1.6,
            sheet: 1.0,
            gizmo: 2.0,
            fixed_marker: 9.0,
            free_marker: 7.0,
        }
    }
}

/// One of the drawing's colours, as palantir takes it.
///
/// A reinterpretation rather than a conversion: [`Color`] holds straight-alpha
/// linear RGB, which is what every triple above already is. Writing one through
/// [`Color::rgb`] instead would linearise a number that is linear and come out
/// dark.
///
/// The overlay's own colours never come through here — it states those as
/// [`Color`] outright. This is for the handful the corner *reports* in, so the
/// readout's amber is the drawing's amber and not a second one chosen to match.
pub(crate) const fn tint(shade: Vec3) -> Color {
    Color::linear_rgb(shade.x, shade.y, shade.z)
}
