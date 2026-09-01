//! The ruled patch a corner two picks do not agree about is filled with.
//!
//! **Ruled because both joins are tangent**, which is one statement rather than
//! two: a patch meeting both blends tangentially along its own two edges has
//! every ruling lying in both tangent planes at once. So the ruling from a
//! place on the fillet is the line lying in the fillet's tangent plane there
//! and running tangent to the round's cylinder — two divisions and one root,
//! with no fit anywhere. `.notes/KERNEL.md` §9.6 is where no quadric is shown
//! to do the same job, and where the whole corner is argued.
#![allow(dead_code)]
// Kept ahead of its caller deliberately: the arm of
// [`Fitted`](super::fitted::Fitted) that would reach this wants a box round the
// patch before it can cull, which `.notes/KERNEL.md` §9.6 leaves open. The
// geometry below is settled and tested, and the route in `Rounding` that raises
// it lands with that arm.

use crate::math::harmonic;
use crate::number::predicate;
use crate::number::tolerance::EXACT;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::surface::Crossings;
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// The patch between two blends of one reach whose picks do not agree.
///
/// **Parameterized by the fillet's own angle and the run along the ruling**, in
/// that order, so that `u` is what runs round it — the convention every curved
/// surface here keeps. `v` of nought is the edge on the fillet and `v` of one
/// the edge on the round.
///
/// **Four fields and no more.** Where the two blends touch is the middle of
/// their axes' common perpendicular, and the plane the first edge is the
/// section of comes off that and [`Gusset::from`] — so neither is held, and
/// neither can come to disagree with what it was derived from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Gusset {
    /// The blend filled into the concave edge. Every ruling leaves it, and the
    /// patch's first edge lies on it.
    pub(crate) filled: Cylinder,
    /// The blend cut into the convex edge. Every ruling lands on it, and the
    /// patch's second edge lies on it.
    pub(crate) cut: Cylinder,
    /// Where the first edge starts, on [`Gusset::filled`]: the corner its other
    /// ruling runs out to.
    pub(crate) from: DVec3,
    /// Which of the two lines through a place tangent to [`Gusset::cut`] the
    /// ruling is.
    ///
    /// **A bit rather than a reading**, because the two close on each other at
    /// the touch point and nothing measured at a place tells them apart there.
    /// Which way round each axis was framed decides it, and a caller does not
    /// choose that.
    pub(crate) turning: bool,
}

/// One ruling of a [`Gusset`], and how fast each of its ends moves.
///
/// The rates are here rather than worked out again because the normal wants
/// both and the place wants neither: everything below the ruling is one cross
/// product, and everything above it would be the same arithmetic twice.
#[derive(Debug, Clone, Copy)]
struct Ruling {
    head: DVec3,
    heading: DVec3,
    foot: DVec3,
    footing: DVec3,
}

impl Gusset {
    /// The patch between the two blends, its first edge starting at `from`.
    pub(crate) fn new(filled: Cylinder, cut: Cylinder, from: DVec3, turning: bool) -> Self {
        let gusset = Self {
            filled,
            cut,
            from,
            turning,
        };
        let room = predicate::slack(EXACT, filled.radius);
        debug_assert!(
            predicate::touching((filled.radius - cut.radius).abs(), room),
            "{gusset:?} blends one corner with two reaches",
        );
        debug_assert!(
            predicate::touching((filled.axis.off(from) - filled.radius).abs(), room),
            "{from} is not on the fillet the patch runs out of",
        );
        gusset
    }

    /// Where the two blends touch, which is the tip the patch closes to.
    ///
    /// **The middle of the two axes' common perpendicular.** Both spines run a
    /// reach off the face the two picks share and run off it on opposite sides,
    /// so the axes stand two reaches apart and touch nothing but each other's
    /// tube at that one place — `.notes/KERNEL.md` §9.6.
    pub(crate) fn met(&self) -> DVec3 {
        let (one, two) = (self.filled.axis, self.cut.axis);
        let across = one.direction.cross(two.direction);
        let apart = two.origin - one.origin;
        let spread = across.length_squared();
        let head = one.origin + one.direction * (apart.cross(two.direction).dot(across) / spread);
        let foot = two.origin + two.direction * (apart.cross(one.direction).dot(across) / spread);
        (head + foot) / 2.0
    }

    /// Where the parameters `uv` land: `u` radians round the fillet, `v` along
    /// the ruling from it.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        let ruling = self.ruled(uv.x, self.side());
        ruling.head + (ruling.foot - ruling.head) * uv.y
    }

    /// Which way the surface faces at `uv`, the way its own parameters wind
    /// about.
    ///
    /// **A ruled surface turns along its ruling**, which is what the second
    /// term is: the place moves at one rate where the ruling leaves the fillet
    /// and at another where it lands, and a reading between the two moves at
    /// the blend of them. So the two edges read the two blends' own normals
    /// back, which is the tangency this patch exists to keep.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let ruling = self.ruled(uv.x, self.side());
        let along = ruling.foot - ruling.head;
        let across = ruling.heading + (ruling.footing - ruling.heading) * uv.y;
        across.cross(along).normalize()
    }

    /// Which parameters `at` stands at, its angle read in `(-π, π]`.
    ///
    /// **Closed form, and the tangency is why.** Every ruling lies in a tangent
    /// plane of the fillet, and those planes are one family — so a place off
    /// that cylinder stands in exactly two of them, `(x − o)·m = r` being one
    /// harmonic in the angle. Which of the two carries the place is a reading
    /// rather than a bit, the two rulings standing well apart wherever the
    /// question is asked.
    ///
    /// The angle comes back off the ruling's own head rather than out of the
    /// harmonic, so a half turn reads the way every other surface here reads
    /// one — see [`Axis::bearing`](super::axis::Axis).
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        let axis = self.filled.axis;
        let to = at - axis.origin;
        let (out, round) = (to.dot(axis.reference), to.dot(axis.quarter()));
        let lean = (self.filled.radius / out.hypot(round))
            .clamp(-1.0, 1.0)
            .acos();
        let bearing = round.atan2(out);
        let held = [bearing - lean, bearing + lean].map(|angle| self.ruled(angle, self.side()));
        let ruling = if held[0].strays(at) <= held[1].strays(at) {
            held[0]
        } else {
            held[1]
        };
        let along = ruling.foot - ruling.head;
        DVec2::new(
            axis.angle_of(ruling.head),
            (at - ruling.head).dot(along) / along.length_squared(),
        )
    }

    /// How far along a ray from `from` running `way` it meets this, in order.
    ///
    /// **Six at most, and the tip is why it is not eight.** A ray meets a ruled
    /// surface where it lies in a plane with a ruling, which here is one
    /// equation in the fillet's own angle: the direction the ray picks out of
    /// the fillet's tangent plane, held against the pair of directions there
    /// that run tangent to the round. Written out that is a harmonic of degree
    /// four — the two top harmonics cancel, which
    /// `.notes/KERNEL.md` §9.6 shows — and it carries a double root at the tip
    /// whatever the ray, the tangent plane there being the face the two blends
    /// share and every line in it touching the round. Divided out, what is left
    /// is a harmonic of degree three and six roots at the most.
    ///
    /// **The tip is never one of them.** The division takes the doubled root —
    /// a graze counts for none, which is the policy [`harmonic::angles`] states
    /// — and the ruling there has closed to nothing, so a ray reaching that far
    /// has no line left to cross.
    ///
    /// **Every root of the harmonic is a crossing of one of the two tangents,
    /// and only one of them is this patch's.** Which is settled by a comparison
    /// rather than a tolerance: the ray stands nearer the ruling it meets than
    /// the one it does not.
    ///
    /// **The ruling and not the line it runs along.** Every other surface here
    /// is unbounded where the faces on it are not, and a ray is answered about
    /// the whole of it; this one closes at the tip and runs out where its two
    /// blends do, so a crossing past either end of a ruling is a crossing of
    /// nothing. Leaving them in would answer places a hundred million reaches
    /// out, which no face holds and the inversion cannot read back.
    ///
    /// **A crossing on one of the patch's own two edges is a boundary place**,
    /// and which side of the edge a rounding puts it is the loops' business
    /// rather than this one's — the same ray meets the blend that edge is
    /// shared with on *its* boundary, where a cast is abandoned outright.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Crossings {
        let tip = self.filled.axis.angle_of(self.met());
        let start = tip + PI;
        let mut readings = [0.0; harmonic::READINGS];
        for (step, reading) in readings.iter_mut().enumerate() {
            let angle = start + TAU * step as f64 / harmonic::READINGS as f64;
            *reading = self.aimed(angle, from, way) / (1.0 - (angle - tip).cos());
        }
        let mut found = Crossings::none();
        for angle in harmonic::angles(readings, start) {
            if let Some(along) = self.crossed(angle, from, way) {
                found.push(along);
            }
        }
        found.all_mut().sort_by(f64::total_cmp);
        found
    }

    /// The ruling the fillet's angle `angle` carries, and how fast its two ends
    /// move.
    ///
    /// **The head is the fillet's own section by the plane the first edge is
    /// cut by**, and the foot is where the line through it in the fillet's
    /// tangent plane runs tangent to the round. The second is two statements:
    /// `(o − x)·m = −r` puts the landing angle where the ruling touches, one
    /// harmonic and so one root, and squaring the ruling against the fillet's
    /// normal puts it along the round's axis, which is linear.
    fn ruled(&self, angle: f64, side: f64) -> Ruling {
        let (axis, other) = (self.filled.axis, self.cut.axis);
        let reach = self.filled.radius;
        let cutting = self.cutting();
        let (facing, turning) = (axis.radial(angle), axis.radial(angle + FRAC_PI_2));
        let leaning = cutting.dot(axis.direction);
        let running = -reach * cutting.dot(turning) / leaning;
        let head = self.headed(angle, cutting);
        let heading = axis.direction * running + turning * reach;

        let to = other.origin - head;
        let toward = -heading;
        let across = to - other.direction * to.dot(other.direction);
        let crossing = toward - other.direction * toward.dot(other.direction);
        let spread = across.length_squared();
        let spreading = 2.0 * across.dot(crossing);
        let square = other.direction.cross(across);
        let squaring = other.direction.cross(crossing);
        let root = (spread - reach * reach).max(0.0).sqrt();
        let rooting = across.dot(crossing) / root;
        let over = across * -reach + square * (side * root);
        let overing = crossing * -reach + (squaring * root + square * rooting) * side;
        let lands = over / spread;
        let landing = (overing * spread - over * spreading) / (spread * spread);

        let slide = other.direction.dot(facing);
        let sliding = other.direction.dot(turning);
        let want = -(to.dot(facing) + reach * lands.dot(facing));
        let wanting = -(toward.dot(facing)
            + to.dot(turning)
            + reach * (landing.dot(facing) + lands.dot(turning)));
        let step = want / slide;
        let stepping = (wanting * slide - want * sliding) / (slide * slide);
        Ruling {
            head,
            heading,
            foot: other.origin + other.direction * step + lands * reach,
            footing: other.direction * stepping + landing * reach,
        }
    }

    /// Where the patch's first edge stands at the fillet's angle `angle`, given
    /// the normal `cutting` of the plane that edge is a section by.
    ///
    /// **The fillet's own section by that plane**, which is one division: the
    /// plane fixes how far along the axis the place stands, and the angle fixes
    /// the rest of it.
    ///
    /// The normal is handed over rather than asked for, both callers holding
    /// one already — [`Gusset::cutting`] runs [`Gusset::met`] to reach it, and
    /// a ruling would work the whole of that out twice.
    fn headed(&self, angle: f64, cutting: DVec3) -> DVec3 {
        let axis = self.filled.axis;
        let reach = self.filled.radius;
        let facing = axis.radial(angle);
        let run = (cutting.dot(self.from - axis.origin) - reach * cutting.dot(facing))
            / cutting.dot(axis.direction);
        axis.origin + axis.direction * run + facing * reach
    }

    /// Which of the two tangents through a head the ruling is, as a sign.
    fn side(&self) -> f64 {
        match self.turning {
            true => 1.0,
            false => -1.0,
        }
    }

    /// How far the ray from `from` running `way` stands from meeting either of
    /// the two tangents the head at `angle` carries — nought where it meets
    /// one of them.
    ///
    /// **Both tangents at once, which is what makes this a polynomial.** The
    /// ray picks one direction out of the fillet's tangent plane, and the two
    /// tangents to the round are the roots of a quadratic form on that plane —
    /// so putting the ray's own direction into the form asks about both without
    /// taking the root that would tell them apart.
    fn aimed(&self, angle: f64, from: DVec3, way: DVec3) -> f64 {
        let (axis, other) = (self.filled.axis, self.cut.axis);
        let turning = axis.radial(angle + FRAC_PI_2);
        let head = self.headed(angle, self.cutting());
        let leaving = head - from;
        let (axial, across) = (
            leaving.cross(axis.direction).dot(way),
            leaving.cross(turning).dot(way),
        );
        let (square, slant) = (
            axis.direction.cross(other.direction),
            turning.cross(other.direction),
        );
        let to = other.origin - head;
        let reach = self.filled.radius;
        (across * to.dot(square) - axial * to.dot(slant)).powi(2)
            - reach
                * reach
                * (across * across * square.length_squared()
                    - 2.0 * across * axial * square.dot(slant)
                    + axial * axial * slant.length_squared())
    }

    /// How far along the ray from `from` running `way` it crosses the ruling at
    /// `angle`, or `None` where what it meets there is the other tangent.
    ///
    /// **A comparison and not a bound.** At a root of [`Gusset::aimed`] the ray
    /// meets one of the two tangents exactly and stands clear of the other, so
    /// which is which is read off the two distances rather than off a
    /// tolerance. A tangent running along the round's own axis carries no
    /// place, and answers nothing.
    ///
    /// **A tie is this patch's.** The two tangents meet each other at the head
    /// and nowhere else, so a ray through the head stands nought from both —
    /// and so does one meeting the single tangent a head on the round itself
    /// carries. Both are crossings of this patch, so only the *other* tangent
    /// standing strictly nearer turns one away. A head standing inside the
    /// round carries no tangent at all, and is refused before either is read.
    fn crossed(&self, angle: f64, from: DVec3, way: DVec3) -> Option<f64> {
        let ruling = self.ruled(angle, self.side());
        if self.cut.axis.off(ruling.head) < self.filled.radius {
            return None;
        }
        let apart = |ruling: &Ruling| {
            let along = ruling.foot - ruling.head;
            let across = along.cross(way);
            (ruling.head - from).dot(across).abs() / across.length()
        };
        let mine = apart(&ruling);
        if mine.is_nan() || apart(&self.ruled(angle, -self.side())) < mine {
            return None;
        }
        let along = ruling.foot - ruling.head;
        let leaving = ruling.head - from;
        let across = way.cross(along);
        let run = leaving.cross(way).dot(across) / across.length_squared();
        (0.0..=1.0)
            .contains(&run)
            .then(|| leaving.cross(along).dot(across) / across.length_squared())
    }

    /// The normal of the plane the patch's first edge is the fillet's section
    /// by, unnormalized.
    ///
    /// **Through both ends of that edge, and square to the fillet's rulings at
    /// the touch point.** The second is what leaves a corner rather than a cusp
    /// where the patch's two edges meet: both leave that point in the shared
    /// face's plane, so an edge running out *along* its own blend would leave
    /// in its neighbour's direction. See `.notes/KERNEL.md` §9.6, where the
    /// choice is named as a choice.
    fn cutting(&self) -> DVec3 {
        let axis = self.filled.axis;
        let met = self.met();
        axis.direction
            .cross(met - axis.origin)
            .cross(self.from - met)
    }
}

impl Ruling {
    /// How far `at` stands from the line this runs along.
    fn strays(&self, at: DVec3) -> f64 {
        let along = (self.foot - self.head).normalize();
        let to = at - self.head;
        (to - along * to.dot(along)).length()
    }
}

#[cfg(test)]
mod tests;
