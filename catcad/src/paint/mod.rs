//! What a drawing looks like: turning one into the strokes, rims and markers a
//! renderer is handed.
//!
//! Everything here is a choice about appearance — which colour says how much
//! freedom is left, how wide an edge is, how far the drawing rides in front of
//! the solids — held apart from the model it is applied to so that neither has
//! to be read to change the other. It is also where the model's `f64` becomes
//! the renderer's `f32`, and the only place it does.

use aperture::{Batch, Curve, Object, Point, Ring, Scene, Styled, Text};
use glam::{Vec2, Vec3};
use palantir::{FontFamily, FontWeight, GlyphFont};
use silverpoint::{Circle, CircleId, Constraint, Entity, Freedom, Segment, SegmentId};
use std::fmt::Write;

use crate::document::Document;
use crate::drawing::Drawing;
use crate::names::Names;
use crate::preview::{Ends, Preview};

/// Marker diameters in logical pixels. A pinned point reads larger because it
/// is the one the drawing hangs off.
const FIXED_MARKER: f32 = 9.0;
const FREE_MARKER: f32 = 7.0;

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

/// Logical pixels.
const EDGE_WIDTH: f32 = 1.6;

/// How far the strokes ride in front of the solids, in steps of depth-buffer
/// resolution. A sketch is what a model is derived from, so where the two share
/// a plane the drawing is the one that reads.
///
/// Needed at all — however exactly the renderer places the stroke — because
/// bit-identical results from two different vertex shaders are not something
/// WGSL promises, so a coplanar tie cannot be left to arithmetic.
///
/// Both ends of the range this can take are measured, and both are pinned by
/// tests. Under about 128 steps the tie starts going the wrong way and strokes
/// come back thinned; over about three million the drawing lifts clear of the
/// model and shows through solids standing in front of it. Reversed depth is
/// what opens that up to four decades — under the old convention the same two
/// bounds sat barely two apart.
const STROKE_LIFT: i32 = 512;

/// How far the markers ride in front of the strokes.
///
/// A point sits exactly on the end of every segment that meets it, so the two
/// arrive at the same depth — and markers are drawn last, where an equal depth
/// loses to whatever already wrote. Without a step between them a corner
/// marker is cut by the very edges it terminates.
///
/// The step is what matters, not the height: the drawing stacks solids, then
/// strokes, then the handles you grab. Doubling puts 512 steps of daylight
/// between the layers, which is four hundred times the odd ULP two shaders
/// disagree by and still three decades short of showing through the model.
const MARKER_LIFT: i32 = STROKE_LIFT * 2;

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
pub(super) fn mark_font() -> GlyphFont {
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

/// The whole picture of `document` as it stands — the solids it holds, its
/// drawing over them, and a name for every part that can be pointed at.
///
/// Where a scene comes from, and the only place one does. Hands back a fresh
/// scene rather than filling one the caller owns, which is the shape the cost
/// deserves: the meshes are copied across, and handing a renderer its objects
/// again has it upload them again. Anything wanting this every frame wants
/// [`redraw`] instead — and the two are shaped as differently as they are so
/// that reaching for the wrong one is a change of code rather than of nothing.
///
/// The solids are written here rather than by the document, so that a document
/// says what it holds and one module decides what all of it looks like.
pub(crate) fn scene(document: &Document, names: &mut Names) -> Scene {
    let mut scene = Scene::default();
    write_solids(document.solids(), &mut scene.objects);
    // No band. Nothing can be half-drawn in a document nobody has looked at yet.
    redraw(document.drawing(), names, None, &mut scene);
    scene
}

/// The solids as the renderer wants them, which is as they already are: a solid
/// is modelled rather than drawn, so unlike everything below there is no
/// appearance to decide about one.
fn write_solids(solids: &[Object], into: &mut Batch<Object>) {
    into.refill(solids, |object, solid| object.clone_from(solid));
}

/// Draw the whole of `drawing`, and `band` over it, naming each part into
/// `names`.
///
/// The half of a picture that moves. A drawing is edited and the solids beside
/// it are not, so this writes the three overlay batches and leaves `into.objects`
/// untouched — which is what keeps a drag from re-uploading every mesh in the
/// model, since a batch nobody wrote to reports nothing to upload.
///
/// Fills buffers rather than returning them, so a drag refills what the renderer
/// already holds instead of handing it new vectors every frame. The tags come
/// out the same across a rewrite, because they are positions in a list built in
/// the same order — which is what lets a drag keep hold of what it grabbed.
///
/// `names` is the caller's, not the drawing's. A tag is an index into a list of
/// what was drawn, so it describes a *layout* of a drawing and not the drawing
/// itself — nothing here would be written down by saving, and whoever laid the
/// drawing out is who has to be able to read its tags back. Emptied here rather
/// than by the caller, because a name list half from one layout and half from
/// another names nothing.
///
/// `band` is what a tool is half-way through and the drawing knows nothing
/// about — which is why this is here rather than on [`Drawing`]. It is written
/// among the strokes and rims and never named, so it cannot be picked; see the
/// two writers below.
pub(crate) fn redraw(
    drawing: &Drawing,
    names: &mut Names,
    band: Option<Preview>,
    into: &mut Scene,
) {
    names.clear();
    write_curves(
        drawing,
        names,
        band.and_then(Preview::line),
        &mut into.curves,
    );
    write_rings(
        drawing,
        names,
        band.and_then(Preview::ring),
        &mut into.rings,
    );
    write_points(drawing, names, &mut into.points);
    write_marks(drawing, names, &mut into.texts);
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
fn write_marks(drawing: &Drawing, names: &mut Names, marks: &mut Batch<Text>) {
    let outcome = drawing.outcome();
    marks.refill(drawing.sketch().constraints(), |mark, (id, constraint)| {
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
        mark.position = drawing.mark_at(constraint);
        mark.font = mark_font();
        // Above the middle of what it names, so the mark clears the geometry it
        // is about rather than sitting on top of it.
        mark.anchor = Vec2::new(0.5, 1.6);
        mark.color = if outcome.is_redundant(id) {
            REDUNDANT
        } else {
            MARK
        };
        mark.z_offset = MARKER_LIFT;
        mark.plane_normal = Some(drawing.plane().normal().as_vec3());
        mark.tag = Some(names.tag(Entity::Constraint(id)));
    });
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
    }
}

/// The sketch's straight strokes, one edge per segment, biased clear of
/// the solids in depth so the drawing reads over them. Circles are not
/// strokes — see [`write_rings`].
fn write_curves(
    drawing: &Drawing,
    names: &mut Names,
    band: Option<Ends>,
    curves: &mut Batch<Curve>,
) {
    let sketch = drawing.sketch();
    let outcome = drawing.outcome();
    let plane = drawing.plane();
    // The drawing rides on one plane and above the solids as one thing, and
    // nothing in it outranks the rest — so the bias and the plane are the same
    // for every stroke.
    let normal = plane.normal().as_vec3();
    // Written over the strokes already there rather than into fresh ones, which
    // for a `Curve` is the difference between a frame that reaches the heap and
    // one that does not — see `Batch::refill`. The band is chained on rather
    // than pushed after for the same reason: appended, it would be dropped by
    // the next rewrite of the drawing and allocated afresh by the one after,
    // once a frame for as long as a line is being drawn.
    curves.refill(
        sketch
            .segments()
            .map(|(id, edge)| Stroke::Edge(id, edge))
            .chain(band.map(Stroke::Band)),
        |curve, stroke| {
            curve.width = EDGE_WIDTH;
            curve.z_offset = STROKE_LIFT;
            curve.plane_normal = Some(normal);
            match stroke {
                Stroke::Edge(id, edge) => {
                    let a = plane.point(sketch.point(edge.a).position).as_vec3();
                    let b = plane.point(sketch.point(edge.b).position).as_vec3();
                    let freedom = outcome.segment(id);
                    curve.set_segment(a, b);
                    curve.color = colour(freedom);
                    curve.tag = Some(names.tag(Entity::Segment(id)));
                }
                // Untagged, which is what keeps the band out of the way: a pick
                // skips a primitive with no tag, so it cannot be hovered,
                // grabbed or picked out, and the click that finishes the line
                // resolves against the geometry behind it.
                Stroke::Band(ends) => {
                    curve.set_segment(ends.from, ends.to);
                    curve.color = GHOST;
                    curve.tag = None;
                }
            }
        },
    );
}

/// One stroke to write: an edge the sketch holds, or the band a tool is in the
/// middle of drawing.
#[derive(Debug)]
enum Stroke {
    Edge(SegmentId, Segment),
    Band(Ends),
}

/// The sketch's points, one marker apiece — larger and pinned-coloured
/// where the solver may not move it.
///
/// The plane comes along for the same reason a stroke's does: a disc is
/// flat in depth and the surface under it is not, so without it the glyph
/// is sliced wherever the plane is seen at an angle.
fn write_points(drawing: &Drawing, names: &mut Names, points: &mut Batch<Point>) {
    let sketch = drawing.sketch();
    let outcome = drawing.outcome();
    let plane = drawing.plane();
    let normal = plane.normal().as_vec3();
    points.refill(sketch.points(), |marker, (id, point)| {
        // Pinned by hand outranks pinned by consequence: a fixed point is
        // determined too, but saying so in the same colour would lose the
        // one thing about it the user chose.
        let (color, size) = if point.fixed {
            (PINNED, FIXED_MARKER)
        } else {
            (colour(outcome.point(id)), FREE_MARKER)
        };
        // Assigned whole where a stroke is edited in place: a marker owns
        // nothing, so replacing one costs what overwriting it would.
        *marker = Point::new(plane.point(point.position).as_vec3())
            .colored(color)
            .size(size)
            .z_offset(MARKER_LIFT)
            .in_plane(normal)
            .tagged(names.tag(Entity::Point(id)));
    });
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
fn write_rings(drawing: &Drawing, names: &mut Names, band: Option<Ends>, rings: &mut Batch<Ring>) {
    let sketch = drawing.sketch();
    let outcome = drawing.outcome();
    let plane = drawing.plane();
    let normal = plane.normal().as_vec3();
    rings.refill(
        sketch
            .circles()
            .map(|(id, circle)| Rim::Circle(id, circle))
            .chain(band.map(Rim::Band)),
        |ring, rim| {
            // Assigned whole, like a marker and unlike a stroke: a rim owns
            // nothing either.
            *ring = match rim {
                Rim::Circle(id, circle) => {
                    let freedom = outcome.circle(id);
                    Ring::new(
                        plane.point(sketch.point(circle.center).position).as_vec3(),
                        circle.radius.abs() as f32,
                        normal,
                    )
                    .colored(colour(freedom))
                    .tagged(names.tag(Entity::Circle(id)))
                }
                // Through the cursor rather than out to it: the second click
                // says how big by naming somewhere on the rim. Untagged, like
                // the band among the strokes.
                Rim::Band(ends) => {
                    Ring::new(ends.from, ends.from.distance(ends.to), normal).colored(GHOST)
                }
            }
            .width(EDGE_WIDTH)
            .z_offset(STROKE_LIFT);
        },
    );
}

/// One rim to write: a circle the sketch holds, or the band a tool is in the
/// middle of drawing.
#[derive(Debug)]
enum Rim {
    Circle(CircleId, Circle),
    Band(Ends),
}

#[cfg(test)]
mod tests;
