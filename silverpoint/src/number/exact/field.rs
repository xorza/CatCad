//! What a storey of the tower needs of the one below it.

use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops::{Add, Mul, Neg, Sub};

/// A set of numbers that can be added, multiplied and divided exactly.
///
/// **Two implementors and one user.** [`Rational`](super::rational::Rational)
/// is the ground floor and [`Quadratic`](super::quadratic::Quadratic) is every
/// storey above it — which is where the trait earns its place, because
/// `Quadratic` is generic over this and so the rule
/// `(a + b√r)(c + d√r) = (ac + bdr) + (ad + bc)√r` is written once and holds
/// both storeys of `ℚ(√δ)(√Δ)`. Written twice it would be two spellings of one
/// relation, which is how they come to disagree.
///
/// **Not a general algebraic-number layer**, which `.notes/KERNEL.md` §4.2 is
/// explicit about wanting no part of: no Sturm sequences, no isolating
/// intervals, nothing that resolves an arbitrary root. Two storeys is what a
/// quadric pencil needs and two is what the kernel builds — the depth is
/// bounded by [`Quadratic`](super::quadratic::Quadratic)'s own note, not by
/// this trait.
///
/// **Nought and one are asked of a value rather than of the type.** A field
/// here carries what it is a field *of*: nought in `ℚ(√2)` is not nought in
/// `ℚ(√3)`, the two being different fields, so there is no one constant to
/// name. Every value can hand back the nought and the one of its own field,
/// which is what the arithmetic actually needs.
pub(crate) trait Field:
    Sized
    + Clone
    + Debug
    + PartialEq
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Neg<Output = Self>
{
    /// Nothing, in the same field as this.
    fn zero(&self) -> Self;

    /// One, in the same field as this.
    fn one(&self) -> Self;

    /// Whether it is nothing at all — exactly, there being no other kind of
    /// nothing here.
    fn is_zero(&self) -> bool;

    /// Which side of nothing it falls.
    fn sign(&self) -> Ordering;

    /// One over it, or `None` for nought — which is the only value without an
    /// inverse, this being a field.
    fn inverse(&self) -> Option<Self>;

    /// Its square root, when that is in this field too, and `None` when it is
    /// not.
    ///
    /// **The question every storey above rests on**, and the reason this is a
    /// trait method rather than something
    /// [`Quadratic`](super::quadratic::Quadratic) could work out for itself: a
    /// storey exists only where the root it is built on is *not* already
    /// downstairs. Answering it is different work at each level and the answer
    /// is needed at each level.
    ///
    /// The root comes back non-negative, there being two and only one of them
    /// worth the name.
    fn rooted(&self) -> Option<Self>;

    /// The nearest `f64`, which is a *reading* of this and not this.
    fn nearest(&self) -> f64;
}
