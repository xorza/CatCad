//! Everything the drawing writes against the *camera* rather than against the
//! document: the controls it puts on screen, and the lines a dimension is drawn
//! with.
//!
//! **Measured in pixels**, which is what a control *is*: how big one is says
//! nothing about the model, and one that shrank with the zoom would stop being
//! grabbable exactly when you had zoomed out to find it. See
//! [`Camera::world_per_pixel`](aperture::Camera). A dimension's gaps, overshoots
//! and arrowheads are pixels for the neighbouring reason — they are read rather
//! than modelled — and a world length grown from a pixel one is a length only a
//! camera can give.
//!
//! So they are written on a different schedule from the drawing beside them. A
//! stroke of the drawing is rewritten when the *drawing* moves; these are built
//! against the camera, so they are rewritten whenever that moves — which during
//! an orbit is every frame. Gating the two together would mean re-cutting every
//! face and every solid on each of those frames, which is the whole cost
//! [`redraw`](crate::paint::redraw)'s own gate exists to avoid.
//!
//! **The schedule is what they have in common, and it is enough.** A control is
//! grabbed and a dimension line is read, so they share no colour, no width and
//! no standing; what they share is that neither can be written without a camera,
//! and one batch written twice is a batch rewritten twice.

use aperture::{Batch, Camera, Curve, Precedence, Viewport};
use glam::{DVec2, Vec3};
use silverpoint::{Constraint, Measurement, Plane};

use crate::model::Models;
use crate::paint::gizmos::dimension::Stroke;
use crate::paint::growing::Growing;
use crate::paint::layout::Layout;
use crate::paint::marks::Placed;
use crate::paint::{EDGE_WIDTH, GHOST, MARK, marks, rule_rise};
use crate::part::Part;

mod dimension;
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

/// Write every control the drawing wants — a datum's axes, and the arrow that
/// carries a solid still being decided — and the lines its dimensions are drawn
/// with.
///
/// A control is named as the plane itself, every piece of it. Several tags
/// reporting one [`Part`] is what [`Names`](crate::names::Names) is already
/// built to allow — a tag is a position in a list and nothing assumes the list
/// holds each part once — so grabbing any of them grabs the datum, and lighting
/// the datum lights them all.
///
/// **A dimension's lines are named nothing.** What a dimension offers a click is
/// its number, which is a label and outranks any stroke running under it — see
/// [`HitAt::rank`](aperture::HitAt). A line named as the dimension would rank as
/// an *edge*, tying with the real ones it is drawn among, so a dimension line
/// lying over a segment would take the click meant for the segment. Telling the
/// two apart wants a standing between `Shaped` and `Aside` that does not exist
/// yet, and a dimension already has a handle.
///
/// **The open sketch's dimensions alone**, matching the marks they belong to: a
/// number you cannot select or type into is a number that only crowds the sketch
/// you are working in — see [`write_marks`](crate::paint::write_marks).
pub(crate) fn write(
    models: Models<'_>,
    layout: &mut Layout,
    growing: Option<Growing>,
    proposed: Option<Constraint>,
    camera: &Camera,
    viewport: Viewport,
    into: &mut Batch<Curve>,
) {
    let Layout {
        names,
        sheets,
        placed,
        ..
    } = layout;
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
                .map(move |piece| (Some(Part::Plane(at)), piece))
            })
            .chain(carried.map(|carried| (Some(Part::Growing), Piece::Depth(carried))))
            .chain(ruled(models, placed, proposed, camera, viewport)),
        |curve, (part, piece)| {
            curve.width = piece.width();
            curve.closed = piece.closes();
            curve.points.clear();
            // Sized where it *stands*, not where the camera is looking: under
            // perspective a pixel covers more world the further off it is, so a
            // control on a distant plane built to the target's scale would come
            // out the wrong size.
            //
            // A dimension's strokes stand nowhere, because they were sized where
            // they stand before they were ever a [`Piece`] — see [`ruled`],
            // which has to know the scale to work out what shape they are at
            // all. Whatever this then answers for one is never read.
            let scale = piece
                .stands_at()
                .map_or(1.0, |at| f64::from(camera.world_per_pixel(at, viewport)));
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
                Piece::Ruled { plane, stroke, .. } => curve
                    .points
                    .extend(stroke.corners().map(|at| plane.point(at).as_vec3())),
            }
            curve.color = piece.ink();
            // A control lies in a plane and is widened in screen space, so it
            // takes that plane's depth rather than its anchor's — the same
            // thing every stroke of the drawing does.
            curve.plane_normal = piece.lies_in().map(|plane| plane.normal().as_vec3());
            curve.precedence = piece.ranks();
            curve.tag = part.map(|part| names.tag(part));
        },
    );
}

/// Every stroke the open sketch's dimensions are drawn from, already sized
/// against the camera.
///
/// Sized here rather than in the closure below, and that is what makes it its
/// own walk: what shape a dimension's strokes *are* depends on the scale — a gap
/// too long for the geometry leaves no extension line, and a head is a triangle
/// only once its reach is a world length — so the scale is spent before there is
/// a [`Piece`] to hold the answer.
///
/// **Off the marks rather than off the constraints**, though both are there to
/// be walked. Where a dimension's number went is not a property of the relation:
/// it is settled a mark at a time and then *stacked*, so two dimensions wanting
/// one place are moved apart — and a line worked out from the constraint alone
/// would stay behind while its own number rose. Reading the list the marks were
/// laid out from is what keeps the figure sitting on the line that carries it.
///
/// The list is [`redraw`](crate::paint::redraw)'s, and current whenever this
/// runs: it is rewritten whenever the drawing moves, and skipped only on the
/// frames where the drawing has not.
fn ruled<'a>(
    models: Models<'a>,
    placed: &'a [Placed],
    proposed: Option<Constraint>,
    camera: &'a Camera,
    viewport: Viewport,
) -> impl Iterator<Item = (Option<Part>, Piece)> + 'a {
    models
        .iter()
        .find(|model| model.live())
        .into_iter()
        .flat_map(move |model| {
            let (sketch, plane) = (model.sketch(), model.plane());
            // The dimension a tool is half-way through placing, which is drawn
            // exactly as a stated one and stands in no stack — see
            // [`marks::previewed`]. Its own mark rather than a `Placed`, because
            // the sketch does not hold it and there is no handle to name it by.
            let previewed = proposed
                .and_then(|constraint| Some((constraint, marks::previewed(sketch, constraint)?)));
            placed
                .iter()
                .map(|placed| (sketch.constraint(placed.of), placed.mark, false))
                .chain(
                    previewed.map(|(constraint, standing)| (constraint, standing.grounded(), true)),
                )
                .filter_map(move |(constraint, placed, proposed)| {
                    let measured = Measurement::of(sketch, constraint)?;
                    // Where the *number* stands, which is what an extension line
                    // is reaching to and so what the gaps at either end of one
                    // dimension have to be sized against.
                    let at = plane.point(placed.at).as_vec3();
                    let scale = f64::from(camera.world_per_pixel(at, viewport));
                    let strokes = dimension::strokes(
                        Measurement {
                            // The mark's own direction and height, so the line
                            // lands under the figure it carries: the first is
                            // settled either side of a cut and the second is
                            // where the stack left it, and neither is anything
                            // the measurement knows.
                            along: placed.along,
                            label: placed.at
                                + placed.along.perp() * (f64::from(rule_rise(placed.lane)) * scale),
                            ..measured
                        },
                        // A radius points at its rim and not at its own centre —
                        // see [`dimension::strokes`].
                        !matches!(constraint, Constraint::Radius { .. }),
                        scale,
                    );
                    Some((strokes, proposed))
                })
                .flat_map(move |(strokes, proposed)| {
                    strokes.into_iter().flatten().map(move |stroke| {
                        (
                            None,
                            Piece::Ruled {
                                plane,
                                stroke,
                                proposed,
                            },
                        )
                    })
                })
        })
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
    /// One stroke of a dimension: an extension line, the dimension line itself,
    /// or an arrowhead. Already sized — see [`ruled`].
    Ruled {
        plane: Plane,
        stroke: Stroke,
        /// Whether it belongs to the dimension a tool is half-way through
        /// placing rather than to one the drawing states.
        proposed: bool,
    },
}

impl Piece {
    /// The plane it lies in, where it lies in one.
    fn lies_in(self) -> Option<Plane> {
        match self {
            Piece::Axis(plane, ..)
            | Piece::Hub(plane)
            | Piece::Corner(plane)
            | Piece::Ruled { plane, .. } => Some(plane),
            Piece::Depth(_) => None,
        }
    }

    /// Where in the world it stands, which is where its size in pixels is
    /// measured — and `None` where it is already sized.
    ///
    /// A dimension's strokes are the `None`. They carry world lengths *and*
    /// pixel ones — an extension line spans the geometry and starts a gap off
    /// it — so what shape they are cannot be known without a scale, and it is
    /// spent before there is a piece to hold the answer. See [`ruled`].
    ///
    /// An `Option` rather than a place nobody reads, because the two are not the
    /// same claim: a plane's origin is a real answer to the wrong question, and
    /// this is the question not applying.
    fn stands_at(self) -> Option<Vec3> {
        match self {
            Piece::Axis(plane, ..) | Piece::Hub(plane) | Piece::Corner(plane) => {
                Some(plane.origin.as_vec3())
            }
            Piece::Depth(carried) => Some(carried.tail),
            Piece::Ruled { .. } => None,
        }
    }

    /// Whether the last corner joins back to the first.
    ///
    /// Every control is a filled outline, and a dimension is filled at its heads
    /// and open along its lines.
    fn closes(self) -> bool {
        match self {
            Piece::Axis(..) | Piece::Hub(_) | Piece::Corner(_) | Piece::Depth(_) => true,
            Piece::Ruled { stroke, .. } => stroke.closes(),
        }
    }

    /// How wide it is stroked, in logical pixels.
    ///
    /// A dimension takes the drawing's own width rather than a control's,
    /// because it *is* drawing: what a heavier stroke says is "take hold of
    /// me", and a dimension line is read.
    fn width(self) -> f32 {
        match self {
            Piece::Axis(..) | Piece::Hub(_) | Piece::Corner(_) | Piece::Depth(_) => GIZMO_WIDTH,
            Piece::Ruled { .. } => EDGE_WIDTH,
        }
    }

    /// What it is stroked in.
    fn ink(self) -> Vec3 {
        match self {
            Piece::Axis(_, _, ink) => ink,
            Piece::Hub(_) | Piece::Corner(_) => AXIS_CORNER,
            Piece::Depth(_) => DEPTH_ARROW,
            // The number's own colour: the lines are that number's, and a hue
            // of their own would read as a third kind of thing on the drawing.
            // Not the redundant red a spare relation wears — a dimension the
            // constraints could do without says so in its figure, and saying it
            // twice would double the ink on the one case that is already loud.
            Piece::Ruled {
                proposed: false, ..
            } => MARK,
            // A proposal has no state to report — the constraints have not been
            // asked about it — so it wears the grey a rubber band does, and for
            // the reason that one does: it is not in the drawing yet.
            Piece::Ruled { proposed: true, .. } => GHOST,
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
            // A dimension's lines rank with the datum for the same reason, and
            // it decides nothing anyway: they carry no tag, so no pick ever
            // reaches them. Stated all the same, because a stroke that later
            // earned a name would otherwise inherit whatever this happened to
            // say.
            Piece::Axis(..) | Piece::Hub(_) | Piece::Corner(_) | Piece::Ruled { .. } => {
                Precedence::Frame
            }
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
