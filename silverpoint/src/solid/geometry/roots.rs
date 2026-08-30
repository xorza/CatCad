//! Where a binary quadratic form is nought, over any field and one root above
//! it.
//!
//! **No production caller yet**, as the rest of M3b's pieces have none. See
//! [`quadric`](super::quadric).
#![allow(dead_code)]

use crate::number::exact::field::Field;
use std::cmp::Ordering;

/// The two `(x : y)` a binary quadratic form is nought at, and what they are
/// written over.
///
/// **The one solve M3b does twice.** A ruled quadric's two directions through a
/// place are the roots of the form its tangent plane carries — see
/// [`Quadric::rulings`](super::quadric::Quadric) — and the two places a line
/// meets a quadric are the roots of the form the substitution leaves. Same
/// shape, same three cases, and one square root apiece.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Roots<T> {
    /// The discriminant, or nought where the roots want no field above `T`.
    ///
    /// A root already in the field is folded into [`Split::plain`] rather than
    /// carried, so a caller reading `plain + times·√under` needs no second
    /// question about which case it is in.
    pub(crate) under: T,
    /// The two roots, each as an `(x : y)` pair.
    pub(crate) at: [[Split<T>; 2]; 2],
}

/// One number of `T(√under)`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Split<T> {
    pub(crate) plain: T,
    pub(crate) times: T,
}

impl<T: Field> Roots<T> {
    /// Where `αx² + βxy + γy²` is nought, or `None` where it is nowhere real.
    ///
    /// **Three ways to write one pair, and which holds turns on which
    /// coefficient is not nought.** `(−β ± √δ : 2α)` divides by `α` and
    /// `(2γ : −β ∓ √δ)` divides by `γ`, and the two are the same roots —
    /// `(−β + √δ)(−β − √δ) = 4αγ` is the whole of why. A form with neither is
    /// `βxy`, whose roots are the two coordinates themselves; and so is a form
    /// with nothing in it at all, where every `(x : y)` is a root and any two
    /// independent ones will do.
    pub(crate) fn of(alpha: &T, beta: &T, gamma: &T) -> Option<Self> {
        let twice = |of: &T| of.clone() + of.clone();
        let delta = Self::discriminant(alpha, beta, gamma);
        if delta.sign() == Ordering::Less {
            return None;
        }
        let flat = |plain: T| Split {
            plain,
            times: alpha.zero(),
        };
        let (under, up) = match delta.rooted() {
            Some(root) => (alpha.zero(), flat(root)),
            None => (
                delta,
                Split {
                    plain: alpha.zero(),
                    times: alpha.one(),
                },
            ),
        };
        let down = Split {
            plain: -up.plain.clone(),
            times: -up.times.clone(),
        };
        let leaning = |by: &Split<T>| Split {
            plain: -beta.clone() + by.plain.clone(),
            times: by.times.clone(),
        };
        let at = if !alpha.is_zero() {
            let across = flat(twice(alpha));
            [[leaning(&up), across.clone()], [leaning(&down), across]]
        } else if !gamma.is_zero() {
            let across = flat(twice(gamma));
            [[across.clone(), leaning(&down)], [across, leaning(&up)]]
        } else {
            [
                [flat(alpha.one()), flat(alpha.zero())],
                [flat(alpha.zero()), flat(alpha.one())],
            ]
        };
        Some(Self { under, at })
    }

    /// `β² − 4αγ`, whose sign says how many real roots the form has.
    ///
    /// **Its own function because two callers read it**, and one of them is not
    /// the roots: a quartic's `Δ` is this as a function of the parameter, read
    /// for its sign where the curve is real and for its coefficients where the
    /// branches end — see
    /// [`Quartic::under_at`](super::quartic::Quartic). Two spellings would be a
    /// curve walked where its discriminant said one thing and its roots said
    /// another.
    ///
    /// The signed value rather than the answer above it. [`Roots::of`] folds a
    /// square into the roots and refuses a negative outright, which is right
    /// for a caller wanting places and useless to one wanting to know *where*
    /// the sign turns.
    pub(crate) fn discriminant(alpha: &T, beta: &T, gamma: &T) -> T {
        let twice = |of: &T| of.clone() + of.clone();
        beta.clone() * beta.clone() - twice(&twice(&(alpha.clone() * gamma.clone())))
    }
}

/// Two 4-vectors, each written over one square root.
///
/// **What both of M3b's solves hand back.** A quadric's rulings are directions
/// read off the roots of what its tangent plane carries, and the places a line
/// meets a quadric are read off the roots of what the substitution leaves —
/// same carrier, so the reading is written once. See
/// [`Quadric::rulings`](super::quadric::Quadric) and
/// [`Quadric::met_by`](super::quadric::Quadric).
///
/// Where `under` is nought the two stand apart in [`Along::plain`] and carry no
/// root at all. Where it is not, they are one expression with a sign —
/// `X₁ ± X₂·√under`, which is the shape `.notes/KERNEL.md` §7.3 commits to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Along<T> {
    pub(crate) under: T,
    pub(crate) plain: [[T; 4]; 2],
    pub(crate) times: [[T; 4]; 2],
}

impl<T: Field> Roots<T> {
    /// The two roots read as `x·one + y·two`.
    pub(crate) fn along(self, one: &[T; 4], two: &[T; 4]) -> Along<T> {
        let read = |[x, y]: &[Split<T>; 2], root: bool| -> [T; 4] {
            let (x, y) = if root {
                (&x.times, &y.times)
            } else {
                (&x.plain, &y.plain)
            };
            std::array::from_fn(|at| x.clone() * one[at].clone() + y.clone() * two[at].clone())
        };
        Along {
            plain: [read(&self.at[0], false), read(&self.at[1], false)],
            times: [read(&self.at[0], true), read(&self.at[1], true)],
            under: self.under,
        }
    }
}
