//! Where a sketch sits in the world, and what it looks like once it's there.

use aperture::{Curve, Point};
use glam::{DVec2, Vec3};
use silverpoint::Sketch;

/// Straight segments per circle. A circle is the only sketch entity that
/// isn't already straight, and this is what it costs to look round.
const CIRCLE_SEGMENTS: usize = 96;

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

    /// The sketch's strokes: an edge per segment and a tessellated circle per
    /// circle, biased clear of the solids in depth so the drawing reads over
    /// them.
    pub(crate) fn curves(&self, sketch: &Sketch) -> Vec<Curve> {
        let mut curves = Vec::new();
        for (_, segment) in sketch.segments() {
            let a = self.point(sketch.point(segment.a));
            let b = self.point(sketch.point(segment.b));
            curves.push(Curve::segment(a, b).colored(EDGE).width(EDGE_WIDTH));
        }
        for (_, circle) in sketch.circles() {
            curves.push(self.circle(sketch.point(circle.center), circle.radius.abs()));
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
    pub(crate) fn points(&self, sketch: &Sketch) -> Vec<Point> {
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
            })
            .collect()
    }

    fn circle(&self, centre: DVec2, radius: f64) -> Curve {
        let points = (0..CIRCLE_SEGMENTS)
            .map(|step| {
                let angle = step as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                self.point(centre + DVec2::new(cos, sin) * radius)
            })
            .collect();
        Curve::new(points).closed().colored(EDGE).width(EDGE_WIDTH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ground_plane_lays_sketch_y_along_negative_z() {
        let plane = SketchPlane::GROUND;
        assert_eq!(plane.point(DVec2::ZERO), Vec3::ZERO);
        assert_eq!(plane.point(DVec2::new(3.0, 0.0)), Vec3::new(3.0, 0.0, 0.0));
        // Sketch +y runs away from the camera, so the drawing lies flat
        // instead of standing up.
        assert_eq!(plane.point(DVec2::new(0.0, 2.0)), Vec3::new(0.0, 0.0, -2.0));
        assert_eq!(
            plane.point(DVec2::new(-1.5, 4.0)),
            Vec3::new(-1.5, 0.0, -4.0)
        );

        // A plane elsewhere carries its sketch with it.
        let raised = SketchPlane {
            origin: Vec3::new(0.0, 5.0, 0.0),
            ..SketchPlane::GROUND
        };
        assert_eq!(
            raised.point(DVec2::new(1.0, 1.0)),
            Vec3::new(1.0, 5.0, -1.0)
        );
    }

    #[test]
    fn every_entity_becomes_a_curve() {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::ZERO);
        let b = sketch.add_point(DVec2::new(10.0, 0.0));
        sketch.fix(a);
        sketch.add_segment(a, b);
        sketch.add_circle(b, 2.0);

        // One edge and one circle. Markers are no longer strokes.
        let curves = SketchPlane::GROUND.curves(&sketch);
        assert_eq!(curves.len(), 2);

        // Every last stroke rides in front of the solids, and names the plane
        // it lies in so the renderer can take its depth off the surface rather
        // than off the centreline. The ground plane's axes are +X and −Z,
        // which face +Y.
        assert!(curves.iter().all(|curve| curve.z_offset == STROKE_LIFT));
        assert!(
            curves
                .iter()
                .all(|curve| curve.plane_normal == Some(Vec3::Y)),
            "the ground plane faces +Y"
        );

        let edge = &curves[0];
        assert_eq!(edge.points, [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)]);
        assert!(!edge.closed);

        let circle = &curves[1];
        assert_eq!(circle.points.len(), CIRCLE_SEGMENTS);
        assert!(circle.closed, "an open circle would leave a gap");
        let centre = Vec3::new(10.0, 0.0, 0.0);
        for point in &circle.points {
            assert!((point.distance(centre) - 2.0).abs() < 1e-5, "{point:?}");
            assert_eq!(point.y, 0.0, "the circle stays in the plane");
        }
        // It starts at angle zero and runs the way the sketch does.
        assert!(circle.points[0].abs_diff_eq(Vec3::new(12.0, 0.0, 0.0), 1e-5));
    }

    #[test]
    fn every_sketch_point_gets_a_marker_the_zoom_cannot_reach() {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::ZERO);
        let b = sketch.add_point(DVec2::new(10.0, 0.0));
        sketch.fix(a);

        let points = SketchPlane::GROUND.points(&sketch);
        assert_eq!(points.len(), 2);
        // Above the strokes, not merely above the solids: a marker lands on
        // the end of the segments meeting it, and is drawn after them.
        assert!(points.iter().all(|point| point.z_offset == MARKER_LIFT));

        // Pinned reads larger and in its own colour; free is the other way.
        let anchor = &points[0];
        assert_eq!(anchor.position, Vec3::ZERO);
        assert_eq!(anchor.color, FIXED_POINT);
        assert_eq!(anchor.size, FIXED_MARKER);

        let free = &points[1];
        assert_eq!(free.position, Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(free.color, FREE_POINT);
        assert_eq!(free.size, FREE_MARKER);
        assert!(free.size < anchor.size);

        let _ = b;
    }

    #[test]
    fn marker_size_ignores_how_big_the_drawing_is() {
        // The whole point of sizing in pixels: a drawing a hundred times the
        // size gets markers the same number of pixels across, where the old
        // model-space square grew with it and swallowed the sketch.
        let mut small = Sketch::default();
        small.add_point(DVec2::ZERO);
        small.add_point(DVec2::new(1.0, 0.0));

        let mut large = Sketch::default();
        large.add_point(DVec2::ZERO);
        large.add_point(DVec2::new(0.0, 100.0));

        let sizes = |sketch: &Sketch| -> Vec<f32> {
            SketchPlane::GROUND
                .points(sketch)
                .iter()
                .map(|point| point.size)
                .collect()
        };
        assert_eq!(sizes(&small), sizes(&large));
        assert_eq!(sizes(&small), vec![FREE_MARKER; 2]);
    }
}
