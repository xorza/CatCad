//! The four natural quadrics, which are the whole of the exact tier.

use crate::math::arc;
use crate::math::bounds::Bounds;
use crate::math::plane::Plane;
use crate::number::predicate::{self, ApproxEq};
use crate::number::tolerance::{ALIGNED, PLACED};
use crate::solid::buckets::Key;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Crossings;
use glam::{BVec2, DVec2, DVec3};
use std::f64::consts::SQRT_2;

/// One of the four *natural quadrics*.
///
/// They arrive together because they are one algebra: a pencil of quadrics does
/// not care which of the four it was handed, so plane-meets-cone is not
/// separate work from plane-meets-cylinder. The set is also exactly what
/// extruding and revolving a drawing of lines and circles can make, which is
/// why it is the set a kernel for this application wants.
///
/// **Every one of them is exact**, and that is what the type says: the
/// parameters below are the surface rather than a fit to one, nothing evaluated
/// off them carries a tolerance, and a pair of these can only ever produce
/// exact geometry. See `.notes/KERNEL.md` §4.1 and the
/// [`Fitted`](super::fitted::Fitted) half beside it.
///
/// **A feature builds every one of the four.** An extrusion raises planes and
/// cylinders, and a revolve raises cones and spheres beside them — a line that
/// leans on the axis sweeps a cone, and an arc centred on it sweeps a sphere.
/// Every arm below answers all four and
/// [`Meeting`](crate::solid::meeting::Meeting) dispatches over the whole pair
/// matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Natural {
    /// The same [`Plane`] a sketch is carried into the world by, which is what
    /// lets an extrusion's base face literally hold the drawing's own frame.
    Plane(Plane),
    Cylinder(Cylinder),
    Cone(Cone),
    Sphere(Sphere),
}

impl Natural {
    /// The key several of these are filed under — see
    /// [`Buckets`](crate::solid::buckets::Buckets).
    ///
    /// Over the numbers the surface is made of, which is what makes it a key
    /// two equal surfaces cannot disagree on: two faces of one surface were
    /// *given* the same value rather than each working one out, so every
    /// number of the two matches bit for bit.
    ///
    /// Which variant it is goes in as well. Two surfaces of different kinds
    /// are never equal, and a key that let them collide would cost a
    /// comparison for nothing.
    pub(crate) fn key(&self) -> u64 {
        match self {
            Self::Plane(plane) => Key::default()
                .word(0)
                .place(plane.origin)
                .place(plane.x)
                .place(plane.y)
                .done(),
            Self::Cylinder(cylinder) => cylinder
                .axis
                .keyed(Key::default().word(1))
                .float(cylinder.radius)
                .done(),
            Self::Cone(cone) => cone
                .axis
                .keyed(Key::default().word(2))
                .float(cone.half_angle)
                .done(),
            Self::Sphere(sphere) => sphere
                .axis
                .keyed(Key::default().word(3))
                .float(sphere.radius)
                .done(),
        }
    }

    /// Where the parameters `uv` land in the world.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Plane(plane) => plane.point(uv),
            Self::Cylinder(cylinder) => cylinder.at(uv),
            Self::Cone(cone) => cone.at(uv),
            Self::Sphere(sphere) => sphere.at(uv),
        }
    }

    /// Which parameters `at` stands at, and the nearest place on the surface
    /// for anything off it.
    ///
    /// Closed form for all four, which is the whole reason there are no
    /// parameter-space curves anywhere in this kernel: a curve that already has
    /// a description in space does not get a second one that could disagree
    /// with it. See `.notes/KERNEL.md` §4.7.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        match self {
            Self::Plane(plane) => plane.flatten(at),
            Self::Cylinder(cylinder) => cylinder.uv(at),
            Self::Cone(cone) => cone.uv(at),
            Self::Sphere(sphere) => sphere.uv(at),
        }
    }

    /// The unit normal at `uv`, pointing the way the surface's own parameters
    /// wind about.
    ///
    /// Which is to say `∂u × ∂v`, normalized. Stating it that way is what makes
    /// the winding of a mesh decidable: a triangle wound counterclockwise in
    /// the parameters is wound counterclockwise about this, so a face that
    /// knows whether material is on this side knows which way to hand its
    /// triangles out.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Plane(plane) => plane.normal(),
            Self::Cylinder(cylinder) => cylinder.normal(uv),
            Self::Cone(cone) => cone.normal(uv),
            Self::Sphere(sphere) => sphere.normal(uv),
        }
    }

    /// How far along a ray from `from` running `way` it meets this, in order,
    /// and how many times.
    ///
    /// **At most twice, because every surface here is a quadric.** A plane is
    /// the degenerate one and answers once; the other three answer twice or
    /// not at all — a graze counts as not at all, for the reason
    /// [`roots`](crate::math::quadratic::roots) gives.
    ///
    /// Distances along `way` rather than places, because what asks is a ray
    /// cast counting crossings ahead of where it started: which of them are
    /// ahead is a comparison on this and nothing else. Unnormalized `way` is
    /// fine and the answer is in units of it.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Crossings {
        let two = |along: Option<[f64; 2]>| match along {
            Some([near, far]) => Crossings::two(near, far),
            None => Crossings::none(),
        };
        match self {
            Self::Plane(plane) => {
                let leaning = way.dot(plane.normal());
                // Along the plane, so it meets it nowhere or everywhere — and
                // a ray *in* a plane crosses out of nothing, which is the
                // answer either way.
                //
                // **To within [`ALIGNED`] rather than exactly**, which is not
                // caution but arithmetic: a ray a hair off parallel crosses the
                // plane genuinely, at a distance of one over that hair, and a
                // place read off the ray that far out has no significant digits
                // left in it. The crossing is real and unusable, so it is not
                // reported. A caller that leans on this is a caller casting
                // rays along a body's own faces, which is what four directions
                // are for.
                if predicate::touching(leaning.abs(), ALIGNED) {
                    return Crossings::none();
                }
                Crossings::one((plane.origin - from).dot(plane.normal()) / leaning)
            }
            Self::Cylinder(cylinder) => two(cylinder.met_by(from, way)),
            Self::Cone(cone) => two(cone.met_by(from, way)),
            Self::Sphere(sphere) => two(sphere.met_by(from, way)),
        }
    }

    /// How far `at` stands from the surface, never signed.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Plane(plane) => (at - plane.origin).dot(plane.normal()).abs(),
            Self::Cylinder(cylinder) => cylinder.off(at),
            Self::Cone(cone) => cone.off(at),
            Self::Sphere(sphere) => sphere.off(at),
        }
    }

    /// Whether the parameterization says nothing at `at` — one place that
    /// every angle names.
    ///
    /// **A cone's apex and a sphere's poles**, and nothing else here: a plane
    /// has no angle to lose, and a point on a cylinder is never on its axis.
    /// [`Natural::uv`] answers *something* at one of these, and what it answers
    /// is arbitrary — which is a lie a face's boundary cannot be flattened
    /// through, a ruling ending at an apex coming back as a run across the
    /// whole face rather than the constant-angle side it is. See
    /// [`Face::flatten`](crate::solid::topology::face::Face), which is where
    /// the lie is caught.
    pub(crate) fn singular(&self, at: DVec3) -> bool {
        match self {
            Self::Plane(_) | Self::Cylinder(_) => false,
            Self::Cone(cone) => at.approx_eq(cone.axis.origin, PLACED),
            // On the axis, which for a sphere is the two poles and nowhere
            // else, the centre not being on the surface.
            Self::Sphere(sphere) => predicate::touching(sphere.axis.off(at), PLACED),
        }
    }

    /// How far the flat triangle on the parameters `corners` strays from this
    /// surface at its furthest.
    ///
    /// **What a mesher owes the sagitta it was asked for.** Flattening a face's
    /// *boundary* finely says nothing about its middle: a triangle with all
    /// three corners on a cylinder can still cut clean across it, and a
    /// triangulation of the boundary alone is free to lay one down. This is the
    /// number that catches it — closed form for all four, an upper bound
    /// everywhere, and the truth exactly for three of them. Only the cone
    /// answers wide, and only because a triangle across one covers rings of
    /// two different radii.
    ///
    /// A degenerate triple is a *chord*, which is what asks about one side of a
    /// triangle: pass a corner twice and the answer is how far that side leaves
    /// the surface.
    pub(crate) fn straying(&self, corners: [DVec2; 3]) -> f64 {
        match self {
            // Exactly nothing, and the reason a block costs the mesher no
            // second thought: every point of a triangle whose corners are in a
            // plane is in that plane.
            Self::Plane(_) => 0.0,
            Self::Cylinder(cylinder) => {
                cylinder.radius * arc::bulge(arc::spread(corners.map(|uv| uv.x)))
            }
            // **The widest ruling the triangle reaches**, because a cone's
            // radius grows along it: the shortfall at each height is that
            // height's radius times the same share, so the tallest corner
            // bounds the lot. Times the cosine, which is what turns a shortfall
            // measured out from the axis into a distance measured square to the
            // surface — the same turn [`Cone::off`] makes.
            Self::Cone(cone) => {
                let reach = corners.iter().fold(0.0_f64, |far, uv| far.max(uv.y.abs()));
                reach * cone.half_angle.sin() * arc::bulge(arc::spread(corners.map(|uv| uv.x)))
            }
            // **How far the nearest point of it passes from the centre**,
            // which no angle in the parameters can stand in for: a small circle
            // near a pole spans every angle there is and strays by nothing. The
            // whole triangle lies inside the ball, so the radius less that
            // distance is what its furthest point stands off by.
            //
            // Every corner is the same distance from the centre, so the centre
            // drops onto the triangle's own *circumcentre* — inside it when the
            // triangle is acute, and otherwise out across its longest side,
            // where the nearest point is that side's middle. Two cases, both
            // closed form, and the second is also the answer for a triple with
            // no area in it: a chord's nearest point to the centre of a sphere
            // its ends are on is its middle.
            Self::Sphere(sphere) => {
                let [a, b, c] = corners.map(|uv| sphere.at(uv));
                let sides = [[b, c], [c, a], [a, b]];
                let across = sides.map(|side| side[0].distance_squared(side[1]));
                let at = (0..3)
                    .max_by(|&one, &two| across[one].total_cmp(&across[two]))
                    .expect("a triangle has three sides");
                // Obtuse — or right, or with no area at all, where either
                // answer is the same one.
                let near = if across[at] >= across.iter().sum::<f64>() - across[at] {
                    let [from, to] = sides[at];
                    ((from + to) * 0.5).distance(sphere.centre())
                } else {
                    let square = (b - a).cross(c - a);
                    (a - sphere.centre()).dot(square).abs() / square.length()
                };
                (sphere.radius - near).max(0.0)
            }
        }
    }

    /// How far apart the parameter lines of the grid a face on this surface may
    /// be cut into cells by must stand, given that no part of the face reaches
    /// further than `reach` along the second parameter.
    ///
    /// **Chosen so that a triangle inside one cell cannot stray further than
    /// `sagitta`**, which is what lets a mesher hold itself to the sagitta by
    /// arithmetic on the grid rather than by comparing against a tolerance —
    /// see `Lattice` in `solid/mesh/`.
    ///
    /// Infinite where the surface does not bend that way at all: a plane both
    /// ways, and the ruling of a cylinder or a cone the second. Any step is as
    /// true as any other along a straight line, so there is no line to cut at
    /// and the caller falls back on how far the face reaches.
    ///
    /// The two ruled surfaces take [`arc::widest`] outright: what they bend by
    /// depends on the turn alone, and a triangle in a column of that width
    /// covers no more turn than one chord of it. **A sphere takes it over the
    /// square root of two**, because a cell there bends both ways and a
    /// triangle inside one stands off by as much as the cell's own
    /// circumcircle — so it is the cell's *diagonal* that must be no wider than
    /// one chord, and a square of that diagonal has sides that much shorter.
    pub(crate) fn strides(&self, reach: f64, sagitta: f64) -> DVec2 {
        let round = |radius: f64| arc::widest(radius, sagitta);
        match self {
            Self::Plane(_) => DVec2::INFINITY,
            Self::Cylinder(cylinder) => DVec2::new(round(cylinder.radius), f64::INFINITY),
            // The circle at the far end of the face, taken square to the
            // surface rather than out from the axis — the same turn
            // [`Natural::straying`] makes, and for the same reason.
            Self::Cone(cone) => DVec2::new(round(reach * cone.half_angle.sin()), f64::INFINITY),
            // **The equator both ways**, which is as fine as anywhere and finer
            // than the parameters need near a pole, where a whole turn of `u`
            // covers hardly any surface. Erring that way costs corners in a cap
            // and never costs a face that strays.
            Self::Sphere(sphere) => DVec2::splat(round(sphere.radius) / SQRT_2),
        }
    }

    /// The box a face on this surface fills, given the box its boundary fills.
    ///
    /// **The boundary is enough for three of the four**, on one argument. Every
    /// world coordinate of a plane, a cylinder or a cone runs monotonically
    /// along one of its two parameters — a plane along both, a cylinder along
    /// its height, a cone along its ruling — so the extreme of one over a region
    /// is taken somewhere on that region's edge. Where a coordinate is *not*
    /// monotone, which is one square to a cylinder's axis peaking at a single
    /// angle, the region's boundary crosses that angle anyway: a region spanning
    /// it is connected, so its edge is somewhere on every angle it covers.
    ///
    /// A sphere has no such parameter and the argument fails on it — the top of
    /// a dome is interior, and the box of the rim below misses it entirely. So a
    /// face on one is given the whole sphere, which is coarse and is not wrong.
    pub(crate) fn fills(&self, boundary: Bounds) -> Bounds {
        match self {
            Self::Plane(_) | Self::Cylinder(_) | Self::Cone(_) => boundary,
            Self::Sphere(sphere) => Bounds::about(sphere.centre(), sphere.radius),
        }
    }

    /// Which of the two parameters run round the surface, so that a face on it
    /// could wrap.
    ///
    /// The first only, here: every natural surface's second parameter is a
    /// height or a distance along a ruling, and none of them closes. A torus
    /// next door is the one that answers otherwise.
    pub(crate) fn round(&self) -> BVec2 {
        match self {
            Self::Plane(_) => BVec2::FALSE,
            Self::Cylinder(_) | Self::Cone(_) | Self::Sphere(_) => BVec2::new(true, false),
        }
    }
}
