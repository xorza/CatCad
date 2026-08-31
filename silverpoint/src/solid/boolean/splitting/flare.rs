//! The cut a plane makes in a cone's own parameters.

use crate::math::arc;
use crate::math::bisect;
use crate::math::bounds::Bounds;
use crate::math::intersect::{self, Span};
use crate::number::tolerance::PLACED;
use crate::solid::boolean::splitting::cut::ROUNDED;
use glam::DVec2;

/// A cut along `v·(level + swing·cos(θ − phase)) = apart`, the side kept being
/// below it in `v` where `under`.
///
/// **What *any* plane is in a cone's own parameters**, which is the one thing
/// that makes this shape worth a file. A cone reads `v` along its axis and
/// scales the radius by `v·tan α`, so a place of it is
/// `apex + v·a + v·tan α·radial(θ)`. Put that into `n·(x − o) = 0` and every
/// term carries one `v`: what is left is the reading above, for `level = n·a`,
/// `swing = tan α·|n − a(n·a)|` and a phase where the normal leans. One arm
/// covers the circle, the ellipse, the parabola and the hyperbola — the four
/// sections differ in `level` against `swing` and in nothing else.
///
/// **It flares rather than waving.** Where `level` is the larger the reading
/// never comes to nought and the cut is a graph over every angle, which is the
/// ellipse. Where `swing` is, the reading has two zeros — the angles the plane
/// runs parallel to a ruling at — and the cut runs away to `±∞` there. Those
/// are the hyperbola's asymptote directions, and the parabola is the one lean
/// between where the two zeros meet.
///
/// **One arc to a face, however many the section has.** A face lies on one
/// nappe, so `v` holds one sign across it; the cut stands at `apart/f(θ)`, so
/// the angles it reaches that nappe at are the angles `f` holds one sign over —
/// which is a single stretch, a cosine crossing each level twice a turn. The
/// other arc is the other branch, on the nappe no face of this body covers.
///
/// **`apart` is never nought.** A plane through the apex cuts straight rulings
/// rather than a conic, and that is answered where the meeting is worked out —
/// see [`Meeting::of`](crate::solid::meeting::Meeting) — so no cut is ever
/// built for one.
///
/// **And the side is read linearly in `v`.** `apart − v·f(θ)` changes sign
/// exactly across the cut wherever `f` is not nought, and holds `apart`'s own
/// sign where it is — which is the right answer there, the whole column beyond
/// a zero standing on the side the apex is on.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Flare {
    /// How the plane's normal leans on the axis, which is `n·a`.
    pub(crate) level: f64,
    /// How far the reading swings either side of that round the angle.
    pub(crate) swing: f64,
    /// The angle the swing peaks at.
    pub(crate) phase: f64,
    /// How far the plane stands off the apex along its own normal.
    pub(crate) apart: f64,
    /// Whether this arc of the section stands on the nappe `v` reads positive
    /// on. A plane past the rulings cuts one arc on each, and this is what
    /// tells the two apart — see [`Flare::reaches`].
    pub(crate) upward: bool,
    /// Whether the side kept is below the cut in `v`.
    pub(crate) under: bool,
    /// The furthest along its own axis the face reaches, which is the size the
    /// chording is held against — see [`Flare::steps`].
    pub(crate) reach: f64,
    /// Which of the caller's runs this is — see `Came::Arc`.
    pub(crate) run: u32,
}

impl Flare {
    /// The reading at the angle `across`, which the cut is where `v` times it
    /// comes to [`Flare::apart`].
    fn leaning(self, across: f64) -> f64 {
        self.level + self.swing * (across - self.phase).cos()
    }

    /// Whether the reading is positive along the arc this face holds.
    ///
    /// `v` on the cut is `apart/f`, so which sign `f` takes there is the face's
    /// own nappe read against which side of the apex the plane stands.
    fn over(self) -> bool {
        (self.apart > 0.0) == self.upward
    }

    /// How far off the cut `point` stands, positive on the side being kept —
    /// see `Cut::side`.
    ///
    /// **Linear in `v` and scaled by the reading**, so it is a distance
    /// understated by however flat the plane lies against the ruling there. The
    /// sign and the nought are exact, which is what the walk reads.
    pub(crate) fn side(self, point: DVec2) -> f64 {
        let off = self.apart - point.y * self.leaning(point.x);
        // The reading falls with `v` at the rate `f`, so a positive `f` puts
        // the side this measures below the cut and a negative one above.
        if self.under == self.over() { off } else { -off }
    }

    /// How far along the cut `point` stands — see `Cut::down`.
    ///
    /// The cone's own angle, which keeps what is kept on the left for the
    /// reason [`Ripple`](super::ripple::Ripple)'s does: below runs against it.
    pub(crate) fn down(self, point: DVec2) -> f64 {
        if self.under { -point.x } else { point.x }
    }

    /// The place `down` along the cut, which is [`Flare::down`] read backwards.
    pub(crate) fn at(self, down: f64) -> DVec2 {
        self.rise(if self.under { -down } else { down })
    }

    /// The place the cut stands at the angle `across`.
    ///
    /// Away at the two angles the reading comes to nought, which is the flare
    /// itself — no face reaches there and [`Flare::grazes`] drops what it lays
    /// down past its own [`Flare::reach`].
    fn rise(self, across: f64) -> DVec2 {
        DVec2::new(across, self.apart / self.leaning(across))
    }

    /// How many chords a stretch of `sweep` of the cut is worth.
    ///
    /// **Bounded by how hard the flare bends at its hardest.** With
    /// `v = apart/f` the second derivative is
    /// `apart·(2f'²/f³ − f''/f²)`, and over the face `|v| ≤ reach` gives
    /// `|f| ≥ |apart|/reach` — so it is at most `reach·(2k² + k)` for
    /// `k = swing·reach/|apart|`, which is the shape's own slackness and
    /// carries no size. Held to [`ROUNDED`] of the reach, so the `reach`
    /// cancels and what is left is a count per radian.
    ///
    /// A plane nearly square across the cone has `k` at nought and the cut is a
    /// line in these parameters, which one chord covers — which is the right
    /// answer rather than a floor.
    pub(crate) fn steps(self, sweep: f64) -> usize {
        let slack = self.swing * self.reach / self.apart.abs();
        arc::chords(slack * (2.0 * slack + 1.0), sweep, ROUNDED)
    }

    /// Whether any of the cut runs through the box `fills`.
    ///
    /// **A half-band in `v` alone**, the angle bounding nothing: the reading is
    /// at most `|level| + swing` anywhere, so the cut never comes nearer the
    /// apex than `|apart|` over that — and it runs away from there without
    /// bound, on whichever nappe the face stands.
    pub(crate) fn reaches(self, fills: Bounds<DVec2>) -> bool {
        let least = self.apart.abs() / (self.level.abs() + self.swing);
        match self.upward {
            true => fills.high.y >= least,
            false => fills.low.y <= -least,
        }
    }

    /// Where the straight run from `from` to `to` crosses it.
    ///
    /// **Bisected on the side rather than solved**, there being nothing to
    /// solve: the reading is a line times a cosine, and neither its roots nor
    /// its derivative's are closed form. [`Bow`](super::bow::Bow) next door is
    /// fenced twice for that — at the closed-form roots of its second
    /// derivative, then at the bisected roots of its first — and this one
    /// cannot be, the linear factor staying in the second derivative and
    /// leaving that with no closed-form root either.
    ///
    /// What is available is the bracket: the two ends stand either side of the
    /// cut, which every caller has just established. The same bargain
    /// [`Traced::crossing`](super::traced::Traced) strikes.
    pub(crate) fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        let at = |along: f64| self.side(from.lerp(to, along));
        let along = bisect::crossed(0.0, 1.0, at).expect("the run crosses the cut");
        from.lerp(to, along)
    }

    /// Where the straight run from `from` to `to` crosses it *twice*, both ends
    /// standing on the same side.
    ///
    /// **Against the chords the cut lays down rather than against the reading**,
    /// which is the one question here a bisection cannot be given a bracket
    /// for: what says there is a dip at all is finding it. The chords are what
    /// the cut puts into a region's boundary — see `Cut::between` — so a dip
    /// found against them is a dip in the loops that come out. The same bargain
    /// [`Traced::grazes`](super::traced::Traced) strikes, and for the same
    /// reason.
    ///
    pub(crate) fn grazes(self, from: DVec2, to: DVec2) -> Option<[DVec2; 2]> {
        let (low, high) = (from.x.min(to.x), from.x.max(to.x));
        let count = self.steps(high - low);
        let span = Span { from, to };
        let mut dipped = [DVec2::ZERO; 2];
        let mut held = 0;
        let mut last = self.rise(low);
        for step in 1..=count {
            let here = self.rise(low + (high - low) * step as f64 / count as f64);
            // Both ends within the face, which is what keeps the flare itself
            // out: past the angles the reading comes to nought the cut runs
            // away, and a chord drawn across that crosses runs it never nears.
            let within = last.y.abs() <= self.reach && here.y.abs() <= self.reach;
            let chord = Span {
                from: last,
                to: here,
            };
            last = here;
            if !within {
                continue;
            }
            for crossing in intersect::spans(span, chord) {
                // A run through a corner of the chords is met by both of them,
                // ends counting for a crossing — see [`intersect::spans`].
                if held > 0 && crossing.at.distance(dipped[held - 1]) <= PLACED {
                    continue;
                }
                if held == dipped.len() {
                    return None;
                }
                (dipped[held], held) = (crossing.at, held + 1);
            }
        }
        (held == dipped.len()).then_some(dipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_3};

    /// The plane `x = 1` across a cone of one across for every two along, apex
    /// up, over a face reaching four down its own axis.
    ///
    /// `level` is nought, the plane running parallel to the axis, so the
    /// reading is `0.5·cos θ` and the cut is `v = 2/cos θ` — the hyperbola
    /// whose vertex stands two along at an angle of nought, and which runs away
    /// at the quarter turns either side.
    fn alongside() -> Flare {
        Flare {
            level: 0.0,
            swing: 0.5,
            phase: 0.0,
            apart: 1.0,
            upward: true,
            under: true,
            reach: 4.0,
            run: 0,
        }
    }

    /// **The cut stands where the reading says**, which is the whole of the
    /// shape: `v·f(θ) = apart`.
    ///
    /// Hand-computed on `v = 2/cos θ`: two along at an angle of nought, and
    /// four at a third of a half turn, where the cosine is a half.
    #[test]
    fn the_cut_stands_where_its_own_reading_says() {
        let flare = alongside();
        assert_eq!(flare.rise(0.0), DVec2::new(0.0, 2.0));
        let far = flare.rise(FRAC_PI_3);
        assert!((far.y - 4.0).abs() < 1e-12, "{far:?}");
        assert!(
            flare.side(flare.rise(0.0)).abs() < 1e-12,
            "the vertex is off"
        );
        assert!(flare.side(far).abs() < 1e-12, "{far:?} is off the cut");
    }

    /// **The side is read linearly in `v`, and reads right past the flare
    /// too.**
    ///
    /// The reading falls with `v`, so the apex's own side is below the cut and
    /// stands positive: at an angle of nought the cut is two along, so one
    /// along reads `1 − 0.5 = 0.5` and three reads `1 − 1.5 = −0.5`.
    ///
    /// **And past the quarter turn the whole column is the apex's.** There the
    /// reading is nought or the other sign, so `apart − v·f` cannot change sign
    /// however far along the face reaches — which is the right answer rather
    /// than an accident: the cut has run away to infinity and everything left
    /// stands on one side of it.
    #[test]
    fn the_side_reads_below_the_cut_and_past_the_flare_alike() {
        let flare = alongside();
        assert_eq!(flare.side(DVec2::new(0.0, 1.0)), 0.5);
        assert_eq!(flare.side(DVec2::new(0.0, 3.0)), -0.5);
        // The reading at the quarter turn is a cosine's own rounding off
        // nought, so the side there is `apart` less that.
        assert!((flare.side(DVec2::new(FRAC_PI_2, 3.0)) - 1.0).abs() < 1e-15);
        assert!(flare.side(DVec2::new(2.0 * FRAC_PI_3, 4.0)) > 0.0);

        // Turned over, which is the other side and the other way round.
        let turned = Flare {
            under: false,
            ..flare
        };
        assert_eq!(turned.side(DVec2::new(0.0, 1.0)), -0.5);
        let at = DVec2::new(0.3, 1.0);
        assert_eq!(turned.down(at), -flare.down(at));
    }

    /// **A place on the cut reads nought off it and comes back from its own
    /// measure**, which the reassembly leans on: it measures where a boundary
    /// met the cut and puts corners back along it by that measure alone.
    #[test]
    fn a_place_on_the_cut_comes_back_from_its_own_measure() {
        for flare in [
            alongside(),
            Flare {
                under: false,
                ..alongside()
            },
        ] {
            for across in [-0.9, -0.3, 0.0, 0.5, 1.0] {
                let at = flare.rise(across);
                assert!(flare.side(at).abs() < 1e-12, "{at:?} is not on the cut");
                let back = flare.at(flare.down(at));
                assert!(back.abs_diff_eq(at, 1e-12), "{at:?} came back {back:?}");
            }
        }
    }

    /// **A flare reaches the box it passes through and no other.**
    ///
    /// The reading is at most `|level| + swing`, which here is a half, so the
    /// cut never stands nearer the apex than `1/0.5 = 2` along the axis. A box
    /// that ends short of that holds none of it however wide it is across.
    #[test]
    fn a_flare_reaches_no_box_nearer_the_apex_than_its_own_least() {
        let flare = alongside();
        let held = |low: f64, high: f64| {
            flare.reaches(Bounds {
                low: DVec2::new(-9.0, low),
                high: DVec2::new(9.0, high),
            })
        };
        assert!(!held(0.0, 1.9), "a box short of the least was met");
        assert!(held(0.0, 2.0), "the least itself was missed");
        assert!(held(3.0, 4.0), "a box past it was missed");
        // On the other nappe the half-band is the other way round.
        let downward = Flare {
            upward: false,
            ..flare
        };
        assert!(!downward.reaches(Bounds {
            low: DVec2::new(-9.0, 0.0),
            high: DVec2::new(9.0, 4.0),
        }));
    }

    /// **A run straddling the cut is crossed where the reading comes to
    /// nought**, which is bisected rather than solved — see [`Flare::crossing`].
    ///
    /// Straight up the axis at an angle of nought, from one along to three: the
    /// cut is at two, which is halfway.
    #[test]
    fn a_run_straddling_the_cut_is_crossed_at_the_place_the_reading_says() {
        let flare = alongside();
        let at = flare.crossing(DVec2::new(0.0, 1.0), DVec2::new(0.0, 3.0));
        assert!(at.abs_diff_eq(DVec2::new(0.0, 2.0), 1e-12), "{at:?}");
    }

    /// **A run that dips across the cut and back is found against the chords it
    /// lays down**, which is the one question a bisection has no bracket for.
    ///
    /// A run along `v = 3` from an angle of minus one to one: the cut stands at
    /// `2/cos θ = 3`, which is `θ = ±acos(2/3) = ±0.8411`. Both ends of the run
    /// are outside that and the middle is inside, so the run dips.
    ///
    /// **Held to a chord's width and not to the last bit**, which is what
    /// finding it against chords buys and all it buys: the corners are for
    /// classification, at [`ROUNDED`] of the cut's own slackness.
    #[test]
    fn a_run_dipping_across_the_cut_is_found_against_its_own_chords() {
        let flare = alongside();
        let dipped = flare
            .grazes(DVec2::new(-1.0, 3.0), DVec2::new(1.0, 3.0))
            .expect("the run dips across the cut");
        let want = (2.0f64 / 3.0).acos();
        assert!((dipped[0].x + want).abs() < 0.02, "{dipped:?}");
        assert!((dipped[1].x - want).abs() < 0.02, "{dipped:?}");

        // A run that stays below the cut the whole way dips nowhere.
        let clear = flare.grazes(DVec2::new(-1.0, 1.0), DVec2::new(1.0, 1.0));
        assert!(clear.is_none(), "a run clear of the cut was met");
    }

    /// **How finely it is cut grows with the stretch, and a plane square across
    /// the cone needs one chord.**
    ///
    /// The cut is `v = apart/level` there, a line in these parameters — so the
    /// bound on how hard it bends comes to nought and one chord covers any
    /// stretch of it, which is the right answer rather than a floor.
    #[test]
    fn a_flare_is_cut_finer_the_further_it_bends() {
        let flare = alongside();
        assert!(flare.steps(1.0) > 1, "a flare was cut into one chord");
        assert!(flare.steps(2.0) > flare.steps(1.0), "no finer over twice");
        let square = Flare {
            swing: 0.0,
            level: 1.0,
            ..flare
        };
        assert_eq!(square.steps(1.0), 1, "a line wants one chord");
    }
}
