//! Where a sketch sits in the world, and what it looks like once it's there.

use aperture::Curve;
use glam::{DVec2, Vec3};
use silverpoint::Sketch;

/// Straight segments per circle. A circle is the only sketch entity that
/// isn't already straight, and this is what it costs to look round.
const CIRCLE_SEGMENTS: usize = 96;

/// Point marker size, as a share of the sketch's longest side.
const MARKER_SHARE: f64 = 0.014;

/// Marker size in sketch units for a sketch with no extent to take one from.
const FALLBACK_MARKER: f64 = 0.1;

/// Linear-RGB, unlit — these reach the target as authored.
const EDGE: Vec3 = Vec3::new(0.35, 0.55, 0.80);
const FREE_POINT: Vec3 = Vec3::new(0.85, 0.55, 0.10);
const FIXED_POINT: Vec3 = Vec3::new(0.80, 0.14, 0.05);

/// Logical pixels.
const EDGE_WIDTH: f32 = 1.6;
const MARKER_WIDTH: f32 = 1.3;

/// How far the drawing rides in front of the solids, in steps of depth-buffer
/// resolution. A sketch is what a model is derived from, so where the two share
/// a plane the drawing is the one that reads.
///
/// Small on purpose. Enough steps to clear the rounding two differently-shaped
/// primitives accumulate over the same plane, and nowhere near enough to lift
/// the drawing out of a solid standing on it — a line running behind a face
/// still goes behind it.
const SKETCH_LIFT: i32 = 32;

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

    /// The sketch as drawable curves: an edge per segment, a tessellated
    /// circle per circle, and a marker per point — a square where the solver
    /// may not move it, a cross where it may. All of it biased clear of the
    /// solids in depth, so the drawing reads over them.
    pub(crate) fn curves(&self, sketch: &Sketch) -> Vec<Curve> {
        let marker = marker_size(sketch);
        let mut curves = Vec::new();
        for segment in sketch.segments() {
            let a = self.point(sketch.point(segment.a));
            let b = self.point(sketch.point(segment.b));
            curves.push(Curve::segment(a, b).colored(EDGE).width(EDGE_WIDTH));
        }
        for circle in sketch.circles() {
            curves.push(self.circle(sketch.point(circle.center), circle.radius.abs()));
        }
        for (id, position) in sketch.points() {
            if sketch.is_fixed(id) {
                curves.push(self.anchor(position, marker));
            } else {
                curves.extend(self.cross(position, marker));
            }
        }
        // Applied here rather than at each constructor: the drawing rides on
        // one plane and above the solids as one thing, and nothing in it
        // outranks the rest.
        let normal = self.normal();
        for curve in &mut curves {
            curve.z_offset = SKETCH_LIFT;
            curve.plane_normal = Some(normal);
        }
        curves
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

    /// A free point: the cross a drawing marks a bare point with.
    fn cross(&self, at: DVec2, size: f64) -> [Curve; 2] {
        let corner = |x: f64, y: f64| self.point(at + DVec2::new(x, y) * size);
        [
            Curve::segment(corner(-1.0, -1.0), corner(1.0, 1.0)),
            Curve::segment(corner(-1.0, 1.0), corner(1.0, -1.0)),
        ]
        .map(|curve| curve.colored(FREE_POINT).width(MARKER_WIDTH))
    }

    /// A pinned point. Squares read as anchors, and the colour agrees.
    fn anchor(&self, at: DVec2, size: f64) -> Curve {
        let corner = |x: f64, y: f64| self.point(at + DVec2::new(x, y) * size);
        Curve::new(vec![
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ])
        .closed()
        .colored(FIXED_POINT)
        .width(MARKER_WIDTH)
    }
}

/// Markers are model-space geometry, so a zoom magnifies them like everything
/// else. Sizing them off the sketch is what keeps them proportionate to the
/// drawing rather than to whatever units it happens to be in.
fn marker_size(sketch: &Sketch) -> f64 {
    let mut positions = sketch.points().map(|(_, position)| position);
    let Some(first) = positions.next() else {
        return FALLBACK_MARKER;
    };
    let (mut min, mut max) = (first, first);
    for position in positions {
        min = min.min(position);
        max = max.max(position);
    }
    let extent = (max - min).max_element();
    if extent > 0.0 {
        extent * MARKER_SHARE
    } else {
        FALLBACK_MARKER
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

        // One edge, one circle, one square for the anchor, two strokes for
        // the free point's cross.
        let curves = SketchPlane::GROUND.curves(&sketch);
        assert_eq!(curves.len(), 5);

        // Every last stroke rides in front of the solids — a marker left
        // behind would sink into the face its edge floats over.
        assert!(curves.iter().all(|curve| curve.z_offset == SKETCH_LIFT));

        // And every one names the plane it lies in, so the renderer can take
        // the stroke's depth off the surface rather than off its centreline.
        // The ground plane's axes are +X and −Z, which face +Y.
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

        // The anchor is a closed square about the fixed point, sized off the
        // sketch's 10-unit span: 10 × 0.014 to a side's half.
        let anchor = &curves[2];
        assert_eq!(anchor.color, FIXED_POINT);
        assert!(anchor.closed);
        assert_eq!(anchor.points.len(), 4);
        let marker = 10.0 * MARKER_SHARE as f32;
        assert!(anchor.points[0].abs_diff_eq(Vec3::new(-marker, 0.0, marker), 1e-6));
        assert!(anchor.points[2].abs_diff_eq(Vec3::new(marker, 0.0, -marker), 1e-6));

        // The free point's cross is two open strokes through it.
        for stroke in &curves[3..] {
            assert_eq!(stroke.color, FREE_POINT);
            assert!(!stroke.closed);
            assert_eq!(stroke.points.len(), 2);
            let midpoint = (stroke.points[0] + stroke.points[1]) * 0.5;
            assert!(midpoint.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-6));
        }
    }

    #[test]
    fn markers_scale_with_the_sketch() {
        let mut small = Sketch::default();
        small.add_point(DVec2::ZERO);
        small.add_point(DVec2::new(1.0, 0.0));
        assert!((marker_size(&small) - MARKER_SHARE).abs() < 1e-12);

        // A hundred times the drawing, a hundred times the marker.
        let mut large = Sketch::default();
        large.add_point(DVec2::ZERO);
        large.add_point(DVec2::new(0.0, 100.0));
        assert!((marker_size(&large) - MARKER_SHARE * 100.0).abs() < 1e-12);

        // Nothing to measure falls back rather than vanishing.
        assert_eq!(marker_size(&Sketch::default()), FALLBACK_MARKER);
        let mut lone = Sketch::default();
        lone.add_point(DVec2::new(4.0, 4.0));
        assert_eq!(marker_size(&lone), FALLBACK_MARKER);
    }
}
