//! Turning a drawing into the primitives a renderer holds: one writer per kind
//! of thing on screen.
//!
//! **Every writer refills rather than builds.** A drag lays the drawing out
//! sixty times a second, so each of these is written over the batch the renderer
//! already holds — see [`Batch::refill`](aperture::Batch) — and the tags come
//! out the same across a rewrite because they are positions in a list built in
//! the same order. What decides *when* any of it runs is
//! [`redraw`](crate::paint::redraw), which is the one caller of all six.
//!
//! **What each takes is what it draws.** The room the drawing is laid out in
//! arrives in pieces — the names, the sheets, the placements — rather than as
//! the [`Layout`](crate::paint::layout::Layout) that holds them, so nothing here
//! can reach the claim that layout makes about what it describes. Stamping that
//! is `redraw`'s alone.
//!
//! Colour, width and standing are decided a module up, in
//! [`paint`](crate::paint): what a drawing looks like is one set of choices, and
//! these are the calls that spend them.

use aperture::{
    Batch, Curve, Facing, Mesh, Object, Point, Precedence, Ring, Styled, Text, Turn, Vertex,
};
use glam::{Mat4, Vec2, Vec3};
use silverpoint::{Body, Circle, CircleId, Constraint, Named, Segment, SegmentId, Sketch};
use std::fmt::Write;

use crate::look::Theme;
use crate::model::{Model, Models};
use crate::paint::growing::{Deciding, Growing, Raising, UNTAKEN};
use crate::paint::layout::Sheets;
use crate::paint::marks::mark::Mark;
use crate::paint::marks::{Placed, Proposed};
use crate::paint::names::Names;
use crate::paint::{
    DECIMALS, FACE_SAGITTA, MARK_FONT, SHEET_NAME_LIFT, marks, shade, standing, symbol,
};
use crate::part::Part;
use crate::preview::Ends;
use crate::wording;

/// The shape a two-click tool is half-way through, and the plane it lies in.
///
/// **The plane rides along rather than being asked for where the band is
/// drawn.** It is the sketch being drawn in — the one plane a band could lie in,
/// since a tool draws where you are and not where you are not — and a line goes
/// among the strokes while a circle goes among the rims. Two writers, one fact:
/// asked at each of them it was the same line of code twice, and each spelling
/// paid its own walk to answer it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Band {
    pub(super) ends: Ends,
    /// The normal of the plane it lies in, which is the depth a stroke of it is
    /// widened at.
    pub(super) normal: Vec3,
}

impl Band {
    /// The band running between `ends` on the sketch `models` has open, or
    /// `None` where no tool is half-way through one.
    ///
    /// The ends are read before the plane, which is what keeps the reading off
    /// every frame that is not drawing a band — and off the writer that is not:
    /// a band is a stroke or a rim and never both, so only one of the two calls
    /// here ever reaches the sketch.
    ///
    /// No sketch open is no band either, and the `?` says so rather than a
    /// guard: a band is what a *tool* is half-way through, and a tool draws in
    /// the sketch you are in.
    pub(super) fn new(models: Models<'_>, ends: Option<Ends>) -> Option<Self> {
        Some(Self {
            ends: ends?,
            normal: models.open()?.plane().normal().as_vec3(),
        })
    }
}

/// The sketch's straight strokes, one edge per segment, biased clear of
/// the solids in depth so the drawing reads over them. Circles are not
/// strokes — see [`rings`].
pub(super) fn curves(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    band: Option<Band>,
    into: &mut Batch<Curve>,
) {
    let drawing = &theme.drawing;
    // Written over the strokes already there rather than into fresh ones, which
    // for a `Curve` is the difference between a frame that reaches the heap and
    // one that does not — see `Batch::refill`. That is also why all three kinds
    // are chained into one refill rather than written in three passes: a stroke
    // appended outside it would be dropped by the next rewrite of the drawing
    // and allocated afresh by the one after, once a frame for as long as a line
    // is being drawn.
    //
    // The drawing, then what is being drawn now. What it is drawn *on* is no
    // part of this batch: a plane's square holds its size on screen, so it is
    // cut against the camera with the other handles — see
    // [`gizmos::write`](crate::paint::gizmos::write).
    into.refill(
        models
            .iter()
            .flat_map(|model| {
                model
                    .sketch()
                    .segments()
                    .map(move |(id, edge)| Stroke::Edge(model, id, edge))
            })
            .chain(band.map(Stroke::Band)),
        |curve, stroke| {
            curve.width = drawing.edge;
            match stroke {
                Stroke::Edge(model, id, edge) => {
                    let (sketch, plane) = (model.sketch(), model.plane());
                    let a = plane.point(sketch.point(edge.a).position).as_vec3();
                    let b = plane.point(sketch.point(edge.b).position).as_vec3();
                    curve.set_segment(a, b);
                    curve.color = shade(theme, model, drawing.freedom(model.outcome().segment(id)));
                    curve.precedence = standing(model);
                    curve.plane_normal = Some(plane.normal().as_vec3());
                    curve.tag = Some(names.tag(model.part(id)));
                }
                // Untagged, which is what keeps the band out of the way: a pick
                // skips a primitive with no tag, so it cannot be hovered,
                // grabbed or picked out, and the click that finishes the line
                // resolves against the geometry behind it.
                Stroke::Band(band) => {
                    curve.set_segment(band.ends.from, band.ends.to);
                    curve.color = drawing.ghost;
                    curve.precedence = Precedence::Shaped;
                    curve.plane_normal = Some(band.normal);
                    curve.tag = None;
                }
            }
        },
    );
}

/// One stroke to write: an edge the sketch holds, or the band a tool is in the
/// middle of drawing.
#[derive(Debug)]
enum Stroke<'a> {
    Edge(Model<'a>, SegmentId, Segment),
    Band(Band),
}

/// The sketch's circles, one ring apiece.
///
/// Not tessellated into strokes: the count that looks round depends on how
/// large the circle lands on screen, and the renderer resolves a ring in
/// the fragment stage instead, which is round at every zoom and needs no
/// rebuilding when the camera moves.
///
/// No plane named, unlike the strokes — a ring's band is widened in its
/// own plane, so the depth it carries is already the surface's.
pub(super) fn rings(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    band: Option<Band>,
    into: &mut Batch<Ring>,
) {
    let drawing = &theme.drawing;
    into.refill(
        models
            .iter()
            .flat_map(|model| {
                model
                    .sketch()
                    .circles()
                    .map(move |(id, circle)| Rim::Circle(model, id, circle))
            })
            .chain(band.map(Rim::Band)),
        |ring, rim| {
            // Assigned whole, like a marker and unlike a stroke: a rim owns
            // nothing either.
            *ring = match rim {
                Rim::Circle(model, id, circle) => {
                    let (sketch, plane) = (model.sketch(), model.plane());
                    Ring::new(
                        plane.point(sketch.point(circle.center).position).as_vec3(),
                        circle.radius.abs() as f32,
                        plane.normal().as_vec3(),
                    )
                    .colored(shade(
                        theme,
                        model,
                        drawing.freedom(model.outcome().circle(id)),
                    ))
                    .precedence(standing(model))
                    .tagged(names.tag(model.part(id)))
                }
                // Through the cursor rather than out to it: the second click
                // says how big by naming somewhere on the rim. Untagged, like
                // the band among the strokes.
                Rim::Band(band) => Ring::new(
                    band.ends.from,
                    band.ends.from.distance(band.ends.to),
                    band.normal,
                )
                .colored(drawing.ghost),
            }
            .width(drawing.edge);
        },
    );
}

/// One rim to write: a circle the sketch holds, or the band a tool is in the
/// middle of drawing.
#[derive(Debug)]
enum Rim<'a> {
    Circle(Model<'a>, CircleId, Circle),
    Band(Band),
}

/// The sketch's points, one marker apiece — larger and pinned-coloured
/// where the solver may not move it.
///
/// The plane comes along for the same reason a stroke's does: a disc is
/// flat in depth and the surface under it is not, so without it the glyph
/// is sliced wherever the plane is seen at an angle.
pub(super) fn points(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    into: &mut Batch<Point>,
) {
    let drawing = &theme.drawing;
    into.refill(
        models
            .iter()
            .flat_map(|model| model.sketch().points().map(move |at| (model, at))),
        |marker, (model, (id, point))| {
            let plane = model.plane();
            // Pinned by hand outranks pinned by consequence: a fixed point is
            // determined too, but saying so in the same colour would lose the
            // one thing about it the user chose.
            let (color, size) = if point.fixed {
                (drawing.pinned, drawing.fixed_marker)
            } else {
                (
                    drawing.freedom(model.outcome().point(id)),
                    drawing.free_marker,
                )
            };
            // Assigned whole where a stroke is edited in place: a marker owns
            // nothing, so replacing one costs what overwriting it would.
            *marker = Point::new(plane.point(point.position).as_vec3())
                .colored(shade(theme, model, color))
                .size(size)
                .in_plane(plane.normal().as_vec3())
                .precedence(standing(model))
                .tagged(names.tag(model.part(id)));
        },
    );
}

/// A mark for every relation the drawing states, saying what holds and where.
///
/// Set in type rather than drawn as geometry, which is what makes the whole set
/// one rule: every relation gets a symbol, the symbol is legible at any zoom
/// because it is sized in pixels, and adding a tenth constraint is a line in
/// [`symbol`] rather than a shape to construct.
///
/// **Turned into the sketch's plane**, so a mark reads as lettering on the
/// drawing rather than as a note pinned over it. Only the *direction* it runs
/// in: it is still sized in pixels, so the zoom cannot reach it and neither can
/// the angle the plane is seen at — see [`Facing`]. Which way up it comes out is
/// the renderer's, and always the way that reads.
///
/// One mark *or two*, and stacked where several want one place — see
/// [`marks::stacked`], which decides all of how many, where, and how high.
/// Where there are two they carry the same name, so a click on either takes
/// the constraint.
///
/// Tagged like everything else, so a mark is picked and deleted the way the
/// geometry it is about is — which is the whole of how an over-constrained
/// sketch gets un-stuck.
pub(super) fn texts(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    placed: &mut Vec<Placed>,
    proposed: Option<Proposed>,
    typed: Option<Part>,
    into: &mut Batch<Text>,
) {
    let drawing = &theme.drawing;
    // **The open sketch alone.** A constraint is a statement *about* a drawing,
    // and one you are not in is not a drawing you can argue with: its marks can
    // neither be selected into a relation nor typed into, so all they do is
    // crowd the sketch you are working in — and a dimension is the densest
    // thing the drawing puts on screen. The geometry of a dormant sketch still
    // shows, dimmed, because where it *is* is something you build against.
    //
    // No sketch open is no marks at all — and the planes take the batch over
    // instead, which is why this hands off rather than emptying it. The
    // placements still have to be cleared: they are kept across frames for the
    // rules drawn under them a phase later, and there are none — see
    // [`gizmos::ruled`](crate::paint::gizmos).
    let Some(live) = models.open() else {
        placed.clear();
        return named_planes(models, theme, names, into);
    };
    // Laid out whole, before anything is left out. What lane a mark rises in
    // depends on how many share its place, so a stack that was worked out from
    // what is *shown* would close ranks the moment a field opened over one of
    // them — and closing ranks under a double-click reads as the click having
    // nudged the drawing.
    marks::stacked(live, placed);
    into.refill(
        placed
            .iter()
            // The one being retyped has a field standing over it — see
            // [`Prompt::show`](crate::prompt::Prompt) — and a mark left
            // under one would be a second copy of the number showing
            // through wherever the field did not quite cover it.
            .filter(move |placed| Some(live.part(placed.of)) != typed)
            .map(|placed| Marked::Stated(*placed))
            // Last, so a dimension being placed is written over the drawing
            // rather than under it — and so the tags the drawing handed out
            // are the same whether or not a tool is half-way through one.
            .chain(proposed.map(Marked::Proposed)),
        |mark, marked| {
            let placed = marked.mark();
            let constraint = marked.constraint(live.sketch());
            // Rewritten in place rather than assigned, so a drawing whose marks are
            // laid out every frame keeps the string it already has — which is what
            // keeps a scrubbed dimension off the heap sixty times a second.
            mark.content.clear();
            match constraint.value() {
                // A dimension reads as its measurement. That *is* the mark: a
                // number beside a length says both that the length is stated and
                // what it is stated as, where a symbol would say only the first and
                // leave the drawing unreadable.
                Some(value) => {
                    let prefix = wording::named(constraint).prefix;
                    write!(mark.content, "{prefix}{value:.*}", DECIMALS)
                        .expect("writing to a string cannot fail");
                }
                None => mark.content.push_str(symbol(constraint)),
            }
            let plane = live.plane();
            mark.position = plane.point(placed.at).as_vec3();
            mark.font = MARK_FONT;
            // **Centred on its own box**, with the clearance carried by the
            // lift below instead. An anchor fraction rides in the run's own
            // frame, and both rules that settle that frame — the mirror and the
            // half turn — would carry it along, swinging the box to the other
            // side of the very line it stands clear of. A centred box is mapped
            // onto itself by either, so it only ever changes direction.
            mark.anchor = Vec2::splat(0.5);
            mark.color = match marked {
                // A proposal has no state to report: the constraints have not
                // been asked about it, so it cannot be redundant and cannot be
                // anything else either. The grey a rubber band wears, and for
                // the same reason — it is not in the drawing yet.
                Marked::Proposed(..) => drawing.ghost,
                Marked::Stated(stated) if live.outcome().is_redundant(stated.of) => {
                    shade(theme, live, drawing.redundant)
                }
                Marked::Stated(..) => shade(theme, live, drawing.mark),
            };
            mark.precedence = standing(live);
            // Lettered on the drawing rather than pinned over it: set along the
            // geometry it is about — the span a dimension measures, the edge a
            // symbol names — so a number reads as belonging to the line under it
            // and turns with the plane it belongs to. Which direction that is,
            // is [`marks::anchors`]'s; what is here is putting it on the plane.
            //
            // Clear of that geometry by the lift, which is stated in the plane's
            // own axes and so is the one thing about a mark the projection
            // cannot move.
            mark.facing = Facing::Turned(placed.turn(live.drawing()));
            // Untagged where it is a proposal, which is what keeps it out of the
            // way: a pick skips a primitive with no tag, so the click that
            // *commits* the dimension resolves against the geometry behind it
            // rather than against the picture of what it is about to make.
            mark.tag = match marked {
                Marked::Stated(stated) => Some(names.tag(live.part(stated.of))),
                Marked::Proposed(..) => None,
            };
        },
    );
}

/// A name against each plane, for a document being looked at rather than drawn
/// in.
///
/// **The same batch the marks use, and never at the same time.** A name says
/// which plane you would be starting on, which is worth knowing exactly when
/// there is no drawing to start from; a mark says what a drawing states, of
/// which there is none. So the two are one batch's two contents rather than two
/// batches, and the refill that writes either is the one that clears the other.
///
/// **Laid into the plane rather than pinned over it**, which is what makes a
/// name read as belonging to the sheet it is on: it runs along that plane's own
/// +x and takes its depth from it, so the plane can hide it and turning the
/// model turns it. The two rules that keep a laid run legible — the mirror that
/// answers a camera behind the plane, and the half turn that keeps it upright —
/// are [`Turn`]'s own, so nothing here has to know where the eye
/// is.
///
/// **Inside the top-left corner of the plane's square**, running along that
/// plane's own +x — a title's place, and the corner a reader's eye starts from.
/// Reached by anchoring the run centred on the plane's origin, where the square
/// is, and carrying it out with a lift: the lift is stated in logical pixels and
/// resolved where the run is drawn, so it lands in the same corner of a square
/// that is itself a fixed number of pixels, at any zoom and whatever that
/// number becomes. It is also the only one of the three ways to shift a run that
/// survives the two rules above — see [`SHEET_NAME_LIFT`](crate::paint).
///
/// A plane somebody put there carries no name: until steps have names of their
/// own every one of them would read the same word — see
/// [`World::named`](crate::timeline::feature::World).
fn named_planes(models: Models<'_>, theme: &Theme, names: &mut Names, into: &mut Batch<Text>) {
    into.refill(
        models
            .planes()
            .filter_map(|sheeted| Some((sheeted, sheeted.world?.named()))),
        |text, (sheeted, named)| {
            // Written over what is there rather than assigned, like a mark and
            // for the same reason: a `Text` owns its content on the heap.
            text.content.clear();
            text.content.push_str(named);
            let plane = sheeted.plane;
            text.position = plane.origin.as_vec3();
            text.font = MARK_FONT;
            text.anchor = Vec2::splat(0.5);
            text.facing = Facing::Turned(
                Turn::new(plane.x.as_vec3(), plane.normal().as_vec3()).lifted(SHEET_NAME_LIFT),
            );
            text.color = theme.drawing.sheet_ink(sheeted.world);
            // A frame, which does two things and both matter. It yields a
            // click to anything ordinary, so a name lying over the model cannot
            // take one; and it is left out of how far the scene reaches — so a
            // camera is not framed on a label whose distance from the origin is
            // a number of pixels rather than anything the model said.
            text.precedence = Precedence::Frame;
            text.tag = Some(names.tag(Part::Step(sheeted.at)));
        },
    );
}

/// One mark to write: a relation the drawing states, or the dimension a tool is
/// half-way through placing.
///
/// One writer for both, because a preview that was drawn by other code would be
/// a second opinion about what a dimension looks like — and the whole of what a
/// preview is for is showing what the click will make. What they differ in is
/// stated in the two places it is real: a proposal has no state to report and
/// nothing to pick.
///
/// Neither carries the drawing it belongs to, unlike [`Stroke`] and [`Rim`]
/// above. Those are written for every sketch the document holds and so name one
/// apiece; marks are written for the open sketch alone, which is one model the
/// writer already has in hand.
#[derive(Debug, Clone, Copy)]
enum Marked {
    Stated(Placed),
    /// The dimension the next click would state, and where its mark goes.
    ///
    /// Carried rather than looked up, because the sketch does not hold it: there
    /// is no handle to ask with, which is exactly what makes it a proposal. Laid
    /// out by [`redraw`](crate::paint::redraw) rather than here, so the rule
    /// drawn under it reads the same answer — see [`Proposed`].
    Proposed(Proposed),
}

impl Marked {
    /// What it says — out of `sketch` where the sketch is what holds it, and
    /// carried where nothing does.
    fn constraint(self, sketch: &Sketch) -> Constraint {
        match self {
            Marked::Stated(placed) => sketch.constraint(placed.of),
            Marked::Proposed(proposed) => proposed.constraint,
        }
    }

    /// Where it stands and which way it runs.
    fn mark(self) -> Mark {
        match self {
            Marked::Stated(placed) => placed.mark,
            Marked::Proposed(proposed) => proposed.mark,
        }
    }
}

/// A sheet per region the drawing's curves shut in.
///
/// The one part of a drawing that is not drawn: a region is what the curves
/// *enclose*, so nothing here reads a segment or a circle — it reads what
/// [`Arrangement`](silverpoint::Arrangement) made of all of them together, and
/// a half-circle cut by an edge is as much a region as a rectangle traced by
/// four.
///
/// Meshes rather than overlays, because a region has area in the world where a
/// stroke has width on the screen. They go to the scene's own batch for them,
/// which is drawn two-sided and biased forward off the plane they lie in — see
/// [`Scene::faces`](aperture::Scene).
///
/// Named like everything else, so a region can be hovered and picked out. A
/// cursor over one still reaches the geometry bounding it first: a surface is
/// the least specific thing a pick can land on — see
/// [`HitAt`](aperture::HitAt) — and every stroke and marker that draws a region
/// lies within it.
///
/// Named *by position*, which is the one thing about a region that is not a
/// handle. See [`Part::Region`](crate::part::Part).
pub(super) fn faces(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    sheets: &mut Sheets,
    into: &mut Batch<Object>,
) {
    let Sheets { filler, fill, .. } = sheets;
    into.refill(
        models
            .iter()
            .flat_map(|model| (0..model.arrangement().faces().len()).map(move |at| (model, at))),
        |object, (model, at)| {
            let plane = model.plane();
            let normal = plane.normal().as_vec3();
            let arrangement = model.arrangement();
            let face = &arrangement.faces()[at];
            filler.fill(arrangement, face, FACE_SAGITTA, fill);
            remesh(
                &mut object.mesh,
                fill.corners.iter().map(|&corner| Vertex {
                    position: plane.point(corner).as_vec3(),
                    normal,
                }),
                &fill.triangles,
            );
            object.transform = Mat4::IDENTITY;
            object.color = if model.live() {
                theme.drawing.face
            } else {
                theme.drawing.dormant_face
            };
            object.precedence = standing(model);
            object.tag = Some(names.tag(model.region(at)));
        },
    );
}

/// The two batches a body can be written into.
///
/// A bundle on the terms [`Raising`] states: both are lent for the length of
/// one call, and two `&mut`s in a row at the call site would be two chances to
/// hand over the wrong one. Which of them a body goes in is what that body
/// *is* — see [`solids`], where that is decided.
#[derive(Debug)]
pub(super) struct Shaping<'a> {
    /// The model, and an answer that already holds it.
    pub(super) solid: &'a mut Batch<Object>,
    /// A proposal standing inside the model, drawn through it rather than
    /// hidden by it.
    pub(super) ghost: &'a mut Batch<Object>,
}

/// An object per face of every solid the document has grown.
///
/// One object per *face* rather than one per solid, which is what makes a solid
/// something you can point at: a tag names a primitive, so a face that is to be
/// hovered, picked out and later built on has to be a primitive of its own.
///
/// Named by what each face was grown from — see [`Grown`](silverpoint::Grown) —
/// rather than by where
/// it fell in this frame's list. That is the same durable vocabulary the region
/// underneath was named in, so a selection survives the drawing moving under it
/// exactly as a sketch entity's does.
///
/// Modelled rather than drawn, so unlike everything else here there is no
/// appearance to decide beyond the one colour: what a solid *is* is the shape,
/// and shading it is the renderer's.
pub(super) fn solids(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    sheets: &mut Sheets,
    growing: Option<Growing>,
    sagitta: f64,
    shaping: Shaping<'_>,
) {
    let Shaping { solid, ghost } = shaping;
    let Sheets {
        mesher,
        patch,
        deciding,
        builder,
        putting,
        raised,
        regions,
        ..
    } = sheets;
    // Worked out here rather than borrowed off the document, unlike every solid
    // there is a step for: the one being decided has no step to be held
    // against. See [`Growing::body`](crate::paint::growing::Growing).
    let showing = growing.map_or(Deciding::Nothing, |growing| {
        let raising = Raising {
            builder,
            putting,
            raised,
            regions,
        };
        growing.body(models, raising, deciding)
    });
    // **The answer stands in for the document's own solids**, because it
    // already holds them: drawing both would put the model on screen twice,
    // two copies of one surface fighting for one depth. Every other answer
    // leaves the document drawn as it always is.
    let standing = (showing != Deciding::Answer).then(|| models.solids());
    // **Which batch the one being decided goes in is what it *is*.** An answer
    // holds the model and stands where the model stands, so it is a solid. A
    // tool standing beside it is a proposal about material that is not there
    // yet — and it sits *inside* the part, whether because it is the cut it
    // would make or because a frame had no time to combine it. Drawn as a
    // solid it would be hidden by the very part it is about. See
    // [`Scene::ghosts`](aperture::Scene).
    let (answered, faintly) = match showing {
        Deciding::Answer => (Some(&*deciding), None),
        Deciding::Beside => (None, Some(&*deciding)),
        Deciding::Nothing => (None, None),
    };
    // One walk and nothing gathered: a body hands out its faces as an iterator,
    // so the whole of a document's solids is written straight into the batch. A
    // list of them first would be an allocation a frame, which is exactly what
    // a rubber band's redraw would pay every frame it lasts. The one being
    // decided is chained on rather than pushed after, so a depth typed a digit
    // at a time rewrites the batch it is already in — see `Batch::refill`.
    // Last, so the tags of everything the document holds come out the same
    // whether or not a form is open.
    let faces = standing
        .into_iter()
        .flatten()
        .map(|(_, body)| body)
        .chain(answered)
        .flat_map(per_face);
    let mut shape = |object: &mut Object, (body, face): (&Body, Named)| {
        mesher.cut(body, face, sagitta, patch);
        remesh(
            &mut object.mesh,
            patch
                .corners
                .iter()
                .zip(&patch.normals)
                .map(|(&corner, &normal)| Vertex {
                    position: corner.as_vec3(),
                    normal: normal.as_vec3(),
                }),
            &patch.triangles,
        );
        object.transform = Mat4::IDENTITY;
        object.color = theme.drawing.solid;
        object.precedence = Precedence::Shaped;
        // **A face carries the step that grew it, so the tag follows the
        // name.** What is being decided has no step — see [`UNTAKEN`] — so it
        // cannot be hovered, picked out or built on, there being nothing yet
        // to name; what is grabbable is the arrow carrying it, which is a
        // control rather than the solid. Every face the model brought through
        // the boolean keeps the tag it always had, so a form open on a depth
        // does not take the rest of the part out of reach.
        object.tag = (face.by != UNTAKEN).then(|| {
            names.tag(Part::Solid {
                of: face.by.into(),
                face: face.grown,
            })
        });
    };
    solid.refill(faces, &mut shape);
    // Refilled whether there is a ghost or not, so a form closing takes the
    // last one away rather than leaving it standing over the model.
    ghost.refill(faintly.into_iter().flat_map(per_face), &mut shape);
}

/// Every face of `body`, each carrying the body it was read off.
///
/// A face knows what grew it and nothing about what it is part of, and cutting
/// one wants both — so the pair is made here rather than at each of the two
/// walks that wants it.
fn per_face(body: &Body) -> impl Iterator<Item = (&Body, Named)> {
    body.names().map(move |face| (body, face))
}

/// Write `corners` and the `triangles` over them into `mesh`.
///
/// What a region's fill and a solid's patch have in common is exactly this, and
/// what they differ in is where the corners come from and what colour goes on
/// afterwards.
///
/// The two of them, not every mesh here: a gizmo is four shapes in one mesh,
/// each rebased onto the corners before it, and rewriting it goes through
/// nothing this could offer without being handed a list of shapes instead of
/// one — see [`gizmos::write`](crate::paint::gizmos::write).
///
/// Written over what is already there rather than assigned, which is what keeps
/// a drag off the heap: every face of a drawing and every face of every solid is
/// cut afresh whenever the document moves, and they come back the same size.
/// Through [`Mesh::rewrite`], which is what hands the buffers over and brings
/// the box the mesh is picked by up to date with what went into them.
fn remesh(mesh: &mut Mesh, corners: impl Iterator<Item = Vertex>, triangles: &[[u32; 3]]) {
    mesh.rewrite(|vertices, wound| {
        vertices.clear();
        vertices.extend(corners);
        wound.clear();
        wound.extend_from_slice(triangles);
    });
}

#[cfg(test)]
mod tests;
