//! The curve two quadrics meet in where that curve is a smooth quartic, and
//! the store a body keeps them in.
//!
//! **What calls it is the boolean**, meeting a pair no row of the reducible
//! table answers — see `.notes/KERNEL.md` §7.3 for the route and §7.4 for the
//! cut it is laid into.
//!
//! The store is here beside what it holds, on the terms
//! [`marchings`](super::marchings) keeps: one file per subject, holding the
//! arena, the handle and whatever small thing either wants.

use crate::inline::Inline;
use crate::math::arc;
use crate::math::quartic;
use crate::number::exact::field::Field;
use crate::number::exact::quadratic::Quadratic;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::pencil::Pencil;
use crate::solid::geometry::quadric::Quadric;
use crate::solid::geometry::roots::{Along, Roots};
use crate::solid::geometry::ruled::Ruled;
use glam::{DVec3, DVec4};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// How many whole places a search for a ruled member will try.
///
/// **A bound rather than a proof.** Every pencil with a smooth quartic
/// intersection holds a ruled member with a rational place on it, and finding
/// one is a search rather than a formula.
///
/// **Not the search §4.2's spike measured.** That one wanted a member whose
/// determinant is a *square*, which parameterizes over ℚ alone, and found none
/// among four thousand three hundred candidates for two of three test pairs —
/// landing one is a rational point on a hyperelliptic curve. This search only
/// wants a member the signature calls ruled, which is a far commoner thing: the
/// origin answers for the cross-drilled pair. A bound that is reached is a pair
/// worth looking at rather than one to widen the bound for.
const CANDIDATES: i64 = 4;

/// An arc of the projective line a branch of the curve is real over.
///
/// **Read as an arc rather than as an interval**, because the parameter runs on
/// a circle and the affine chart cuts it. `Δ`'s roots divide that circle into
/// arcs, and the one holding the chart's own edge looks like *two* intervals to
/// anything that only knows the chart — a piece reaching past `+∞` and a piece
/// reaching back from `−∞`, which are one piece meeting at a place the chart
/// cannot name.
///
/// **`from > to` says the arc wraps.** Either end may also be infinite, which
/// is the arc that has no root to stop at.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Stretch {
    pub(crate) from: f64,
    pub(crate) to: f64,
}

impl Stretch {
    /// Where the branch stands `part` of the way along it, as the projective
    /// parameter [`Quartic::at`] takes.
    ///
    /// **Walked round the projective line rather than along the chart.** A
    /// stretch may reach either way without end — two crossing cylinders meet
    /// in one loop that closes through the ruling's own place at infinity — and
    /// a walk in the affine parameter can never arrive there. Written as
    /// `[cos θ, sin θ]` every place of the line is a finite angle, and the one
    /// at infinity is `θ = 0` like any other.
    ///
    /// `u = cot θ`, so the angle falls where the chart rises and the start of a
    /// stretch is its *larger* angle. Nothing here reverses for it — the two
    /// ends are read in the order they are stated and interpolated between —
    /// and what it buys is that no reader anywhere has to know the chart has an
    /// edge.
    pub(super) fn at(self, part: f64) -> [Rational; 2] {
        let (start, mut end) = (Self::angle(self.from), Self::angle(self.to));
        // **The long way round, where the arc wraps.** An angle and that angle
        // less a half turn are one place of the projective line, so an arc
        // through the chart's edge is walked by letting the angle fall past
        // nought rather than by climbing between the two ends the chart sees.
        if self.from > self.to {
            end -= std::f64::consts::PI;
        }
        let (sin, cos) = (start + (end - start) * part).sin_cos();
        [Rational::of(cos), Rational::of(sin)]
    }

    /// How far along it the place `u` stands, which is [`Stretch::at`] the
    /// other way round.
    ///
    /// **Brought onto the arc's own turn first.** An angle and that angle less
    /// a half turn are one place of the projective line, so a reading has to be
    /// put on the branch the stretch was walked over — the one within a quarter
    /// turn either side of its middle, which no place of an arc a half turn
    /// long or shorter is outside of.
    ///
    /// Outside `[0, 1]` where `u` stands off the arc, which is a place of the
    /// curve this component is not — see [`Filed::along`], which is what holds
    /// the answer to the component it was asked of.
    pub(super) fn along(self, u: [f64; 2]) -> f64 {
        let (start, mut end) = (Self::angle(self.from), Self::angle(self.to));
        if self.from > self.to {
            end -= PI;
        }
        // A stretch between two roots of `Δ` that fell together, which is one
        // place and not an arc: every part of it is that place, so any answer
        // is the answer.
        if end == start {
            return 0.0;
        }
        let middle = (start + end) / 2.0;
        let angle = middle + (u[1].atan2(u[0]) - middle + FRAC_PI_2).rem_euclid(PI) - FRAC_PI_2;
        (angle - start) / (end - start)
    }

    /// The angle on the projective line the affine parameter `u` stands at.
    ///
    /// **`atan2` of the pair rather than `π/2` less the angle of `u`**, which
    /// is the same number and not the same answer. The subtraction cancels
    /// exactly where the angle is small: at `u = 10⁸` it keeps seven digits of
    /// it, and at `10³⁰⁰` it keeps none — rounding a far but finite end onto
    /// the place at infinity, which is a different place. Asked of the pair the
    /// angle is computed rather than differenced, and an infinite `u` answers
    /// nought and `π` like any other.
    fn angle(u: f64) -> f64 {
        1.0f64.atan2(u)
    }
}

/// The curve two quadrics meet in, parameterized exactly.
///
/// **`X₁(u) ± X₂(u)·√Δ(u)`**, which is `.notes/KERNEL.md` §7.3's committed
/// shape and the literature's: `X₁` cubic, `X₂` linear and `Δ` quartic in the
/// parameter, with everything over `ℚ(√δ)` and the branch one root above that.
/// Two square roots in all, which is the two storeys §4.2 caps the tower at.
///
/// **Held as what makes it rather than as coefficients.** A ruled member of the
/// pencil written bilinearly, and the other quadric — from which any place of
/// the curve is a substitution and a solve. That is §4.2's own rule for a
/// construction: hold what made it and work it out again, rather than hold a
/// number that has already rounded.
#[derive(Debug, Clone)]
pub(crate) struct Quartic {
    /// The ruled member, written so a place on it is bilinear in two
    /// parameters.
    ruled: Ruled<Quadratic<Rational>>,
    /// The quadric the ruling is cut against.
    against: Quadric,
    /// `√δ`, and with it the field the ruling is written over.
    field: Quadratic<Rational>,
    /// The same member as the machine holds it — see [`Quartic::at`], which is
    /// the only thing that reads it.
    read: Ruled<f64>,
}

impl Quartic {
    /// The curve `one` and `two` meet in, or `None` where they do not meet in a
    /// smooth quartic.
    ///
    /// **The whole algebraic route in one call.** The pencil's characteristic
    /// form has to have four distinct roots, or the intersection is a node, a
    /// cusp or a break into conics and each is a case of its own. Then a ruled
    /// member is found by choosing whole places and reading off which member
    /// holds each — see [`Pencil::through`] — and the first whose rulings there
    /// are two distinct real lines is taken.
    ///
    /// `None` is not a failure to meet: two quadrics that cross in conics are
    /// answered by the geometric route (§7.3's table) and never reach here.
    pub(crate) fn of(one: Quadric, two: Quadric) -> Option<Self> {
        let against = two.clone();
        let pencil = Pencil::of(one, two);
        if pencil.discriminant().is_zero() {
            return None;
        }
        // The first whole place whose member is ruled *and* writes bilinearly.
        // A member of rank three is a cone, whose two rulings at a place are
        // one line and whose `δ` is therefore nought — so the strict sign
        // below turns those away, and the writing turns away whatever is left
        // that it cannot hold.
        whole().find_map(|place| {
            let member = pencil.at(&pencil.through(place)?);
            let found = member.rulings(place)?;
            if !found.under.sign().is_gt() {
                return None;
            }
            let field = Quadratic::root(found.under.clone())?;
            let lift = |of: &Rational| field.at(of.clone(), Rational::ZERO);
            let along: [[Quadratic<Rational>; 4]; 2] = std::array::from_fn(|which| {
                std::array::from_fn(|at| {
                    field.at(
                        found.plain[which][at].clone(),
                        found.times[which][at].clone(),
                    )
                })
            });
            let raised: [Quadratic<Rational>; 4] = Quadric::raised(place).map(|of| lift(&of));
            let ruled = Ruled::of(&member, &raised, &along, &lift)?;
            Some(Self {
                read: ruled.read(),
                ruled,
                against: against.clone(),
                field,
            })
        })
    }

    /// Where the ruling at `u` meets the other quadric, or `None` where it
    /// misses it.
    ///
    /// **The whole of the substitution, stated once.** Both a place on the
    /// curve and the question of whether there is one at `u` come off this, and
    /// two spellings of it would be a curve that could be walked where it does
    /// not exist. [`Roots::of`](super::roots::Roots) refuses a negative
    /// discriminant, so a `None` here *is* the answer that `u` names no real
    /// place.
    fn met(&self, u: &[Rational; 2]) -> Option<Along<Quadratic<Rational>>> {
        let [from, along] = self.ruling(u);
        self.against
            .met_by(&from, &along, &|of: &Rational| self.lift(of))
    }

    /// Where the ruling stands at `u`: a place on it, and the way it runs.
    fn ruling(&self, u: &[Rational; 2]) -> [[Quadratic<Rational>; 4]; 2] {
        self.ruled.ruling(&[self.lift(&u[0]), self.lift(&u[1])])
    }

    /// A rational carried into the field the ruling is written over.
    ///
    /// **One statement of how this curve's numbers enter its own field.**
    /// Written out at each reader, a curve read through one field and cut
    /// against another would be a curve nobody could hold against either.
    ///
    /// [`Quartic::of`] keeps its own, and has to: it lifts while the field is
    /// still a local and there is no curve yet to ask.
    fn lift(&self, of: &Rational) -> Quadratic<Rational> {
        self.field.at(of.clone(), Rational::ZERO)
    }

    /// The stretches of the affine parameter the curve is real over.
    ///
    /// **A quartic's branches are what `Δ ≥ 0` cuts out of the line.** `Δ` is
    /// degree four in `u` — the ruling is linear in it and the substitution
    /// squares that — so it changes sign at most four times, and the stretches
    /// where it does not are the pieces of curve there are to walk. At a root
    /// of it the two branches meet, which is what closes a piece into a loop
    /// rather than leaving two ends.
    ///
    /// **Bracketed in floats and decided exactly.** The roots come off
    /// [`quartic::roots`](crate::math::quartic), which isolates and bisects
    /// rather than solving in closed form; which side of each bracket is real
    /// is then asked of [`Quartic::met`], where the arithmetic has no digits to
    /// lose. So where a stretch *ends* is as good as a float, and *which*
    /// stretches there are is exact.
    ///
    /// The line rather than the projective line, so a curve running through the
    /// ruling's own place at infinity comes back as a stretch with an infinite
    /// end. Nothing may walk to one — see [`Quartic::at`], which has no place
    /// there to give.
    pub(crate) fn real(&self) -> Inline<Stretch, 3> {
        let [e, d, c, b, a] = self.coefficients();
        let mut fence = Inline::<f64, 6>::one(f64::NEG_INFINITY);
        for root in quartic::roots(a, b, c, d, e) {
            fence.push(root);
        }
        fence.push(f64::INFINITY);
        let mut found = Inline::none();
        for pair in fence.all().windows(2) {
            let stretch = Stretch {
                from: pair[0],
                to: pair[1],
            };
            // Half way round it in angle, which is strictly inside whatever
            // ends it has: the middle of an unbounded stretch is not a number
            // where the middle of its arc is.
            if self.met(&stretch.at(0.5)).is_some() {
                found.push(stretch);
            }
        }
        Self::joined(found)
    }

    /// The arcs `found` really is, the chart's own edge healed.
    ///
    /// **Two pieces reaching off the chart either way are one arc.** `+∞` and
    /// `−∞` are one place of the projective line, so a stretch ending at the
    /// first and one beginning at the second meet there — and nothing between
    /// them stops, `Δ` having no root at a place the chart cannot name. Joined,
    /// the two become the wrapping arc [`Stretch`] describes.
    ///
    /// The one arc that reaches both ways and is *not* joined to anything is
    /// the whole circle, which is what a `Δ` with no real root leaves.
    fn joined(found: Inline<Stretch, 3>) -> Inline<Stretch, 3> {
        let all = found.all();
        let (Some(first), Some(last)) = (all.first(), all.last()) else {
            return found;
        };
        if all.len() < 2 || first.from.is_finite() || last.to.is_finite() {
            return found;
        }
        let mut joined = Inline::one(Stretch {
            from: last.from,
            to: first.to,
        });
        for held in &all[1..all.len() - 1] {
            joined.push(*held);
        }
        joined
    }

    /// The closed pieces this curve comes in.
    ///
    /// **Two shapes, and which it is turns on whether `Δ` reaches nought.**
    /// Where it does, its roots cut the projective line into arcs and each arc
    /// is one loop — the near branch out and the far branch back, the two
    /// shutting on each other at the ends. Where it does not, the line is one
    /// arc with no end to shut at, and each branch closes on itself: the pair
    /// meets in *two* loops rather than one.
    ///
    /// Measured rather than reasoned — see the fixtures in `solid::geometry`,
    /// where both shapes are walked. Two crossing cylinders are the second: `y²
    /// ≤ 4` on one puts `z² ≥ 5` on the other, so nothing they share crosses
    /// the middle.
    pub(crate) fn components(&self) -> Inline<Component, 3> {
        let arcs = self.real();
        let mut found = Inline::none();
        match arcs.all() {
            [whole] if !whole.from.is_finite() && !whole.to.is_finite() => {
                for far in [false, true] {
                    found.push(Component {
                        arc: *whole,
                        closing: Closing::Alone { far },
                    });
                }
            }
            held => {
                for arc in held {
                    found.push(Component {
                        arc: *arc,
                        closing: Closing::Round,
                    });
                }
            }
        }
        found
    }

    /// `Δ`'s five coefficients, the constant first.
    ///
    /// **Interpolated rather than expanded**, which is [`Pencil`]'s own trick
    /// one degree down: `Δ` is a quartic in `u`, so five readings determine it,
    /// and five readings are a great deal less arithmetic than a symbolic
    /// square of a bilinear form. Lagrange over `−2..=2`, whose weights are the
    /// small fractions below.
    pub(super) fn coefficients(&self) -> [f64; 5] {
        // Indexed by the power, so the row and the name agree. Each entry is
        // what the reading at that node contributes to that coefficient.
        const WEIGHTS: [[f64; 5]; 5] = [
            [0.0, 0.0, 1.0, 0.0, 0.0],
            [1.0 / 12.0, -2.0 / 3.0, 0.0, 2.0 / 3.0, -1.0 / 12.0],
            [-1.0 / 24.0, 2.0 / 3.0, -1.25, 2.0 / 3.0, -1.0 / 24.0],
            [-1.0 / 12.0, 1.0 / 6.0, 0.0, -1.0 / 6.0, 1.0 / 12.0],
            [1.0 / 24.0, -1.0 / 6.0, 0.25, -1.0 / 6.0, 1.0 / 24.0],
        ];
        let read: [f64; 5] = std::array::from_fn(|at| self.under_at(at as f64 - 2.0));
        std::array::from_fn(|power| {
            std::iter::zip(WEIGHTS[power], read)
                .map(|(weight, had)| weight * had)
                .sum()
        })
    }

    /// `Δ` at `u`, as a float.
    ///
    /// Negative where the ruling misses the other quadric, which is what makes
    /// it the thing to find the roots of: the curve is real exactly where this
    /// is not.
    pub(super) fn under_at(&self, u: f64) -> f64 {
        let [from, along] = self.ruling(&[Rational::of(u), Rational::ONE]);
        let form = self
            .against
            .spanned(&from, &along, &|of: &Rational| self.lift(of));
        Roots::discriminant(&form.alpha, &form.beta, &form.gamma).nearest()
    }

    /// Where the curve stands at `u`, on the `far` branch of the root.
    ///
    /// `None` where that place is at infinity, which a projective curve has and
    /// a modeller has nothing to do with, and where `u` names no real place at
    /// all.
    ///
    /// **Read in the machine's own field, where [`Quartic::met`] decides in the
    /// exact one.** The two are the same routines — the same ruling, the same
    /// substitution, the same roots in the same order — instantiated over
    /// `f64` here and over `ℚ(√δ)` there, which is what keeps the branch a
    /// reader asks for and the branch a decision was taken on the same branch.
    ///
    /// **Why a reading rather than the exact place.** A place is three floats
    /// whatever it is worked out in, and the exact route spends a few hundred
    /// bignum operations to land on the same three — measured at 2.3 ms against
    /// a circle's handful of nanoseconds, which is a walk no boolean can pay
    /// for. What stays exact is every *decision*: which arcs are real, where
    /// they end, and which member rules — none of which a rounding may settle.
    ///
    /// No storey stands over the reading, every non-negative double having a
    /// root that is a double — so the answer is whole and
    /// [`Along::plain`](super::roots::Along) holds all of it.
    pub(crate) fn at(&self, u: &[Rational; 2], far: bool) -> Option<DVec3> {
        self.both(u)[usize::from(far)]
    }

    /// Both places the ruling at `u` meets the other quadric in, in the order
    /// the branch names them.
    ///
    /// **One substitution for the pair.** The two are the two roots of one
    /// quadratic, so a reader wanting both — see [`Filed::along`], which asks
    /// which of them a place is — would otherwise solve it twice over.
    fn both(&self, u: &[Rational; 2]) -> [Option<DVec3>; 2] {
        let nearest = |of: &Rational| of.nearest();
        let [from, along] = self.read.ruling(&[nearest(&u[0]), nearest(&u[1])]);
        let Some(found) = self.against.met_by(&from, &along, &nearest) else {
            return [None; 2];
        };
        found.plain.map(|held| {
            let across = held[3];
            (across != 0.0).then(|| DVec3::new(held[0], held[1], held[2]) / across)
        })
    }

    /// Which ruling the place `at` stands on, which is [`Quartic::at`] the
    /// other way round as far as `u`.
    ///
    /// The branch is not in it: both branches of one `u` stand on the one
    /// ruling, and what tells them apart is which of the two the place is —
    /// see [`Filed::along`], where that is asked.
    fn along(&self, at: DVec3) -> [f64; 2] {
        self.read.through(DVec4::new(at.x, at.y, at.z, 1.0))
    }
}

/// A curve of the exact tier, written down rather than laid out.
///
/// **The handle, on the terms [`Marched`](super::marchings::Marched) states.**
/// What holds the construction is [`Quartics`], which a body keeps beside its
/// topology; what is here is which component of it, and the two numbers a
/// reader answers without reaching for the store.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Quartered {
    /// Which component of the body's own store it is.
    pub(crate) run: u32,
    /// What it is filed under — see
    /// [`Curve::key`](super::curve::Curve::key).
    pub(crate) key: u64,
    /// How large the numbers reading it work in.
    pub(crate) reach: f64,
}

/// One closed piece of a quartic: the arc it runs over, and how it closes.
///
/// **What a body files, one edge apiece** — see [`Quartics::add`], and
/// [`Quartic::components`], which is the only thing that makes one.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Component {
    pub(crate) arc: Stretch,
    pub(crate) closing: Closing,
}

/// How one component of a quartic closes on itself.
///
/// **The two shapes a component comes in**, and which it is turns on whether
/// `Δ` reaches nought — see [`Quartic::real`], where the arcs are cut.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum Closing {
    /// Both branches of one arc, walked out on the near and back on the far.
    ///
    /// The two shut on each other at the arc's ends, `Δ` being nought there and
    /// a root of nought leaving one place rather than two.
    #[default]
    Round,
    /// One branch of the whole circle, closed on itself.
    ///
    /// What a `Δ` with no real root leaves: the branches never meet, so each is
    /// a loop of its own and the pair meets in *two*. Two crossing cylinders
    /// are the case — `y² ≤ 4` on one puts `z² ≥ 5` on the other, so nothing
    /// they share crosses the middle.
    Alone { far: bool },
}

impl Closing {
    /// Where round the loop the angle `t` stands: how far along the arc, and on
    /// which branch.
    ///
    /// **A whole turn to the loop**, which is what every closed curve here is
    /// parameterized by — a circle's own angle, an ellipse's, and a marched
    /// run's, which is normalized to one turn. An arm measuring its parameter
    /// differently would be an arm whose `Edge::bounds` and whose chord count
    /// meant something else from every other.
    fn walk(self, t: f64) -> Walk {
        let turn = t.rem_euclid(TAU) / TAU;
        match self {
            // Out on the near branch and back on the far, so the halves meet at
            // both ends of the arc and the whole is one loop.
            Self::Round => match turn < 0.5 {
                true => Walk {
                    part: turn * 2.0,
                    far: false,
                },
                false => Walk {
                    part: 2.0 - turn * 2.0,
                    far: true,
                },
            },
            Self::Alone { far } => Walk { part: turn, far },
        }
    }

    /// The angle a walk stands at, which is [`Closing::walk`] the other way
    /// round.
    fn along(self, walk: Walk) -> f64 {
        let turn = match self {
            Self::Round if walk.far => 1.0 - walk.part / 2.0,
            Self::Round => walk.part / 2.0,
            Self::Alone { .. } => walk.part,
        };
        turn * TAU
    }
}

/// Where a walk of a component stands: how far along its arc, and which branch.
#[derive(Debug, Clone, Copy)]
struct Walk {
    part: f64,
    far: bool,
}

/// One component of one quartic, and what a reader answers without evaluating
/// it.
#[derive(Debug, Clone)]
struct Filed {
    curve: Quartic,
    arc: Stretch,
    closing: Closing,
    /// Which parts of the arc read, as [`Filed::trimmed`] found them.
    ///
    /// **Held beside the arc rather than written into it.** What the trim
    /// finds is which parts of the arc read, and that is a fact about the
    /// reading rather than about where the arc is — an arc rewritten to its
    /// own trim would have to name the hair in the affine chart, where the
    /// place at infinity has no name.
    holds: [f64; 2],
    /// How hard the loop can turn, which is what a chord count is taken
    /// against — see [`arc::chords`], and
    /// [`Saddle::bending`](super::saddle::Saddle), which works its own out
    /// where this measures.
    bending: f64,
    /// How large the numbers reading it work in.
    reach: f64,
}

/// Every quartic one body's curves are cut from.
///
/// **Not flat, where [`Marchings`](super::marchings::Marchings) is**, and that
/// is the one thing about it worth arguing. A marched run is places, which pack
/// into one buffer; a quartic is a ruled member and a quadric over exact
/// rationals, which are bignums and own blocks of their own. Clearing keeps the
/// room for the constructions and hands back the room inside them.
///
/// **What that costs is an allocation per quartic edge per rebuild**, on a body
/// the drawing under it rebuilds every frame of a drag. A handful of blocks for
/// a handful of edges, against a marched run's none — worth naming, and not
/// worth a second representation of an exact curve to avoid.
#[derive(Debug, Default)]
pub(crate) struct Quartics {
    held: Vec<Filed>,
}

/// How many places a component is measured at when it is filed.
///
/// **Enough to bound a bend rather than to draw one.** What the two numbers
/// below are for is a chord count and a size, both of which want the worst of
/// the loop and neither of which wants it to a digit. Thirty-two is a
/// hundredth of a turn's error on a circle, and a quartic bends no more sharply
/// than the tightest circle through three of its places.
const MEASURED: usize = 32;

/// The angle the `at`th of [`MEASURED`] steps round a loop stands at.
///
/// Stated once because the two measurements below both want it and a slip in
/// one would be a measurement quietly taken somewhere else. `stepped(1)` is the interval
/// itself, a step being what one of them is.
fn stepped(at: usize) -> f64 {
    TAU * at as f64 / MEASURED as f64
}

/// How many times a search over the parameter halves what it is looking in.
///
/// **What an `f64` holds between two ends**, which is the same standing
/// [`bisect`](crate::math::bisect) takes: halved until the middle is one of the
/// two, and no tolerance anywhere in it. Both readers below are bisections —
/// one for the last part of an arc that reads, one for the parameter nearest a
/// place — and neither wants a bound of its own.
const HALVINGS: usize = 60;

impl Quartics {
    /// Forget every curve, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.held.clear();
    }

    /// Take a copy of what `of` holds, over the room these took — see
    /// [`Carried::take_from`](super::carried::Carried), where the reason for a
    /// copy rather than a trade is.
    pub(crate) fn take_from(&mut self, of: &Self) {
        self.held.clear();
        self.held.extend_from_slice(&of.held);
    }

    /// How many are filed, which is the number the next one takes.
    pub(crate) fn len(&self) -> u32 {
        self.held.len() as u32
    }

    /// File one component of `curve` — the `arc` it runs over, closed as
    /// `closing` says — and say which it is.
    ///
    /// **Measured on the way in**, which is the arrangement
    /// [`Marchings::add`](super::marchings::Marchings) keeps and for the same
    /// reason: how hard the loop turns and how large its numbers work are
    /// asked on every walk over it, and working either out from the
    /// construction each time would be a walk inside a walk.
    pub(crate) fn add(&mut self, curve: Quartic, arc: Stretch, closing: Closing) -> u32 {
        let mut filed = Filed {
            curve,
            arc,
            closing,
            holds: [0.0, 1.0],
            bending: 0.0,
            reach: 0.0,
        };
        // In that order: a measurement outside what reads would be a
        // measurement of nothing.
        filed.holds = filed.trimmed();
        filed.bending = filed.bending();
        filed.reach = filed.reaching();
        let at = self.len();
        self.held.push(filed);
        at
    }

    /// Where the parameter `t` of the component at `run` lands.
    pub(crate) fn at(&self, run: u32, t: f64) -> DVec3 {
        self.held[run as usize]
            .place(t)
            .expect("a trimmed arc reads everywhere")
    }

    /// How large the numbers reading the component at `run` work in.
    pub(crate) fn reach(&self, run: u32) -> f64 {
        self.held[run as usize].reach
    }

    /// How many chords of the component at `run` a `span` of it wants.
    pub(crate) fn steps(&self, run: u32, span: f64, sagitta: f64) -> usize {
        arc::chords(self.held[run as usize].bending, span, sagitta)
    }

    /// Which angle round the component at `run` the place `at` stands at.
    ///
    /// **Solved rather than searched** — see [`Ruled::through`], which is the
    /// whole of it: a place of the ruled member is written in the member's own
    /// corner basis by one four by four solve, and the ruling falls out of the
    /// weights. What is left is the two walks the store lays over that ruling,
    /// each read the other way round.
    ///
    /// Exact for a place *of* the curve, which is what every caller asks about:
    /// a corner of a region's boundary was laid down off this curve, and a
    /// crossing was solved onto it. A place beside the curve is answered as far
    /// beside its true parameter as it stands off, the solve being linear. A
    /// place nowhere near it is answered with some parameter of the component
    /// and no promise which — and that is still sound for the one caller that
    /// asks a component about a place that may belong to another, because what
    /// that reads off the answer is a distance from a place the component
    /// really holds, which can only be longer than the nearest.
    pub(crate) fn along(&self, run: u32, at: DVec3) -> f64 {
        self.held[run as usize].along(at)
    }
}

impl Filed {
    /// Where the parameter `t` lands, or `None` where it names no real place.
    ///
    /// **The one walk of a component**, which is what the two measurements
    /// below and every reader above go through. Two spellings of it would be a
    /// curve measured on one path and read on another.
    fn place(&self, t: f64) -> Option<DVec3> {
        let walk = self.closing.walk(t);
        self.walked(walk.part)[usize::from(walk.far)]
    }

    /// Both branches `part` of the way along the arc that reads.
    fn walked(&self, part: f64) -> [Option<DVec3>; 2] {
        let [low, high] = self.holds;
        self.curve.both(&self.arc.at(low + (high - low) * part))
    }

    /// Which angle round it the place `at` stands at — see
    /// [`Quartics::along`], which is what this answers.
    fn along(&self, at: DVec3) -> f64 {
        let [low, high] = self.holds;
        // Held to the arc that reads. A place off the component is answered
        // with the end of it nearest, which is a place the component holds —
        // and a component that reads at one place only holds nothing else.
        let part = match high == low {
            true => 0.0,
            false => ((self.arc.along(self.curve.along(at)) - low) / (high - low)).clamp(0.0, 1.0),
        };
        let far = match self.closing {
            Closing::Alone { far } => far,
            // Both branches of one ruling stand at the one `u`, so which of the
            // two the place is is settled by reading, not by the solve.
            Closing::Round => {
                let apart = self
                    .walked(part)
                    .map(|had| had.map_or(f64::INFINITY, |had| had.distance(at)));
                apart[1] < apart[0]
            }
        };
        self.closing.along(Walk { part, far })
    }

    /// The parts of the arc that read, each end halved inward until one does.
    ///
    /// An end is a root of `Δ` found in floats, so a reading exactly at one
    /// falls a rounding either side of it — and a hair beyond, the branch is
    /// not real and there is no place to give. Halved rather than stepped in by
    /// a tolerance: what is wanted is the last place the curve *is*.
    fn trimmed(&self) -> [f64; 2] {
        let reads = |part: f64| self.curve.at(&self.arc.at(part), false).is_some();
        let inward = |from: f64, toward: f64| {
            let (mut out, mut held) = (from, toward);
            if reads(out) {
                return out;
            }
            for _ in 0..HALVINGS {
                let middle = (out + held) / 2.0;
                match reads(middle) {
                    true => held = middle,
                    false => out = middle,
                }
            }
            held
        };
        [inward(0.0, 0.5), inward(1.0, 0.5)]
    }

    /// How hard the component turns, as a bound on the second derivative of the
    /// place with the parameter.
    ///
    /// Measured rather than worked out: a quartic has no closed form for it the
    /// way a saddle does, and the construction is evaluable everywhere.
    fn bending(&self) -> f64 {
        let step = stepped(1);
        (0..MEASURED).fold(0.0f64, |most, at| {
            let t = stepped(at);
            let (Some(back), Some(here), Some(on)) =
                (self.place(t - step), self.place(t), self.place(t + step))
            else {
                return most;
            };
            most.max((back - here * 2.0 + on).length() / (step * step))
        })
    }

    /// How far from the origin the component reaches.
    fn reaching(&self) -> f64 {
        (0..MEASURED).fold(0.0f64, |most, at| match self.place(stepped(at)) {
            Some(place) => most.max(place.length()),
            None => most,
        })
    }
}

/// Whole places to try a pencil against, in shells outward from the origin.
///
/// Small ones first, because a member found through a small place has small
/// coefficients — which is what §4.2 measured the whole route's cost in. One
/// visit apiece: a place belongs to the shell its widest coordinate names, so
/// walking every box out to each reach would try the middle of them five times
/// over.
fn whole() -> impl Iterator<Item = DVec3> {
    (0..=CANDIDATES).flat_map(|reach| {
        (-reach..=reach).flat_map(move |x| {
            (-reach..=reach).flat_map(move |y| {
                (-reach..=reach)
                    .filter(move |z| x.abs().max(y.abs()).max(z.abs()) == reach)
                    .map(move |z| DVec3::new(x as f64, y as f64, z as f64))
            })
        })
    })
}
