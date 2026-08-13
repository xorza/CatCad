//! A circle drawn as a circle, not as a great many short straight lines.

use crate::aim::Aim;
use crate::hit::{Hit, HitAt};
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

    /// Whether the cursor landed on this rim, and where round it.
    pub(crate) fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let near = self.nearest_to(aim)?;
        (near.screen <= aim.reach(self.width)).then(|| {
            aim.hit(
                tag,
                HitAt::Ring { angle: near.angle },
                self.at(near.angle),
                near.screen,
            )
        })
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

/// How near the cursor came to a ring's rim, and where round it.
#[derive(Debug, Clone, Copy)]
struct NearestOnRing {
    /// Radians round from the ring's own `x_axis`, in `0..TAU`.
    angle: f32,
    /// How far the cursor was from it on screen.
    screen: f32,
}

impl Ring {
    /// The point of `ring` whose *projection* comes nearest `cursor`.
    ///
    /// Measured on screen rather than in the ring's own plane. The two agree while
    /// the ring faces the eye and part company as it tilts: the in-plane answer
    /// runs radially out from the centre, the screen answer along the normal of
    /// the ellipse the circle projects to. At three degrees off edge-on — which is
    /// well inside what this renderer is asked to draw — a cursor two pixels from
    /// the rim measures as thirty-five, and every click near it misses.
    ///
    /// A circle projects to a conic, so the closed-form answer is a quartic, and a
    /// different one for each way that conic degenerates once the circle crosses
    /// the near plane. Walking the rim costs a few dozen matrix multiplies and has
    /// none of those cases.
    ///
    /// The coarse pass is not there for accuracy — refinement supplies all of
    /// that. It is there because distance round a rim has two minima, the near
    /// side and the far, and a search started from a guess can settle on the wrong
    /// one.
    fn nearest_to(&self, aim: &Aim) -> Option<NearestOnRing> {
        /// Arcs the rim is cut into before refining. Enough to tell the near side
        /// of the ellipse from the far one, which is all this pass has to do.
        const RIM_PROBES: usize = 8;
        /// Ternary steps within the winning arc, each cutting the bracket by a
        /// third. What they have to overcome is angular, so it is the rim's size
        /// on screen rather than the number of arcs that decides how many are
        /// wanted: sixteen holds a hundredth of a pixel at a rim of a hundred and
        /// drifts to a fifth at a rim of a few thousand, where twenty-four holds
        /// a thousandth.
        const RIM_STEPS: usize = 24;

        // A point off the far side of the near plane has no screen position to
        // measure. Reading as infinitely far keeps it out of the answer and walks
        // both passes away from it.
        let screen_at = |angle: f32| aim.reach_to(self.at(angle)).unwrap_or(f32::INFINITY);

        let arc = std::f32::consts::TAU / RIM_PROBES as f32;
        let mut nearest = 0;
        let mut nearest_screen = f32::INFINITY;
        for probe in 0..RIM_PROBES {
            let screen = screen_at(probe as f32 * arc);
            if screen < nearest_screen {
                nearest_screen = screen;
                nearest = probe;
            }
        }
        if !nearest_screen.is_finite() {
            return None;
        }

        let (mut low, mut high) = ((nearest as f32 - 1.0) * arc, (nearest as f32 + 1.0) * arc);
        for _ in 0..RIM_STEPS {
            let third = (high - low) / 3.0;
            if screen_at(low + third) < screen_at(high - third) {
                high -= third;
            } else {
                low += third;
            }
        }

        let angle = (low + high) * 0.5;
        let screen = screen_at(angle);
        screen.is_finite().then(|| NearestOnRing {
            angle: angle.rem_euclid(std::f32::consts::TAU),
            screen,
        })
    }
}
