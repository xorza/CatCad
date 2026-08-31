//! The cut a cylinder of another radius makes in a cylinder's own parameters.

use crate::inline::Inline;
use crate::math::arc;
use crate::math::bisect;
use crate::math::quadratic;
use crate::math::sinusoid;
use crate::solid::boolean::splitting::cut::ROUNDED;
use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// A cut along `v = level ± √(across² − (reach·sin(θ − phase) − off)²)`.
///
/// **What a *cross drilling* is in either cylinder's own parameters**, and the
/// case `.notes/KERNEL.md` M5 had left. Two cylinders on crossing axes meet in
/// an ellipse where their radii agree — [`Ripple`](super::ripple::Ripple)
/// carries that one, `v = level + swing·cos(θ − phase)`. Where the radii differ
/// the meeting is a quartic in space, [`Saddle`](crate::solid::geometry::saddle::Saddle),
/// and on either cylinder's own parameters it is this: a graph over the angle
/// again, with a root in it rather than a cosine.
///
/// **Derived rather than fitted.** With `d` the axis this is written on, `e`
/// the other, and `p(θ, v)` a place of this cylinder, being on the other one is
/// `|(p − o) × e|² = across²`. Split that into the part along `d` and the part
/// across it and it is a quadratic in `v` — and for axes that *cross square*,
/// which every drilling does, the linear term of that quadratic vanishes
/// outright. What is left is `(v − level)²` against a constant less a square,
/// which is the form above.
///
/// **Offset axes come free.** A second axis that passes this one by `off`
/// rather than meeting it changes nothing but that term: the linear part is
/// still nought, and `off` slides the swing.
///
/// **Two regimes, and one drilling gives both.** Where the other cylinder is
/// wide enough to swallow this one's width — `across ≥ reach + |off|` — each
/// branch of the `±` spans every angle and the cut is open, like a ripple, so
/// [`Bow::upper`] says which of the two it is. Where the other cylinder passes
/// wholly *inside* this one — `across + |off| < reach` — the two branches join
/// where the root closes and together make one closed loop, like an oval, so
/// `upper` says nothing and the cut is that whole loop. Which regime it is in
/// is [`Bow::closed`].
///
/// Nested cross-sections either way, and the overlapping case is refused
/// further up — see
/// [`Meeting::saddled`](crate::solid::meeting::Meeting), which says why.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bow {
    /// The other cylinder's radius.
    pub(crate) across: f64,
    /// This cylinder's own.
    pub(crate) reach: f64,
    /// The angle the other axis stands at.
    pub(crate) phase: f64,
    /// How far the other axis passes this one by, square to both.
    pub(crate) off: f64,
    /// How far along this cylinder's axis the two come nearest.
    pub(crate) level: f64,
    /// Which branch this is, where the two never join.
    pub(crate) upper: bool,
    /// Whether the side kept is the one the other axis stands on.
    pub(crate) inward: bool,
    /// Which of the caller's runs this is — see `Came::Arc`.
    pub(crate) run: u32,
}

/// How much of a bow a straight run crosses, and where.
///
/// **The widest answer any cut gives**, which is why `Cut::met` hands back one
/// of these whatever shape it was asked of.
///
/// Six, which is what the fences allow rather than what the geometry is likely
/// to give. Squared, the difference of a run and a bow turns where a line meets
/// a sinusoid of twice the angle — at most five times over a span less than a
/// whole turn — so there are six stretches it can be monotone over and one root
/// apiece. See [`Bow::bowed`], where those fences are laid.
pub(crate) type Bowed = Inline<f64, 6>;

impl Bow {
    /// How high the branch stands at `angle`, or `None` where the loop does
    /// not reach that far.
    ///
    /// `None` only in the closed regime, and only outside the angles the loop
    /// covers. An open bow answers everywhere, the other cylinder being wide
    /// enough to swallow this one.
    pub(crate) fn crest(self, angle: f64) -> Option<f64> {
        let under = self.under(angle);
        (under >= 0.0).then(|| {
            let root = under.sqrt();
            self.level + if self.upper { root } else { -root }
        })
    }

    /// Whether the two branches join rather than each spanning every angle.
    pub(crate) fn closed(self) -> bool {
        self.across + self.off.abs() < self.reach
    }

    /// How far off the cut `at` stands, positive on the side kept.
    ///
    /// **Two measures, because the two regimes bound different things.** A
    /// closed bow bounds a disc, and how far a place stands off it is how far
    /// it stands from the other cylinder's axis against that cylinder's own
    /// radius — a true distance, as `Oval::reach` is. An open bow divides the
    /// face into a strip above the branch and one below, and what says which is
    /// the straight climb to the branch, which overstates the distance to it by
    /// however steeply it runs and shares its sign and its nought.
    pub(crate) fn side(self, at: DVec2) -> f64 {
        let off = if self.closed() {
            // **Unwound, so the far half of this cylinder is never read as the
            // near one.** The loop about `phase` and the one about `phase + π`
            // are the same numbers apart from that, and a plain sine would
            // hold both at once — see [`unwound`].
            let leaning = self.reach * unwound(at.x - self.phase) - self.off;
            self.across - (at.y - self.level).hypot(leaning)
        } else if self.upper {
            self.spanned(at.x) - at.y
        } else {
            at.y - self.spanned(at.x)
        };
        if self.inward { off } else { -off }
    }

    /// How far along the cut `at` stands — see `Cut::down`, where what the
    /// number has to do is written down.
    pub(crate) fn down(self, at: DVec2) -> f64 {
        if self.closed() {
            // Counterclockwise keeps the disc on the left, so keeping
            // everything *but* it runs the other way round.
            let turned = self.turn(at).rem_euclid(TAU);
            if self.inward { turned } else { TAU - turned }
        } else if self.above() {
            at.x
        } else {
            -at.x
        }
    }

    /// The place `down` along the cut, which is [`Bow::down`] read backwards.
    pub(crate) fn at(self, down: f64) -> DVec2 {
        if self.closed() {
            let (up, round) = if self.inward { down } else { -down }.sin_cos();
            let leaning = self.across * round;
            let angle = self.phase + ((leaning + self.off) / self.reach).asin();
            DVec2::new(angle, self.level + self.across * up)
        } else {
            let angle = if self.above() { down } else { -down };
            DVec2::new(angle, self.spanned(angle))
        }
    }

    /// How many chords a stretch of `sweep` of the cut is worth.
    ///
    /// Within [`ROUNDED`] of the cut's own size, which is the classification
    /// tolerance the corners are for rather than a tolerance on any geometry —
    /// see there.
    pub(crate) fn steps(self, sweep: f64) -> usize {
        arc::chords(self.bending(), sweep, self.size() * ROUNDED)
    }

    /// A place well inside the loop, where every corner of a region stands on
    /// the cut and none of them can say which side it is.
    ///
    /// Where the other cylinder's axis pierces this one, which is as far inside
    /// the loop as anything gets. Only a closed bow has an inside, and only a
    /// closed bow is asked.
    pub(crate) fn middle(self) -> DVec2 {
        debug_assert!(self.closed(), "an open bow has no inside to stand in");
        DVec2::new(self.phase + (self.off / self.reach).asin(), self.level)
    }

    /// Where along the run from `from` to `to` the bow is met, in order.
    ///
    /// **Squared first, so that there is a function to fence.** A run against a
    /// root has no closed form and the root is not even defined everywhere; its
    /// *square* is a polynomial in the run's parameter against a sinusoid of
    /// twice the angle, defined all through. So what is solved is
    /// `(v − level)² + (reach·sin(θ−phase) − off)² = across²`, and each root of
    /// that is kept only where it lies on this cut rather than on the other
    /// branch or the other loop.
    ///
    /// **Fenced twice, which is what makes it rigorous.** The difference turns
    /// where its derivative is nought, and *that* has no closed form either —
    /// but its own derivative does: `cos 2ψ` written as `1 − 2sin²ψ` makes it a
    /// quadratic in `sin ψ`, at most two values and at most four angles over a
    /// turn. Fenced there the derivative is monotone and bisects; fenced at
    /// *its* roots the difference is monotone and bisects too. No tolerance
    /// anywhere in it, and no count taken on trust.
    pub(crate) fn bowed(self, from: DVec2, to: DVec2) -> Bowed {
        let run = to - from;
        let at = |along: f64| {
            let place = from.lerp(to, along);
            let rise = place.y - self.level;
            rise * rise - self.under(place.x)
        };
        // `d/dalong` of the above, worked out rather than differenced.
        let slope = |along: f64| {
            let place = from.lerp(to, along);
            let leaning = self.leaning(place.x);
            2.0 * (place.y - self.level) * run.y
                + 2.0 * leaning * self.reach * (place.x - self.phase).cos() * run.x
        };
        let mut fences: Inline<f64, 6> = Inline::two(0.0, 1.0);
        for turn in self.turning(from, to) {
            fences.push(turn);
        }
        let fences = sorted(fences.all_mut());
        // The derivative's own roots, which are where the difference turns.
        let mut turns: Inline<f64, 8> = Inline::two(0.0, 1.0);
        for pair in fences.windows(2) {
            if let Some(root) = bisect::crossed(pair[0], pair[1], slope) {
                turns.push(root);
            }
        }
        let turns = sorted(turns.all_mut());
        let mut bowed = Bowed::none();
        for pair in turns.windows(2) {
            let Some(root) = bisect::crossed(pair[0], pair[1], at) else {
                continue;
            };
            if self.holds(from.lerp(to, root)) {
                bowed.push(root);
            }
        }
        bowed
    }

    /// Whether a place the squared form answers for is on this cut.
    ///
    /// **The squared form holds more than one cut**, which is what this sorts
    /// out. Open, it holds both branches and each keeps its own side of the
    /// axis. Closed, it holds the loop about `phase` and the one about
    /// `phase + π`, and each keeps its own half of the cylinder.
    fn holds(self, at: DVec2) -> bool {
        if self.closed() {
            (at.x - self.phase).cos() > 0.0
        } else {
            (at.y >= self.level) == self.upper
        }
    }

    /// Whether what is kept lies above the branch rather than below it.
    ///
    /// The inside of the other cylinder is *below* the upper branch and
    /// *above* the lower one, so which way the two answers fall together is
    /// the one thing worth writing down.
    fn above(self) -> bool {
        self.upper != self.inward
    }

    /// How far the ruling at `angle` stands from the other axis, square to
    /// this cylinder's own.
    fn leaning(self, angle: f64) -> f64 {
        self.reach * (angle - self.phase).sin() - self.off
    }

    /// What stands under the root at `angle`.
    fn under(self, angle: f64) -> f64 {
        let leaning = self.leaning(angle);
        self.across * self.across - leaning * leaning
    }

    /// The same, where the two branches never join and there is always an
    /// answer.
    ///
    /// The level where there is not, which is a rounding at a tangency and
    /// nothing else: an open bow has `across ≥ reach + |off|`, so what stands
    /// under the root comes to `across² − (reach + |off|)²` at its lowest and
    /// that is not negative.
    fn spanned(self, angle: f64) -> f64 {
        self.crest(angle).unwrap_or(self.level)
    }

    /// Which angle round the loop `at` stands at.
    ///
    /// **The loop is a circle, in the two numbers that say where a place is
    /// against the other cylinder**: how far along this axis it stands, and how
    /// far off the other axis square to that. Both are `across` at their
    /// largest and the sum of their squares is `across²` the whole way round,
    /// so the loop is a plain angle in them — which is what makes a shape with
    /// a square root in it one closed run rather than two.
    fn turn(self, at: DVec2) -> f64 {
        (at.y - self.level).atan2(self.leaning(at.x))
    }

    /// How large the cut is in its own parameters, which is what [`ROUNDED`] is
    /// a fraction of.
    fn size(self) -> f64 {
        if self.closed() {
            let low = ((self.off - self.across) / self.reach).asin();
            let high = ((self.off + self.across) / self.reach).asin();
            self.across.max((high - low) / 2.0)
        } else {
            self.across
        }
    }

    /// How hard the cut bends, as a bound on the second derivative of the
    /// place with the parameter [`Bow::at`] reads.
    ///
    /// **Both regimes are held by the same three quantities**, and neither
    /// bound runs away where the root is shallow.
    ///
    /// Closed, the angle is `asin` of a cosine, so with `q = across/reach` and
    /// `s = (across + |off|)/reach` its second derivative is held by
    /// `q/√(1−s²) + s·q²/(1−s²)^{3/2}`, and the height is `across·sin` of the
    /// same parameter.
    ///
    /// Open, the height is `√(across² − m²)` against the angle and its second
    /// derivative is three terms over `√U`, `U` being `across² − (reach+|off|)²`
    /// at its smallest. Two of them are `reach²` and `across·reach`; the third
    /// would run away as the root shallows and does not, because where the root
    /// is shallowest the angle's own swing is stationary and the two cancel —
    /// it comes to `4√2/(3√3)` of `across·reach`, and three covers all of it.
    fn bending(self) -> f64 {
        if self.closed() {
            let quick = self.across / self.reach;
            let most = (self.across + self.off.abs()) / self.reach;
            let leaning = (1.0 - most * most).sqrt();
            quick / leaning + most * quick * quick / (leaning * leaning * leaning) + self.across
        } else {
            let widest = self.reach + self.off.abs();
            let spare = self.across * self.across - widest * widest;
            self.reach * (self.reach + 3.0 * self.across) / spare.sqrt()
        }
    }

    /// Where along the run the *slope* of the difference turns.
    ///
    /// `2m² + 2·reach²·cos 2ψ + 2·off·reach·sin ψ` is nought, which with
    /// `cos 2ψ = 1 − 2sin²ψ` is a quadratic in `sin ψ`: two values at most, and
    /// two angles apiece over a turn.
    fn turning(self, from: DVec2, to: DVec2) -> Inline<f64, 4> {
        let run = to - from;
        let mut found = Inline::none();
        if run.x == 0.0 {
            return found;
        }
        let (a, b, c) = (
            -2.0 * self.reach * self.reach,
            self.off * self.reach,
            run.y * run.y / (run.x * run.x) + self.reach * self.reach,
        );
        for sine in quadratic::roots(a, b, c).into_iter().flatten() {
            for turn in sinusoid::met(sine, self.phase, from.x, to.x) {
                found.push(turn);
            }
        }
        found
    }
}

/// The sine of `turn`, run on past a quarter turn instead of coming back.
///
/// **What tells the near half of a cylinder from the far half.** A closed bow
/// is one of two loops the same numbers describe — the drilling's entry and its
/// exit — and `sin` reads them alike, being as small half a turn away as it is
/// here. This runs on to two instead, so it is one to one over a whole turn and
/// the far loop stands further off than any radius reaches.
///
/// Monotone from `−2` at a half turn behind to `2` at a half turn ahead, and
/// cut there as an angle always is. That cut is a half turn from the loop's own
/// middle, where a bow is well outside itself either way.
fn unwound(turn: f64) -> f64 {
    let turn = (turn + PI).rem_euclid(TAU) - PI;
    if turn.abs() <= FRAC_PI_2 {
        turn.sin()
    } else {
        2.0 * turn.signum() - turn.sin()
    }
}

/// The places in `of`, in order.
fn sorted(of: &mut [f64]) -> &[f64] {
    of.sort_by(f64::total_cmp);
    of
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_6;

    /// How near a crossing has to land, which is the last bit of a bisection.
    const NEAR: f64 = 1e-12;

    /// **A bar drilled square through gives both regimes at once**, and every
    /// number below is read off the two cylinders rather than off the code.
    ///
    /// A bar of radius two about `z`, drilled by a hole of radius one about
    /// `x`. On the *bar* the imprint is `4sin²θ + v² = 1`, so
    /// `v = ±√(1 − 4sin²θ)`: it reaches only where `|sin θ| ≤ ½`, which is a
    /// closed loop between `±π/6`, and it stands `±1` at the angle facing the
    /// hole. On the *hole* the same meeting is `u² + cos²ψ = 4`, so
    /// `u = ±√(4 − cos²ψ)`, which is defined at every angle — the bar is wider
    /// than the hole, so the hole's wall is cut right round. One drilling, one
    /// shape, two regimes.
    ///
    /// **And a run across the closed one is met where the algebra says.** The
    /// level `v = ½` meets the loop where `¼ = 1 − 4sin²θ`, which is
    /// `sin θ = ±√3/4` — twice.
    #[test]
    fn a_bar_drilled_square_through_gives_both_regimes_and_the_right_crossings() {
        let bar = Bow {
            across: 1.0,
            reach: 2.0,
            phase: 0.0,
            off: 0.0,
            level: 0.0,
            upper: true,
            inward: true,
            run: 0,
        };
        let hole = |upper| Bow {
            across: 2.0,
            reach: 1.0,
            phase: -FRAC_PI_2,
            upper,
            ..bar
        };

        assert!(bar.closed(), "the bar's imprint is a loop");
        assert!(!hole(true).closed(), "the hole is cut right round");

        assert_eq!(bar.crest(0.0), Some(1.0), "facing the hole");
        let edge = bar.crest(FRAC_PI_6).expect("the loop reaches its end");
        assert!(
            edge.abs() < NEAR,
            "the loop closes at {edge} rather than nought"
        );
        assert_eq!(bar.crest(PI / 4.0), None, "past the end of the loop");

        let out = hole(true).crest(0.0).expect("the hole is cut everywhere");
        assert!((out - 3.0f64.sqrt()).abs() < NEAR, "{out} rather than √3");
        assert_eq!(hole(true).crest(FRAC_PI_2), Some(2.0), "square to the bar");

        // A level run across the loop, met twice.
        let want = (3.0f64.sqrt() / 4.0).asin();
        let (from, to) = (DVec2::new(-0.6, 0.5), DVec2::new(0.6, 0.5));
        let found = bar.bowed(from, to);
        assert_eq!(found.all().len(), 2, "{:?}", found.all());
        for (got, want) in found.all().iter().zip([-want, want]) {
            let at = from.lerp(to, *got).x;
            assert!((at - want).abs() < NEAR, "met at {at} rather than {want}");
        }

        // Clear over the top of the loop, and clear to one side of it.
        assert!(
            bar.bowed(DVec2::new(-0.6, 1.5), DVec2::new(0.6, 1.5))
                .all()
                .is_empty(),
            "a run over the loop met it",
        );
        assert!(
            bar.bowed(DVec2::new(1.0, -1.5), DVec2::new(1.0, 1.5))
                .all()
                .is_empty(),
            "a run beside the loop met it",
        );
        // The hole's own branches, each keeping its own side of the axis.
        let (from, to) = (DVec2::new(0.0, -3.0), DVec2::new(0.0, 3.0));
        for upper in [true, false] {
            let found = hole(upper).bowed(from, to);
            assert_eq!(found.all().len(), 1, "{upper} met {:?}", found.all());
            let at = from.lerp(to, found.all()[0]).y;
            let want = if upper { 3.0f64.sqrt() } else { -3.0f64.sqrt() };
            assert!((at - want).abs() < NEAR, "met at {at} rather than {want}");
        }
    }

    /// **The far loop is the same numbers with the other axis turned round**,
    /// and each of the two answers only for its own half of the bar.
    ///
    /// The hole of the test above leaves an entry at `θ = 0` and an exit at
    /// `θ = π`, and `phase + π` with `off` negated is the second. Held by
    /// asking each one where a place at the other's middle stands: inside its
    /// own loop, and outside the far one however small a plain sine would have
    /// made it look there.
    #[test]
    fn the_two_loops_of_one_drilling_keep_to_their_own_halves() {
        let near = Bow {
            across: 1.0,
            reach: 2.0,
            phase: 0.0,
            off: 0.5,
            level: 3.0,
            upper: true,
            inward: true,
            run: 0,
        };
        let far = Bow {
            phase: PI,
            off: -0.5,
            ..near
        };
        // Where each axis pierces the bar, which is `sin θ = off/reach` — one
        // answer either side of a quarter turn.
        assert!((near.middle().x - (0.25f64).asin()).abs() < NEAR);
        assert!((far.middle().x - (PI - (0.25f64).asin())).abs() < NEAR);
        assert_eq!(near.middle().y, 3.0, "the level rides along with it");

        for (here, there) in [(near, far), (far, near)] {
            assert!(here.side(here.middle()) > 0.0, "outside its own loop");
            assert!(here.side(there.middle()) < 0.0, "inside the far loop");
        }
        // A plain sine reads the far middle as the near one exactly: both
        // stand where `2 sin θ` comes to the offset, so it puts the far place
        // on the near loop's own middle line.
        let far_middle = far.middle().x;
        assert!(near.leaning(far_middle).abs() < NEAR, "the plain read");
        assert!(
            near.reach * unwound(far_middle - near.phase) - near.off > near.across,
            "the unwound read stands clear of the loop",
        );
    }

    /// **The parameter round a closed loop is an angle, and it inverts.**
    ///
    /// A whole turn of it walks the loop once, so the two halves of the root
    /// are one run: a quarter turn stands at the top of the loop, three
    /// quarters at the bottom, and nought and a half turn at the two angles
    /// the root closes at. Every place read back gives the parameter it came
    /// from, and every place is on the cut.
    #[test]
    fn a_closed_bow_walks_its_loop_once_and_reads_back() {
        let bow = Bow {
            across: 1.0,
            reach: 2.0,
            phase: 0.3,
            off: 0.5,
            level: -1.0,
            upper: true,
            inward: true,
            run: 0,
        };
        for step in 0..16 {
            let down = TAU * step as f64 / 16.0;
            let at = bow.at(down);
            assert!(bow.side(at).abs() < NEAR, "{at:?} is off the cut");
            let back = bow.down(at);
            assert!(
                (back - down)
                    .rem_euclid(TAU)
                    .min((down - back).rem_euclid(TAU))
                    < NEAR
            );
        }
        // A quarter turn is the top and three quarters the bottom, both a
        // radius of the other cylinder away from the level.
        assert!((bow.at(TAU / 4.0).y - (-1.0 + 1.0)).abs() < NEAR);
        assert!((bow.at(3.0 * TAU / 4.0).y - (-1.0 - 1.0)).abs() < NEAR);
        // And the ends, where the root closes: `sin ψ = (off ± across)/reach`.
        for (down, want) in [(0.0, 0.75f64), (PI, -0.25)] {
            let at = bow.at(down);
            assert!((at.y - bow.level).abs() < NEAR, "the root did not close");
            assert!(((at.x - bow.phase).sin() - want).abs() < NEAR, "{at:?}");
        }
        // And the side kept is on the left of the way it runs, which is what
        // the whole reassembly reads.
        assert!(kept_on_the_left(bow), "the loop ran the wrong way round");
        assert!(kept_on_the_left(Bow {
            inward: false,
            ..bow
        }));

        // Turned over, the same loop runs the other way: the same parameter
        // gives the place a whole turn less it.
        let turned = Bow {
            inward: false,
            ..bow
        };
        for step in 1..16 {
            let down = TAU * step as f64 / 16.0;
            let there = turned.at(down);
            assert!((there - bow.at(TAU - down)).length() < NEAR, "{there:?}");
        }
    }

    /// **An open bow keeps the side its two flags name**, which is four
    /// answers off two questions.
    ///
    /// The hole of the first test, whose upper branch stands at `+√3` at the
    /// angle facing the bar. Above it is *outside* the bar and below it is
    /// inside, so keeping the inside is keeping what is below — and the lower
    /// branch is that read the other way round.
    #[test]
    fn an_open_bow_keeps_the_side_its_two_flags_name() {
        let hole = |upper, inward| Bow {
            across: 2.0,
            reach: 1.0,
            phase: -FRAC_PI_2,
            off: 0.0,
            level: 0.0,
            upper,
            inward,
            run: 0,
        };
        // Three places, one under both branches, one between them and one
        // over both. The branches stand at `±√3` at this angle.
        let below = DVec2::new(0.0, -3.0);
        let between = DVec2::new(0.0, 0.0);
        let above = DVec2::new(0.0, 3.0);
        for (upper, inward, kept) in [
            (true, true, [below, between]),
            (true, false, [above, above]),
            (false, true, [between, above]),
            (false, false, [below, below]),
        ] {
            let bow = hole(upper, inward);
            for at in [below, between, above] {
                let want = kept.contains(&at);
                let side = bow.side(at);
                assert_eq!(side > 0.0, want, "{upper} {inward} put {at:?} at {side}");
            }
            assert!(kept_on_the_left(bow), "{upper} {inward} ran the wrong way");
        }
    }

    /// Whether a step to the left of the way the cut runs lands on the side it
    /// keeps, which is the one rule every cut is walked by.
    fn kept_on_the_left(bow: Bow) -> bool {
        let here = bow.at(0.0);
        let ahead = bow.at(0.1) - here;
        let left = DVec2::new(-ahead.y, ahead.x).normalize() * 1e-4;
        bow.side(here + left) > 0.0
    }
}
