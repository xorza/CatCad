//! Everything a body promises, checked from scratch.

use crate::number::predicate::{self, slack};
use crate::solid::meeting::Meeting;
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::FaceId;
use crate::solid::topology::shell::ShellId;

/// The room the validity check works in, kept across runs.
///
/// A struct rather than a set of functions because the checks all want side
/// tables, and because a body is checked after every operation — which on the
/// path a drag runs is every frame. Held, they cost nothing after the first
/// body; stood up per call they would be the only thing on that path reaching
/// the heap. A [`Builder`](crate::Builder) keeps one, and a test that reaches
/// for `Body::check` makes a throwaway.
///
/// Everything here is indexed by arena slot rather than hashed. A slot is what
/// a handle already carries, so a side table costs a vector and an index where
/// a map would cost a hash per lookup — the same trick every side table in this
/// crate plays.
#[derive(Debug, Default)]
pub(crate) struct Checking {
    /// How often each edge is walked, backwards then forwards.
    walked: Vec<[usize; 2]>,
    /// How many shells hold each face.
    shelled: Vec<usize>,
    /// Which faces a walk across shared edges has reached, and which are left
    /// to step off.
    standing: Vec<bool>,
    waiting: Vec<FaceId>,
    /// Which edges and vertices a count of one shell has already taken in.
    counted: Vec<bool>,
    cornered: Vec<bool>,
}

impl Checking {
    /// Run every check over `body`, panicking on the first thing broken.
    ///
    /// In the order a failure is most usefully reported: structure before
    /// geometry, because a geometric complaint about a body whose loops do not
    /// close says nothing about the geometry.
    pub(crate) fn run(&mut self, body: &Body) {
        let topology = body.topology();
        self.loops_close(topology);
        self.edges_are_used_twice(topology);
        self.faces_belong_to_one_shell(topology);
        self.shells_are_connected(topology);
        self.shells_are_closed_surfaces(topology);
        self.geometry_agrees(topology);
        self.creases_are_flagged(topology);
        self.tolerances_ladder(topology);
    }

    /// Every loop is closed: each coedge ends where the next begins.
    fn loops_close(&self, topology: &Topology) {
        for (id, face) in topology.faces() {
            for (at, walk) in topology.loops_of(face).enumerate() {
                assert!(
                    !walk.is_empty(),
                    "face {id:?} has an empty loop at {at} — a loop shuts nothing in",
                );
                for (one, two) in walk.iter().zip(walk.iter().cycle().skip(1)) {
                    let [_, ends] = topology.ends(*one);
                    let [starts, _] = topology.ends(*two);
                    assert!(
                        ends == starts,
                        "face {id:?} loop {at} breaks between {one:?} and {two:?}",
                    );
                }
            }
        }
    }

    /// Every edge is walked exactly twice, once each way, by the two faces it
    /// says it lies between.
    ///
    /// The manifold condition and the orientability condition at once — which
    /// is why it is one check. An edge walked twice the same way is two faces
    /// facing opposite ways across it, and an edge walked three times is a
    /// non-manifold body this kernel has no representation for.
    fn edges_are_used_twice(&mut self, topology: &Topology) {
        let walked = &mut self.walked;
        walked.clear();
        walked.resize(topology.edge_slots(), [0; 2]);
        for (id, face) in topology.faces() {
            for coedge in topology.loops_of(face).flatten() {
                let edge = topology.edge(coedge.edge);
                assert!(
                    edge.between.contains(&id),
                    "face {id:?} walks edge {:?}, which lies between {:?}",
                    coedge.edge,
                    edge.between,
                );
                walked[coedge.edge.slot()][usize::from(coedge.forward)] += 1;
            }
        }
        for (id, edge) in topology.edges() {
            let [back, forth] = walked[id.slot()];
            assert!(
                back == 1 && forth == 1,
                "edge {id:?} is walked {forth} times forward and {back} back, not once each",
            );
            assert!(
                edge.between[0] != edge.between[1],
                "edge {id:?} lies between one face and itself",
            );
        }
    }

    /// Every face belongs to exactly one shell.
    fn faces_belong_to_one_shell(&mut self, topology: &Topology) {
        let shelled = &mut self.shelled;
        shelled.clear();
        shelled.resize(topology.face_slots(), 0);
        for (_, lump) in topology.lumps() {
            for shell in topology.shells_of(lump) {
                for &face in topology.faces_of(shell) {
                    shelled[face.slot()] += 1;
                }
            }
        }
        for (id, _) in topology.faces() {
            let held = shelled[id.slot()];
            assert!(held == 1, "face {id:?} is held by {held} shells, not one");
        }
    }

    /// Every shell is one connected sheet rather than several told apart only
    /// by having been put in the same list.
    fn shells_are_connected(&mut self, topology: &Topology) {
        for (id, lump) in topology.lumps() {
            for shell in topology.shells_of(lump) {
                let reached = self.reachable(topology, shell);
                let faces = topology.faces_of(shell);
                assert!(
                    reached == faces.len(),
                    "shell {shell:?} of lump {id:?} lists {} faces and reaches {reached}",
                    faces.len(),
                );
            }
        }
    }

    /// How many faces of `shell` are reachable from the first by stepping
    /// across shared edges.
    fn reachable(&mut self, topology: &Topology, shell: ShellId) -> usize {
        let Self {
            standing, waiting, ..
        } = self;
        standing.clear();
        standing.resize(topology.face_slots(), false);
        waiting.clear();
        let Some(&first) = topology.faces_of(shell).first() else {
            return 0;
        };
        standing[first.slot()] = true;
        waiting.push(first);
        let mut found = 1;
        while let Some(face) = waiting.pop() {
            for coedge in topology.loops_of(topology.face(face)).flatten() {
                for across in topology.edge(coedge.edge).between {
                    if !standing[across.slot()] {
                        standing[across.slot()] = true;
                        found += 1;
                        waiting.push(across);
                    }
                }
            }
        }
        found
    }

    /// Every shell is a closed surface: its Euler characteristic is even, so
    /// the genus it implies is a whole number, and that genus is not negative.
    ///
    /// **Euler–Poincaré**, `V − E + F − R = 2(S − G)`, per shell so `S` is one.
    /// It is the one check that reads the whole of a shell at once, and it
    /// catches what the local checks cannot: a face left out, an edge counted
    /// into the wrong loop, a hole recorded as an outline.
    fn shells_are_closed_surfaces(&mut self, topology: &Topology) {
        for (_, lump) in topology.lumps() {
            for shell in topology.shells_of(lump) {
                let Reckoning {
                    characteristic,
                    genus,
                } = self.reckoning(topology, shell);
                assert!(
                    characteristic % 2 == 0,
                    "shell {shell:?} has an odd Euler characteristic of {characteristic}, \
                     so it is not a closed surface",
                );
                assert!(genus >= 0, "shell {shell:?} has a genus of {genus}");
            }
        }
    }

    /// What one shell comes to: `V − E + F − R`, and the genus that implies.
    pub(crate) fn reckoning(&mut self, topology: &Topology, shell: ShellId) -> Reckoning {
        let characteristic = self.characteristic(topology, shell);
        Reckoning {
            characteristic,
            genus: (2 - characteristic) / 2,
        }
    }

    /// `V − E + F − R` over one shell.
    fn characteristic(&mut self, topology: &Topology, shell: ShellId) -> i64 {
        let Self {
            counted, cornered, ..
        } = self;
        counted.clear();
        counted.resize(topology.edge_slots(), false);
        cornered.clear();
        cornered.resize(topology.vertex_slots(), false);
        let faces = topology.faces_of(shell);
        let (mut edges, mut vertices, mut rings) = (0i64, 0i64, 0i64);
        for &face in faces {
            let face = topology.face(face);
            rings += face.holes() as i64;
            for coedge in topology.loops_of(face).flatten() {
                if !std::mem::replace(&mut counted[coedge.edge.slot()], true) {
                    edges += 1;
                }
                for end in topology.ends(*coedge) {
                    if !std::mem::replace(&mut cornered[end.slot()], true) {
                        vertices += 1;
                    }
                }
            }
        }
        vertices - edges + faces.len() as i64 - rings
    }

    /// The geometry agrees with the topology: every vertex stands where the
    /// curves and surfaces meeting it say it does.
    ///
    /// The redundancy is deliberate and this is what makes it safe. An edge
    /// knows both the parameters it runs between and the vertices at its ends,
    /// which are two statements of one fact — and two statements of one fact
    /// are worth having exactly when something checks that they agree.
    fn geometry_agrees(&self, topology: &Topology) {
        for (id, edge) in topology.edges() {
            for (end, bound) in edge.ends(true).into_iter().zip(edge.bounds) {
                let vertex = topology.vertex(end);
                let given = slack(vertex.tolerance);
                assert!(
                    predicate::coincident(vertex.at, edge.curve.at(bound), given),
                    "edge {id:?} at {bound} is {} from vertex {end:?}, which stands for {given}",
                    vertex.at.distance(edge.curve.at(bound)),
                );
            }
            for face in edge.between {
                self.edge_lies_on(topology, id, edge, face);
            }
        }
    }

    /// An edge lies on the surface of a face that uses it, all the way along.
    ///
    /// Sampled rather than argued, because the exact statement — that this
    /// curve is a component of that surface's intersection with the other one —
    /// is what the intersection routine already promised. What can go wrong
    /// afterwards is bookkeeping: an edge attached to the wrong face, a
    /// parameter range copied from its neighbour. Samples catch those.
    fn edge_lies_on(&self, topology: &Topology, id: EdgeId, edge: &Edge, face: FaceId) {
        const SAMPLES: usize = 5;
        let surface = topology.face(face).surface;
        let given = slack(edge.tolerance);
        let [from, to] = edge.bounds;
        for sample in 0..=SAMPLES {
            let at = edge
                .curve
                .at(from + (to - from) * sample as f64 / SAMPLES as f64);
            let off = surface.off(at);
            assert!(
                predicate::touching(off, given),
                "edge {id:?} leaves face {face:?} by {off} a fifth of the way at {sample}, \
                 where it stands for {given}",
            );
        }
    }

    /// An edge is flagged as no crease exactly when its two faces lie on one
    /// surface.
    ///
    /// A flag re-derived rather than trusted, which is what a validity check is
    /// for. What sets it today could hardly get it wrong; what sets it after a
    /// boolean is joining faces from two bodies, and an edge wrongly called
    /// smooth is a crease a fillet would decline to round and an export would
    /// quietly drop.
    fn creases_are_flagged(&self, topology: &Topology) {
        for (id, edge) in topology.edges() {
            let [one, two] = edge.between.map(|face| topology.face(face).surface);
            assert!(
                edge.artificial == (Meeting::of(&one, &two) == Meeting::Same),
                "edge {id:?} calls itself {} between {one:?} and {two:?}",
                if edge.artificial {
                    "smooth"
                } else {
                    "a crease"
                },
            );
        }
    }

    /// The tolerance ladder holds: a vertex covers every edge meeting it, and
    /// an edge covers both faces using it.
    ///
    /// A body whose ladder is upside down is claiming to know a corner more
    /// precisely than it knows the curves that make it, which is how a
    /// tolerance model quietly stops meaning anything. See
    /// `.notes/KERNEL.md` §4.3.
    fn tolerances_ladder(&self, topology: &Topology) {
        for (id, edge) in topology.edges() {
            for face in edge.between {
                let face = topology.face(face);
                assert!(
                    edge.tolerance >= face.tolerance,
                    "edge {id:?} is tighter than the face it lies on",
                );
            }
            for end in edge.ends(true) {
                let vertex = topology.vertex(end);
                assert!(
                    vertex.tolerance >= edge.tolerance,
                    "vertex {end:?} is tighter than edge {id:?}, which ends there",
                );
            }
        }
        for (id, face) in topology.faces() {
            assert!(
                face.tolerance == 0.0,
                "face {id:?} carries a tolerance of {}, and a surface here is exact",
                face.tolerance,
            );
        }
    }
}

/// What a body's own reckoning of itself comes to.
///
/// Held apart from the assertions because a caller may want the numbers
/// without them — a test saying a profile with a hole gives a shell of genus
/// one asserts on this rather than on the checker having passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reckoning {
    pub(crate) characteristic: i64,
    pub(crate) genus: i64,
}
