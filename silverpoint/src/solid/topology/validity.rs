//! Everything a body promises, checked from scratch.

use crate::math::chorded::Chorded;
use crate::math::intersect::{self, Span};
use crate::number::predicate::{self, ApproxEq, slack};
use crate::number::tolerance::CHORDED;
use crate::solid::meeting::Meeting;
use crate::solid::mesh::{Mesher, Patch};
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::FaceId;
use crate::solid::topology::shell::ShellId;
use crate::solid::topology::spreading::Spreading;
use glam::{DVec2, DVec3};

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
    /// The walk that says what one shell holds — see [`Spreading`].
    spreading: Spreading,
    /// Which edges and vertices a count of one shell has already taken in.
    counted: Vec<bool>,
    cornered: Vec<bool>,
    /// The room measuring a shell takes — see [`Checking::volumes_are_signed`].
    mesher: Mesher,
    patch: Patch,
    /// One loop walked in the world, and the same loop in its face's own
    /// parameters — see [`Checking::loops_do_not_cross_themselves`].
    traced: Vec<DVec3>,
    flattened: Vec<DVec2>,
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
        self.loops_do_not_cross_themselves(topology);
        self.geometry_agrees(topology);
        self.creases_are_flagged(topology);
        self.tolerances_ladder(topology);
        self.volumes_are_signed(body);
    }

    /// No loop of a face crosses itself in that face's own parameters.
    ///
    /// **The one break that leaves a face looking like a face.** A loop that
    /// crosses itself still closes, still walks each of its edges once, and
    /// still lies on the surface it names — so nothing above sees it. What it
    /// is not is a *boundary*: the region it is supposed to enclose is on both
    /// sides of it at the crossing, so what a triangulation makes of it, what a
    /// sounding says about a place in it, and what area it covers are all
    /// answers to a question with no answer.
    ///
    /// **Chorded, and that is the whole of what it costs.** A loop is walked at
    /// [`CHORDED`] into the surface's parameters, exactly as the boolean and the
    /// mesher walk it, and a chorded loop that crosses itself is a true loop
    /// that does too — the chords lie within a sagitta of the curve, and a
    /// crossing is not a thing a sagitta hides.
    ///
    /// **Through [`intersect::spans`], which decides it exactly.** Two chords
    /// that meet at a shared corner are the adjacent pair every loop has and
    /// are skipped; every other pair that meets at all is a fold in the
    /// boundary. Ends count, so a chord that merely touches the middle of
    /// another is caught as well — that is a pinch rather than a crossing, and
    /// a boundary that touches itself is no more a boundary than one that
    /// crosses.
    fn loops_do_not_cross_themselves(&mut self, topology: &Topology) {
        let Self {
            traced, flattened, ..
        } = self;
        for (at, face) in topology.faces() {
            for walk in topology.loops_of(face) {
                traced.clear();
                for &coedge in walk {
                    topology.walked(coedge).walk(CHORDED, traced);
                }
                flattened.clear();
                // One loop at a time and against itself alone, which is what
                // this check asks — so each is read on its own branch and there
                // is nothing for it to agree with.
                face.flatten(&traced[..], None, flattened);
                let held = flattened.len();
                let chord = |step: usize| Span {
                    from: flattened[step],
                    to: flattened[(step + 1) % held],
                };
                // How far each chord reaches, so that the pair test below is
                // four comparisons for the great many that are nowhere near
                // each other and an exact predicate only for the few that are.
                // A curved face's loop is a hundred and more chords and this
                // runs after every operation — without it the application's own
                // suites took five times as long.
                let around = |step: usize| {
                    let of = chord(step);
                    [of.from.min(of.to), of.from.max(of.to)]
                };
                for one in 0..held {
                    let [low, high] = around(one);
                    // From two on, because the pair either side of every corner
                    // shares it; and stopping short of the wrap for the same
                    // reason at the other end.
                    for two in one + 2..held - usize::from(one == 0) {
                        let [other, across] = around(two);
                        if low.cmpgt(across).any() || other.cmpgt(high).any() {
                            continue;
                        }
                        let found = intersect::spans(chord(one), chord(two));
                        assert!(
                            found.all().is_empty(),
                            "{at:?}: its loop folds over itself between chords \
                             {one} and {two}, at {:?}",
                            found.all().first().map(|of| of.at),
                        );
                    }
                }
            }
        }
    }

    /// Every lump shuts in material, and every cavity shuts in the lack of it.
    ///
    /// **The one break nothing above can see.** A shell turned through itself
    /// still walks each of its edges twice, still satisfies Euler, and still
    /// has every face on the surface it names, so every check so far passes on
    /// a body that is inside out. What gives it away is the sign of what it
    /// encloses: a face bounds material on the side it does not face, so a
    /// cavity's faces point *into* it and a lump wound the wrong way reads as
    /// one.
    ///
    /// Re-derived rather than trusted. The sewing sorts outer shells from
    /// cavities by this same sign, and a check that read its answer back would
    /// be checking that a number equals itself.
    ///
    /// **Through the mesher, chorded at [`CHORDED`]**, which is the one form of
    /// the divergence theorem that does not care what the faces lie on — see
    /// [`Mesher::shut_in`]. Only the sign is read, and no chording turns one
    /// over.
    fn volumes_are_signed(&mut self, body: &Body) {
        let Self { mesher, patch, .. } = self;
        let topology = body.topology();
        for (id, lump) in topology.lumps() {
            let shut_in = mesher.shut_in(body, topology.faces_of(lump.outer), CHORDED, patch);
            assert!(
                shut_in > 0.0,
                "lump {id:?} shuts in {shut_in}, so its shell {:?} faces inward",
                lump.outer,
            );
            for &shell in topology.voids_of(lump) {
                let shut_in = mesher.shut_in(body, topology.faces_of(shell), CHORDED, patch);
                assert!(
                    shut_in < 0.0,
                    "cavity {shell:?} of lump {id:?} shuts in {shut_in}, so it faces outward",
                );
            }
        }
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
                let faces = topology.faces_of(shell);
                let Some(&first) = faces.first() else {
                    continue;
                };
                // A clean sheet per shell: this asks what one shell reaches on
                // its own, where the sewing asks what a whole body falls into.
                self.spreading.restart(topology);
                let reached = self.spreading.across(topology, first).len();
                assert!(
                    reached == faces.len(),
                    "shell {shell:?} of lump {id:?} lists {} faces and reaches {reached}",
                    faces.len(),
                );
            }
        }
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
                // Over how large the arithmetic worked, which is the curve's
                // to say rather than the answer's: a line reaching back from
                // far away lands next to the origin off terms a hundred
                // million wide. See [`slack`] and [`Curve::reach`].
                let given = slack(
                    vertex.tolerance,
                    edge.curve.reach(bound).max(vertex.at.length()),
                );
                let at = edge.curve.at(bound, topology.carried());
                assert!(
                    vertex.at.approx_eq(at, given),
                    "edge {id:?} at {bound} is {} from vertex {end:?}, which stands for {given}",
                    vertex.at.distance(at),
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
        let [from, to] = edge.bounds;
        for sample in 0..=SAMPLES {
            let t = from + (to - from) * sample as f64 / SAMPLES as f64;
            let at = edge.curve.at(t, topology.carried());
            let off = surface.off(at);
            // Per sample rather than per edge, the samples of a long edge
            // standing at wildly different sizes and each written down to a
            // proportion of what its own arithmetic worked in.
            let given = slack(edge.tolerance, edge.curve.reach(t).max(at.length()));
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
