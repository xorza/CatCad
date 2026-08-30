//! The curve two quadrics meet in where that curve is a smooth quartic, and
//! the store a body keeps them in.
//!
//! **No production caller yet.** What would call it is
//! [`Meeting`](crate::solid::meeting::Meeting)'s algebraic arm, and that waits
//! on a `Curve` variant to hand one back through — which in turn waits on the
//! splitter having a cut it can make from one. See `.notes/KERNEL.md` §7.3 and
//! §7.4.
//!
//! The store is here beside what it holds, on the terms
//! [`marchings`](super::marchings) keeps: one file per subject, holding the
//! arena, the handle and whatever small thing either wants.
#![allow(dead_code)]

use crate::inline::Inline;
use crate::math::quartic;
use crate::number::exact::field::Field;
use crate::number::exact::quadratic::Quadratic;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::pencil::Pencil;
use crate::solid::geometry::quadric::Quadric;
use crate::solid::geometry::roots::{Along, Roots};
use crate::solid::geometry::ruled::Ruled;
use glam::DVec3;

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
}

impl Quartic {
    /// The curve `one` and `two` meet in, or `None` where they do not meet in a
    /// smooth quartic.
    ///
    /// **The whole algebraic route in one call.** The pencil's characteristic
    /// form has to have four distinct roots, or the intersection is a node, a
    /// cusp or a break into conics and each is a case of its own. Then a ruled
    /// member is found by choosing whole places and reading off which member
    /// holds each — see [`Pencil::through`] — and the first one the signature
    /// calls ruled is taken.
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
            Some(Self {
                ruled: Ruled::of(&member, &raised, &along, &lift)?,
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
    /// all — see [`Quartic::met`].
    pub(crate) fn at(&self, u: &[Rational; 2], far: bool) -> Option<DVec3> {
        let found = self.met(u)?;
        let which = usize::from(far);
        // Over the second storey, which is where the branch lives: the two
        // places share a rootless half and carry opposite roots of `Δ`.
        let storey = Quadratic::root(found.under.clone());
        let read = |at: usize| match &storey {
            Some(storey) => storey
                .at(
                    found.plain[which][at].clone(),
                    found.times[which][at].clone(),
                )
                .nearest(),
            // `Δ` was a square in the field below, so the branch needed no
            // storey of its own and the answer is already whole.
            None => found.plain[which][at].nearest(),
        };
        let across = read(3);
        (across != 0.0).then(|| DVec3::new(read(0), read(1), read(2)) / across)
    }
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
    held: Vec<Quartic>,
}

impl Quartics {
    /// Forget every curve, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.held.clear();
    }

    /// How many are filed, which is the number the next one takes.
    pub(crate) fn len(&self) -> u32 {
        self.held.len() as u32
    }

    /// File `curve` as one of its own, and say which it is.
    pub(crate) fn add(&mut self, curve: Quartic) -> u32 {
        let at = self.len();
        self.held.push(curve);
        at
    }

    /// The curve filed at `at`.
    ///
    /// Panics on a handle this store never minted, which is a mistake in
    /// whatever named it rather than a state a reader has to handle — the same
    /// standing every arena here takes.
    pub(crate) fn at(&self, at: u32) -> &Quartic {
        &self.held[at as usize]
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
