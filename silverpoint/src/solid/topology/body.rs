//! A whole solid.

use crate::solid::buckets::Buckets;
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
    /// The distinct names its faces carry, in the order they were made.
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
    names: Vec<Named>,
    /// Which of those names key alike, so asking whether a name is already
    /// here reads a handful rather than all of them — see [`Body::named`],
    /// which every face raised goes through.
    known: Buckets,
}

impl Body {
    /// Every face it has, each named once, in the order they were made.
    ///
    /// The base, the far end, then one wall per curve bounding the region —
    /// which is the order a prism answered in before there was a body, so that
    /// everything naming a face of a solid goes on naming the same one.
    pub fn names(&self) -> impl Iterator<Item = Named> + '_ {
        self.names.iter().copied()
    }

    /// Whether `named` is one of its faces.
    ///
    /// What anything keeping hold of a face across an edit has to ask. Answered
    /// off the list above rather than by a rule of its own, so what a body
    /// *has* and what it answers for cannot come to differ — the index only
    /// says which few of them are worth comparing.
    pub fn holds(&self, named: Named) -> bool {
        self.known
            .under(named.key())
            .any(|at| self.names[at as usize] == named)
    }

    /// Whether it shuts in nothing at all.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
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

    /// The pieces of surface `named` covers — several where one face of the
    /// body comes in disjoint patches.
    pub(crate) fn patches(&self, named: Named) -> impl Iterator<Item = (FaceId, &Face)> {
        self.topology
            .faces()
            .filter(move |(_, face)| face.name == named)
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
        self.names.clear();
        self.known.clear();
    }

    /// Record that `named` is a face of this body, if it is not already.
    ///
    /// Called by whatever adds a face rather than derived afterwards, so the
    /// order the names come back in is the order the faces were made in.
    ///
    /// **Asked of the index rather than of the list**, because every face
    /// raised asks: a walk of the names compared one against every name
    /// already recorded, and the cost of that grows as the square of the body.
    pub(crate) fn named(&mut self, named: Named) {
        if self.holds(named) {
            return;
        }
        let at = self.known.file(named.key());
        debug_assert_eq!(at as usize, self.names.len(), "the index lost step");
        self.names.push(named);
    }
}

#[cfg(test)]
pub(crate) mod internals {
    use crate::solid::topology::body::Body;
    use crate::solid::topology::validity::{Checking, Reckoning};

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
    }
}
