//! Everything past the quadrics, where an intersection is marched rather than
//! written down.

use crate::math::arc;
use crate::math::bounds::Bounds;
use crate::solid::buckets::Key;
use crate::solid::geometry::gusset::Gusset;
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
/// **Two members.** The torus is what a revolve makes of an arc swept about a
/// line it does not touch, and what a fillet down a rim is — see §7.5. The
/// ruled patch fills the corner a pair of picks do not agree about, its two
/// joins exact and its second edge walked — see `.notes/KERNEL.md` §9.6.
///
/// **The two are fitted for different reasons, and the arm is what says so.** A
/// torus is written down exactly and meets its neighbours in curves no closed
/// form parameterizes; a patch is written down exactly *and* meets them
/// exactly, and what it cannot do is answer about itself without measuring —
/// its second edge, its own box, the nearest place on it, how far a triangle
/// strays, how fine a grid wants to be and how far a normal read at a place can
/// turn are every one of them a reading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Fitted {
    Torus(Torus),
    Gusset(Gusset),
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
            Self::Gusset(gusset) => gusset.key(),
        }
    }

    /// Where the parameters `uv` land in the world.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Torus(torus) => torus.at(uv),
            Self::Gusset(gusset) => gusset.at(uv),
        }
    }

    /// Which parameters `at` stands at, and the nearest place on the surface
    /// for anything off it.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        match self {
            Self::Torus(torus) => torus.uv(at),
            Self::Gusset(gusset) => gusset.uv(at),
        }
    }

    /// The unit normal at `uv`, pointing the way the surface's own parameters
    /// wind about.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Torus(torus) => torus.normal(uv),
            Self::Gusset(gusset) => gusset.normal(uv),
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
            Self::Gusset(gusset) => gusset.met_by(from, way),
        }
    }

    /// Whether any of it passes within `slack` of `fills`, where that has a
    /// closed form — see [`Natural::spans`](super::natural::Natural), which is
    /// where the two that do are.
    ///
    /// Never for a torus, so neither argument is read there: the nearest place
    /// of a box to one wants the nearest place of it to a *circle*, which is
    /// the cylinder's own question one level up and no more closed than that.
    ///
    /// **A patch has to answer**, having no distance for the halving to fall
    /// back on — see [`Gusset::spans`], which settles it off the patch's own
    /// box.
    pub(crate) fn spans(&self, fills: Bounds<DVec3>, slack: f64) -> Option<bool> {
        match self {
            Self::Torus(_) => None,
            Self::Gusset(gusset) => Some(gusset.spans(fills, slack)),
        }
    }

    /// How far `at` stands from the surface, never signed.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Torus(torus) => torus.off(at),
            Self::Gusset(gusset) => gusset.off(at),
        }
    }

    /// The nearest place of the surface to `at` — see
    /// [`Natural::nearest`](super::natural::Natural), where the one that parts
    /// company is argued.
    ///
    /// Through the inversion for a torus, which is what this tier has where the
    /// exact one has closed forms: [`Fitted::uv`] answers with the nearest
    /// place for anything off the surface, so evaluating what it says *is* that
    /// place. A patch's inversion answers the ruling a place stands nearest the
    /// *line* of, which for a place off the patch is another's — so it seeks
    /// instead, see [`Gusset::nearest`].
    pub(crate) fn nearest(&self, at: DVec3) -> DVec3 {
        match self {
            Self::Torus(_) => self.at(self.uv(at)),
            Self::Gusset(gusset) => gusset.nearest(at),
        }
    }

    /// The surface everywhere `by` off this one, along its own normal — see
    /// [`Natural::offset`](super::natural::Natural).
    ///
    /// Nothing here answers. A torus offsets to a torus and no caller asks yet;
    /// a patch offsets to nothing written down, its two joins being what fix it
    /// and neither surviving the move.
    pub(crate) fn offset(&self, _by: f64) -> Option<Self> {
        match self {
            Self::Torus(_) | Self::Gusset(_) => None,
        }
    }

    /// The place `by` along this surface from `at`, setting out along the unit
    /// tangent `way` — see [`Natural::walked`](super::natural::Natural).
    ///
    /// Nothing here answers: a geodesic of a torus is an elliptic integral, and
    /// one of a ruled patch is no better written down.
    pub(crate) fn walked(&self, _at: DVec3, _way: DVec3, _by: f64) -> Option<DVec3> {
        match self {
            Self::Torus(_) | Self::Gusset(_) => None,
        }
    }

    /// Which of the two parameters a singular place leaves free — see
    /// [`Natural::freed`](super::natural::Natural).
    ///
    /// **The two arms part company here.** A torus has no singular place and is
    /// never asked; a ruled patch's tip stands at one *angle* and every run
    /// along the ruling, where every other surface in either tier that has one
    /// leaves the angle free instead.
    pub(crate) fn freed(&self) -> usize {
        match self {
            Self::Torus(_) => 0,
            Self::Gusset(gusset) => gusset.freed(),
        }
    }

    /// Whether the parameterization says nothing at `at`.
    ///
    /// Never for a torus: both of its parameters are angles about a circle that
    /// never closes on itself, so no place has two names and none has every
    /// name. A patch has one — the tip, where every ruling has closed to
    /// nothing and `v` names no direction, which is a cone's apex read one tier
    /// up.
    pub(crate) fn singular(&self, at: DVec3) -> bool {
        match self {
            Self::Torus(_) => false,
            Self::Gusset(gusset) => gusset.singular(at),
        }
    }

    /// How far a normal read back at `at` may turn from the surface's own, as
    /// a sine, where `at` may stand as much as `off` from the surface — see
    /// [`Natural::wavering`](super::natural::Natural), which is where the
    /// reading is argued.
    ///
    /// **A torus writes it down and a patch reads it**, which is the two arms
    /// being fitted for different reasons all over again. A torus turns its
    /// normal about the tube and about the ring, at the tube's own radius one
    /// way and at what the ring has left the other — so the tighter of the two
    /// bounds both. A patch has no curvature written down at all, and answers
    /// off its own parameters — see [`Gusset::wavering`].
    pub(crate) fn wavering(&self, at: DVec3, off: f64) -> f64 {
        match self {
            Self::Torus(torus) => off / torus.minor.min(torus.major - torus.minor),
            Self::Gusset(gusset) => gusset.wavering(at, off),
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
    ///
    /// **A patch answers by its three sides**, which is exact where the sum
    /// above is a bound: a ruled surface is affine along the ruling, so what a
    /// triangle leaves out stands on its own boundary — see
    /// [`Gusset::straying`]. Two of the three terms of a side are written down
    /// and the third is probed, which is the one reading in the whole tier that
    /// is measured rather than derived. `.notes/KERNEL.md` §9.6 is where that
    /// is decided and where what it costs is named.
    pub(crate) fn straying(&self, corners: [DVec2; 3]) -> f64 {
        match self {
            Self::Torus(torus) => {
                (torus.major + torus.minor) * arc::bulge(arc::spread(corners.map(|uv| uv.x)))
                    + torus.minor * arc::bulge(arc::spread(corners.map(|uv| uv.y)))
            }
            Self::Gusset(gusset) => gusset.straying(corners),
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
    ///
    /// **A patch halves it the same way and for the same reason**, its own
    /// reading being a sum of two terms as well — see [`Gusset::strides`],
    /// where the angle takes one half and the run along the ruling the other.
    pub(crate) fn strides(&self, _reach: f64, sagitta: f64) -> DVec2 {
        match self {
            Self::Torus(torus) => DVec2::new(
                arc::widest(torus.major + torus.minor, sagitta / 2.0),
                arc::widest(torus.minor, sagitta / 2.0),
            ),
            Self::Gusset(gusset) => gusset.strides(sagitta),
        }
    }

    /// The box a face on this surface fills, given the box its boundary fills.
    ///
    /// The whole surface for a torus, the boundary going unread, for the reason
    /// a sphere is given the whole of itself: it has no parameter every world
    /// coordinate runs monotonically along, so the top of a bulge is interior
    /// and the box of the rim below misses it. Coarse, and not wrong.
    pub(crate) fn fills(&self, boundary: Bounds<DVec3>) -> Bounds<DVec3> {
        match self {
            Self::Torus(torus) => Bounds::about(torus.axis.origin, torus.major + torus.minor),
            // Every ruling has both ends on the boundary, so a face lies inside
            // its convex hull and the boundary's own box holds it.
            Self::Gusset(_) => boundary,
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
            Self::Gusset(gusset) => gusset.round(),
        }
    }
}
