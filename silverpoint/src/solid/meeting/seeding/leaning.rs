//! A drill that leans on the ring it goes through.

use crate::inline::Inline;
use crate::math::bisect;
use crate::math::quartic;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::torus::Torus;
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

/// How many cells a turn of the tube is cut into to bracket the ends.
///
/// The count of turns only moves where the equation's own discriminant is
/// nought, and that is a trigonometric polynomial of degree twelve in `v` —
/// five harmonics of degree two, and a quartic's discriminant is degree six in
/// its coefficients. So it moves at most [`ENDS`] times. This many cells leaves
/// ten between ends that are evenly spread, and two ends inside one cell are a
/// stretch of the tube a two-hundred-and-fifty-sixth of a turn wide that opens
/// and shuts again.
const CELLS: usize = 256;

/// How many angles the chart's blind spot is offered before one is taken.
///
/// A degree-two equation is met at four angles at most, so half of these stand
/// clear of it however it lies — see [`Harmonics::charted`].
const CHARTS: usize = 8;

/// The most ends a leaning drill's curve can have, which is what a
/// discriminant of degree twelve allows — see [`CELLS`].
pub(super) const ENDS: usize = 24;

/// Where a cylinder whose axis leans on the ring's stands in the ring's own
/// two angles.
///
/// **Two waves where the written pairs have one.** Standing on a cylinder is
/// `|q|² − (q·w)² = r²`, and the axial part squared carries `(e(u)·w)²` into
/// it — a second harmonic of size `out(v)²·W²/2`, where `W` is how much of the
/// drill's direction stands square to the ring's axis. A drill that runs
/// parallel offers no `W` at all, which is why one wave answers it and this
/// arm never meets it.
///
/// So the equation is `A₀ + A₁cos u + B₁sin u + A₂cos 2u + B₂sin 2u = 0` — see
/// [`Harmonics`] — and a single `acos` no longer solves it. Up to four angles
/// answer it at one `v` where the written pairs offer two.
#[derive(Debug, Clone, Copy)]
pub(super) struct Leaning {
    /// Where the ring's own origin stands from the drill's, read in the ring's
    /// frame: square to the axis first, along it last.
    off: DVec3,
    /// Which way the drill runs, in that same frame.
    way: DVec3,
    radius: f64,
}

impl Leaning {
    /// How `tube` reads against `torus`, its axis leaning on the ring's.
    pub(super) fn of(tube: &Cylinder, torus: &Torus) -> Self {
        let axis = torus.axis;
        let framed = |of: DVec3| {
            DVec3::new(
                of.dot(axis.reference),
                of.dot(axis.quarter()),
                of.dot(axis.direction),
            )
        };
        Self {
            off: framed(axis.origin - tube.axis.origin),
            way: framed(tube.axis.direction),
            radius: tube.radius,
        }
    }

    /// The angles round the axis the curve stands at at `v`, in no order.
    pub(super) fn turns(&self, torus: &Torus, v: f64) -> Inline<f64, 4> {
        self.held(torus, v).turns()
    }

    /// The angles round the tube a piece of the curve begins or ends at, in
    /// order.
    ///
    /// **Where the count of turns moves**, which is what the seeding asks of
    /// them and what the equation's own discriminant only nearly answers. A
    /// drill that stands square across the ring folds all four of its turns at
    /// one `v`: its equation is `out²/2 + along² − r² = (out²/2)·cos 2u`, whose
    /// two pairs both fall together at `|along| = r` — so the discriminant has
    /// a repeated zero there and no sign to change. The count moves from four
    /// to none, which nothing hides.
    ///
    /// **Bracketed and bisected rather than solved.** A companion matrix at the
    /// degree the discriminant comes to in `v` can lose a pair of roots to
    /// conditioning and swallow a piece of the curve without saying so, where a
    /// count that moves cannot be argued with.
    ///
    /// **`None` where it moves more often than [`ENDS`] allows**, which is the
    /// algebra saying the arithmetic has stopped agreeing with it. A drill of
    /// the tube's own radius running along the tube — its axis tangent to the
    /// ring's centre circle — lies against the ring rather than crossing it, so
    /// the equation holds four roots that are pairwise within a rounding of
    /// each other over most of a turn. The count then flickers cell by cell,
    /// and every flicker is an end that is not there. What comes of a refusal
    /// is the boolean's own, and it is the answer a curve nothing can chart
    /// deserves — see [`seeded`](super::seeded).
    pub(super) fn ends(&self, torus: &Torus) -> Option<Inline<f64, ENDS>> {
        let mut ends = Inline::none();
        let counted = |v: f64| self.held(torus, v).count();
        let cell = |step: usize| TAU * step as f64 / CELLS as f64;
        let mut last = counted(0.0);
        for step in 0..CELLS {
            let (lo, hi) = (cell(step), cell(step + 1));
            let next = counted(hi);
            if next != last {
                let side = |v: f64| match counted(v) == last {
                    true => 1.0,
                    false => -1.0,
                };
                if let Some(end) = bisect::crossed(lo, hi, side) {
                    if ends.all().len() == ENDS {
                        return None;
                    }
                    ends.push(end);
                }
            }
            last = next;
        }
        Some(ends)
    }

    /// The equation the drill comes to at one angle round the tube.
    ///
    /// Standing on the drill is `|q|² − (q·w)² = r²` for `q` the place taken
    /// from the drill's axis, and the place of the ring at `(u, v)` puts
    /// `out(v)·e(u)` into that — where `e(u)` is the unit direction at `u`
    /// square to the axis. Everything that survives is a harmonic of `u`.
    fn held(&self, torus: &Torus, v: f64) -> Harmonics {
        let (up, across) = v.sin_cos();
        let out = torus.major + torus.minor * across;
        let along = torus.minor * up;
        // How far along its own axis the place stands, less the part `u`
        // carries.
        let axial = self.off.dot(self.way) + along * self.way.z;
        let square = self.way.truncate().length_squared();
        Harmonics {
            flat: self.off.length_squared() + out * out + along * along
                - axial * axial
                - out * out * square / 2.0
                - self.radius * self.radius
                + 2.0 * along * self.off.z,
            round: 2.0 * out * (self.off.truncate() - axial * self.way.truncate()),
            twice: -out
                * out
                * DVec2::new(
                    (self.way.x * self.way.x - self.way.y * self.way.y) / 2.0,
                    self.way.x * self.way.y,
                ),
        }
    }
}

/// One equation in the angle round the ring's axis:
/// `flat + round·(cos u, sin u) + twice·(cos 2u, sin 2u) = 0`.
///
/// What a whole pair comes to at one angle round the tube, and the only shape
/// [`Leaning`] solves anything in.
#[derive(Debug, Clone, Copy)]
struct Harmonics {
    flat: f64,
    round: DVec2,
    twice: DVec2,
}

impl Harmonics {
    /// How far the equation stands from being met at `u`.
    fn at(&self, u: f64) -> f64 {
        self.flat
            + self.round.dot(DVec2::from_angle(u))
            + self.twice.dot(DVec2::from_angle(2.0 * u))
    }

    /// The same equation read from `by` radians round.
    fn turned(&self, by: f64) -> Self {
        Self {
            flat: self.flat,
            round: DVec2::from_angle(-by).rotate(self.round),
            twice: DVec2::from_angle(-2.0 * by).rotate(self.twice),
        }
    }

    /// The quartic in `tan(u/2)` this comes to, leading coefficient first.
    ///
    /// The leading coefficient is the equation read at `u = π`, which is the
    /// one angle the chart cannot name.
    fn quartic(&self) -> [f64; 5] {
        [
            self.flat - self.round.x + self.twice.x,
            2.0 * self.round.y - 4.0 * self.twice.y,
            2.0 * self.flat - 6.0 * self.twice.x,
            2.0 * self.round.y + 4.0 * self.twice.y,
            self.flat + self.round.x + self.twice.x,
        ]
    }

    /// The angles the equation is met at, in no order.
    fn turns(&self) -> Inline<f64, 4> {
        let mut found = Inline::none();
        let charted = self.charted();
        let [a, b, c, d, e] = charted.quartic;
        for at in quartic::roots(a, b, c, d, e) {
            found.push((2.0 * at.atan() + charted.by).rem_euclid(TAU));
        }
        found
    }

    /// How many angles the equation is met at.
    ///
    /// **The same number in every chart**, which is what lets [`Leaning::ends`]
    /// compare one `v` against the next: a turn of the angle carries each root
    /// to a root, so a chart taken up at one `v` and dropped at the next moves
    /// where the roots are read and not how many there are.
    fn count(&self) -> usize {
        let [a, b, c, d, e] = self.charted().quartic;
        quartic::counted(a, b, c, d, e)
    }

    /// The quartic to read this in, and where its own angle starts from.
    ///
    /// **The chart is turned before it is used.** A half-angle chart is blind
    /// at `u = π`, where its quartic drops to a cubic and a root runs off to
    /// infinity — and that is no corner case, a drill straight across the ring
    /// crossing it. So the blind spot is put at the angle of [`CHARTS`] the
    /// equation stands furthest from being met at, which leaves every root of
    /// the quartic as near nought as the equation allows.
    fn charted(&self) -> Charted {
        let (mut by, mut furthest) = (0.0, -1.0);
        for step in 0..CHARTS {
            let angle = TAU * step as f64 / CHARTS as f64;
            let held = self.at(angle).abs();
            if held > furthest {
                (furthest, by) = (held, angle - PI);
            }
        }
        Charted {
            quartic: self.turned(by).quartic(),
            by,
        }
    }
}

/// A [`Harmonics`] written as a quartic, and the angle that chart starts from.
#[derive(Debug, Clone, Copy)]
struct Charted {
    quartic: [f64; 5],
    by: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::geometry::axis::Axis;

    /// The ring every drill below is held against: three out to the tube's own
    /// centre, one thick, about the world's `+Y` through the origin.
    fn ring() -> Torus {
        Torus {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            major: 3.0,
            minor: 1.0,
        }
    }

    /// A drill of `radius` through `off`, running `way`, leaning on the ring's
    /// own axis.
    fn drill(off: DVec3, way: DVec3, radius: f64) -> Cylinder {
        Cylinder {
            axis: Axis::about(off, way.normalize()),
            radius,
        }
    }

    /// **The harmonics are the drill's own equation**, and what says so is the
    /// drill's own distance function at the place the ring's two angles name.
    ///
    /// `|q|² − (q·w)² − r²` is what [`Leaning::held`] gathers, so the two have
    /// to agree place for place — over a grid that covers both angles, and for
    /// a drill that leans out of the plane its axis shares with the ring's so
    /// that every one of the five harmonics carries something.
    #[test]
    fn the_harmonics_are_the_drills_own_equation() {
        let torus = ring();
        let tube = drill(DVec3::new(3.0, 0.0, 0.0), DVec3::new(0.3, 1.0, 0.4), 0.35);
        let leaning = Leaning::of(&tube, &torus);
        let mut worst = 0.0_f64;
        for down in 0..24 {
            let v = TAU * f64::from(down) / 24.0;
            let held = leaning.held(&torus, v);
            for round in 0..24 {
                let u = TAU * f64::from(round) / 24.0;
                let at = torus.at(DVec2::new(u, v)) - tube.axis.origin;
                let along = at.dot(tube.axis.direction);
                let want = at.length_squared() - along * along - tube.radius * tube.radius;
                worst = worst.max((held.at(u) - want).abs());
            }
        }
        // The terms gathered are of size ten and the equation is a difference
        // of them, so the last bits of an `f64` at that size is all there is
        // to lose.
        assert!(worst < 1e-13, "{worst} off the drill's own equation");
    }

    /// **Every turn the quartic answers puts a place on the drill**, and it
    /// answers all of them wherever the chart's blind spot lies.
    ///
    /// A drill straight across the ring is the one that crosses `u = π`: its
    /// curve reaches every angle round the axis, so at some `v` the equation is
    /// met exactly where the half-angle chart cannot name it. Held over a fine
    /// sweep of the tube, and counted against the equation's own sign changes
    /// so that a turn quietly lost is a turn missing here.
    #[test]
    fn every_turn_is_answered_wherever_the_blind_spot_lies() {
        let torus = ring();
        let leaning = Leaning::of(&drill(DVec3::new(0.0, 0.4, 0.0), DVec3::X, 0.3), &torus);
        let (mut worst, mut counted) = (0.0_f64, 0);
        for step in 0..200 {
            let v = TAU * f64::from(step) / 200.0;
            let held = leaning.held(&torus, v);
            let turns = leaning.turns(&torus, v);
            for &u in turns.all() {
                worst = worst.max(held.at(u).abs());
            }
            let mut crossings = 0;
            for round in 0..720 {
                let (lo, hi) = (
                    TAU * f64::from(round) / 720.0,
                    TAU * f64::from(round + 1) / 720.0,
                );
                if held.at(lo).is_sign_negative() != held.at(hi).is_sign_negative() {
                    crossings += 1;
                }
            }
            assert_eq!(
                turns.all().len(),
                crossings,
                "{crossings} crossings at v = {v} against {:?}",
                turns.all(),
            );
            counted += crossings;
        }
        assert!(counted > 0, "the drill missed the ring altogether");
        assert!(worst < 1e-12, "{worst} off the equation");
    }

    /// **The count read off the coefficients is the count the roots come to**,
    /// and neither of them turns with the chart.
    ///
    /// [`Leaning::ends`] reads the count off three polynomials in the quartic's
    /// coefficients where [`Harmonics::turns`] bisects for the roots
    /// themselves, and a disagreement between the two would move an end to
    /// where no turn arrives or leaves. Held in every chart, the one that hides
    /// a root at infinity included: a turn of the angle is a unimodular change
    /// of the quartic's own variable, which the count cannot see.
    #[test]
    fn the_count_is_the_count_the_roots_come_to_in_every_chart() {
        let torus = ring();
        let leaning = Leaning::of(
            &drill(DVec3::new(2.6, 0.0, 0.0), DVec3::new(0.4, 1.0, 0.2), 0.5),
            &torus,
        );
        let mut counted = 0;
        for step in 0..64 {
            let v = TAU * f64::from(step) / 64.0;
            let at = leaning.held(&torus, v);
            let want = at.turns().all().len();
            counted += want;
            for turn in 0..8 {
                let by = TAU * f64::from(turn) / 8.0;
                let [a, b, c, d, e] = at.turned(by).quartic();
                let got = quartic::counted(a, b, c, d, e);
                assert_eq!(got, want, "{got} rather than {want} at v = {v} from {by}");
            }
        }
        assert!(counted > 0, "the drill missed the ring altogether");
    }

    /// **The count of turns holds between the ends**, which is the whole of
    /// what the seeding asks of them.
    ///
    /// A stretch of the tube the ends cut out carries a fixed number of pieces
    /// of the curve. So the count of turns is read at the middle of every
    /// stretch and at three more places inside it, and a stretch whose count
    /// moves is an end that was missed.
    #[test]
    fn the_count_of_turns_holds_between_the_ends() {
        for (what, off, way, radius) in [
            (
                "leaning in the plane of the axes",
                DVec3::new(3.0, 0.0, 0.0),
                DVec3::new(0.3, 1.0, 0.0),
                0.3,
            ),
            (
                "leaning out of it",
                DVec3::new(3.0, 0.0, 0.0),
                DVec3::new(0.3, 1.0, 0.4),
                0.3,
            ),
            ("straight across the ring", DVec3::ZERO, DVec3::X, 0.3),
            (
                "across and raised",
                DVec3::new(0.0, 0.4, 0.0),
                DVec3::X,
                0.3,
            ),
        ] {
            let torus = ring();
            let leaning = Leaning::of(&drill(off, way, radius), &torus);
            let laid = leaning
                .ends(&torus)
                .unwrap_or_else(|| panic!("{what}: the ends outran the algebra"));
            let ends = laid.all();
            assert!(!ends.is_empty(), "{what}: no end anywhere");
            for step in 0..ends.len() {
                let (from, to) = (ends[step], ends[(step + 1) % ends.len()]);
                let span = (to - from).rem_euclid(TAU);
                let want = leaning.turns(&torus, from + span / 2.0).all().len();
                for share in [0.2, 0.4, 0.8] {
                    let v = from + span * share;
                    let got = leaning.turns(&torus, v).all().len();
                    assert_eq!(
                        got, want,
                        "{what}: {got} turns at {v}, {want} in the middle"
                    );
                }
            }
        }
    }
}
