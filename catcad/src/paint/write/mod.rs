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

use aperture::{Batch, Curve, Facing, Mesh, Object, Point, Precedence, Ring, Styled, Text, Vertex};
use glam::{DVec2, Mat4, Vec2, Vec3};
use silverpoint::{Circle, CircleId, Constraint, Plane, Segment, SegmentId, Sketch};
use std::fmt::Write;

use crate::model::{Model, Models, Spread};
use crate::paint::growing::Growing;
use crate::paint::layout::Sheets;
use crate::paint::marks::mark::Mark;
use crate::paint::marks::{Placed, Proposed};
use crate::paint::names::Names;
use crate::paint::{
    DECIMALS, DORMANT_FACE, EDGE_WIDTH, FACE, FACE_SAGITTA, FIXED_MARKER, FREE_MARKER, GHOST, MARK,
    MARK_FONT, PINNED, REDUNDANT, SHEET, SHEET_WIDTH, SOLID, SOLID_SAGITTA, colour, ink, marks,
    standing, symbol,
};
use crate::part::Part;
use crate::preview::Ends;
use crate::timeline::FeatureId;
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
    pub(super) fn new(models: Models<'_>, ends: Option<Ends>) -> Option<Self> {
        Some(Self {
            ends: ends?,
            normal: models.open().plane().normal().as_vec3(),
        })
    }
}

/// The sketch's straight strokes, one edge per segment, biased clear of
/// the solids in depth so the drawing reads over them. Circles are not
/// strokes — see [`rings`].
pub(super) fn curves(
    models: Models<'_>,
    names: &mut Names,
    band: Option<Band>,
    sheet: Spread,
    into: &mut Batch<Curve>,
) {
    // The sketch being drawn in, which supplies both halves of the one sheet
    // written here: the step its plane is, for the tag, and where that plane
    // lies. The second is the model's own — a drawing lies in the plane it is
    // drawn on — so there is no second lookup to disagree with it.
    let open = models.open();
    // Written over the strokes already there rather than into fresh ones, which
    // for a `Curve` is the difference between a frame that reaches the heap and
    // one that does not — see `Batch::refill`. That is also why all three kinds
    // are chained into one refill rather than written in three passes: a stroke
    // appended outside it would be dropped by the next rewrite of the drawing
    // and allocated afresh by the one after, once a frame for as long as a line
    // is being drawn.
    //
    // Back to front, so the order reads the way the picture does: the plane a
    // drawing is done on, then the drawing, then what is being drawn now.
    into.refill(
        std::iter::once(Stroke::Sheet {
            at: models.open_plane(),
            plane: open.plane(),
        })
        .chain(models.iter().flat_map(|model| {
            model
                .sketch()
                .segments()
                .map(move |(id, edge)| Stroke::Edge(model, id, edge))
        }))
        .chain(band.map(Stroke::Band)),
        |curve, stroke| {
            curve.width = EDGE_WIDTH;
            match stroke {
                Stroke::Edge(model, id, edge) => {
                    let (sketch, plane) = (model.sketch(), model.plane());
                    let a = plane.point(sketch.point(edge.a).position).as_vec3();
                    let b = plane.point(sketch.point(edge.b).position).as_vec3();
                    curve.set_segment(a, b);
                    curve.color = ink(model, colour(model.outcome().segment(id)));
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
                    curve.color = GHOST;
                    curve.precedence = Precedence::Shaped;
                    curve.plane_normal = Some(band.normal);
                    curve.tag = None;
                }
                // The one stroke here that is not part of a drawing but of
                // what a drawing is done *on*, which is the whole of why it
                // stands as a frame: it runs right round the geometry, and
                // ranked as a shape it would take clicks meant for whatever it
                // encircled. Standing as a frame is also what keeps it from
                // deciding where the camera goes — see
                // [`Scene::extent`](aperture::Scene), which leaves the furniture
                // out because furniture is sized to what it stands around.
                //
                // Written over the points already there rather than into a
                // fresh list, like the two above and for the same reason — a
                // closed outline is four `Vec3`s, and a rewrite that allocated
                // them would do it on every frame a band moves.
                Stroke::Sheet { at, plane } => {
                    curve.points.clear();
                    curve.points.extend(corners(plane, sheet));
                    curve.closed = true;
                    curve.width = SHEET_WIDTH;
                    curve.color = SHEET;
                    curve.precedence = Precedence::Frame;
                    curve.plane_normal = Some(plane.normal().as_vec3());
                    curve.tag = Some(names.tag(Part::Plane(at)));
                }
            }
        },
    );
}

/// The four corners of the sheet `spread` names in `plane`, wound the way they
/// are stroked.
///
/// In the world rather than in logical pixels, which is what tells a sheet from
/// the handles that sit on the same plane: a plane is a place, where a handle is
/// a thing to grab and holds its size on screen. So this is here with the
/// drawing rather than in [`gizmos::shape`](crate::paint::gizmos), whose shapes
/// are all measured on screen.
fn corners(plane: Plane, spread: Spread) -> [Vec3; 4] {
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| {
        plane
            .point(spread.middle + DVec2::new(x, y) * spread.reach)
            .as_vec3()
    })
}

/// One stroke to write: an edge the sketch holds, or the band a tool is in the
/// middle of drawing.
#[derive(Debug)]
enum Stroke<'a> {
    Edge(Model<'a>, SegmentId, Segment),
    Band(Band),
    /// The outline of the plane the drawing is standing on.
    ///
    /// **That one and no other**, though a document holds several and every one
    /// of them is somewhere a drawing could be started. A plane is drawn where
    /// it is what you are working with: while a sketch is open that is the plane
    /// under it, and the rest would be furniture standing in front of the model
    /// — the three the world comes with are square to one another and cross at
    /// the origin, so two of them cut across whatever is built there, taking
    /// presses meant for it and drawing over the solids it hides behind.
    ///
    /// The other reading of that rule is the one that has nowhere to be stated
    /// yet: with no sketch open there is no drawing to stand in front of, and
    /// every plane wants showing because picking one is the whole of what there
    /// is to do. Nothing can close a sketch today — see
    /// [`Session::editing`](crate::session::Session).
    Sheet {
        at: FeatureId,
        plane: Plane,
    },
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
    names: &mut Names,
    band: Option<Band>,
    into: &mut Batch<Ring>,
) {
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
                    .colored(ink(model, colour(model.outcome().circle(id))))
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
                .colored(GHOST),
            }
            .width(EDGE_WIDTH);
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
pub(super) fn points(models: Models<'_>, names: &mut Names, into: &mut Batch<Point>) {
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
                (PINNED, FIXED_MARKER)
            } else {
                (colour(model.outcome().point(id)), FREE_MARKER)
            };
            // Assigned whole where a stroke is edited in place: a marker owns
            // nothing, so replacing one costs what overwriting it would.
            *marker = Point::new(plane.point(point.position).as_vec3())
                .colored(ink(model, color))
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
    names: &mut Names,
    placed: &mut Vec<Placed>,
    proposed: Option<Proposed>,
    typed: Option<Part>,
    into: &mut Batch<Text>,
) {
    // **The open sketch alone.** A constraint is a statement *about* a drawing,
    // and one you are not in is not a drawing you can argue with: its marks can
    // neither be selected into a relation nor typed into, so all they do is
    // crowd the sketch you are working in — and a dimension is the densest
    // thing the drawing puts on screen. The geometry of a dormant sketch still
    // shows, dimmed, because where it *is* is something you build against.
    let live = models.open();
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
                Marked::Proposed(..) => GHOST,
                Marked::Stated(stated) if live.outcome().is_redundant(stated.of) => {
                    ink(live, REDUNDANT)
                }
                Marked::Stated(..) => ink(live, MARK),
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
            object.color = if model.live() { FACE } else { DORMANT_FACE };
            object.precedence = standing(model);
            object.tag = Some(names.tag(model.region(at)));
        },
    );
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
    names: &mut Names,
    sheets: &mut Sheets,
    growing: Option<Growing>,
    into: &mut Batch<Object>,
) {
    let Sheets { skinner, patch, .. } = sheets;
    // One walk and nothing gathered: a prism is `Copy` and hands out its faces
    // as an iterator, so the whole of a document's solids is written straight
    // into the batch. A list of them first would be an allocation a frame, which
    // is exactly what a rubber band's redraw would pay every frame it lasts.
    // The one being decided is chained on rather than pushed after, so a depth
    // typed a digit at a time rewrites the batch it is already in — see
    // `Batch::refill`. Last, so the tags of everything the document holds come
    // out the same whether or not a form is open.
    let faces = models
        .solids()
        .map(|(at, prism)| (Some(at), prism))
        .chain(
            growing
                .and_then(|growing| growing.prism(models))
                .map(|prism| (None, prism)),
        )
        .flat_map(|(at, prism)| prism.grown().map(move |face| (at, prism, face)));
    into.refill(faces, |object, (at, prism, face)| {
        skinner.cut(&prism, face, SOLID_SAGITTA, patch);
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
        object.color = SOLID;
        object.precedence = Precedence::Shaped;
        // Untagged while it is being decided, which is what keeps a solid that
        // is not in the document out of everything that names one: it cannot be
        // hovered, picked out, or built on, because there is nothing yet to
        // name. What *is* grabbable is the arrow that carries it, which is a
        // control rather than the solid.
        object.tag = at.map(|of| names.tag(Part::Solid { of, face }));
    });
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
///
/// The indices are reserved for exactly and the corners are not, and the
/// asymmetry is the iterators': flattening triangles hides the count, where an
/// `extend` over corners carries its own.
fn remesh(mesh: &mut Mesh, corners: impl Iterator<Item = Vertex>, triangles: &[[u32; 3]]) {
    mesh.rewrite(|vertices, indices| {
        vertices.clear();
        vertices.extend(corners);
        indices.clear();
        indices.reserve_exact(triangles.len() * 3);
        indices.extend(triangles.iter().flatten());
    });
}

#[cfg(test)]
mod tests;
