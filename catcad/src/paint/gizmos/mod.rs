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
//! no standing; what they share is that neither can be written without a
//! [`Lens`], and one batch written twice is a batch rewritten twice.

use aperture::{Batch, Curve, Precedence};
use glam::{DVec2, Vec3};
use silverpoint::{Constraint, Measurement, Plane};

use crate::lens::Lens;
use crate::look::Theme;
use crate::model::models::Models;
use crate::notation::Notation;
use crate::paint::gizmos::dimension::Stroke;
use crate::paint::layout::{Framed, Layout, Made};
use crate::paint::marks::{Placed, Proposed};
use crate::paint::showing::Showing;

use crate::part::Part;

mod dimension;
mod shape;

/// Write every control the drawing wants — the square standing for each plane,
/// and the arrow that carries a solid still being decided — and the lines its
/// dimensions are drawn with.
///
/// A plane's square is named as the plane itself, which makes it the whole of
/// what there is to point at: the outline used to be written with the drawing
/// and world-measured, and a datum's handles were arrows laid beside it. Both
/// are gone — one square says where a plane is, and grabbing it grabs the plane.
///
/// **Not every plane.** Which ones show a square is a question about what you
/// are working on rather than about what the document holds — see
/// [`Piece::Sheet`].
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
/// you are working in — see [`texts`](crate::paint::write::texts).
pub(crate) fn write(
    models: Models<'_>,
    notation: Notation,
    theme: &Theme,
    layout: &mut Layout,
    showing: Showing,
    lens: Lens,
    into: &mut Batch<Curve>,
) {
    // The same shape the drawing's own gate has, and here for the same reason:
    // what the controls claim to describe and what was written into them are
    // decided in one place. Without it these ran on every frame there was —
    // every axis, hub, corner and dimension rule rebuilt while the view sat
    // still, which on a sketch of two hundred dimensions was the entire cost of
    // a frame that had nothing to draw.
    let framed = Framed {
        made: Made::of(models, showing, layout.chorded(Some(lens)), notation).kept(),
        lens,
    };
    if !layout.recontrol(framed) {
        return;
    }
    let Layout {
        names,
        sheets,
        placed,
        proposed,
        cut,
        sweep,
        ..
    } = layout;
    // Back to what the drawing named, and no further. These are appended after
    // it and rewritten far more often, so without this the list would grow by a
    // gizmo's worth every frame the camera moved.
    names.truncate_to_drawn();
    let open = models.open_plane();
    // The region is cut only where the last cut was of another one — see
    // [`Cut`](crate::paint::cut::Cut), which is what keeps the filler off the
    // camera's schedule.
    // Two handles and one cut, because a form decides one number or the other:
    // an extrude has a depth and a revolve has a turn, and neither has both.
    //
    // Emptied first because the two ways there is no circle are early returns
    // below — no form open, and a form whose region has gone — and a circle left
    // standing is one a form goes on clearing after the thing that cast it has
    // gone.
    sweep.clear();
    let handled = showing.growing.and_then(|growing| {
        // The first region, a handle standing on one face — see
        // [`Profile::first_face_of`](crate::profile::Profile).
        let region = growing.profile.first_face_of(models)?;
        let cut = cut.region(models, sheets, growing.profile.sketch(), region)?;
        // Everywhere a turn handle could go, for the form that has to stand
        // clear of it — see [`Layout::sweep`]. Written here rather than read
        // back off the scene, because this is where the region's own distance
        // from the axle is in hand.
        growing.sweeps(models, cut, sweep);
        let carried = growing
            .carried(models, cut, lens)
            .map(|carried| (Part::Growing, carried));
        carried.or_else(|| {
            growing
                .turned(models, cut, lens)
                .map(|turned| (Part::Turning, turned))
        })
    });
    into.refill(
        // Which planes show a square — see [`Piece::Sheet`].
        models
            .planes()
            .filter(move |sheeted| sheeted.movable || open.is_none_or(|on| on == sheeted.at))
            .map(move |sheeted| {
                (
                    Some(Part::Step(sheeted.at)),
                    Piece::Sheet(sheeted.plane, theme.geometry.sheet_ink(sheeted.world)),
                )
            })
            .chain(handled.map(|(part, handle)| (Some(part), Piece::Handle(handle))))
            .chain(ruled(models, placed, *proposed, lens)),
        |curve, (part, piece)| {
            piece.stroke(curve, theme, lens);
            // The one thing left to the caller, because it is the only one that
            // is not the piece's: a name is minted out of the list this walk is
            // appending to.
            curve.tag = part.map(|part| names.tag(part));
        },
    );
    layout.controlled(framed);
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
///
/// Nothing at all where no sketch is open, which is one of the two ways this
/// answers with nothing and the less interesting: a document being looked at
/// rather than drawn in shows no dimensions, so there are no rules to draw
/// under them. Walked through the `Option` rather than returned early, because
/// an early return would have to name a second iterator type for the empty
/// case.
fn ruled<'a>(
    models: Models<'a>,
    placed: &'a [Placed],
    proposed: Option<Proposed>,
    lens: Lens,
) -> impl Iterator<Item = (Option<Part>, Piece)> + 'a {
    models.open().into_iter().flat_map(move |model| {
        let (sketch, plane) = (model.sketch(), model.plane());
        // The dimension a tool is half-way through placing, which is drawn exactly
        // as a stated one and stands in no stack. Handed in rather than placed here:
        // the figure above it is written by the other half of a frame, and the two
        // working it out apart is a number and a rule drawn about two different
        // constraints — see [`Proposed`].
        placed
            .iter()
            .map(move |placed| (sketch.constraint(placed.of), placed.mark, false))
            .chain(proposed.map(|proposed| (proposed.constraint, proposed.mark, true)))
            .filter_map(move |(constraint, placed, proposed)| {
                let measured = Measurement::of(sketch, constraint)?;
                // Where the *number* stands, which is what an extension line
                // is reaching to and so what the gaps at either end of one
                // dimension have to be sized against.
                let at = plane.point(placed.at).as_vec3();
                let scale = f64::from(lens.world_per_pixel(at));
                let strokes = dimension::strokes(
                    Measurement {
                        // The mark's own direction and height, so the line
                        // lands under the figure it carries: the first is
                        // settled either side of a cut and the second is
                        // where the stack left it, and neither is anything
                        // the measurement knows.
                        along: placed.along,
                        label: placed.at
                            + placed.along.perp() * (f64::from(placed.rule_rise()) * scale),
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

/// One stroke a gizmo is made of.
#[derive(Debug, Clone, Copy)]
enum Piece {
    /// The square that stands for a whole plane, at its origin and in the ink
    /// its world axis wears.
    ///
    /// **At the origin and not over the drawing.** The square is a symbol for a
    /// plane rather than a backdrop under one — it holds its size on screen, so
    /// it could not cover a drawing at every zoom in any case — and put at the
    /// origin the three the world comes with cross there, which is the mark
    /// every modeller draws for an origin and reads at a glance. One that
    /// followed whatever was sketched on it would leave them crossing nowhere.
    ///
    /// **A plane is drawn where it is what you are working with.** With no
    /// sketch open every one of them is, because picking a plane is then the
    /// whole of what there is to do. With one open it is the plane under it —
    /// which is what says where you are — and any plane that can be *moved*,
    /// whose square is the only thing there is to take hold of it by. The rest
    /// stay away: the three the world comes with are square to one another and
    /// cross at the origin, so two of them would stand across whatever is built
    /// there.
    Sheet(Plane, Vec3),
    /// An arrow handling a number a form is still deciding: the depth a solid
    /// is carried, or how much of a turn it is spun.
    ///
    /// One shape for both, because a handle on a number is a handle on a
    /// number — what tells them apart is where it stands and which way it
    /// points, and both of those are the caller's. Which one a press found is
    /// the *tag*, not the shape.
    Handle(Carried),
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
    /// Write it into `curve`, corners and all.
    ///
    /// **One match rather than one per property.** What a piece answers — the
    /// depth it takes, whether it closes, how wide, in what ink, how hard it
    /// competes for a click, and the corners themselves — is not six questions
    /// about three kinds but two families that share almost nothing: the flat
    /// outlines a control is cut from, differing in little but their shape and
    /// their colour, and a dimension's stroke sharing none of it. Asked a
    /// property at a time, every answer had to spell out all three kinds to say
    /// that.
    ///
    /// The tag is not here: a name is minted out of the list the caller is
    /// appending to, and this is handed a `Curve` rather than the walk.
    fn stroke(self, curve: &mut Curve, theme: &Theme, lens: Lens) {
        let geometry = &theme.geometry;
        curve.points.clear();
        match self {
            Piece::Sheet(plane, ink) => {
                control(curve, theme, ink, Some(plane));
                // Hairline, where a handle is a shade heavier than the drawing:
                // the square is a symbol for a plane before it is something to
                // grab, and at a handle's weight it would read as a thing to
                // take hold of everywhere it passed.
                curve.width = geometry.sheet;
                // **Aside**, so it yields a click to anything drawn on the plane
                // it stands for. Not a frame: a frame does not merely lose the
                // click, it *hides* what is behind it from a pick — right for a
                // backdrop drawn round a drawing, and wrong for a small square
                // standing at the origin, where a model is built. Tagged and
                // framed, it swallowed dimension marks lying a little further
                // back.
                curve.precedence = Precedence::Aside;
                lay(curve, plane, &shape::sheet(), lens);
            }
            Piece::Handle(carried) => {
                // Standing out of a plane rather than lying in one, so it takes
                // no plane's depth — see [`Carried`].
                control(curve, theme, geometry.depth_arrow, None);
                // **The one control that does not yield.** A plane's square
                // stands aside, being what the drawing is done *on*. This is
                // what the gesture is *for* — a form is open and the arrow is
                // the thing being dragged — so it has to take the click over
                // the geometry it stands over. Ranking it as a frame would also
                // enter it among the occluders, so it would go on to hide what
                // is behind it from a pick as well as losing to it.
                curve.precedence = Precedence::Shaped;
                // Sized where it stands, like every control — see [`lay`] — but
                // laid out in a frame of its own rather than on a plane.
                let scale = f64::from(lens.world_per_pixel(carried.tail));
                curve
                    .points
                    .extend(shape::arrow(DVec2::X).map(|at| carried.at(at, scale)));
            }
            // **Drawing rather than a handle**, which is where it parts from
            // the four above. It takes the drawing's own width, because what a
            // heavier stroke says is "take hold of me" and a dimension line is
            // read; it closes at its heads and runs open along its lines, where
            // a control is a filled outline throughout; and it is already sized,
            // having had to know the scale before it was a [`Piece`] at all —
            // see [`ruled`].
            //
            // The depth it takes and how hard it competes it *does* share, and
            // writes out all the same: three of the five differ, so going
            // through [`control`] would be setting more than it kept. The
            // standing decides nothing either way — a dimension's lines carry no
            // tag, so no pick ever reaches them — and is stated because a stroke
            // that later earned a name would otherwise inherit whatever this
            // happened to say.
            Piece::Ruled {
                plane,
                stroke,
                proposed,
            } => {
                // The number's own colour: the lines are that number's, and a
                // hue of their own would read as a third kind of thing on the
                // drawing. Not the redundant red a spare relation wears — a
                // dimension the constraints could do without says so in its
                // figure, and saying it twice would double the ink on the one
                // case that is already loud. A proposal has no state to report
                // at all, so it wears the grey a rubber band does.
                curve.color = if proposed {
                    geometry.ghost
                } else {
                    geometry.mark
                };
                curve.width = geometry.edge;
                curve.closed = stroke.closes();
                curve.plane_normal = Some(plane.normal().as_vec3());
                curve.precedence = Precedence::Frame;
                curve
                    .points
                    .extend(stroke.corners().map(|at| plane.point(at).as_vec3()));
            }
        }
    }
}

/// Set `curve` up as a control in `ink`, lying in `plane` where it lies in one.
///
/// What the controls share, which is everything but their outline, their colour
/// and how hard they compete for a click: the width a handle is stroked at — see
/// [`Drawing::gizmo`](crate::look::geometry::Geometry) — and closed, because
/// every control is a filled outline.
///
/// **Not the standing.** No two controls here want the same one and each says
/// why where it sets it, so a default set here would be a value every caller
/// overwrote — which reads as the rule while being the one thing that never
/// holds.
///
/// A control lies in a plane and is widened in screen space, so it takes that
/// plane's depth rather than its anchor's — the same thing every stroke of the
/// drawing does. `None` is the arrow that stands *out* of a plane instead.
fn control(curve: &mut Curve, theme: &Theme, ink: Vec3, plane: Option<Plane>) {
    curve.color = ink;
    curve.width = theme.geometry.gizmo;
    curve.closed = true;
    curve.plane_normal = plane.map(|plane| plane.normal().as_vec3());
}

/// Put `outline` on `plane`, at the scale a pixel is worth where that plane
/// stands.
///
/// **Sized where it stands, not where the camera is looking**: under perspective
/// a pixel covers more world the further off it is, so a control on a distant
/// plane built to the target's scale would come out the wrong size.
///
/// The outline arrives in logical pixels and in coordinates of its own — see
/// [`shape`] — so this is the one step that turns one into geometry.
fn lay(curve: &mut Curve, plane: Plane, outline: &[DVec2], lens: Lens) {
    let scale = f64::from(lens.world_per_pixel(plane.origin.as_vec3()));
    curve
        .points
        .extend(outline.iter().map(|&at| plane.point(at * scale).as_vec3()));
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
