//! The cut a cylinder of another radius makes in a cylinder's own parameters.
//!
//! **Not a [`Cut`](super::cut::Cut) arm yet.** The shape and its crossing solve
//! land first; the nine questions a cut answers, and the two regimes it answers
//! them in, come with the arm.
#![allow(dead_code)]

use crate::inline::Inline;
use crate::math::bisect;
use crate::math::quadratic;
use glam::DVec2;
use std::f64::consts::{PI, TAU};

/// A cut along `v = ±√(across² − (reach·sin(θ − phase) − off)²)`.
///
/// **What a *cross drilling* is in the bar's own parameters**, and the case
/// `.notes/KERNEL.md` M5 has left. Two cylinders on crossing axes meet in an
/// ellipse where their radii agree — [`Ripple`](super::ripple::Ripple) carries
/// that one, `v = level + swing·cos(θ − phase)`. Where the radii differ the
/// meeting is a quartic in space, and on either cylinder's own parameters it is
/// this: a graph over the angle again, with a root in it rather than a cosine.
///
/// **Derived rather than fitted.** With `d` the axis this is written on, `e`
/// the other, `w` between the two origins and `p(θ, v)` a place of this
/// cylinder, being on the other one is `|(p − o) × e|² = across²`. Split that
/// into the part along `d` and the part across it and it is a quadratic in `v`
/// — and for axes that *cross square*, which every drilling does, the linear
/// term of that quadratic vanishes outright. What is left is `v²` against a
/// constant less a square, which is the form above.
///
/// **Offset axes come free.** A second axis that passes this one by `off`
/// rather than meeting it changes nothing but that term: the linear part is
/// still nought, and `off` slides the swing. So the drilled, the offset and the
/// tangent case are one shape with three sets of numbers.
///
/// **Two branches and two regimes.** The `±` is two cuts, not one, exactly as
/// two ellipses are two ripples. Where the other cylinder is wide enough to
/// swallow this one's width — `across ≥ reach + |off|` — each branch spans
/// every angle and the cut is open, like a ripple. Where it is not, the two
/// branches join at the angles the root closes at and together make one closed
/// loop, like an oval. Which regime it is in is [`Bow::closed`].
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
    /// Which of the two branches this is.
    pub(crate) upper: bool,
}

/// How much of a bow a straight run crosses, and where.
///
/// Six, which is what the fences allow rather than what the geometry is likely
/// to give. Squared, the difference of a run and a bow turns where a line meets
/// a sinusoid of twice the angle — at most five times over a span less than a
/// whole turn — so there are six stretches it can be monotone over and one root
/// apiece. See [`Bow::bowed`], where those fences are laid.
pub(crate) type Bowed = Inline<f64, 6>;

impl Bow {
    /// How high the branch stands at the angle `across`, or `None` where it
    /// does not reach that far.
    ///
    /// `None` only in the closed regime, and only outside the angles the loop
    /// covers. An open bow answers everywhere.
    pub(crate) fn crest(self, across: f64) -> Option<f64> {
        let under = self.under(across);
        (under >= 0.0).then(|| {
            let root = under.sqrt();
            if self.upper { root } else { -root }
        })
    }

    /// Whether the two branches join rather than each spanning every angle.
    pub(crate) fn closed(self) -> bool {
        self.across < self.reach + self.off.abs()
    }

    /// What stands under the root at the angle `across`.
    fn under(self, across: f64) -> f64 {
        let leaning = self.reach * (across - self.phase).sin() - self.off;
        self.across * self.across - leaning * leaning
    }

    /// Where along the run from `from` to `to` the bow is met, in order.
    ///
    /// **Squared first, so that there is a function to fence.** A run against a
    /// root has no closed form and the root is not even defined everywhere; its
    /// *square* is a polynomial in the run's parameter against a sinusoid of
    /// twice the angle, defined all through. So what is solved is
    /// `v² + (reach·sin(θ−phase) − off)² = across²`, and each root of that is
    /// kept only where the run's own `v` is on this branch's side of nought.
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
            place.y * place.y - self.under(place.x)
        };
        // `d/dalong` of the above, worked out rather than differenced.
        let slope = |along: f64| {
            let place = from.lerp(to, along);
            let leaning = self.reach * (place.x - self.phase).sin() - self.off;
            2.0 * place.y * run.y
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
            let place = from.lerp(to, root);
            // The squared form holds both branches; this one keeps its own.
            if (place.y >= 0.0) == self.upper {
                bowed.push(root);
            }
        }
        bowed
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
            if sine.abs() > 1.0 {
                continue;
            }
            let first = sine.asin();
            for turn in [first, PI - first] {
                let (lo, hi) = (from.x.min(to.x), from.x.max(to.x));
                let over = ((lo - self.phase - turn) / TAU).ceil();
                let angle = self.phase + turn + TAU * over;
                let along = (angle - from.x) / run.x;
                if angle < hi && (0.0..1.0).contains(&along) {
                    found.push(along);
                }
            }
        }
        found
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
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_6, PI};

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
    /// level `v = ½` meets the upper branch where `¼ = 1 − 4sin²θ`, which is
    /// `sin θ = ±√3/4` — twice, and the lower branch not at all. That the
    /// branch filter holds is the whole of what tells one half of a squared
    /// equation from the other.
    #[test]
    fn a_bar_drilled_square_through_gives_both_regimes_and_the_right_crossings() {
        let bar = |upper| Bow {
            across: 1.0,
            reach: 2.0,
            phase: 0.0,
            off: 0.0,
            upper,
        };
        let hole = |upper| Bow {
            across: 2.0,
            reach: 1.0,
            phase: -FRAC_PI_2,
            off: 0.0,
            upper,
        };

        assert!(bar(true).closed(), "the bar's imprint is a loop");
        assert!(!hole(true).closed(), "the hole is cut right round");

        assert_eq!(bar(true).crest(0.0), Some(1.0), "facing the hole");
        assert_eq!(bar(false).crest(0.0), Some(-1.0), "the other branch");
        let edge = bar(true)
            .crest(FRAC_PI_6)
            .expect("the loop reaches its end");
        assert!(
            edge.abs() < NEAR,
            "the loop closes at {edge} rather than nought"
        );
        assert_eq!(bar(true).crest(PI / 4.0), None, "past the end of the loop");

        let out = hole(true).crest(0.0).expect("the hole is cut everywhere");
        assert!((out - 3.0f64.sqrt()).abs() < NEAR, "{out} rather than √3");
        assert_eq!(hole(true).crest(FRAC_PI_2), Some(2.0), "square to the bar");

        // A level run across the loop, met twice on the upper branch and never
        // on the lower.
        let want = (3.0f64.sqrt() / 4.0).asin();
        let (from, to) = (DVec2::new(-0.6, 0.5), DVec2::new(0.6, 0.5));
        let found = bar(true).bowed(from, to);
        assert_eq!(found.all().len(), 2, "{:?}", found.all());
        for (got, want) in found.all().iter().zip([-want, want]) {
            let at = from.lerp(to, *got).x;
            assert!((at - want).abs() < NEAR, "met at {at} rather than {want}");
        }
        assert!(
            bar(false).bowed(from, to).all().is_empty(),
            "the lower branch answered a run above it",
        );

        // Clear over the top of the loop, and clear to one side of it.
        assert!(
            bar(true)
                .bowed(DVec2::new(-0.6, 1.5), DVec2::new(0.6, 1.5))
                .all()
                .is_empty(),
            "a run over the loop met it",
        );
        assert!(
            bar(true)
                .bowed(DVec2::new(1.0, -1.5), DVec2::new(1.0, 1.5))
                .all()
                .is_empty(),
            "a run beside the loop met it",
        );
    }
}
