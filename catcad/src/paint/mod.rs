//! What a drawing looks like: turning one into the strokes, rims and markers a
//! renderer is handed.
//!
//! Everything here is a choice about appearance — which colour says how much
//! freedom is left, how wide an edge is, how far the drawing rides in front of
//! the solids — held apart from the model it is applied to so that neither has
//! to be read to change the other. It is also where the model's `f64` becomes
//! the renderer's `f32`, and the only place it does.

use aperture::{Batch, Curve, Mesh, Object, Point, Precedence, Ring, Scene, Styled, Text, Vertex};
use glam::{DVec2, Mat4, Vec2, Vec3};
use palantir::{FontFamily, FontWeight, GlyphFont};
use silverpoint::{Circle, CircleId, Constraint, Freedom, Prism, Segment, SegmentId};
use std::fmt::Write;

use crate::model::{Model, Models};
use crate::names::Names;
use crate::paint::layout::{Layout, Made, Sheets};
use crate::part::Part;
use crate::preview::{Ends, Preview};
use crate::timeline::FeatureId;

pub(crate) mod layout;
mod shape;

/// Marker diameters in logical pixels. A pinned point reads larger because it
/// is the one the drawing hangs off.
const FIXED_MARKER: f32 = 9.0;
const FREE_MARKER: f32 = 7.0;

/// How far a face's edge may sit from the curve it was cut from, in sketch
/// units.
///
/// A face is flattened once, when the drawing moves, rather than again whenever
/// the camera does — so this is chosen for the drawing rather than for the
/// zoom. At the size a sketch is worked at it puts forty-odd corners around a
/// rim, which reads round without giving the triangulation a hundred to chew.
const FACE_SAGITTA: f64 = 0.005;

/// The same, for the walls and ends of a solid.
///
/// Its own number rather than [`FACE_SAGITTA`] reused, because the two are read
/// at different sizes: a region is a flat sheet seen against the plane it lies
/// in, and a solid is turned about and lit, so a wall that is a shade too coarse
/// shows up as banding across a shaded surface where the same coarseness on a
/// flat fill shows up as nothing at all.
const SOLID_SAGITTA: f64 = 0.002;

/// What a solid the document has grown is shaded in.
///
/// Warm grey, and the one colour here that is not about state. Everything a
/// *drawing* is painted in says how much freedom the constraints have left it;
/// a solid has no freedom to report — it is what a feature made, and either it
/// is there or its profile was lost — so it reads as material rather than as a
/// thing with something left to decide.
const SOLID: Vec3 = Vec3::new(0.62, 0.60, 0.56);

/// Linear-RGB, unlit — these reach the target as authored.
///
/// Geometry is coloured by how much freedom its constraints have left it, cool
/// for none and warm for all of it, so a sketch starts hot and cools as it is
/// pinned down — which is the convention every constrained modeller draws on,
/// and reads at a glance as how much work the drawing still needs.
///
/// A point the user pinned by hand keeps its own colour regardless. It is
/// determined, but by a different authority, and the two are worth telling
/// apart: constraints can be argued with by adding more, and `fix` cannot.
const DETERMINED: Vec3 = Vec3::new(0.35, 0.55, 0.80);
const PARTLY: Vec3 = Vec3::new(0.85, 0.74, 0.20);
const FREE: Vec3 = Vec3::new(0.88, 0.50, 0.10);
const PINNED: Vec3 = Vec3::new(0.80, 0.14, 0.05);

/// What a datum's two axis arrows are drawn in.
///
/// The convention every modeller shares — x red, y green — which is worth
/// having because it is the one thing about a gizmo a user already knows.
///
/// It collides, and knowingly: [`PINNED`] is a red and [`FREE`] is close to it,
/// so a red arrow and a pinned point are two things saying different things in
/// one hue. What keeps them apart for now is that they are never the same
/// *shape* — an axis is a great flat arrow and a pinned point is a small disc —
/// and both of these are muted well below the drawing's own, so they read as
/// chrome rather than as state. A palette is where this gets settled properly.
const AXIS_X: Vec3 = Vec3::new(0.62, 0.20, 0.18);
const AXIS_Y: Vec3 = Vec3::new(0.24, 0.52, 0.24);

/// What the square joining the two is drawn in.
///
/// Neither hue, because it belongs to neither axis — it is the corner they make
/// rather than a third direction, and giving it one of theirs would say it was.
const AXIS_CORNER: Vec3 = Vec3::new(0.50, 0.52, 0.55);

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
const DORMANT: Vec3 = Vec3::new(0.42, 0.45, 0.50);

/// What a face of one is filled with — the same step down from [`FACE`].
const DORMANT_FACE: Vec3 = Vec3::new(0.11, 0.20, 0.29);

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
const FACE: Vec3 = Vec3::new(0.18, 0.32, 0.46);

/// What a shape still being drawn is drawn in — a grey that belongs to none of
/// the states above, because a rubber band has no freedom to report: it is not
/// geometry yet, and the constraints have not been asked about it.
const GHOST: Vec3 = Vec3::new(0.72, 0.74, 0.78);

/// What geometry with this much freedom left is drawn in.
fn colour(freedom: Freedom) -> Vec3 {
    match freedom {
        Freedom::Determined => DETERMINED,
        Freedom::Partly => PARTLY,
        Freedom::Free => FREE,
    }
}

/// `lit` where `model` is the sketch being edited, and ground where it is not.
///
/// The one place the difference is made, so every kind of mark in a dormant
/// sketch is dimmed by the same rule rather than each writer having its own
/// idea of what "not here" looks like.
fn ink(model: Model<'_>, lit: Vec3) -> Vec3 {
    if model.live() { lit } else { DORMANT }
}

/// Where a sketch's marks stand in the competition for a click.
///
/// The same branch as [`ink`] beside it, and for the same reason: a sketch
/// nobody is working in is drawn to be read rather than aimed at, so a click
/// that lands on it and on the open sketch at once was meant for the open one.
fn standing(model: Model<'_>) -> Precedence {
    if model.live() {
        Precedence::Shaped
    } else {
        Precedence::Aside
    }
}

/// How wide a sketch stroke is drawn, in logical pixels.
///
/// Not [`aperture::Curve`]'s own default, which is narrower: a drawing is read
/// at a glance against a shaded model behind it and wants a little more weight
/// than a bare overlay does. Every stroke and every rim is set to this, so the
/// default is never seen — and the visual suite measures against it through
/// `internals`, rather than keeping a second opinion about what it should be.
pub(super) const EDGE_WIDTH: f32 = 1.6;

/// Type size of a constraint's mark, in logical pixels. Small: a drawing may
/// carry dozens, and what they have to be is legible rather than prominent.
const MARK_SIZE: f32 = 13.0;

/// What a mark is set in.
///
/// Mono and bold, which are two claims about legibility rather than about
/// style. A mark is one character read at a glance against a drawing behind it,
/// so it wants the weight to hold its own over a stroke it may be sitting on —
/// and the mono face is drawn on a fixed body, so ⊥ and ∥ and ∈ come out the
/// same size as each other instead of set to the widths a running line would
/// want.
///
/// Named rather than written where it is used, so that
/// `every_mark_has_a_glyph_to_draw_it` asks about the faces the drawing
/// actually sets marks in. A coverage check against a font nobody uses would
/// pass while the drawing showed nothing.
pub(crate) fn mark_font() -> GlyphFont {
    GlyphFont {
        family: FontFamily::Mono,
        weight: FontWeight::Bold,
        ..GlyphFont::new(MARK_SIZE)
    }
}

/// What a mark is drawn in.
///
/// Grey-violet, which is the one hue the drawing does not already spend:
/// geometry runs blue through yellow to orange for how much freedom is left,
/// red for pinned, and green for what is picked out. A mark is *about* the
/// geometry rather than part of it, and reads as a different kind of thing for
/// being a different kind of colour.
const MARK: Vec3 = Vec3::new(0.62, 0.58, 0.78);

/// What a mark the constraints could do without is drawn in.
///
/// The one thing a drawing can say that a count in the corner cannot: *this*
/// relation is the spare one. Red, because it is the same news as a conflict —
/// and on a sketch whose constraints disagree, it is exactly the mark to delete.
const REDUNDANT: Vec3 = Vec3::new(0.90, 0.30, 0.25);

/// The whole picture of a document as it stands — the `solids` standing around
/// it, its drawing over them, and a name for every part that can be pointed at.
///
/// Where a scene comes from, and the only place one does. Hands back a fresh
/// scene rather than filling one the caller owns, which is the shape the cost
/// deserves: the meshes are copied across, and handing a renderer its objects
/// again has it upload them again. Anything wanting this every frame wants
/// [`redraw`] instead — and the two are shaped as differently as they are so
/// that reaching for the wrong one is a change of code rather than of nothing.
///
/// The solids arrive beside the model rather than out of it, because no step
/// made them — see [`demo::scenery`](crate::demo::scenery). That is also why
/// they are written once here and never by [`redraw`]: what the drawing says
/// changes sixty times a second, and what stands around it does not change at
/// all.
pub(crate) fn scene(models: Models<'_>, layout: &mut Layout) -> Scene {
    let mut scene = Scene::default();
    // No band and nothing being retyped. Nothing can be half-drawn or half-typed
    // in a document nobody has looked at yet.
    redraw(models, layout, None, None, None, &mut scene);
    scene
}

/// An object per face of every solid the document has grown.
///
/// One object per *face* rather than one per solid, which is what makes a solid
/// something you can point at: a tag names a primitive, so a face that is to be
/// hovered, picked out and later built on has to be a primitive of its own.
///
/// Named by what each face was grown from — see [`Grown`] — rather than by where
/// it fell in this frame's list. That is the same durable vocabulary the region
/// underneath was named in, so a selection survives the drawing moving under it
/// exactly as a sketch entity's does.
///
/// Modelled rather than drawn, so unlike everything else here there is no
/// appearance to decide beyond the one colour: what a solid *is* is the shape,
/// and shading it is the renderer's.
fn write_solids(
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
                    color: Vec3::ONE,
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

/// A solid being decided: a region, and how deep it currently reads.
///
/// What a form asking for a depth hands the drawing, so the solid is on screen
/// from the moment it is asked for. A [`Prism`](silverpoint::Prism) is a
/// reading rather than a thing the document holds — an arrangement, a region, a
/// plane and a distance — so all of this is drawable without a step existing,
/// and cancelling the form leaves the timeline never having heard of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Growing {
    pub(crate) sketch: FeatureId,
    pub(crate) region: usize,
    pub(crate) distance: f64,
}

impl Growing {
    /// What it currently reads as, or `None` where the sketch no longer holds
    /// the region it names.
    fn prism(self, models: Models<'_>) -> Option<Prism<'_>> {
        let model = models.at(self.sketch)?;
        let arrangement = model.arrangement();
        (self.region < arrangement.faces().len())
            .then(|| Prism::new(arrangement, self.region, model.plane(), self.distance))
    }
}

/// Two arrows per datum, from its origin along each of its axes.
///
/// Cut here rather than shaped by a renderer, which is what lets a control be
/// ordinary world geometry: an arrow lying in its plane is a polygon in that
/// plane's coordinates, and mapping seven of them through
/// [`Plane::point`](silverpoint::Plane::point) is the whole of the work. It
/// foreshortens as the plane turns and vanishes edge-on because it genuinely
/// lies there, rather than because a shader was told to pretend.
///
/// Named as the plane itself, both of them: two tags reporting one
/// [`Part`] is what [`Names`] is already built to allow — a tag is a position
/// in a list and nothing assumes the list holds each part once — so grabbing
/// either axis grabs the datum, and lighting the datum lights both.
///
/// The whole of how a datum is drawn, since the outline around what was
/// sketched on it is gone: a plane's origin and the way its axes run are facts
/// about the plane, where a box around the drawing was a fact about the
/// drawing, restated in furniture.
///
/// Standing as a frame, like the outline around it: what a datum is *for* has
/// not changed by being drawn differently, and it still yields a click to the
/// geometry drawn on it.
fn write_gizmos(models: Models<'_>, names: &mut Names, gizmos: &mut Batch<Object>) {
    gizmos.refill(models.planes(), |object, (at, plane)| {
        let normal = plane.normal().as_vec3();
        let mesh = &mut object.mesh;
        // Written over what is there rather than through [`remesh`], which
        // takes one shape: this is four, and each has to be rebased onto the
        // corners already written. Cleared rather than replaced all the same,
        // so a plane that moves costs no allocation.
        mesh.vertices.clear();
        mesh.indices.clear();
        let mut piece = |corners: &[DVec2], triangles: &[[u32; 3]], ink: Vec3| {
            let base = mesh.vertices.len() as u32;
            mesh.vertices.extend(corners.iter().map(|&at| Vertex {
                position: plane.point(at).as_vec3(),
                normal,
                color: ink,
            }));
            mesh.indices
                .extend(triangles.iter().flatten().map(|corner| base + corner));
        };
        piece(&shape::hub(), &shape::SQUARE_TRIANGLES, AXIS_CORNER);
        piece(&shape::arrow(DVec2::X), &shape::ARROW_TRIANGLES, AXIS_X);
        piece(&shape::arrow(DVec2::Y), &shape::ARROW_TRIANGLES, AXIS_Y);
        piece(&shape::corner(), &shape::SQUARE_TRIANGLES, AXIS_CORNER);

        object.transform = Mat4::IDENTITY;
        // White, so each corner's own colour is what lands — see
        // [`Vertex::color`](aperture::Vertex). What the object's colour is still
        // for is a highlight: a `Tint::Ink` replaces it and flattens the gizmo
        // to one colour, where a `Tint::Lift` multiplies and leaves the axes
        // telling themselves apart.
        object.color = Vec3::ONE;
        object.precedence = Precedence::Frame;
        object.tag = Some(names.tag(Part::Plane(at)));
    });
}

/// Draw the whole of `model`, and `band` over it, into the room `layout` keeps
/// and under the names it holds.
///
/// The half of a picture that moves. A drawing is edited and the solids beside
/// it are not, so this rewrites what the drawing is made of — the four overlay
/// batches, and the sheets its curves enclose — and leaves `into.solids`
/// untouched, which is what keeps a drag from re-uploading every mesh in the
/// model: a batch nobody wrote to reports nothing to upload.
///
/// Fills buffers rather than returning them, so a drag refills what the renderer
/// already holds instead of handing it new vectors every frame. The tags come
/// out the same across a rewrite, because they are positions in a list built in
/// the same order — which is what lets a drag keep hold of what it grabbed.
///
/// `layout` is the caller's, not the drawing's. A tag is an index into a list
/// of what was drawn, so it describes a *layout* of a drawing and not the
/// drawing itself — nothing in one would be written down by saving, and whoever
/// laid the drawing out is who has to be able to read its tags back. Its names
/// are emptied here rather than by the caller, because a name list half from
/// one layout and half from another names nothing.
///
/// `band` is what a tool is half-way through and the drawing knows nothing
/// about — which is why this is here rather than on
/// [`Drawing`](crate::drawing::Drawing). It is written among the strokes and
/// rims and never named, so it cannot be picked; see the two writers below.
///
/// Does nothing where the layout already describes what it would draw, and says
/// so on the layout when it has drawn — so a caller settles a frame by calling
/// this and reading nothing back.
pub(crate) fn redraw(
    models: Models<'_>,
    layout: &mut Layout,
    band: Option<Preview>,
    typed: Option<Part>,
    growing: Option<Growing>,
    into: &mut Scene,
) {
    // The check is here rather than at the call, so that what a layout claims
    // to describe and what was drawn into it are decided in one place. A caller
    // that skipped the call would leave a stale picture; one that made it and
    // forgot to stamp would redraw for ever.
    let made = Made {
        revision: models.revision(),
        editing: models.editing(),
        band,
        typed,
        growing,
    };
    if !layout.stale(made) {
        return;
    }
    let Layout { names, sheets, .. } = &mut *layout;
    names.clear();
    write_curves(
        models,
        names,
        band.and_then(Preview::line),
        &mut into.curves,
    );
    write_rings(models, names, band.and_then(Preview::ring), &mut into.rings);
    write_points(models, names, &mut into.points);
    write_marks(models, typed, names, &mut into.texts);
    write_faces(models, names, sheets, &mut into.faces);
    write_solids(models, names, sheets, growing, &mut into.solids);
    write_gizmos(models, names, &mut into.gizmos);
    layout.drawn(made);
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
/// one — see [`write_gizmos`].
///
/// Written over what is already there rather than assigned, which is what keeps
/// a drag off the heap: every face of a drawing and every face of every solid is
/// cut afresh whenever the document moves, and they come back the same size.
///
/// The indices are reserved for exactly and the corners are not, and the
/// asymmetry is the iterators': flattening triangles hides the count, where an
/// `extend` over corners carries its own.
fn remesh(mesh: &mut Mesh, corners: impl Iterator<Item = Vertex>, triangles: &[[u32; 3]]) {
    mesh.vertices.clear();
    mesh.vertices.extend(corners);
    mesh.indices.clear();
    mesh.indices.reserve_exact(triangles.len() * 3);
    mesh.indices.extend(triangles.iter().flatten());
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
fn write_faces(
    models: Models<'_>,
    names: &mut Names,
    sheets: &mut Sheets,
    faces: &mut Batch<Object>,
) {
    let Sheets { filler, fill, .. } = sheets;
    faces.refill(
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
                    color: Vec3::ONE,
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

/// Where the region at `region` of `sketch` lies in the world, as the corners
/// its fill was cut from.
///
/// What a form standing *beside* a region is placed against — see
/// [`Stands`](crate::prompt::Stands). The fill rather than the boundary curves,
/// because that is the shape the region actually covers: a crescent's bounding
/// box taken off its two arcs' endpoints would sit half outside it.
///
/// Cut through the same [`Filler`] the sheets are, so what a form stands clear
/// of is exactly what is drawn.
pub(crate) fn region_corners(
    models: Models<'_>,
    sheets: &mut Sheets,
    sketch: FeatureId,
    region: usize,
    into: &mut Vec<Vec3>,
) {
    into.clear();
    let Sheets { filler, fill, .. } = sheets;
    let Some(model) = models.at(sketch) else {
        return;
    };
    let arrangement = model.arrangement();
    let Some(face) = arrangement.faces().get(region) else {
        return;
    };
    filler.fill(arrangement, face, FACE_SAGITTA, fill);
    let plane = model.plane();
    into.extend(fill.corners.iter().map(|&at| plane.point(at).as_vec3()));
}

/// A mark per constraint, saying what relation holds and where.
///
/// Set in type rather than drawn as geometry, which is what makes the whole set
/// one rule: every relation gets a symbol, the symbol is legible at any zoom
/// because it is sized in pixels, and adding a tenth constraint is a line in
/// [`symbol`] rather than a shape to construct.
///
/// Tagged like everything else, so a mark is picked and deleted the way the
/// geometry it is about is — which is the whole of how an over-constrained
/// sketch gets un-stuck.
fn write_marks(
    models: Models<'_>,
    typed: Option<Part>,
    names: &mut Names,
    marks: &mut Batch<Text>,
) {
    marks.refill(
        models
            .iter()
            // **The open sketch alone.** A constraint is a statement *about* a
            // drawing, and one you are not in is not a drawing you can argue
            // with: its marks can neither be selected into a relation nor typed
            // into, so all they do is crowd the sketch you are working in — and
            // a dimension is the densest thing the drawing puts on screen. The
            // geometry of a dormant sketch still shows, dimmed, because where it
            // *is* is something you build against.
            .filter(|model| model.live())
            .flat_map(|model| {
                model
                    .sketch()
                    .constraints()
                    .map(move |stated| (model, stated))
            })
            // The one being retyped has a field standing over it — see
            // [`Prompt::show`](crate::prompt::Prompt) — and a mark left under
            // one would be a second copy of the number showing through wherever
            // the field did not quite cover it.
            .filter(move |(model, (id, _))| Some(model.part(*id)) != typed),
        |mark, (model, (id, constraint))| {
            let outcome = model.outcome();
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
                    let unit = radius_prefix(constraint);
                    write!(mark.content, "{unit}{value:.*}", DECIMALS)
                        .expect("writing to a string cannot fail");
                }
                None => mark.content.push_str(symbol(constraint)),
            }
            mark.position = model.drawing().mark_at(constraint);
            mark.font = mark_font();
            // Above the middle of what it names, so the mark clears the geometry it
            // is about rather than sitting on top of it.
            mark.anchor = MARK_ANCHOR;
            mark.color = ink(
                model,
                if outcome.is_redundant(id) {
                    REDUNDANT
                } else {
                    MARK
                },
            );
            mark.precedence = standing(model);
            mark.plane_normal = Some(model.plane().normal().as_vec3());
            mark.tag = Some(names.tag(model.part(id)));
        },
    );
}

/// Where the dimension's own position sits in the box of its number: centred
/// across, and better than a box-height above, so the mark clears the geometry
/// it is about rather than sitting on top of it.
///
/// Read by the field as well as by the mark, and that is the whole reason it is
/// a constant: the field is drawn *instead of* the mark and must land on the
/// same pixels, so the two anchoring differently is the one mistake that would
/// make a double-click look like it had nudged the number.
const MARK_ANCHOR: Vec2 = Vec2::new(0.5, 1.6);

/// How far above the point it names a dimension's number is drawn, in logical
/// pixels.
///
/// [`MARK_ANCHOR`] as a length rather than a fraction, which is what anything
/// placing something *else* over the number needs — the field that stands in
/// for it while it is retyped, and the tests that aim at one. Named here so
/// that a caller working out where the number is cannot come to disagree with
/// the call that draws it.
pub(crate) fn mark_lift() -> f32 {
    MARK_ANCHOR.y * mark_font().line_height_px
}

/// Decimal places a dimension is read out to.
///
/// Two, which is a hundredth of a sketch unit — fine enough to draw with and
/// coarse enough that a solve's own drift never shows. What a *unit* is remains
/// the document's to decide; until it decides, a number is a number.
pub(crate) const DECIMALS: usize = 2;

/// What a dimension's number is prefixed with, where the kind is not obvious
/// from where it sits.
///
/// A radius is the one that needs it: a bare number beside a circle would read
/// as a diameter to half of everyone, and `R` is what a drawing puts there. A
/// distance needs nothing — it is written along the span it measures.
fn radius_prefix(constraint: Constraint) -> &'static str {
    match constraint {
        Constraint::Radius { .. } => "R",
        _ => "",
    }
}

/// The symbol a relation is drawn as.
///
/// The draughtsman's marks where there is one, because a drawing is read at a
/// glance and a word is not: ⊥ and ∥ say what they mean to anyone who has seen
/// a technical drawing, and are what every modeller uses. Every symbol here was
/// checked to have a glyph in the faces the shaper falls back through — see
/// `every_mark_has_a_glyph_to_draw_it`.
fn symbol(constraint: Constraint) -> &'static str {
    match constraint {
        // A coincidence makes two points one, so it is drawn as the one.
        Constraint::Coincident { .. } => "\u{2022}",
        Constraint::Distance { .. } => "\u{2194}",
        Constraint::Horizontal { .. } => "\u{2015}",
        Constraint::Vertical { .. } => "\u{2502}",
        Constraint::Parallel { .. } => "\u{2225}",
        Constraint::Perpendicular { .. } => "\u{22A5}",
        // "is on", which is the same relation whether what it is on is straight
        // or curved.
        Constraint::PointOnSegment { .. } | Constraint::PointOnCircle { .. } => "\u{2208}",
        Constraint::Radius { .. } => "R",
        // Equal length and equal radius are one mark for the same reason: what
        // the drawing has to say is that two things match, and which two is
        // plain from what the mark sits between.
        Constraint::EqualLength { .. } | Constraint::EqualRadius { .. } => "=",
        // A letter, like the radius above it. Tangency has no draughtsman's
        // mark that carries into a font — the drawings that need one letter it
        // too.
        Constraint::Tangent { .. } => "T",
    }
}

/// The sketch's straight strokes, one edge per segment, biased clear of
/// the solids in depth so the drawing reads over them. Circles are not
/// strokes — see [`write_rings`].
fn write_curves(
    models: Models<'_>,
    names: &mut Names,
    band: Option<Ends>,
    curves: &mut Batch<Curve>,
) {
    // The band belongs to the sketch being drawn in, which is the one plane it
    // could lie in: a tool draws where you are, not where you are not. Worked
    // out only where there is one, since most frames have none.
    let banding = band.map(|_| models.open().plane().normal().as_vec3());
    // Written over the strokes already there rather than into fresh ones, which
    // for a `Curve` is the difference between a frame that reaches the heap and
    // one that does not — see `Batch::refill`. The band is chained on rather
    // than pushed after for the same reason: appended, it would be dropped by
    // the next rewrite of the drawing and allocated afresh by the one after,
    // once a frame for as long as a line is being drawn.
    curves.refill(
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
                Stroke::Band(ends) => {
                    curve.set_segment(ends.from, ends.to);
                    curve.color = GHOST;
                    curve.precedence = Precedence::Shaped;
                    curve.plane_normal = banding;
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
    Band(Ends),
}

/// The sketch's points, one marker apiece — larger and pinned-coloured
/// where the solver may not move it.
///
/// The plane comes along for the same reason a stroke's does: a disc is
/// flat in depth and the surface under it is not, so without it the glyph
/// is sliced wherever the plane is seen at an angle.
fn write_points(models: Models<'_>, names: &mut Names, points: &mut Batch<Point>) {
    points.refill(
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

/// The sketch's circles, one ring apiece.
///
/// Not tessellated into strokes: the count that looks round depends on how
/// large the circle lands on screen, and the renderer resolves a ring in
/// the fragment stage instead, which is round at every zoom and needs no
/// rebuilding when the camera moves.
///
/// No plane named, unlike the strokes — a ring's band is widened in its
/// own plane, so the depth it carries is already the surface's.
fn write_rings(models: Models<'_>, names: &mut Names, band: Option<Ends>, rings: &mut Batch<Ring>) {
    // The band's plane is the open sketch's, exactly as among the strokes.
    let banding = band.map(|_| models.open().plane().normal().as_vec3());
    rings.refill(
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
                Rim::Band(ends) => Ring::new(
                    ends.from,
                    ends.from.distance(ends.to),
                    banding.expect("a band is drawn only where there is one"),
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
    Band(Ends),
}

#[cfg(test)]
mod tests;
