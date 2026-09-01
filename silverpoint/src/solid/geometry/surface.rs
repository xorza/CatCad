//! What a face is a piece of.

use crate::inline::Inline;
use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::solid::buckets::Key;
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
/// **Six is the most, and the ruled patch is what asks for the last two.**
/// Every *natural* surface is a quadric and answers at most twice; a torus is a
/// quartic and a ray through its hole crosses the tube twice going in and twice
/// coming out. A [`Gusset`](super::gusset::Gusset) is neither: where a ray
/// meets it is a harmonic of degree three in the fillet's own angle, which has
/// six roots at the most — see [`Gusset::met_by`], where the degree is argued.
/// A graze counts for none of them, for the reason
/// [`roots`](crate::math::quartic::roots) gives.
pub(crate) type Crossings = Inline<f64, 6>;

/// How many times [`Surface::reaches`] splits a box before it gives the box the
/// benefit of the doubt.
///
/// **Four, which brings the ball round a cube down by a factor of four.** Each
/// split halves the widest axis, so four of them take a cube to a sixteenth of
/// its volume and its diagonal to a quarter. The cost is the other half of it:
/// a box the surface really does pass through is narrowed down one branch and
/// answers in about eight readings, and one that neither reaches nor culls
/// cleanly costs at most thirty-one. Both are a handful of arithmetic against a
/// face split that would otherwise be taken for nothing.
const SPLITS: usize = 4;

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

    /// This as the exact zero set of a symmetric 4×4, or `None` where it is
    /// not one.
    ///
    /// **The exact tier is quadrics and the fitted tier is not**, which is the
    /// whole of the answer: a natural surface *is* a quadric — see [`Quadric`]
    /// — and a torus is a quartic that no matrix holds. So a pair with a fitted
    /// half in it is marched rather than written down, and this is where that
    /// is asked.
    pub(crate) fn quadric(&self) -> Option<Quadric> {
        match self {
            Self::Natural(natural) => Some(Quadric::of(natural)),
            Self::Fitted(_) => None,
        }
    }

    /// The key several of these are filed under — see
    /// [`Buckets`](crate::solid::buckets::Buckets).
    pub(crate) fn key(&self) -> u64 {
        match self {
            Self::Natural(of) => of.key(),
            Self::Fitted(of) => of.key(),
        }
    }

    /// What the two of these are filed under, together.
    ///
    /// **The same key from either side**, which is what everything filing a
    /// *meeting* needs: a face on one surface and a face on the other have to
    /// reach the identical number, whichever of them is asking. So the pair is
    /// keyed here rather than at each caller, two spellings of it being two
    /// answers free to drift.
    ///
    /// Handed back unfinished, so a caller may add which curve of the meeting
    /// it means before it reads the number off — see
    /// [`Marched::key`](super::marchings::Marched).
    pub(crate) fn paired(&self, other: &Self) -> Key {
        Key::default().pair(self.key(), other.key())
    }

    /// Where the parameters `uv` land in the world.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Natural(of) => of.at(uv),
            Self::Fitted(of) => of.at(uv),
        }
    }

    /// Which parameters `at` stands at.
    ///
    /// **A parameterization and not a projection**, and the two part company on
    /// a cone: its `v` is the axial coordinate, so reading a place off the
    /// surface through here and back lands at the same height rather than at
    /// the foot of the perpendicular. A caller wanting the nearest place asks
    /// for it by name — see `nearest`.
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

    /// The place on this nearest `at`.
    ///
    /// **[`Surface::at`] of [`Surface::uv`] for every surface but the cone** —
    /// see [`Natural::nearest`], where the one that parts company is argued.
    pub(crate) fn nearest(&self, at: DVec3) -> DVec3 {
        match self {
            Self::Natural(of) => of.nearest(at),
            Self::Fitted(of) => of.at(of.uv(at)),
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

    /// Whether any of it passes within `slack` of `fills`, where that is
    /// answerable in closed form.
    fn spans(&self, fills: Bounds<DVec3>, slack: f64) -> Option<bool> {
        match self {
            Self::Natural(of) => of.spans(fills, slack),
            Self::Fitted(of) => of.spans(),
        }
    }

    /// The surface everywhere `by` off this one, along its own normal — see
    /// [`Natural::offset`].
    ///
    /// Nothing of the fitted tier answers: a torus offsets to a torus, and no
    /// caller asks yet.
    pub(crate) fn offset(&self, by: f64) -> Option<Self> {
        match self {
            Self::Natural(of) => of.offset(by).map(Self::Natural),
            Self::Fitted(_) => None,
        }
    }

    /// The place `by` along this surface from `at`, setting out along the unit
    /// tangent `way` — see [`Natural::walked`].
    pub(crate) fn walked(&self, at: DVec3, way: DVec3, by: f64) -> Option<DVec3> {
        match self {
            Self::Natural(of) => of.walked(at, way, by),
            Self::Fitted(_) => None,
        }
    }

    /// How far `at` stands from the surface, never signed.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Natural(of) => of.off(at),
            Self::Fitted(of) => of.off(at),
        }
    }

    /// Whether any of this surface passes within `slack` of the box `fills`.
    ///
    /// **What decides which faces a cut has to be taken across.** A body is
    /// divided by the other's *surfaces* rather than by its faces, and a
    /// surface is unbounded where the faces standing on it are not — so a wall
    /// at the far end of a model is cut by a surface whose own faces are
    /// nowhere near it. What it must never be cut by is a surface that misses
    /// it, and this is the question that says so.
    ///
    /// **A plane and a sphere answer outright** — see [`Natural::spans`], which
    /// is where the two closed forms are and why the other three have none.
    ///
    /// **The rest are narrowed.** [`Surface::off`] is a true distance, so it
    /// changes no faster than the place does: a box whose middle stands further
    /// off than the box's own half diagonal holds no part of the surface. Read
    /// once that is a ball round the box, which is far larger than a long thin
    /// box and so never culls an unbounded surface standing clear of one — a
    /// block's wall nine units off a cone reads seven against a half diagonal of
    /// ten, and the pair is cut although the two never meet. Which costs the
    /// whole boolean where the crossing is a conic nothing writes down. So the
    /// box is halved where one reading cannot settle it, [`SPLITS`] times over,
    /// each half being nearer its own middle.
    ///
    /// **Conservative where the halving runs out.** A box still unsettled at the
    /// last split is called reached. That drops work and never an answer, which
    /// is the whole of what a cull owes.
    ///
    /// **Sound across a shared edge**, which is what a uniform cut needs: a
    /// surface reaching an edge of this face reaches a place on the box of
    /// every face that edge bounds, so a surface refused here is refused by the
    /// face beside it too. Nothing is divided on one side of an edge and left
    /// whole on the other — see `.notes/KERNEL.md` §7.4.
    pub(crate) fn reaches(&self, fills: Bounds<DVec3>, slack: f64) -> bool {
        debug_assert!(
            fills.half().is_finite(),
            "an empty box holds no ball to reach",
        );
        self.spans(fills, slack)
            .unwrap_or_else(|| self.narrowed(fills, slack, SPLITS))
    }

    /// The same question by [`Surface::off`] alone, with `splits` halvings of
    /// the box left to narrow it by.
    fn narrowed(&self, fills: Bounds<DVec3>, slack: f64, splits: usize) -> bool {
        if self.off(fills.middle()) > fills.half().length() + slack {
            return false;
        }
        splits == 0
            || fills
                .halved()
                .into_iter()
                .any(|half| self.narrowed(half, slack, splits - 1))
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
