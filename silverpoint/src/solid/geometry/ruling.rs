//! The lines a quadric holds through one of its own places.
//!
//! **No production caller yet**, as [`quadric`](super::quadric) and
//! [`pencil`](super::pencil) have none: this is M3b's fourth piece and the last
//! before the parameterization itself. The tests in `solid::geometry` hold it
//! up until there is one.
#![allow(dead_code)]

use crate::number::exact::field::Field;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::quadric::Quadric;
use glam::DVec3;
use std::cmp::Ordering;

/// The two lines a quadric holds that run through one of its places.
///
/// **What a pencil is parameterized through, and where the tower's first storey
/// comes from.** A ruled quadric holds two lines through each of its places. A
/// line meets the other quadric of the pencil in two places, so a point of the
/// intersection is *linear* in how far along its ruling it stands — and that
/// linearity is what turns the substitution into a quadratic whose two roots
/// are `X₁ ± X₂·√Δ`. See `.notes/KERNEL.md` §7.3.
///
/// **One square root and no more.** Both lines run through the place, so both
/// lie in the tangent plane there — and the place is in the radical of what the
/// quadric comes to on that plane, which leaves a *binary* form in two
/// directions. A binary form has one discriminant, and that is `δ`.
///
/// **One radicand for the pair rather than one per component.** A direction is
/// eight rationals and a root they share, where eight
/// [`Quadratic`](crate::number::exact::quadratic::Quadratic) values would be
/// twenty-four and eight copies of the same `δ` — and that type's own note says
/// as much about carrying the radicand along. What wants the tower is the
/// *next* step, where two of these are multiplied together.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rulings {
    radicand: Rational,
    plain: [[Rational; 4]; 2],
    times: [[Rational; 4]; 2],
}

impl Rulings {
    /// The two lines of `quadric` through `place`, or `None` where it holds
    /// none.
    ///
    /// **`None` has two meanings and both are answers.** A discriminant under
    /// nought is a place with no *real* line through it, which every place of a
    /// sphere is — the same fact
    /// [`Signature::ruled`](super::quadric::Signature) reports about the whole
    /// surface. And a place the quadric is singular at, a cone's apex, has no
    /// tangent plane to take a binary form on.
    ///
    /// `place` has to be on the quadric: a line through a place off it meets it
    /// in at most two, and there is nothing here to answer.
    pub(crate) fn of(quadric: &Quadric, place: DVec3) -> Option<Self> {
        debug_assert!(
            quadric.on(place).is_zero(),
            "{place:?} is not on the quadric to begin with",
        );
        let at = Quadric::raised(place);
        // `Qp`, whose zero set is the tangent plane. Nought all through is a
        // place the quadric is singular at, where that plane is everything and
        // what it carries is not a binary form.
        let facing: [Rational; 4] = std::array::from_fn(|row| {
            (0..4).fold(Rational::ZERO, |total, col| {
                total + quadric.held(row, col).clone() * at[col].clone()
            })
        });
        let across = (0..4).find(|&held| !facing[held].is_zero())?;
        // Solving the plane for `across` leaves one direction in it per other
        // coordinate, and the place is their combination with its own
        // coordinates as the weights. So dropping the direction for a
        // coordinate the place does not vanish at leaves two that span the
        // plane *beside* the place, which is the quotient the binary form lives
        // on.
        let standing = (0..4).find(|&held| held != across && !at[held].is_zero())?;
        let stepped = |held: usize| {
            let mut of = [const { Rational::ZERO }; 4];
            of[held] = Rational::ONE;
            of[across] = -(facing[held].clone() / facing[across].clone());
            of
        };
        let mut rest = (0..4).filter(|&held| held != across && held != standing);
        let one = stepped(rest.next().expect("four less two leaves two"));
        let two = stepped(rest.next().expect("four less two leaves two"));

        // `αx² + βxy + γy²` over `x·one + y·two`, which is the whole of what
        // the quadric says on the tangent plane once the place is divided out.
        let alpha = quadric.between(&one, &one);
        let beta = Rational::whole(2) * quadric.between(&one, &two);
        let gamma = quadric.between(&two, &two);
        let delta =
            beta.clone() * beta.clone() - Rational::whole(4) * alpha.clone() * gamma.clone();
        if delta.sign() == Ordering::Less {
            return None;
        }
        // **A rational root is folded into the plain half rather than
        // carried**, so that a nought radicand means what it says: these
        // directions want no field above ℚ. Where the root is not rational the
        // plain half is nought and one root is counted instead, which is the
        // ordinary case.
        let (radicand, up) = match delta.rooted() {
            Some(root) => (
                Rational::ZERO,
                Split {
                    plain: root,
                    times: Rational::ZERO,
                },
            ),
            None => (
                delta,
                Split {
                    plain: Rational::ZERO,
                    times: Rational::ONE,
                },
            ),
        };
        let weights = weighed(&alpha, &beta, &gamma, &up);
        Some(Self {
            radicand,
            plain: weights
                .clone()
                .map(|[x, y]| along(&one, &two, &x.plain, &y.plain)),
            times: weights.map(|[x, y]| along(&one, &two, &x.times, &y.times)),
        })
    }

    /// What is under the root the directions are written over.
    ///
    /// Nought exactly when they are rational, and a positive non-square
    /// otherwise — so a caller reads `plain + times·√radicand` and needs no
    /// second question about which case it is in.
    pub(crate) fn radicand(&self) -> &Rational {
        &self.radicand
    }

    /// The rootless half of each direction.
    pub(crate) fn plain(&self) -> &[[Rational; 4]; 2] {
        &self.plain
    }

    /// How many roots each direction carries, by component.
    pub(crate) fn times(&self) -> &[[Rational; 4]; 2] {
        &self.times
    }
}

/// One number as `plain + times·√δ`.
#[derive(Debug, Clone)]
struct Split {
    plain: Rational,
    times: Rational,
}

/// The two `(x : y)` that `αx² + βxy + γy²` is nought at, given `+√δ` already
/// split into its two halves.
///
/// **Three ways to write one pair of roots, and which holds turns on which
/// coefficient is not nought.** `(−β ± √δ : 2α)` divides by `α` and
/// `(2γ : −β ∓ √δ)` divides by `γ`, and the two are the same roots —
/// `(−β + √δ)(−β − √δ) = 4αγ` is the whole of why. A form with neither is
/// `βxy`, whose roots are the two coordinates themselves; and so is a form with
/// nothing in it at all, where the plane lies inside the quadric and any two of
/// its directions will do.
fn weighed(alpha: &Rational, beta: &Rational, gamma: &Rational, up: &Split) -> [[Split; 2]; 2] {
    let flat = |plain: Rational| Split {
        plain,
        times: Rational::ZERO,
    };
    let twice = |of: &Rational| Rational::whole(2) * of.clone();
    let leaning = |by: &Split| Split {
        plain: -beta.clone() + by.plain.clone(),
        times: by.times.clone(),
    };
    let down = Split {
        plain: -up.plain.clone(),
        times: -up.times.clone(),
    };
    if !alpha.is_zero() {
        let across = flat(twice(alpha));
        return [[leaning(up), across.clone()], [leaning(&down), across]];
    }
    if !gamma.is_zero() {
        let across = flat(twice(gamma));
        return [[across.clone(), leaning(&down)], [across, leaning(up)]];
    }
    [
        [flat(Rational::ONE), flat(Rational::ZERO)],
        [flat(Rational::ZERO), flat(Rational::ONE)],
    ]
}

/// `x·one + y·two`, one half of the split at a time.
fn along(one: &[Rational; 4], two: &[Rational; 4], x: &Rational, y: &Rational) -> [Rational; 4] {
    std::array::from_fn(|held| x.clone() * one[held].clone() + y.clone() * two[held].clone())
}
