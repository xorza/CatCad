//! What a drawing looks like: turning one into the strokes, rims and markers a
//! renderer is handed.
//!
//! Everything here is a choice about appearance — which colour says how much
//! freedom is left, how wide an edge is, how far the drawing rides in front of
//! the solids — held apart from the model it is applied to so that neither has
//! to be read to change the other. It is also where the model's `f64` becomes
//! the renderer's `f32`, and the only place it does.

use aperture::{Batch, Curve, Point, Ring, Styled};
use glam::Vec3;
use silverpoint::Freedom;
use silverpoint::{Circle, CircleId, Segment, SegmentId};

use crate::drawing::Drawing;
use crate::named::{Named, Names};
use crate::preview::Ends;

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

/// The sketch's straight strokes, one edge per segment, biased clear of
/// the solids in depth so the drawing reads over them. Circles are not
/// strokes — see [`write_rings`].
pub(crate) fn write_curves(
    drawing: &Drawing,
    names: &mut Names,
    band: Option<Ends>,
    curves: &mut Batch<Curve>,
) {
    let sketch = drawing.sketch();
    let freedoms = drawing.freedoms();
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
                    let a = plane.point(sketch.point(edge.a)).as_vec3();
                    let b = plane.point(sketch.point(edge.b)).as_vec3();
                    // An edge is only as settled as its looser end: one end
                    // free to travel is an edge free to travel with it.
                    let freedom = freedoms.point(edge.a).max(freedoms.point(edge.b));
                    curve.set_segment(a, b);
                    curve.color = colour(freedom);
                    curve.tag = Some(names.tag(Named::Segment(id)));
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
pub(crate) fn write_points(drawing: &Drawing, names: &mut Names, points: &mut Batch<Point>) {
    let sketch = drawing.sketch();
    let freedoms = drawing.freedoms();
    let plane = drawing.plane();
    let normal = plane.normal().as_vec3();
    points.refill(sketch.points(), |point, (id, position)| {
        // Pinned by hand outranks pinned by consequence: a fixed point is
        // determined too, but saying so in the same colour would lose the
        // one thing about it the user chose.
        let (color, size) = if sketch.is_fixed(id) {
            (PINNED, FIXED_MARKER)
        } else {
            (colour(freedoms.point(id)), FREE_MARKER)
        };
        // Assigned whole where a stroke is edited in place: a marker owns
        // nothing, so replacing one costs what overwriting it would.
        *point = Point::new(plane.point(position).as_vec3())
            .colored(color)
            .size(size)
            .z_offset(MARKER_LIFT)
            .in_plane(normal)
            .tagged(names.tag(Named::Point(id)));
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
pub(crate) fn write_rings(
    drawing: &Drawing,
    names: &mut Names,
    band: Option<Ends>,
    rings: &mut Batch<Ring>,
) {
    let sketch = drawing.sketch();
    let freedoms = drawing.freedoms();
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
                    // A circle can move with its centre or grow on its own, so
                    // it is settled only when both are — the demo's is pinned
                    // to the middle of a rigid frame and still has its rim to
                    // give.
                    let freedom = freedoms.point(circle.center).max(freedoms.radius(id));
                    Ring::new(
                        plane.point(sketch.point(circle.center)).as_vec3(),
                        circle.radius.abs() as f32,
                        normal,
                    )
                    .colored(colour(freedom))
                    .tagged(names.tag(Named::Circle(id)))
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
