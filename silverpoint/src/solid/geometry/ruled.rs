//! A ruled quadric written so a place on it is bilinear in two parameters.
//!
//! What writes one is [`Quartic::of`](super::quartic::Quartic). See
//! [`quadric`](super::quadric).

use crate::number::exact::field::Field;
use crate::number::exact::rational::Rational;
use crate::solid::geometry::quadric::Quadric;
use glam::{DMat4, DVec4};

/// A ruled quadric as the four places its two parameters weigh.
///
/// **The whole reason the algebraic route's curve has the form it does.** A place is
/// `u₀t₀·A + u₀t₁·B + u₁t₀·C + u₁t₁·D`, which is linear in `t` for each `u` and
/// linear in `u` for each `t` — two families of lines, and every place of the
/// quadric on one of each. Substituting that into the other quadric of the
/// pencil is therefore a *quadratic* in `t` whose coefficients are quadratic in
/// `u`, so its discriminant is a **quartic** in `u` and its roots are
/// `X₁(u) ± X₂(u)·√Δ(u)` with `X₁` cubic and `X₂` linear. Those are the degrees
/// `.notes/KERNEL.md` §4.1 quotes from the literature, and reaching them is
/// what says this construction is the right one.
///
/// **And it costs no square root of its own.** The two ruling directions at a
/// place already stand one root above ℚ — see
/// [`Quadric::rulings`](super::quadric::Quadric) — and everything below is
/// divisions in that field. So the whole route from two quadrics to their
/// curve takes `√δ` once and `√Δ` once, which is the two storeys §4.2 caps the
/// tower at and not one more.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Ruled<T> {
    corner: [[T; 4]; 4],
}

impl<T: Field> Ruled<T> {
    /// `quadric` written about `place` and the two directions `along` it rules
    /// in there, or `None` where there is no such writing.
    ///
    /// **The Gram matrix collapses, and that is the whole derivation.** Over
    /// the basis `{p, d₊, d₋, e}`, `pᵀQp` is nought because the place is on the
    /// quadric, `pᵀQd±` because the directions lie in its tangent plane, and
    /// `d±ᵀQd±` because they are rulings. What is left of the form is
    /// `2m·αε + 2k·βγ` with `m = pᵀQe` and `k = d₊ᵀQd₋` — and `e` can be moved
    /// by multiples of the other three until `d±ᵀQe` and `eᵀQe` vanish too,
    /// none of which needs a root.
    ///
    /// `αε·m + βγ·k = 0` is `XY = ZW` under other letters, and
    /// `(α, β, γ, ε) = (u₀t₀, n·u₀t₁, u₁t₀, u₁t₁)` with `n = −m/k` solves it
    /// for every `u` and `t`.
    ///
    /// `None` three ways, and only the first is reachable from a caller that
    /// took its directions from the rulings. `d₊ᵀQd₋` nought is the tangent
    /// plane lying *inside* the quadric rather than touching it, which is a
    /// rank the pencil's ruled member does not have. The other two — no
    /// direction the place is unsquare to, and `pᵀQe` nought — are the quadric
    /// being singular at the place, which is what the rulings refuse first.
    ///
    /// `lift` carries the quadric's own rationals into the field the directions
    /// are written over.
    pub(crate) fn of(
        quadric: &Quadric,
        place: &[T; 4],
        along: &[[T; 4]; 2],
        lift: &impl Fn(&Rational) -> T,
    ) -> Option<Self> {
        let [up, down] = along;
        let over = quadric.spanning(up, down, lift).inverse()?;
        let stepped = |held: usize| -> [T; 4] {
            std::array::from_fn(|at| {
                lift(&if at == held {
                    Rational::ONE
                } else {
                    Rational::ZERO
                })
            })
        };
        let scaled = |of: &[T; 4], by: &T| -> [T; 4] {
            std::array::from_fn(|at| by.clone() * of[at].clone())
        };
        let less = |of: &[T; 4], other: &[T; 4]| -> [T; 4] {
            std::array::from_fn(|at| of[at].clone() - other[at].clone())
        };
        // Any direction the place is not square to. There is one, `Qp` being
        // nought all through only where the quadric is singular there.
        let mut fourth = (0..4)
            .map(stepped)
            .find(|of| !quadric.spanning(place, of, lift).is_zero())?;
        // Moved until it is square to both rulings, which neither disturbs.
        fourth = less(
            &fourth,
            &scaled(down, &(quadric.spanning(up, &fourth, lift) * over.clone())),
        );
        fourth = less(
            &fourth,
            &scaled(up, &(quadric.spanning(down, &fourth, lift) * over.clone())),
        );
        // And until it is on the quadric, which the place being on it lets it
        // reach without disturbing anything either.
        let leaning = quadric.spanning(place, &fourth, lift);
        let squared = quadric.spanning(&fourth, &fourth, lift);
        let twice = leaning.clone() + leaning.clone();
        fourth = less(&fourth, &scaled(place, &(squared * twice.inverse()?)));
        Some(Self {
            corner: [
                place.clone(),
                scaled(up, &-(leaning * over)),
                down.clone(),
                fourth,
            ],
        })
    }

    /// The same member as the machine holds it.
    ///
    /// **What a walk reads**, where the exact one is what a decision is taken
    /// on — see [`Field`]'s own note on `f64`. The corners are the whole of the
    /// member, so reading them is the whole of reading it.
    pub(crate) fn read(&self) -> Ruled<f64> {
        Ruled {
            corner: std::array::from_fn(|at| {
                std::array::from_fn(|of| self.corner[at][of].nearest())
            }),
        }
    }

    /// The place at `(u₀ : u₁)` along and `(t₀ : t₁)` across.
    pub(crate) fn at(&self, u: &[T; 2], t: &[T; 2]) -> [T; 4] {
        let weight = [
            u[0].clone() * t[0].clone(),
            u[0].clone() * t[1].clone(),
            u[1].clone() * t[0].clone(),
            u[1].clone() * t[1].clone(),
        ];
        std::array::from_fn(|at| {
            weight
                .iter()
                .zip(&self.corner)
                .fold(u[0].zero(), |total, (by, of)| {
                    total + by.clone() * of[at].clone()
                })
        })
    }

    /// The line the ruling at `(u₀ : u₁)` runs along, as a place on it and a
    /// direction.
    ///
    /// What a substitution is taken over: the place is `at(u, (1 : 0))` and the
    /// direction `at(u, (0 : 1))`, so a point of the ruling is the first plus
    /// `t` of the second.
    pub(crate) fn ruling(&self, u: &[T; 2]) -> [[T; 4]; 2] {
        let (one, zero) = (u[0].one(), u[0].zero());
        [
            self.at(u, &[one.clone(), zero.clone()]),
            self.at(u, &[zero, one]),
        ]
    }
}

impl Ruled<f64> {
    /// Which of its rulings the place `at` stands on, as `(u₀ : u₁)`.
    ///
    /// **A solve and not a search**, which is the whole of why an inversion
    /// costs what a reading costs. A place of the member is
    /// `w₀·A + w₁·B + w₂·C + w₃·D` for the weights `[u₀t₀, u₀t₁, u₁t₀, u₁t₁]`
    /// [`Ruled::at`] builds — so the four corners are a basis of the space the
    /// places are written in, the weights come off one four by four solve, and
    /// `(u₀ : u₁)` is `(w₀ : w₂)` and `(w₁ : w₃)` over again.
    ///
    /// **The larger of those two pairs answers.** They are the one ratio
    /// scaled by `t₀` and by `t₁`, so for a place at either end of its own
    /// ruling one of them comes to nought and carries no answer, and neither
    /// comes to nought at once. A place that is *not* on the member has no
    /// ruling through it at all and the two disagree — by as much as it stands
    /// off, and no more, the solve being linear. See
    /// [`Quartics::along`](super::quartics::Quartics::along), which is what that
    /// bounds.
    pub(crate) fn through(&self, at: DVec4) -> [f64; 2] {
        let weight = DMat4::from_cols_array_2d(&self.corner).inverse() * at;
        debug_assert!(weight.is_finite(), "a ruled member with no basis in it");
        match weight.x * weight.x + weight.z * weight.z >= weight.y * weight.y + weight.w * weight.w
        {
            true => [weight.x, weight.z],
            false => [weight.y, weight.w],
        }
    }
}
