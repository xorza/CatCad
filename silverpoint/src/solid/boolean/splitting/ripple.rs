//! The open cut an ellipse makes in a cylinder's own parameters.

use crate::inline::Inline;
use crate::math::arc;
use crate::math::bisect;
use crate::math::sinusoid;
use crate::solid::boolean::splitting::cut::ROUNDED;
use glam::DVec2;

/// A cut along `v = level + swing·cos(θ − phase)`, everything above it kept
/// where `above`.
///
/// **What an ellipse is in a cylinder's own parameters.** A plane meeting a
/// cylinder obliquely crosses it in one, and so does a second cylinder of the
/// same radius on a crossing axis — which is the mitred pipe and the Steinmetz
/// solid, and between them the whole of what M5 had left. On a plane that curve
/// is an ellipse and `Oval` carries it; on the cylinder it is a *graph over the
/// angle*, which is why it is its own shape rather than a case of one.
///
/// Open, not closed: the parameter it is a graph over wraps, but a face may not
/// — `.notes/KERNEL.md` §4.4 — so within any one face it runs right across like
/// a line. It can still be met twice by one straight run, which a line cannot,
/// and that is the one place the two part company.
///
/// **A type rather than a variant's fields**, for the reason `Oval` beside it
/// is one: written on the enum, each of the three below had to answer a
/// straight cut and an ellipse with a made-up number.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ripple {
    /// The height it swings about, and how far either way.
    pub(crate) level: f64,
    pub(crate) swing: f64,
    /// The angle its high side stands at.
    pub(crate) phase: f64,
    /// Whether what is kept is above it rather than below.
    pub(crate) above: bool,
    /// Which of the caller's runs this is — see `Came::Arc`.
    pub(crate) run: u32,
}

/// How much of a wave a straight run crosses, and where.
///
/// Three, because that is the most there can be: a run less than a whole turn
/// wide meets `v = swing·cos(θ − phase)` where a line meets a cosine, and the
/// difference of the two turns at most twice over that span — see
/// [`Ripple::crested`].
pub(crate) type Crested = Inline<f64, 3>;

impl Ripple {
    /// How high the wave stands at the angle `across`.
    pub(crate) fn crest(self, across: f64) -> f64 {
        self.level + self.swing * (across - self.phase).cos()
    }

    /// The place `down` along the cut, which is `Cut::down` read backwards —
    /// see there for why both are written.
    pub(crate) fn at(self, down: f64) -> DVec2 {
        // Keeping what is below runs the cut against the angle.
        let across = if self.above { down } else { -down };
        DVec2::new(across, self.crest(across))
    }

    /// How many chords a stretch of `sweep` of the wave is worth.
    ///
    /// Within [`ROUNDED`] of its own swing, which is both how far the wave
    /// reaches and how hard it bends — `level + swing·cos` has a second
    /// derivative of exactly `swing`. Through [`arc::chords`], which is the one
    /// rule everything that turns a curve into corners reads.
    pub(crate) fn steps(self, sweep: f64) -> usize {
        let swing = self.swing.abs();
        arc::chords(swing, sweep, swing * ROUNDED)
    }

    /// Where along the run from `from` to `to` the wave is met, in order.
    ///
    /// **Bisected, there being nothing to solve.** A straight run against
    /// `v = level + swing·cos(θ − phase)` is a line against a cosine, and that
    /// has no closed form — the one crossing in this kernel that has not. What
    /// there *is* in closed form is where the difference of the two turns:
    /// `swing·sin(θ − phase)·dθ = −dv`, which has at most two answers over a
    /// run less than a whole turn wide. Split there, the difference is monotone
    /// on each piece, so a sign change brackets exactly one root and bisection
    /// walks it down to the last bit the two ends can be told apart by.
    ///
    /// Converged rather than tolerated, which is the distinction that matters:
    /// what comes back is the root to the precision the numbers hold, not a
    /// place within some bound of it. Through [`bisect::crossed`], which is
    /// where both that and the graze policy are stated — an end that comes to
    /// nought is a turning place touching the wave rather than crossing it, and
    /// `Bow` next door is fenced and bisected the same way.
    pub(crate) fn crested(self, from: DVec2, to: DVec2) -> Crested {
        let run = to - from;
        let at = |along: f64| {
            let place = from.lerp(to, along);
            place.y - self.crest(place.x)
        };
        // The run split where the difference turns, ends included. Two turns at
        // most, and each of them once: the run is a stretch of one face's own
        // parameters and no face wraps, so it reaches over less than a whole
        // turn — and which way round it runs says nothing about that, which is
        // why the span is taken as a range rather than walked from one end.
        let mut turns: Inline<f64, 4> = Inline::two(0.0, 1.0);
        for turn in sinusoid::met(-run.y / (self.swing * run.x), self.phase, from.x, to.x) {
            turns.push(turn);
        }
        let turns = turns.all_mut();
        turns.sort_by(f64::total_cmp);
        let mut crested = Crested::none();
        for pair in turns.windows(2) {
            if let Some(root) = bisect::crossed(pair[0], pair[1], at) {
                crested.push(root);
            }
        }
        crested
    }
}
