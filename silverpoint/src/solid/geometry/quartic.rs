//! The curve two quadrics meet in, where that curve is a smooth quartic.
//!
//! **No production caller yet.** What would call it is
//! [`Meeting`](crate::solid::meeting::Meeting)'s algebraic arm, and that waits
//! on a `Curve` variant to hand one back through — which in turn waits on the
//! splitter having a cut it can make from one. See `.notes/KERNEL.md` §7.3 and
//! §7.4.
#![allow(dead_code)]

use crate::number::exact::field::Field;
use crate::number::exact::quadratic::Quadratic;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::pencil::Pencil;
use crate::solid::geometry::quadric::Quadric;
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

    /// Where the curve stands at `u`, on the `far` branch of the root.
    ///
    /// `None` where that place is at infinity, which a projective curve has and
    /// a modeller has nothing to do with.
    pub(crate) fn at(&self, u: &[Rational; 2], far: bool) -> Option<DVec3> {
        let lift = |of: &Rational| self.field.at(of.clone(), Rational::ZERO);
        let [from, along] = self.ruled.ruling(&[lift(&u[0]), lift(&u[1])]);
        let found = self.against.met_by(&from, &along, &lift)?;
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
