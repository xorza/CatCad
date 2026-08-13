//! Where a sketch sits in the world, and what it looks like once it's there.

use aperture::{Curve, Point, Ring};
use glam::{DVec2, Vec3};
use silverpoint::Sketch;

use crate::named::{Named, Names};

/// Marker diameters in logical pixels. A pinned point reads larger because it
/// is the one the drawing hangs off.
const FIXED_MARKER: f32 = 9.0;
const FREE_MARKER: f32 = 7.0;

/// Linear-RGB, unlit — these reach the target as authored.
const EDGE: Vec3 = Vec3::new(0.35, 0.55, 0.80);
const FREE_POINT: Vec3 = Vec3::new(0.85, 0.55, 0.10);
const FIXED_POINT: Vec3 = Vec3::new(0.80, 0.14, 0.05);

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

/// The plane a [`Sketch`] is drawn on: an origin, and the world directions its
/// two axes run along.
///
/// Sketch space is 2D and says nothing about where in the world it lives.
/// This is what answers that, and the only place the two coordinate systems
/// meet — silverpoint never learns its sketch was drawn, and aperture never
/// learns the curves it draws came from one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SketchPlane {
    pub origin: Vec3,
    /// World direction of the sketch's +x. Expected to be unit length.
    pub x: Vec3,
    /// World direction of the sketch's +y.
    pub y: Vec3,
}

impl SketchPlane {
    /// The world's horizontal plane through the origin, with the sketch's +y
    /// running to world −Z. Seen from above with +Y up, that reads the way the
    /// sketch was drawn, and anything modelled from it stands up in +Y.
    pub(crate) const GROUND: Self = Self {
        origin: Vec3::ZERO,
        x: Vec3::X,
        y: Vec3::NEG_Z,
    };

    /// Where a sketch point lands in the world.
    pub(crate) fn point(&self, point: DVec2) -> Vec3 {
        self.origin + self.x * point.x as f32 + self.y * point.y as f32
    }

    /// The plane's unit normal. Which face it points out of follows from the
    /// order of the axes and doesn't matter to anything that uses it.
    pub(crate) fn normal(&self) -> Vec3 {
        self.x.cross(self.y).normalize()
    }

    /// The sketch's straight strokes, one edge per segment, biased clear of
    /// the solids in depth so the drawing reads over them. Circles are not
    /// strokes — see [`SketchPlane::rings`].
    pub(crate) fn curves(&self, sketch: &Sketch, names: &mut Names) -> Vec<Curve> {
        let mut curves = Vec::new();
        for (id, segment) in sketch.segments() {
            let a = self.point(sketch.point(segment.a));
            let b = self.point(sketch.point(segment.b));
            curves.push(
                Curve::segment(a, b)
                    .colored(EDGE)
                    .width(EDGE_WIDTH)
                    .tagged(names.tag(Named::Segment(id))),
            );
        }
        // Applied here rather than at each constructor: the drawing rides on
        // one plane and above the solids as one thing, and nothing in it
        // outranks the rest.
        let normal = self.normal();
        for curve in &mut curves {
            curve.z_offset = STROKE_LIFT;
            curve.plane_normal = Some(normal);
        }
        curves
    }

    /// The sketch's points, one marker apiece — larger and pinned-coloured
    /// where the solver may not move it.
    ///
    /// The plane comes along for the same reason a stroke's does: a disc is
    /// flat in depth and the surface under it is not, so without it the glyph
    /// is sliced wherever the plane is seen at an angle.
    pub(crate) fn points(&self, sketch: &Sketch, names: &mut Names) -> Vec<Point> {
        let normal = self.normal();
        sketch
            .points()
            .map(|(id, position)| {
                let fixed = sketch.is_fixed(id);
                let (color, size) = if fixed {
                    (FIXED_POINT, FIXED_MARKER)
                } else {
                    (FREE_POINT, FREE_MARKER)
                };
                Point::new(self.point(position))
                    .colored(color)
                    .size(size)
                    .z_offset(MARKER_LIFT)
                    .in_plane(normal)
                    .tagged(names.tag(Named::Point(id)))
            })
            .collect()
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
    pub(crate) fn rings(&self, sketch: &Sketch, names: &mut Names) -> Vec<Ring> {
        let normal = self.normal();
        sketch
            .circles()
            .map(|(id, circle)| {
                Ring::new(
                    self.point(sketch.point(circle.center)),
                    circle.radius.abs() as f32,
                    normal,
                )
                .colored(EDGE)
                .width(EDGE_WIDTH)
                .z_offset(STROKE_LIFT)
                .tagged(names.tag(Named::Circle(id)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
