//! The controls a drawing puts on screen, and the shapes they are cut from.
//!
//! **Measured in pixels**, which is what a control *is*: how big one is says
//! nothing about the model, and one that shrank with the zoom would stop being
//! grabbable exactly when you had zoomed out to find it. See
//! [`Camera::world_per_pixel`](aperture::Camera).
//!
//! So they are written on a different schedule from the drawing beside them. A
//! stroke of the drawing is rewritten when the *drawing* moves; a control is
//! built against the camera, so it is rewritten whenever that moves — which
//! during an orbit is every frame. Gating the two together would mean re-cutting
//! every face and every solid on each of those frames, which is the whole cost
//! [`redraw`](crate::paint::redraw)'s own gate exists to avoid.

use aperture::{Batch, Camera, Curve, Precedence, Viewport};
use glam::{DVec2, Vec3};
use silverpoint::Plane;

use crate::model::Models;
use crate::paint::growing::Growing;
use crate::paint::layout::Layout;
use crate::part::Part;

mod shape;

/// What a datum's two axis arrows are drawn in.
///
/// The convention every modeller shares — x red, y green — which is worth
/// having because it is the one thing about a gizmo a user already knows.
///
/// It collides, and knowingly: [`PINNED`](crate::paint::PINNED) is a red and
/// [`FREE`](crate::paint::FREE) is close to it, so a red arrow and a pinned
/// point are two things saying different things in one hue. What keeps them
/// apart for now is that they are never the same
/// *shape* — an axis is a great flat arrow and a pinned point is a small disc —
/// and both of these are muted well below the drawing's own, so they read as
/// chrome rather than as state. A palette is where this gets settled properly.
const AXIS_X: Vec3 = Vec3::new(0.62, 0.20, 0.18);
const AXIS_Y: Vec3 = Vec3::new(0.24, 0.52, 0.24);

/// What the arrow carrying a solid's depth is drawn in.
///
/// The same warm grey the solid itself is, because it is *that solid's* handle
/// and nothing else in the drawing — a hue off the freedom ladder would say it
/// had a state, and the axis colours would say it was an axis.
const DEPTH_ARROW: Vec3 = Vec3::new(0.78, 0.76, 0.70);

/// What the square joining the two is drawn in.
///
/// Neither hue, because it belongs to neither axis — it is the corner they make
/// rather than a third direction, and giving it one of theirs would say it was.
const AXIS_CORNER: Vec3 = Vec3::new(0.50, 0.52, 0.55);

/// Write every control the drawing wants: a datum's axes, and the arrow that
/// carries a solid still being decided.
///
/// Named as the plane itself, every piece of it. Several tags reporting one
/// [`Part`] is what [`Names`](crate::names::Names) is already built to allow —
/// a tag is a position in a list and nothing assumes the list holds each part
/// once — so grabbing any of them grabs the datum, and lighting the datum
/// lights them all.
pub(crate) fn write(
    models: Models<'_>,
    layout: &mut Layout,
    growing: Option<Growing>,
    camera: &Camera,
    viewport: Viewport,
    into: &mut Batch<Curve>,
) {
    let Layout { names, sheets, .. } = layout;
    // Back to what the drawing named, and no further. These are appended after
    // it and rewritten far more often, so without this the list would grow by a
    // gizmo's worth every frame the camera moved.
    names.truncate_to_drawn();
    let carried = growing.and_then(|growing| growing.carried(models, sheets, camera));
    into.refill(
        models
            .planes()
            .flat_map(|(at, plane)| {
                [
                    Piece::Axis(plane, DVec2::X, AXIS_X),
                    Piece::Axis(plane, DVec2::Y, AXIS_Y),
                    Piece::Hub(plane),
                    Piece::Corner(plane),
                ]
                .map(move |piece| (Part::Plane(at), piece))
            })
            .chain(carried.map(|carried| (Part::Growing, Piece::Depth(carried)))),
        |curve, (part, piece)| {
            curve.width = GIZMO_WIDTH;
            curve.closed = true;
            curve.points.clear();
            // Sized where it *stands*, not where the camera is looking: under
            // perspective a pixel covers more world the further off it is, so a
            // control on a distant plane built to the target's scale would come
            // out the wrong size.
            let scale = f64::from(camera.world_per_pixel(piece.stands_at(), viewport));
            match piece {
                Piece::Axis(plane, along, _) => curve
                    .points
                    .extend(shape::arrow(along).map(|at| plane.point(at * scale).as_vec3())),
                Piece::Hub(plane) => curve
                    .points
                    .extend(shape::hub().map(|at| plane.point(at * scale).as_vec3())),
                Piece::Corner(plane) => curve
                    .points
                    .extend(shape::corner().map(|at| plane.point(at * scale).as_vec3())),
                Piece::Depth(carried) => curve
                    .points
                    .extend(shape::arrow(DVec2::X).map(|at| carried.at(at, scale))),
            }
            curve.color = piece.ink();
            // A control lies in a plane and is widened in screen space, so it
            // takes that plane's depth rather than its anchor's — the same
            // thing every stroke of the drawing does.
            curve.plane_normal = piece.lies_in().map(|plane| plane.normal().as_vec3());
            curve.precedence = piece.ranks();
            curve.tag = Some(names.tag(part));
        },
    );
}

/// How wide a control is stroked, in logical pixels.
///
/// A shade heavier than the drawing's own, so a handle reads as something to
/// take hold of rather than as another edge among the edges it stands on.
const GIZMO_WIDTH: f32 = 2.0;

/// One stroke a gizmo is made of.
#[derive(Debug, Clone, Copy)]
enum Piece {
    /// An arrow along one of a plane's axes, in that axis's colour.
    Axis(Plane, DVec2, Vec3),
    /// The block the two axes cross at.
    Hub(Plane),
    /// The square in the quadrant the two axes shut in.
    Corner(Plane),
    /// The arrow carrying a solid's depth, which stands out of its plane rather
    /// than lying in one.
    Depth(Carried),
}

impl Piece {
    /// The plane it lies in, where it lies in one.
    fn lies_in(self) -> Option<Plane> {
        match self {
            Piece::Axis(plane, ..) | Piece::Hub(plane) | Piece::Corner(plane) => Some(plane),
            Piece::Depth(_) => None,
        }
    }

    /// Where in the world it stands, which is where its size in pixels is
    /// measured.
    fn stands_at(self) -> Vec3 {
        match self {
            Piece::Axis(plane, ..) | Piece::Hub(plane) | Piece::Corner(plane) => {
                plane.origin.as_vec3()
            }
            Piece::Depth(carried) => carried.tail,
        }
    }

    /// What it is stroked in.
    fn ink(self) -> Vec3 {
        match self {
            Piece::Axis(_, _, ink) => ink,
            Piece::Hub(_) | Piece::Corner(_) => AXIS_CORNER,
            Piece::Depth(_) => DEPTH_ARROW,
        }
    }

    /// How hard it competes for a click that lands on several things at once.
    ///
    /// A datum stands as a frame: it is what the drawing is done *on*, so it
    /// yields to anything drawn on it. The depth arrow does not, and the
    /// difference is that one of them is what the gesture is *for* — a form is
    /// open, the arrow is the thing being dragged, and it has to take the click
    /// over the geometry it stands over. Ranking it as a frame also enters it
    /// among the occluders, so it would go on to hide what is behind it from a
    /// pick as well as losing to it.
    fn ranks(self) -> Precedence {
        match self {
            Piece::Axis(..) | Piece::Hub(_) | Piece::Corner(_) => Precedence::Frame,
            Piece::Depth(_) => Precedence::Shaped,
        }
    }
}

/// Where a depth arrow stands and which way it points, as the frame its outline
/// is laid out in.
///
/// `x` runs along the plane's normal, which is the direction the depth grows
/// in, and `y` across it. So the arrow stands *out* of the plane rather than
/// lying in it, which is what a handle carrying something off a face has to do.
///
/// Only `along` is the model's; `across` is the camera's, turned so the flat
/// outline faces the viewer. So this is rebuilt whenever the camera moves, like
/// everything else [`write()`] writes.
#[derive(Debug, Clone, Copy)]
pub(super) struct Carried {
    tail: Vec3,
    along: Vec3,
    across: Vec3,
}

impl Carried {
    /// Where the arrow stands, the way its depth grows, and the way it is
    /// widest.
    pub(super) fn new(tail: Vec3, along: Vec3, across: Vec3) -> Self {
        Self {
            tail,
            along,
            across,
        }
    }

    /// A corner of the outline, put in the world at `scale` world units per
    /// pixel.
    fn at(self, corner: DVec2, scale: f64) -> Vec3 {
        let at = corner * scale;
        self.tail + self.along * at.x as f32 + self.across * at.y as f32
    }
}

#[cfg(test)]
mod tests;
