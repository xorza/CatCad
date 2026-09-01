//! Everything past the quadrics, where an intersection is marched rather than
//! written down.

use crate::math::arc;
use crate::math::bounds::Bounds;
use crate::solid::buckets::Key;
use crate::solid::geometry::surface::Crossings;
use crate::solid::geometry::torus::Torus;
use glam::{BVec2, DVec2, DVec3};

/// One of the surfaces past the quadrics.
///
/// **The other half of `.notes/KERNEL.md` §4.1's tier, made structural.** A
/// pair of [`Natural`](super::natural::Natural)s can only produce exact
/// geometry; a pair with one of these in it cannot, and what comes out carries
/// the bound its fit was made to. So "is this body exact?" is a walk over its
/// surfaces asking which arm each is, and an algorithm that would quietly widen
/// a tolerance has to name the arm that did it.
///
/// **One member so far.** A ruled patch arrives with the corner a pair of picks
/// do not agree about — `.notes/KERNEL.md` §9.6, where its two joins are exact
/// and its second edge is walked. The torus is what a revolve makes of an arc
/// swept about a line it does not touch, and what a fillet down a rim is — see
/// §7.5.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Fitted {
    Torus(Torus),
}

impl Fitted {
    /// The key several of these are filed under — see
    /// [`Natural::key`](super::natural::Natural), which is where the argument
    /// for it is.
    ///
    /// The word carries on from the naturals' four rather than starting again,
    /// so no two surfaces of the whole set can collide on it.
    pub(crate) fn key(&self) -> u64 {
        match self {
            Self::Torus(torus) => torus
                .axis
                .keyed(Key::default().word(4))
                .float(torus.major)
                .float(torus.minor)
                .done(),
        }
    }

    /// Where the parameters `uv` land in the world.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Torus(torus) => torus.at(uv),
        }
    }

    /// Which parameters `at` stands at, and the nearest place on the surface
    /// for anything off it.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        match self {
            Self::Torus(torus) => torus.uv(at),
        }
    }

    /// The unit normal at `uv`, pointing the way the surface's own parameters
    /// wind about.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Torus(torus) => torus.normal(uv),
        }
    }

    /// How far along a ray from `from` running `way` it meets this, in order.
    ///
    /// **Four, where a quadric answers two**: a ray through the hole of a torus
    /// crosses the tube twice going in and twice coming out. Widened into
    /// [`Crossings`], which carries the six a ruled patch can answer.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Crossings {
        match self {
            Self::Torus(torus) => torus.met_by(from, way).widened(),
        }
    }

    /// Whether any of it passes within `slack` of `fills`, where that has a
    /// closed form — see [`Natural::spans`](super::natural::Natural), which is
    /// where the two that do are.
    ///
    /// Never, here, so neither argument is read: the nearest place of a box to
    /// a torus wants the nearest place of it to a *circle*, which is the
    /// cylinder's own question one level up and no more closed than that one.
    pub(crate) fn spans(&self, _fills: Bounds<DVec3>, _slack: f64) -> Option<bool> {
        match self {
            Self::Torus(_) => None,
        }
    }

    /// How far `at` stands from the surface, never signed.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Torus(torus) => torus.off(at),
        }
    }

    /// The nearest place of the surface to `at` — see
    /// [`Natural::nearest`](super::natural::Natural), where the one that parts
    /// company is argued.
    ///
    /// Through the inversion, which is what this tier has where the exact one
    /// has closed forms: [`Fitted::uv`] already answers with the nearest place
    /// for anything off the surface, so evaluating what it says *is* that
    /// place.
    pub(crate) fn nearest(&self, at: DVec3) -> DVec3 {
        self.at(self.uv(at))
    }

    /// The surface everywhere `by` off this one, along its own normal — see
    /// [`Natural::offset`](super::natural::Natural).
    ///
    /// Nothing here answers: a torus offsets to a torus, and no caller asks
    /// yet.
    pub(crate) fn offset(&self, _by: f64) -> Option<Self> {
        match self {
            Self::Torus(_) => None,
        }
    }

    /// The place `by` along this surface from `at`, setting out along the unit
    /// tangent `way` — see [`Natural::walked`](super::natural::Natural).
    ///
    /// Nothing here answers: a geodesic of a torus is an elliptic integral and
    /// no closed form walks one.
    pub(crate) fn walked(&self, _at: DVec3, _way: DVec3, _by: f64) -> Option<DVec3> {
        match self {
            Self::Torus(_) => None,
        }
    }

    /// Whether the parameterization says nothing at `at`.
    ///
    /// Never, for a torus: both of its parameters are angles about a circle
    /// that never closes on itself, so no place has two names and none has
    /// every name. A cone's apex and a sphere's poles are the cases this
    /// question exists for, and neither has a counterpart here.
    pub(crate) fn singular(&self, _at: DVec3) -> bool {
        match self {
            Self::Torus(_) => false,
        }
    }

    /// How far the flat triangle on the parameters `corners` strays from this
    /// surface at its furthest.
    ///
    /// **The two turns added, which is an upper bound and not the truth.** A
    /// torus bends both ways: across `u` a place traces a circle of at most
    /// `major + minor`, and across `v` one of `minor`. What a triangle leaves
    /// out is no more than what each of those turns leaves out on its own, so
    /// the sum bounds it — loosely where the triangle leans, which costs a
    /// mesher corners and never costs it a face that strays.
    pub(crate) fn straying(&self, corners: [DVec2; 3]) -> f64 {
        match self {
            Self::Torus(torus) => {
                (torus.major + torus.minor) * arc::bulge(arc::spread(corners.map(|uv| uv.x)))
                    + torus.minor * arc::bulge(arc::spread(corners.map(|uv| uv.y)))
            }
        }
    }

    /// How far apart the parameter lines of a face's grid must stand, given
    /// that no part of the face reaches further than `reach` along the second
    /// parameter.
    ///
    /// `reach` is the cone's alone and is not read here — a torus bends by the
    /// same amount wherever a face on it stands.
    ///
    /// **Half the sagitta each way**, which is what [`Fitted::straying`] being a
    /// *sum* of two turns asks for: a cell as wide as one whole sagitta in each
    /// angle leaves a triangle in its corner straying by both. Each angle takes
    /// its own radius — the first turns at the outermost the tube reaches and
    /// the second at the tube itself — and `radius · bulge(widest(radius, s))`
    /// is `s` again by [`arc::widest`]'s own identity, so the two halves add to
    /// exactly the sagitta with no argument about how a triangle leans.
    ///
    /// A sphere next door divides by the square root of two instead, and can:
    /// its own straying is the true distance rather than a sum of bounds.
    pub(crate) fn strides(&self, _reach: f64, sagitta: f64) -> DVec2 {
        match self {
            Self::Torus(torus) => DVec2::new(
                arc::widest(torus.major + torus.minor, sagitta / 2.0),
                arc::widest(torus.minor, sagitta / 2.0),
            ),
        }
    }

    /// The box a face on this surface fills, given the box its boundary fills.
    ///
    /// The whole surface, so the boundary is not read, for the reason a sphere
    /// is given the whole of itself:
    /// a torus has no parameter every world coordinate runs monotonically
    /// along, so the top of a bulge is interior and the box of the rim below
    /// misses it. Coarse, and not wrong.
    pub(crate) fn fills(&self, _boundary: Bounds<DVec3>) -> Bounds<DVec3> {
        match self {
            Self::Torus(torus) => Bounds::about(torus.axis.origin, torus.major + torus.minor),
        }
    }

    /// Which of the two parameters run round the surface, so that a face on it
    /// could wrap.
    ///
    /// Both of a torus's do, where every other surface here has at most one —
    /// which is why §4.4's rule about wrapping bites twice on one, and the
    /// whole reason this is a pair of answers rather than one.
    pub(crate) fn round(&self) -> BVec2 {
        match self {
            Self::Torus(_) => BVec2::TRUE,
        }
    }
}
