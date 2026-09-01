//! A circle drawn as a circle, not as a great many short straight lines.

use crate::aim::{self, Aim};
use crate::hit::{Hit, HitAt};
use crate::precedence::Precedence;
use crate::primitive::{DEFAULT_STROKE_WIDTH, Flatten, Primitive};
use crate::renderer::record::ring_instance::RingInstance;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::{Vec2, Vec3, Vec4Swizzles};
use palantir::Rect;

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
///
/// `Default` draws nothing — its radius is zero, and its axes with it, where a
/// ring meant to be drawn carries a unit pair.
#[derive(Default, Debug, Clone, Copy)]
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
    /// What this is for, which decides what a click meant for two things at
    /// once lands on. See [`Precedence`].
    pub precedence: Precedence,
    /// What a pick that lands on this stroke reports. See
    /// [picking](crate#picking).
    pub tag: Option<Tag>,
}

impl Ring {
    /// A white ring of default width, in the plane through `center` square to
    /// `normal`.
    ///
    /// Where angle zero ends up is arbitrary and unspecified — a full circle
    /// reads the same whatever basis it is built on. An arc would need to be
    /// told, which is why the axes are carried rather than the normal.
    pub fn new(center: Vec3, radius: f32, normal: Vec3) -> Self {
        // Zero rather than NaN where the caller hands over nothing to point
        // along: a ring with no plane has no axes either, and comes out drawing
        // nothing — where a NaN would be carried into every vertex of the band
        // and take the frame with it.
        let normal = normal.normalize_or_zero();
        // Any seed that isn't along the normal; the far one is picked so the
        // cross product never collapses. Which threshold is arbitrary — the
        // shader picks its own basis by the same rule and the two never have to
        // agree, because neither answer is read by the other.
        let seed = if normal.x.abs() > 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let x_axis = normal.cross(seed).normalize_or_zero();
        Self {
            center,
            radius,
            x_axis,
            y_axis: normal.cross(x_axis),
            color: Vec3::ONE,
            width: DEFAULT_STROKE_WIDTH,
            precedence: Precedence::default(),
            tag: None,
        }
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

    /// A box on screen the whole rim lands inside, or `None` where it has no
    /// bound at all.
    ///
    /// A *bound* and not a box, which is what separates this from the box a
    /// [`Text`](crate::Text) answers with: a label's box is where the label is,
    /// and a pick measures against it. A rim only touches this one's edges and
    /// is nowhere else near it, so nothing may be measured against it — it
    /// answers whether the rim is worth walking, and nothing else.
    ///
    /// One-sided on purpose. A bound too large costs only a walk that need not
    /// have happened, where one too small loses a click outright. `None` says
    /// that more strongly still: a rim reaching the eye plane has no bound at
    /// all, and the walk decides.
    ///
    /// Clip space is linear in world position, so the rim is exactly
    /// `centre + cos θ · across + sin θ · up` there, component by component.
    /// Each component of that runs over `centre ± hypot(across, up)`, being the
    /// amplitude of `a cos θ + b sin θ` — which is not a bound but the true
    /// range. The only slack comes of taking the numerator's range and `w`'s
    /// apart when in truth the two move together, and a rim square to the view
    /// has no spread in `w` at all: there this is the rim's own box.
    ///
    /// The alternative was a bound on the projected conic, which is what the
    /// walk exists to avoid — it has no simple form once the circle crosses the
    /// near plane. This has no form there either, and says so.
    fn bound_on_screen(&self, aim: &Aim) -> Option<Rect> {
        let centre = aim.view_proj * self.center.extend(1.0);
        // Directions rather than positions, so the translation stays out and
        // what comes back is how far the rim reaches from its own centre.
        let across = aim.view_proj * (self.x_axis * self.radius).extend(0.0);
        let up = aim.view_proj * (self.y_axis * self.radius).extend(0.0);

        let amplitude = |a: f32, b: f32| (a * a + b * b).sqrt();
        let depth = amplitude(across.w, up.w);
        // The whole rim in front of the eye plane, which is the whole of what
        // makes dividing by `w` mean anything.
        let near = centre.w - depth;
        if near <= 0.0 {
            return None;
        }
        let far = centre.w + depth;
        let spread = Vec2::new(amplitude(across.x, up.x), amplitude(across.y, up.y));
        let (low, high) = (centre.xy() - spread, centre.xy() + spread);
        // Each end over both depths, keeping the outermost: the numerator's
        // range and `w`'s are known apart, so every pairing has to be allowed
        // for. Both are positive by the refusal above, so the two are the
        // extremes and nothing between them can escape.
        let (min, max) = ((low / near).min(low / far), (high / near).max(high / far));

        // Squared up again after the trip through pixels, because NDC counts y
        // the other way and which corner is which does not survive it.
        let (a, b) = (
            aim.viewport.pixel_from_ndc(min),
            aim.viewport.pixel_from_ndc(max),
        );
        let (top_left, size) = (a.min(b), (b - a).abs());
        Some(Rect::new(top_left.x, top_left.y, size.x, size.y))
    }

    /// The point of the ring whose *projection* comes nearest the cursor.
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
    ///
    /// Thirty-two projections, which is why [`Ring::pick`] refuses what it can
    /// before reaching here: a rim costs fifty times what a stroke's segment
    /// does and a hundred and eighty times what a marker does, so a drawing of
    /// many circles is a drawing that spends its pick almost entirely on rims
    /// the cursor is nowhere near.
    fn nearest_to(&self, aim: &Aim) -> Option<NearestOnRing> {
        /// Arcs the rim is cut into before refining. Enough to tell the near side
        /// of the ellipse from the far one, which is all this pass has to do.
        const RIM_PROBES: usize = 8;
        /// Golden-section steps within the winning arc, each cutting the bracket
        /// to 0.618 of itself and costing *one* probe, because the other end of
        /// the new bracket is the point it already measured. A ternary split
        /// cuts harder per step — to two thirds — but pays two probes to do it,
        /// which is 0.667 per probe against 0.618 and so the worse bargain at
        /// every budget.
        ///
        /// What the steps have to overcome is angular, so it is the rim's size
        /// on screen rather than the number of arcs that decides how many are
        /// wanted. Twenty-two leaves the bracket at `0.618²² ≈ 2.3e-5` of the
        /// arc in twenty-four probes, where a ternary search needs forty-eight
        /// to reach `(2/3)²⁴ ≈ 5.9e-5`.
        const RIM_STEPS: usize = 22;

        /// The reciprocal of the golden ratio, which is the fraction a
        /// golden-section step leaves behind.
        const INV_PHI: f32 = 0.618_034;

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

        // Two interior probes to open the bracket; every step after that moves
        // one of them to where the other already is and measures a single new
        // point.
        let (mut low, mut high) = ((nearest as f32 - 1.0) * arc, (nearest as f32 + 1.0) * arc);
        let mut span = high - low;
        let (mut lower, mut upper) = (high - INV_PHI * span, low + INV_PHI * span);
        let (mut at_lower, mut at_upper) = (screen_at(lower), screen_at(upper));
        for _ in 0..RIM_STEPS {
            span *= INV_PHI;
            if at_lower < at_upper {
                (high, upper, at_upper) = (upper, lower, at_lower);
                lower = high - INV_PHI * span;
                at_lower = screen_at(lower);
            } else {
                (low, lower, at_lower) = (lower, upper, at_upper);
                upper = low + INV_PHI * span;
                at_upper = screen_at(upper);
            }
        }

        // The better of the two the bracket already holds, rather than its
        // midpoint measured afresh. Both are inside a bracket 2.3e-5 of an arc
        // wide, so which one is taken is immaterial — but one of them is a
        // reading and the midpoint is a guess, and taking the reading is what
        // makes the count above the whole count.
        let (angle, screen) = if at_lower <= at_upper {
            (lower, at_lower)
        } else {
            (upper, at_upper)
        };
        screen.is_finite().then(|| NearestOnRing {
            angle: angle.rem_euclid(std::f32::consts::TAU),
            screen,
        })
    }

    /// Set the stroke width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

impl Styled for Ring {
    fn color_mut(&mut self) -> &mut Vec3 {
        &mut self.color
    }

    fn tag_mut(&mut self) -> &mut Option<Tag> {
        &mut self.tag
    }

    fn precedence_mut(&mut self) -> &mut Precedence {
        &mut self.precedence
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

impl Flatten for Ring {
    type Record = RingInstance;

    /// One, however large it is drawn — the fragment stage resolves the circle
    /// rather than a count of chords standing in for it.
    fn record_count(&self) -> usize {
        1
    }

    fn records(&self) -> impl Iterator<Item = Self::Record> {
        std::iter::once(RingInstance::of(self))
    }
}

impl Primitive for Ring {
    fn tag(&self) -> Option<Tag> {
        self.tag
    }

    fn standing(&self) -> Precedence {
        self.precedence
    }

    fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let reach = aim.reach(self.width);
        // Dismissed before the walk wherever the whole rim lands clear of the
        // cursor, which in a drawing of many circles is nearly every one of
        // them. A rim with no bound is not a rim that can be dismissed, which
        // is why this reads as "said so" rather than "did not say otherwise" —
        // see [`Ring::bound_on_screen`].
        if self
            .bound_on_screen(aim)
            .is_some_and(|bound| aim::reach_to_box(aim.cursor, bound) > reach)
        {
            return None;
        }
        let near = self.nearest_to(aim)?;
        (near.screen <= reach).then(|| {
            aim.hit(
                tag,
                HitAt::Ring { angle: near.angle },
                self.precedence,
                self.at(near.angle),
                near.screen,
            )
        })
    }

    /// A circle reaches its radius along every world axis except in so far as
    /// its plane leans away from that axis, which is what the normal's
    /// component in it measures.
    fn reaches(&self, mut include: impl FnMut(Vec3)) {
        let normal = self.normal();
        let spread = Vec3::new(
            (1.0 - normal.x * normal.x).max(0.0).sqrt(),
            (1.0 - normal.y * normal.y).max(0.0).sqrt(),
            (1.0 - normal.z * normal.z).max(0.0).sqrt(),
        ) * self.radius;
        include(self.center - spread);
        include(self.center + spread);
    }
}

#[cfg(test)]
mod tests;
