//! What a face is a piece of.

use crate::inline::Inline;
use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::quadric::Quadric;
use glam::{BVec2, DVec2, DVec3};

/// One of the surfaces a face may lie on, told apart by its *tier*.
///
/// **`.notes/KERNEL.md` §4.1's tier made structural**, which is the whole
/// reason the split is two levels rather than one flat list. A pair of
/// [`Natural`]s can only produce exact geometry; a pair with a [`Fitted`] in it
/// cannot. So "is this body exact?" is a walk over its surfaces asking which
/// arm each is — see [`Body::exact`](crate::solid::topology::body::Body) — and
/// an algorithm that would quietly widen a tolerance has to name the arm that
/// did it.
///
/// A closed enum rather than a trait, because intersection dispatches on a
/// *pair* of surfaces. That is a matrix, and a matrix wants a `match` on the
/// pair; a trait needs double dispatch to express it and then cannot be
/// exhaustive. The two levels earn their keep there as well: every pair with a
/// fitted half in it is answered by one arm rather than by an entry apiece.
///
/// Everything below is one dispatch and no arithmetic. What each tier makes of
/// a question is that tier's own business.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Surface {
    Natural(Natural),
    Fitted(Fitted),
}

/// Where a ray met a surface, as distances along it.
///
/// **Four is the most.** Every *natural* surface is a quadric and answers at
/// most twice; a torus is a quartic and a ray through its hole crosses the tube
/// twice going in and twice coming out. A graze counts for none of them, for
/// the reason [`roots`](crate::math::quartic::roots) gives.
pub(crate) type Crossings = Inline<f64, 4>;

impl Surface {
    /// Whether this is of the exact tier.
    ///
    /// The one question the split exists to make answerable, and it is a match
    /// rather than a property of the numbers: a fitted surface is fitted
    /// because of what it *is*, not because of how well it happens to have been
    /// fitted this time.
    pub(crate) fn exact(&self) -> bool {
        matches!(self, Self::Natural(_))
    }

    /// The key several of these are filed under — see
    /// [`Buckets`](crate::solid::buckets::Buckets).
    /// This as the exact zero set of a symmetric 4×4, or `None` where it is
    /// not one.
    ///
    /// **The exact tier is quadrics and the fitted tier is not**, which is the
    /// whole of the answer: a natural surface *is* a quadric — see
    /// [`Quadric`] — and a torus is a quartic that no
    /// matrix holds. So a pair with a fitted half in it is marched rather than
    /// written down, and this is where that is asked.
    pub(crate) fn quadric(&self) -> Option<Quadric> {
        match self {
            Self::Natural(natural) => Some(Quadric::of(natural)),
            Self::Fitted(_) => None,
        }
    }

    pub(crate) fn key(&self) -> u64 {
        match self {
            Self::Natural(of) => of.key(),
            Self::Fitted(of) => of.key(),
        }
    }

    /// Where the parameters `uv` land in the world.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Natural(of) => of.at(uv),
            Self::Fitted(of) => of.at(uv),
        }
    }

    /// Which parameters `at` stands at, and the nearest place on the surface
    /// for anything off it.
    ///
    /// Closed form for every one of them, which is the whole reason there are
    /// no parameter-space curves anywhere in this kernel: a curve that already
    /// has a description in space does not get a second one that could disagree
    /// with it. See `.notes/KERNEL.md` §4.7.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        match self {
            Self::Natural(of) => of.uv(at),
            Self::Fitted(of) => of.uv(at),
        }
    }

    /// The unit normal at `uv`, pointing the way the surface's own parameters
    /// wind about.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Natural(of) => of.normal(uv),
            Self::Fitted(of) => of.normal(uv),
        }
    }

    /// How far along a ray from `from` running `way` it meets this, in order,
    /// and how many times.
    ///
    /// Distances along `way` rather than places, because what asks is a ray
    /// cast counting crossings ahead of where it started: which of them are
    /// ahead is a comparison on this and nothing else. Unnormalized `way` is
    /// fine and the answer is in units of it.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Crossings {
        match self {
            Self::Natural(of) => of.met_by(from, way),
            Self::Fitted(of) => of.met_by(from, way),
        }
    }

    /// How far `at` stands from the surface, never signed.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Natural(of) => of.off(at),
            Self::Fitted(of) => of.off(at),
        }
    }

    /// Whether the parameterization says nothing at `at` — one place that
    /// every angle names.
    pub(crate) fn singular(&self, at: DVec3) -> bool {
        match self {
            Self::Natural(of) => of.singular(at),
            Self::Fitted(of) => of.singular(at),
        }
    }

    /// How far the flat triangle on the parameters `corners` strays from this
    /// surface at its furthest.
    ///
    /// **What a mesher owes the sagitta it was asked for.** Flattening a face's
    /// *boundary* finely says nothing about its middle: a triangle with all
    /// three corners on a cylinder can still cut clean across it.
    ///
    /// A degenerate triple is a *chord*, which is what asks about one side of a
    /// triangle: pass a corner twice and the answer is how far that side leaves
    /// the surface.
    pub(crate) fn straying(&self, corners: [DVec2; 3]) -> f64 {
        match self {
            Self::Natural(of) => of.straying(corners),
            Self::Fitted(of) => of.straying(corners),
        }
    }

    /// How far apart the parameter lines of the grid a face on this surface may
    /// be cut into cells by must stand, given that no part of the face reaches
    /// further than `reach` along the second parameter.
    ///
    /// **Chosen so that a triangle inside one cell cannot stray further than
    /// `sagitta`**, which is what lets a mesher hold itself to the sagitta by
    /// arithmetic on the grid rather than by comparing against a tolerance.
    ///
    /// `reach` is the cone's alone — every other surface bends by the same
    /// amount wherever a face on it stands.
    pub(crate) fn strides(&self, reach: f64, sagitta: f64) -> DVec2 {
        match self {
            Self::Natural(of) => of.strides(reach, sagitta),
            Self::Fitted(of) => of.strides(sagitta),
        }
    }

    /// The box a face on this surface fills, given the box its boundary fills.
    pub(crate) fn fills(&self, boundary: Bounds<DVec3>) -> Bounds<DVec3> {
        match self {
            Self::Natural(of) => of.fills(boundary),
            Self::Fitted(of) => of.fills(),
        }
    }

    /// Which of the two parameters run round the surface, so that a face on it
    /// could wrap.
    ///
    /// What the split in `.notes/KERNEL.md` §4.4 is decided by, and what tells
    /// a reader of a face's own parameters which of them a loop traced into
    /// them has to be unwrapped in.
    ///
    /// **A pair and not one answer**, because a torus runs round both ways: a
    /// face on one may straddle the far side of the ring and the far side of
    /// the tube at once, and a reading that unwrapped only the first would cut
    /// such a face in half.
    pub(crate) fn round(&self) -> BVec2 {
        match self {
            Self::Natural(of) => of.round(),
            Self::Fitted(of) => of.round(),
        }
    }

    /// `uv` carried on from `from`, in whichever parameters run round.
    ///
    /// **The rule that keeps a flattened loop in one piece.** An inversion
    /// answers in a half turn either side of the reference, so a loop crossing
    /// the far side of a cylinder comes back as two stretches a whole turn
    /// apart unless each reading is taken at the turn the last one was. Both of
    /// a torus's parameters want it, which is §4.4 biting twice.
    pub(crate) fn carried(&self, uv: DVec2, from: DVec2) -> DVec2 {
        let round = self.round();
        DVec2::new(
            if round.x {
                branch::nearest(uv.x, from.x)
            } else {
                uv.x
            },
            if round.y {
                branch::nearest(uv.y, from.y)
            } else {
                uv.y
            },
        )
    }
}
