//! The open cut a parabola or a hyperbola makes in a plane's own parameters.

use crate::inline::Inline;
use crate::math::arc;
use crate::math::bounds::Bounds;
use crate::math::quadratic;
use crate::solid::boolean::splitting::cut::ROUNDED;
use glam::DVec2;

/// A cut along `y = x²/(L + √(L² + ε·x²))` about its own vertex, everything
/// above it kept where `above`.
///
/// **What a plane cuts out of a cone once the section stops closing.** A plane
/// leaning less than a cone's own rulings cuts an ellipse, which `Oval` carries;
/// one leaning past them cuts a hyperbola and one parallel to a ruling a
/// parabola, and both of those are this. The face being cut is the plane's, so
/// what is wanted is the conic laid out flat — see
/// [`Hyperbola`](crate::solid::geometry::hyperbola::Hyperbola) and
/// [`Parabola`](crate::solid::geometry::parabola::Parabola), which are the same
/// two curves in space.
///
/// **One shape for the pair, and the vertex form is what makes it one.** Every
/// conic reads `ε·y² + 2L·y − x² = 0` about its own vertex, for a semi-latus
/// rectum `L` and an `ε` that is `e² − 1` — nought for a parabola, positive for
/// a hyperbola, negative for an ellipse. Solved for `y` and rationalized, that
/// is the graph above: one expression, no case for `ε = 0`, and no cancellation
/// where the two branches of the algebra would meet.
///
/// **One branch, and that is what a cut needs.** The whole of a hyperbola
/// divides a plane into three, and a cut divides a face into two — so a meeting
/// hands its two branches over as two curves and each is its own cut. The graph
/// above is single-valued over every `x` where `ε ≥ 0`, so the two sides of it
/// are the two sides of the face.
///
/// Open, like [`Ripple`](super::ripple::Ripple) next door and unlike an oval: it
/// runs from one edge of the face to the other. It can still be met twice by one
/// straight run, which a line cannot.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bough {
    /// Where the branch turns, which is where the graph reads nought.
    pub(crate) at: DVec2,
    /// Unit, the way the graph's own first parameter runs. The branch opens
    /// along its [`DVec2::perp`].
    pub(crate) across: DVec2,
    /// The semi-latus rectum, which is the half-width at the focus and what
    /// says how hard the branch turns about its vertex.
    pub(crate) latus: f64,
    /// `e² − 1`: nought for a parabola and positive for a hyperbola's branch.
    pub(crate) bend: f64,
    /// Whether what is kept is the side the branch opens toward.
    pub(crate) above: bool,
    /// Which of the caller's runs this is — see `Came::Arc`.
    pub(crate) run: u32,
}

/// Where a straight run crosses a branch, and how far along it.
///
/// Two, because that is the most there can be: a line meets a conic where a
/// quadratic has roots.
pub(crate) type Crossed = Inline<f64, 2>;

impl Bough {
    /// The direction the branch opens toward.
    fn up(self) -> DVec2 {
        self.across.perp()
    }

    /// How far the branch stands from its vertex line at `across`.
    ///
    /// **Rationalized, so one expression covers both.** Solving
    /// `ε·y² + 2L·y − x² = 0` gives `y = (√(L² + εx²) − L)/ε`, which is a
    /// difference of near-equal numbers for a shallow branch and nothing at all
    /// for a parabola. Multiplied above and below by the sum, it is the form
    /// here — which reads `x²/2L` at `ε = 0` rather than dividing by it.
    pub(crate) fn crest(self, across: f64) -> f64 {
        let square = across * across;
        square / (self.latus + (self.latus * self.latus + self.bend * square).sqrt())
    }

    /// How far off the cut `point` stands, positive on the side being kept —
    /// see `Cut::side`.
    ///
    /// **Straight up the way the branch opens**, which is a distance in that
    /// direction and an overstatement of the distance to the branch itself by
    /// however steeply it runs. The sign and the nought are exact, and those
    /// are what the walk reads.
    pub(crate) fn side(self, point: DVec2) -> f64 {
        let out = point - self.at;
        let off = self.up().dot(out) - self.crest(self.across.dot(out));
        if self.above { off } else { -off }
    }

    /// How far along the cut `point` stands — see `Cut::down`.
    ///
    /// The branch's own first parameter, which is a length rather than an angle
    /// and grows the way the cut runs for the same reason
    /// [`Ripple`](super::ripple::Ripple)'s does.
    pub(crate) fn down(self, point: DVec2) -> f64 {
        let across = self.across.dot(point - self.at);
        if self.above { across } else { -across }
    }

    /// The place `down` along the cut, which is [`Bough::down`] read backwards —
    /// see `Cut::down` for why both are written.
    pub(crate) fn at(self, down: f64) -> DVec2 {
        // Keeping what is below runs the cut against its own first parameter.
        let across = if self.above { down } else { -down };
        self.at + self.across * across + self.up() * self.crest(across)
    }

    /// How many chords a stretch of `sweep` of the branch is worth.
    ///
    /// **Measured in its own semi-latus rectum, where every branch bends no
    /// harder than the unit circle.** The graph's second derivative is
    /// `L²/(L² + εx²)^{3/2}`, largest at the vertex where it is `1/L` — so read
    /// in units of `L` it is at most one, and [`arc::chords`] answers for a
    /// radius of one. Which is the same rule everything else here reads, asked
    /// of a curve whose parameter is a length rather than an angle.
    pub(crate) fn steps(self, sweep: f64) -> usize {
        arc::chords(1.0, sweep / self.latus, ROUNDED)
    }

    /// Where along the run from `from` to `to` the branch is met, in order.
    ///
    /// **Solved rather than walked**, which is what the vertex form buys: a
    /// straight run against `ε·y² + 2L·y − x² = 0` is a quadratic in the run's
    /// own parameter, and `Ripple` next door has to bisect for want of one.
    ///
    /// **The far branch is dropped here.** For a hyperbola that equation holds
    /// both branches, and this cut is one of them — so a root is kept only
    /// where it lands on the side of the vertex line the branch is on.
    ///
    /// A graze counts for none, on the terms
    /// [`quadratic::roots`] states.
    pub(crate) fn crossed(self, from: DVec2, to: DVec2) -> Crossed {
        let (start, run) = (from - self.at, to - from);
        let (x, dx) = (self.across.dot(start), self.across.dot(run));
        let (y, dy) = (self.up().dot(start), self.up().dot(run));
        let square = self.bend * dy * dy - dx * dx;
        let linear = 2.0 * (self.bend * y * dy + self.latus * dy - x * dx);
        let constant = self.bend * y * y + 2.0 * self.latus * y - x * x;
        // A run along an asymptote, or up a parabola's own axis: the quadratic
        // has lost its leading term and one crossing is all there is.
        let found = match (square == 0.0, quadratic::roots(square, linear, constant)) {
            (true, _) if linear != 0.0 => Crossed::one(-constant / linear),
            (false, Some([one, two])) => Crossed::two(one, two),
            _ => Crossed::none(),
        };
        let mut crossed = Crossed::none();
        for along in found {
            if y + along * dy >= 0.0 {
                crossed.push(along);
            }
        }
        crossed
    }

    /// Whether any of the branch runs through the box `fills`.
    ///
    /// **In the branch's own frame**, which the box is not aligned to — so what
    /// is compared is the box's *support* each way, which holds the true box
    /// and is four multiplications. Coarse where the frame leans and not wrong,
    /// which is the whole of what a cull owes.
    ///
    /// The graph rises with `|x|` from nought at the vertex, so the least it
    /// stands over the box's own stretch of `x` is at whichever end is nearer
    /// nought and the most is at the further.
    pub(crate) fn reaches(self, fills: Bounds<DVec2>) -> bool {
        let (middle, half) = (fills.middle() - self.at, fills.half());
        let support = |way: DVec2| way.x.abs() * half.x + way.y.abs() * half.y;
        let (across, reach) = (self.across.dot(middle), support(self.across));
        let (up, rise) = (self.up().dot(middle), support(self.up()));
        let (low, high) = (across - reach, across + reach);
        let nearest = match low <= 0.0 && 0.0 <= high {
            true => 0.0,
            false => low.abs().min(high.abs()),
        };
        self.crest(nearest) <= up + rise && up - rise <= self.crest(low.abs().max(high.abs()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `y = x²/4` about the origin, which is `L = 2` and no eccentricity to
    /// spare.
    fn parabola() -> Bough {
        Bough {
            at: DVec2::ZERO,
            across: DVec2::X,
            latus: 2.0,
            bend: 0.0,
            above: true,
            run: 0,
        }
    }

    /// `y = √(1 + x²) − 1`, which is the upper branch of `(y+1)² − x² = 1`
    /// brought to its own vertex — one across and one along, so `L = b²/a = 1`
    /// and `ε = b²/a² = 1`.
    fn hyperbola() -> Bough {
        Bough {
            latus: 1.0,
            bend: 1.0,
            ..parabola()
        }
    }

    /// **The graph is the conic it says it is**, which the rationalized form
    /// makes worth holding: what it computes is
    /// `x²/(L + √(L² + εx²))` and what it means is `(√(L² + εx²) − L)/ε`, and
    /// the second is nothing at all at `ε = 0`.
    ///
    /// Held against `x²/2L` for the parabola and `√(1 + x²) − 1` for the
    /// branch, each written out rather than derived from the same expression.
    #[test]
    fn the_graph_is_the_conic_its_two_numbers_name() {
        for x in [0.0, 0.5, 1.0, 2.0, -3.0, 7.0] {
            let want = x * x / 4.0;
            assert!(
                (parabola().crest(x) - want).abs() < 1e-12,
                "{x} on a parabola"
            );
            let want = (1.0 + x * x).sqrt() - 1.0;
            assert!(
                (hyperbola().crest(x) - want).abs() < 1e-12,
                "{x} on a branch"
            );
        }
    }

    /// **A straight run meets a branch where a quadratic has roots**, which is
    /// the whole reason the vertex form is what is carried.
    ///
    /// Hand-computed. `y = x²/4` stands at `4` where `x = ±4`, so the run from
    /// `(−6, 4)` to `(6, 4)` is met a sixth and five sixths along it. One below
    /// the vertex is met nowhere. And one straight up the axis has lost the
    /// quadratic's leading term — `x` does not move — so it is met once, at the
    /// vertex.
    #[test]
    fn a_straight_run_meets_a_branch_where_its_quadratic_has_roots() {
        let bent = parabola();
        let met = bent.crossed(DVec2::new(-6.0, 4.0), DVec2::new(6.0, 4.0));
        assert_eq!(met.all().len(), 2, "{:?}", met.all());
        assert!((met.all()[0] - 1.0 / 6.0).abs() < 1e-12, "{:?}", met.all());
        assert!((met.all()[1] - 5.0 / 6.0).abs() < 1e-12, "{:?}", met.all());

        let under = bent.crossed(DVec2::new(-6.0, -1.0), DVec2::new(6.0, -1.0));
        assert!(under.all().is_empty(), "a run under the vertex was met");

        let up = bent.crossed(DVec2::new(0.0, -1.0), DVec2::new(0.0, 3.0));
        assert_eq!(up.all(), [0.25], "a run up the axis meets the vertex once");
    }

    /// **The other branch is dropped**, which the conic the roots come from
    /// does not do on its own: `ε·y² + 2L·y − x² = 0` holds both.
    ///
    /// Hand-computed. On `y = √(1 + x²) − 1` the far branch is
    /// `y = −1 − √(1 + x²)`, so a run up `x = 0` from `(0, −4)` to `(0, 4)`
    /// meets the pair at `y = −2` and `y = 0` — a quarter and a half along it.
    /// Only the second is on this cut.
    #[test]
    fn only_the_branch_the_cut_is_survives_its_own_quadratic() {
        let met = hyperbola().crossed(DVec2::new(0.0, -4.0), DVec2::new(0.0, 4.0));
        assert_eq!(met.all(), [0.5], "the far branch was kept");
    }

    /// **`side` reads nought on the cut and `at` reads the cut back**, which is
    /// what the reassembly leans on: it measures where a boundary met the cut
    /// and puts corners back along it by that measure alone.
    ///
    /// And **which way it runs keeps what is kept on the left**, which is the
    /// one thing the flag decides: the branch opens toward `+y`, so keeping
    /// that side runs along `+x` and keeping the other runs against it.
    #[test]
    fn a_place_on_the_cut_reads_nought_off_it_and_comes_back_from_its_own_measure() {
        for bough in [parabola(), hyperbola()] {
            for down in [-4.0, -1.5, 0.0, 0.75, 3.0] {
                let at = bough.at(down);
                assert!(bough.side(at).abs() < 1e-12, "{at:?} is not on the cut");
                assert!((bough.down(at) - down).abs() < 1e-12, "{at:?} came back");
            }
            // Above the branch is the side it opens toward, and the two flags
            // are the two sides of it.
            let over = bough.at(2.0) + DVec2::Y;
            assert!(bough.side(over) > 0.0, "the opened side reads low");
            let turned = Bough {
                above: false,
                ..bough
            };
            assert!(turned.side(over) < 0.0, "turning it kept the same side");
            assert_eq!(turned.down(over), -bough.down(over), "and ran the same way");
        }
    }

    /// **A branch reaches the box it passes through and no other**, which is
    /// what says a region is not worth walking.
    ///
    /// Hand-computed against `y = x²/4`. A box about the origin holds the
    /// vertex. One wholly under the vertex holds none of it, the graph never
    /// reading below nought. And one out at `x ∈ [4, 6]` is met only where its
    /// own `y` reaches the `[4, 9]` the branch covers there.
    #[test]
    fn a_branch_reaches_the_box_it_passes_through_and_no_other() {
        let bent = parabola();
        let held = |low: (f64, f64), high: (f64, f64)| {
            bent.reaches(Bounds {
                low: DVec2::new(low.0, low.1),
                high: DVec2::new(high.0, high.1),
            })
        };
        assert!(held((-1.0, -1.0), (1.0, 1.0)), "the vertex was missed");
        assert!(!held((-1.0, -3.0), (1.0, -2.0)), "a box under it was met");
        assert!(!held((4.0, 0.0), (6.0, 1.0)), "a box under the arm was met");
        assert!(held((4.0, 3.0), (6.0, 5.0)), "the arm was missed");
    }

    /// **How finely it is cut grows with the stretch and never comes to
    /// nothing**, which is the rule every curve here is chorded by.
    ///
    /// In its own semi-latus rectum, so a branch twice as slack over twice the
    /// stretch is cut into the same number of pieces.
    #[test]
    fn a_branch_is_cut_finer_the_further_it_is_followed() {
        let bent = parabola();
        assert!(
            bent.steps(0.0) >= 1,
            "a stretch of nothing is still a chord"
        );
        assert!(
            bent.steps(8.0) > bent.steps(2.0),
            "no finer over four times"
        );
        let slack = Bough { latus: 4.0, ..bent };
        assert_eq!(
            slack.steps(4.0),
            bent.steps(2.0),
            "the same shape twice over"
        );
    }
}
