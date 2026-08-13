//! A circle drawn as a circle, not as a great many short straight lines.

use glam::Vec3;

/// Default stroke width, in logical pixels.
const DEFAULT_WIDTH: f32 = 1.5;

/// A stroked circle lying in a plane, resolved in the fragment shader rather
/// than tessellated — an [overlay](crate#overlays), like [`Curve`](crate::Curve).
///
/// A polyline approximating a circle is only as round as the count it was
/// built with, and the count that suffices depends on how large the circle
/// lands on screen: the chord dips `r(1 − cos(π/n))` inside the arc, so a fixed
/// count facets visibly once the radius grows past it. Zoom decides that, and
/// zoom is exactly what the renderer refuses to rebuild geometry for.
///
/// So the circle is shipped as a circle. The vertex stage lays a coarse band
/// around the rim, wide enough to contain the true curve at any zoom, and the
/// fragment stage measures each pixel's distance to the real circle. Round at
/// every magnification, one record however large, and nothing to rebuild when
/// the camera moves.
#[derive(Debug, Clone, Copy)]
pub struct Ring {
    pub center: Vec3,
    pub radius: f32,
    /// Unit, in the ring's plane, and where angle zero points.
    ///
    /// Held rather than derived from a normal so that only one language ever
    /// picks a basis: the shader is handed the axes and never has to agree
    /// with Rust about how they were chosen.
    pub x_axis: Vec3,
    /// Unit, in the plane and square to [`Ring::x_axis`] — a quarter turn on
    /// from it, the way `y` follows `x`.
    pub y_axis: Vec3,
    /// Linear-RGB.
    pub color: Vec3,
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Depth-test bias in steps of depth-buffer resolution, positive toward
    /// the viewer. See [overlays](crate#overlays).
    pub z_offset: i32,
    /// What a pick that lands on this stroke reports. See
    /// [picking](crate#picking).
    pub tag: Option<u64>,
}

impl Ring {
    /// A white ring of default width, in the plane through `center` square to
    /// `normal`.
    ///
    /// Where angle zero ends up is arbitrary and unspecified — a full circle
    /// reads the same whatever basis it is built on. An arc would need to be
    /// told, which is why the axes are carried rather than the normal.
    pub fn new(center: Vec3, radius: f32, normal: Vec3) -> Self {
        let normal = normal.normalize();
        // Any seed that isn't along the normal; the far one is picked so the
        // cross product never collapses.
        let seed = if normal.x.abs() > 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let x_axis = normal.cross(seed).normalize();
        Self {
            center,
            radius,
            x_axis,
            y_axis: normal.cross(x_axis),
            color: Vec3::ONE,
            width: DEFAULT_WIDTH,
            z_offset: 0,
            tag: None,
        }
    }

    /// Set the linear-RGB colour.
    pub fn colored(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    /// Set the stroke width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Set the depth-test bias. See [`Ring::z_offset`].
    pub fn z_offset(mut self, z_offset: i32) -> Self {
        self.z_offset = z_offset;
        self
    }

    /// Name this ring to whatever a pick will be reported to. See
    /// [`Ring::tag`].
    pub fn tagged(mut self, tag: u64) -> Self {
        self.tag = Some(tag);
        self
    }

    /// The plane the ring lies in, as a unit normal.
    pub fn normal(&self) -> Vec3 {
        self.x_axis.cross(self.y_axis)
    }

    /// Where `angle` radians round from [`Ring::x_axis`] lands in the world.
    pub fn at(&self, angle: f32) -> Vec3 {
        let (sin, cos) = angle.sin_cos();
        self.center + (self.x_axis * cos + self.y_axis * sin) * self.radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_derived_axes_are_orthonormal_and_right_handed() {
        // Deliberately not axis-aligned, and not unit length: `new` has to
        // normalize before it can build a basis on it.
        for normal in [
            Vec3::Y,
            Vec3::X * 3.0,
            Vec3::NEG_Z,
            Vec3::new(1.0, 2.0, -0.5),
            Vec3::new(-0.99, 0.1, 0.05),
        ] {
            let ring = Ring::new(Vec3::ZERO, 2.0, normal);
            let unit = normal.normalize();
            assert!((ring.x_axis.length() - 1.0).abs() < 1e-6, "{normal:?}");
            assert!((ring.y_axis.length() - 1.0).abs() < 1e-6, "{normal:?}");
            assert!(ring.x_axis.dot(ring.y_axis).abs() < 1e-6, "{normal:?}");
            assert!(ring.x_axis.dot(unit).abs() < 1e-6, "{normal:?}");
            assert!(ring.y_axis.dot(unit).abs() < 1e-6, "{normal:?}");
            // x cross y comes back to the normal rather than its opposite,
            // which is what makes the angle a pick reports run anticlockwise
            // seen from the front.
            assert!(ring.normal().abs_diff_eq(unit, 1e-6), "{normal:?}");
        }
    }

    #[test]
    fn a_quarter_turn_walks_from_one_axis_to_the_other() {
        let ring = Ring::new(Vec3::new(1.0, 0.0, 2.0), 3.0, Vec3::Y);
        // Angle zero is on `x_axis`, a quarter turn on is `y_axis`, and both
        // sit a radius away from the centre.
        assert!(
            ring.at(0.0)
                .abs_diff_eq(ring.center + ring.x_axis * 3.0, 1e-6)
        );
        assert!(
            ring.at(std::f32::consts::FRAC_PI_2)
                .abs_diff_eq(ring.center + ring.y_axis * 3.0, 1e-6)
        );
        assert!(
            ring.at(std::f32::consts::PI)
                .abs_diff_eq(ring.center - ring.x_axis * 3.0, 1e-6)
        );
        // Every point of it is exactly a radius out, in the ring's own plane.
        for step in 0..16 {
            let angle = step as f32 / 16.0 * std::f32::consts::TAU;
            let out = ring.at(angle) - ring.center;
            assert!((out.length() - 3.0).abs() < 1e-5, "{angle}");
            assert!(out.dot(ring.normal()).abs() < 1e-5, "{angle}");
        }
    }
}
