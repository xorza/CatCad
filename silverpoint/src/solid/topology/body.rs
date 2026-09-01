//! A whole solid.

use crate::solid::buckets::NONE;
use crate::solid::keyed::Keyed;
use crate::solid::named::Named;
use crate::solid::topology::Topology;
use crate::solid::topology::face::{Face, FaceId};

/// Everything one feature history has built: one or more disconnected volumes,
/// and the vocabulary their faces are named in.
///
/// The thing a document holds where it used to hold a reading. A prism was an
/// arrangement, a region and two numbers, worked out afresh wherever it was
/// asked about; a body is *made*, once, by an operation that can take material
/// away as readily as add it — which is the whole of why there is a kernel
/// here.
///
/// Empty is a body. An extrusion of no depth encloses nothing, and answering
/// with a body that has no faces is more honest than one with six that shut in
/// nothing: there is no solid, so there is nothing to draw, nothing to pick and
/// nothing to build on.
#[derive(Debug, Default)]
pub struct Body {
    topology: Topology,
    /// The distinct names its faces carry, in the order they were made, each
    /// with the chain of faces carrying it.
    ///
    /// **A face of a body is the set of faces sharing a name**, and this is
    /// that set's index — see `.notes/KERNEL.md` §5. A pocket cut across the
    /// top of a block leaves two islands of one face; both answer to the same
    /// name, and anything holding it lights both.
    ///
    /// Kept in emission order rather than sorted, so a caller writing one
    /// drawable per name writes them in the same order every rebuild. That is
    /// what lets a renderer's batch be refilled in place rather than
    /// renumbered — the same reasoning as
    /// [`Arrangement::faces`](crate::Arrangement).
    ///
    /// Keyed, so asking whether a name is already here reads a handful rather
    /// than all of them — see [`Body::add_face`], which every face raised goes
    /// through.
    named: Keyed<Naming>,
    /// Every face of the body in the order it was made, and which entry
    /// carries the next face of the same name.
    ///
    /// **Forward-linked where [`Keyed`] links back**, which is the whole reason
    /// it is written out here rather than filed there: what reads a face of the
    /// body wants its patches in the order they were made, and a chain grown
    /// from the head hands them back newest first.
    faced: Vec<FaceId>,
    after: Vec<u32>,
}

/// One name a body's faces carry, and the chain of faces carrying it.
///
/// **The chain rides with the name** rather than in two lists beside it: the
/// three are written in one breath and read in one, and held apart they are two
/// more places for an index to lose step with what it names.
///
/// The end as well as the beginning, so that adding a face to a name that
/// already has one is a link rather than a walk of everything it has.
#[derive(Debug, Clone, Copy)]
struct Naming {
    named: Named,
    /// Where its chain begins and ends in [`Body::faced`].
    first: u32,
    last: u32,
}

impl Body {
    /// Every face it has, each named once, in the order they were made.
    ///
    /// The base, the far end, then one wall per curve bounding the region —
    /// which is the order a prism answered in before there was a body, so that
    /// everything naming a face of a solid goes on naming the same one.
    pub fn names(&self) -> impl Iterator<Item = Named> + '_ {
        self.named.all().iter().map(|held| held.named)
    }

    /// Whether `named` is one of its faces.
    ///
    /// What anything keeping hold of a face across an edit has to ask. Answered
    /// off the list above rather than by a rule of its own, so what a body
    /// *has* and what it answers for cannot come to differ — the index only
    /// says which few of them are worth comparing.
    pub fn holds(&self, named: Named) -> bool {
        self.at(named).is_some()
    }

    /// Where `named` sits among the body's own names, or `None` where no face
    /// of it carries that name.
    fn at(&self, named: Named) -> Option<u32> {
        self.named
            .under(named.key())
            .find(|(_, held)| held.named == named)
            .map(|(at, _)| at)
    }

    /// Whether it shuts in nothing at all.
    pub fn is_empty(&self) -> bool {
        self.named.is_empty()
    }

    /// Whether every surface it stands on is of the exact tier.
    ///
    /// **`.notes/KERNEL.md` §4.1's claim, asked of a body rather than argued
    /// about.** A body made only of extrudes, revolves and booleans over
    /// planes, cylinders, cones and spheres is exact, and it can say so; one
    /// with a torus or a NURBS anywhere in it carries the bound whatever fitted
    /// it was made to, and it says that instead.
    ///
    /// A walk and not a stored flag, so nothing can set it and be wrong: the
    /// surfaces are the evidence, and this reads them. Edges are not asked —
    /// an edge is the meeting of two surfaces and cannot be exact where they
    /// are not.
    pub fn exact(&self) -> bool {
        self.topology.faces().all(|(_, face)| face.surface.exact())
    }

    /// How far the worst edge of it strays from the curve it lies on.
    ///
    /// **What [`Body::exact`] answers `false` for, in a number.** An exact body
    /// strays nowhere and answers nought; one with a marched curve in it
    /// answers the sagitta that curve was walked at, which is fixed where it
    /// was laid down and cannot be refined afterwards: nothing that reads a
    /// marched curve holds the surfaces it would take to walk it again.
    ///
    /// Nought is not the same claim as [`Body::exact`]: a body may stand on a
    /// torus, and so be inexact, while every edge of it is a circle.
    pub fn strays(&self) -> f64 {
        self.topology.carried().strays()
    }

    /// The pieces of surface `named` covers — several where one face of the
    /// body comes in disjoint patches — in the order they were made.
    ///
    /// **Down the name's own chain rather than by a walk of every face.** A
    /// caller drawing a body asks this once per name, so a walk costs the two
    /// counts multiplied and grows as the square of the body — the same
    /// argument [`Body::add_face`] makes for the name index beside it.
    ///
    /// **[`NONE`] stands past the end of the chain by construction**, so one
    /// reading both stops the walk where the name runs out and answers nothing
    /// at all for a name no face carries.
    pub(crate) fn patches(&self, named: Named) -> impl Iterator<Item = (FaceId, &Face)> {
        let mut at = self
            .at(named)
            .map_or(NONE, |name| self.named.get(name).first);
        std::iter::from_fn(move || {
            let step = at as usize;
            let &id = self.faced.get(step)?;
            at = self.after[step];
            Some(id)
        })
        .map(move |id| (id, self.topology.face(id)))
    }

    pub(crate) fn topology(&self) -> &Topology {
        &self.topology
    }

    pub(crate) fn topology_mut(&mut self) -> &mut Topology {
        &mut self.topology
    }

    /// Empty it, keeping every buffer it holds.
    ///
    /// What a rebuild does before it fills one, so that a solid redrawn as the
    /// drawing moves under it reaches the heap once rather than every frame.
    /// Every handle minted before this stops resolving — see
    /// `Topology::clear`, which this is the public half of.
    pub fn clear(&mut self) {
        self.topology.clear();
        self.named.clear();
        self.faced.clear();
        self.after.clear();
    }

    /// Add `face`, recording the name it carries among the body's own.
    ///
    /// **The one way a face joins a body**, which is what keeps the two indexes
    /// beside the topology true: a face of the body is the set of faces sharing
    /// a name, and the name a face is added under is the name it carries. Two
    /// calls could be given two different names, or one of them forgotten.
    ///
    /// **Asked of the index rather than of the list**, because every face
    /// raised asks: a walk of the names compared one against every name
    /// already recorded, and the cost of that grows as the square of the body.
    ///
    /// The names come back in the order their first faces were made in, and
    /// each name's patches in the order they were — see [`Body::names`] and
    /// [`Body::patches`], which two rebuilds of one drawing have to agree
    /// about for a renderer to refill its batches in place.
    pub(crate) fn add_face(&mut self, face: Face) -> FaceId {
        let named = face.name;
        let id = self.topology.add_face(face);
        let at = self.faced.len() as u32;
        self.faced.push(id);
        self.after.push(NONE);
        match self.at(named) {
            Some(name) => {
                let held = self.named.get_mut(name);
                let last = held.last;
                held.last = at;
                self.after[last as usize] = at;
            }
            None => {
                self.named.file(
                    named.key(),
                    Naming {
                        named,
                        first: at,
                        last: at,
                    },
                );
            }
        }
        id
    }
}

#[cfg(test)]
pub(crate) mod internals {
    use crate::number::tolerance::EXACT;
    use crate::solid::geometry::axis::Axis;
    use crate::solid::geometry::circle::Circle;
    use crate::solid::geometry::curve::Curve;
    use crate::solid::geometry::fitted::Fitted;
    use crate::solid::geometry::surface::Surface;
    use crate::solid::geometry::torus::Torus;
    use crate::solid::grown::Grown;
    use crate::solid::named::Step;
    use crate::solid::topology::body::Body;
    use crate::solid::topology::coedge::Coedge;
    use crate::solid::topology::edge::Edge;
    use crate::solid::topology::face::{Face, FaceId};
    use crate::solid::topology::lump::Lump;
    use crate::solid::topology::validity::{Checking, Reckoning};
    use crate::solid::topology::vertex::Vertex;
    use glam::DVec3;
    use std::f64::consts::{PI, TAU};

    impl Body {
        /// Everything a body promises, checked from scratch — panicking on the
        /// first thing broken and naming it.
        ///
        /// **The primary debugging tool**, and the single highest-leverage
        /// habit available while a kernel is being written: a kernel that
        /// cannot produce an invalid body has only local bugs.
        ///
        /// Here rather than beside the body because an *operation* checks its
        /// own output through a [`Checking`] it keeps — see
        /// [`Builder`](crate::Builder), which runs the same checks over the
        /// body it just filled, guarded by `cfg!(debug_assertions)` so a
        /// release build pays nothing. What this is for is a test holding a
        /// body it has taken apart by hand.
        pub(crate) fn check(&self) {
            Checking::default().run(self);
        }

        /// What the shell around its one lump comes to — its Euler characteristic
        /// and the genus that implies.
        ///
        /// The one lump, because one is all an extrusion makes. A boolean leaving
        /// several will want this per lump, and moving it then is a rename.
        pub(crate) fn reckoning(&self) -> Reckoning {
            let (_, lump) = self
                .topology
                .lumps()
                .next()
                .expect("a body with no lumps encloses nothing to reckon");
            Checking::default().reckoning(self.topology(), lump.outer)
        }

        /// A torus of `major` by `minor` about the world's `+Y` through the
        /// origin, quartered at the angles either of its parameters reads
        /// nought and a half turn at.
        ///
        /// **Four faces, eight edges, four vertices**, and `4 − 8 + 4` is
        /// nought, which is `2(1 − 1)`: a ring. Both parameters run round, so
        /// `.notes/KERNEL.md` §4.4's rule about wrapping bites twice — the ring
        /// is cut at two angles about the axis *and* at two round the tube, and
        /// four faces is the fewest that leaves none of them wrapping either
        /// way.
        ///
        /// **Every edge is a circle and no face is.** The four seams are exact
        /// curves — the two equators about the axis, of `major ± minor`, and
        /// the two tube circles of `minor` — so what puts this body in the
        /// fitted tier is its surfaces alone.
        ///
        /// Here rather than beside one of the tests that want it because two
        /// do: what a body is made of is asked of it in one place, and what a
        /// ray through its hole crosses in another.
        pub(crate) fn ring(major: f64, minor: f64) -> Self {
            let axis = Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X);
            let surface = Surface::Fitted(Fitted::Torus(Torus { axis, major, minor }));
            // The two equators run round the torus's own axis, so their
            // parameter is its first; the two tube circles are framed so theirs
            // is its second.
            let equator = |radius| Circle { axis, radius };
            let tube = |out: DVec3| Circle {
                axis: Axis::new(out * major, out.cross(DVec3::Y), out),
                radius: minor,
            };

            let mut body = Self::default();
            let named = Step::default().grew(Grown::Base);
            let mut corner = |out: f64| {
                body.topology_mut().add_vertex(Vertex {
                    at: DVec3::X * out,
                    tolerance: EXACT,
                })
            };
            // Where the two seams cross, which is the four places both angles
            // read nought or a half turn: out on the equator and in on it, each
            // way round.
            let (out_near, in_near) = (corner(major + minor), corner(major - minor));
            let (out_far, in_far) = (corner(-major - minor), corner(-major + minor));

            let mut face = || {
                body.add_face(Face {
                    surface,
                    outward: true,
                    loops: 0..0,
                    name: named,
                    tolerance: EXACT,
                })
            };
            // The quarters, named for the half of each angle they cover.
            let (near_top, far_top) = (face(), face());
            let (far_under, near_under) = (face(), face());

            let mut seam = |curve, bounds: [f64; 2], from, to, between| {
                body.topology_mut().add_edge(Edge {
                    curve: Curve::Circle(curve),
                    bounds,
                    from,
                    to,
                    between,
                    // One torus on both sides of it — see §4.4.
                    artificial: true,
                    tolerance: EXACT,
                })
            };
            let (half, back) = ([0.0, PI], [PI, TAU]);
            let (outer, inner) = (equator(major + minor), equator(major - minor));
            let (here, there) = (tube(DVec3::X), tube(DVec3::NEG_X));
            let out_over = seam(outer, half, out_near, out_far, [near_top, near_under]);
            let out_back = seam(outer, back, out_far, out_near, [far_top, far_under]);
            let in_over = seam(inner, half, in_near, in_far, [near_top, near_under]);
            let in_back = seam(inner, back, in_far, in_near, [far_top, far_under]);
            let near_over = seam(here, half, out_near, in_near, [near_top, far_top]);
            let near_back = seam(here, back, in_near, out_near, [far_under, near_under]);
            let far_over = seam(there, half, out_far, in_far, [near_top, far_top]);
            let far_back = seam(there, back, in_far, out_far, [far_under, near_under]);

            // Counterclockwise in each quarter's own parameters: along the seam
            // it starts at, round the tube at the far angle, back along the
            // other seam and round the tube again.
            for (face, walk) in [
                (
                    near_top,
                    [
                        (out_over, true),
                        (far_over, true),
                        (in_over, false),
                        (near_over, false),
                    ],
                ),
                (
                    far_top,
                    [
                        (out_back, true),
                        (near_over, true),
                        (in_back, false),
                        (far_over, false),
                    ],
                ),
                (
                    far_under,
                    [
                        (in_back, true),
                        (near_back, true),
                        (out_back, false),
                        (far_back, false),
                    ],
                ),
                (
                    near_under,
                    [
                        (in_over, true),
                        (far_back, true),
                        (out_over, false),
                        (near_back, false),
                    ],
                ),
            ] {
                let at = body.topology_mut().add_loop(|into| {
                    into.extend(walk.map(|(edge, forward)| Coedge { edge, forward }));
                });
                body.topology_mut().face_mut(face).loops = at..at + 1;
            }

            body.sealed(&[near_top, far_top, far_under, near_under]);
            body
        }

        /// Gather `faces` into one shell and that shell into one lump, which is
        /// how a closed body ends however it was built.
        pub(crate) fn sealed(&mut self, faces: &[FaceId]) {
            let outer = self.topology_mut().add_shell_of(faces.iter().copied());
            self.topology_mut().add_lump(Lump { outer, voids: 0..0 });
        }
    }
}
