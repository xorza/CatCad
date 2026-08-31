use crate::math::chorded::Chorded;
use crate::math::plane::Plane;
use crate::number::tolerance::{CHORDED, EXACT};
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::builder::Extrusion;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use crate::solid::grown::Grown;
use crate::solid::mesh::Mesher;
use crate::solid::named::Step;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::Edge;
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::vertex::Vertex;
use glam::DVec3;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// A two-by-two block three deep — the simplest thing the checker has an
/// opinion about, and the fixture every mutation below breaks.
fn block() -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, &[0], Plane::GROUND, 3.0, Step::default()).body()
}

/// A body that was built properly passes every check, and asking twice changes
/// nothing.
///
/// The floor the rest of this file stands on: a checker that refused a valid
/// body would make every `should_panic` below pass for the wrong reason.
#[test]
fn a_body_off_the_builder_passes_everything() {
    let body = block();
    body.check();
    body.check();
}

/// An edge walked the same way twice is two faces facing opposite ways across
/// it — the manifold condition and the orientability condition at once.
#[test]
#[should_panic(expected = "not once each")]
fn an_edge_walked_twice_the_same_way_is_refused() {
    let mut body = block();
    // The whole loop turned round rather than one coedge of it, which keeps it
    // closed: what breaks is that both faces across every one of those four
    // edges now walk it the same way.
    let wall = wall_loop(&body);
    let outline = body.topology_mut().loop_mut(wall);
    outline.reverse();
    for coedge in outline.iter_mut() {
        *coedge = coedge.turned();
    }
    body.check();
}

/// A loop that does not close bounds nothing, whatever it is a list of.
#[test]
#[should_panic(expected = "breaks between")]
fn a_loop_that_does_not_close_is_refused() {
    let mut body = block();
    // Swap two of the four coedges, so the walk jumps rather than joining.
    let wall = wall_loop(&body);
    body.topology_mut().loop_mut(wall).swap(0, 1);
    body.check();
}

/// **A loop that folds over itself bounds nothing**, however well every other
/// thing about it holds.
///
/// The break every check before this one passes. The loop still closes, every
/// edge is still walked twice and once each way, Euler is still two, and every
/// face still lies on the surface it names — a boundary that crosses itself is
/// none of those things' business. What it *is* is a region on both sides of
/// its own edge, so what a triangulation makes of it, what a sounding says
/// about a place in it, and what area it covers have no answers.
///
/// **The corner is moved and not the curve**, because a loop is walked through
/// the places its vertices are stored at rather than through the curve
/// evaluated at its ends — see [`Chorded::cut`](crate::math::chorded::Chorded),
/// which is what keeps two faces meeting at a corner from landing a rounding
/// apart there.
///
/// `(2, 0, 0)` is carried out past the far side of the wall it helps bound, to
/// `(−1, 1.5, 0)`. The chord that leaves it then runs from `x = −1` across to
/// `x = 2` and meets the wall's left edge at `y = 2`, which is inside the three
/// that edge covers. The two are not neighbours in the loop, so nothing excuses
/// them.
#[test]
#[should_panic(expected = "folds over itself")]
fn a_loop_that_folds_over_itself_is_refused() {
    let mut body = block();
    let corner = body
        .topology()
        .vertices()
        .find(|(_, vertex)| vertex.at == DVec3::new(2.0, 0.0, 0.0))
        .map(|(at, _)| at)
        .expect("the block has a corner there");
    body.topology_mut().vertex_mut(corner).at = DVec3::new(-1.0, 1.5, 0.0);
    body.check();
}

/// Where one of the block's walls keeps its loop.
fn wall_loop(body: &Body) -> usize {
    body.topology()
        .faces()
        .map(|(_, face)| face.loops.start)
        .nth(2)
        .expect("a block has walls")
}

/// A face left out of every shell is a face the body does not really hold.
#[test]
#[should_panic(expected = "is held by 0 shells")]
fn a_face_in_no_shell_is_refused() {
    let mut body = block();
    let (_, lump) = body
        .topology()
        .lumps()
        .next()
        .expect("a block has one lump");
    let outer = lump.outer;
    // One face short at the end of the shell's stretch, so it belongs to none.
    body.topology_mut().shell_mut(outer).faces.end -= 1;
    body.check();
}

/// A vertex inside what it stands for is where it says it is.
///
/// The pair to the test below, and they matter together: one alone would be
/// passed by a check that read a constant of its own rather than the vertex's
/// number, or by one that had been switched off.
#[test]
fn a_vertex_may_stand_anywhere_within_what_it_stands_for() {
    let mut body = block();
    nudged(&mut body, 0.5);
    body.check();
}

/// A vertex further off its curves than it stands for is caught.
#[test]
#[should_panic(expected = "from vertex")]
fn a_vertex_beyond_what_it_stands_for_is_refused() {
    let mut body = block();
    nudged(&mut body, 8.0);
    body.check();
}

/// How wide a corner is made to stand for, so that there is something to move
/// it within.
///
/// **A block's corners are exact**, nothing having folded into any of them —
/// see [`Arrangement::reached`](crate::Arrangement) — so the body says they
/// stand for nothing at all, and a test about a vertex standing *within* what
/// it stands for has to give it something first.
const STANDS: f64 = 1e-6;

/// Move one corner of `body` off its curves by `much` of what it is made to
/// stand for.
fn nudged(body: &mut Body, much: f64) {
    let corner = body
        .topology()
        .edges()
        .next()
        .expect("a block has edges")
        .1
        .from;
    let vertex = body.topology_mut().vertex_mut(corner);
    vertex.tolerance = STANDS;
    vertex.at.x += STANDS * much;
}

/// The ladder holds downward: a vertex covers the edges meeting it, and an edge
/// covers the faces it lies between.
///
/// A body whose ladder is upside down claims to know a corner more precisely
/// than the curves that make it, which is how a tolerance model quietly stops
/// meaning anything. See `.notes/KERNEL.md` §4.3.
#[test]
#[should_panic(expected = "is tighter than")]
fn a_vertex_tighter_than_its_edge_is_refused() {
    let mut body = block();
    let edge = body.topology().edges().next().expect("a block has edges").0;
    // The edge widened rather than the vertex tightened, there being nothing
    // below exact to tighten a corner of a block to.
    body.topology_mut().edge_mut(edge).tolerance = STANDS;
    body.check();
}

/// A shell turned through itself is the one break every other check passes.
///
/// Every edge of the block is still walked twice and once each way, Euler is
/// still two, every face still lies on the surface it names, and the ladder is
/// untouched — turning a face round says nothing about any of them. What is
/// left is the sign: the block shuts in `2 × 2 × 3`, and inside out it shuts in
/// `−12`.
#[test]
#[should_panic(expected = "faces inward")]
fn a_lump_facing_inward_is_refused() {
    let mut body = block();
    let faces: Vec<FaceId> = body.topology().faces().map(|(at, _)| at).collect();
    for at in faces {
        let face = body.topology_mut().face_mut(at);
        face.outward = !face.outward;
    }
    body.check();
}

/// **A cone, built by hand and validated as a body** — which nothing in the
/// kernel constructs, so without this the surface is tested and the *solid* is
/// not. The pair to the sphere below, and the easier of the two.
///
/// A party hat: the apex at the origin, opening up `+y` at forty-five degrees,
/// cut off by its base two units up — so the base circle has radius two, the
/// tangent of forty-five being one.
///
/// **Three faces, and the reason is §4.4.** No face may wrap, so the lateral
/// surface is split down two rulings into halves that each cover half a turn,
/// and the base disc closes it. Three vertices — the apex and the two places
/// the rulings meet the base — and four edges: two arcs of the base circle and
/// the two rulings. `3 − 4 + 3` is two, which is a sphere, which is what the
/// boundary of a solid cone is.
///
/// **The apex is one vertex, not two**, and both rulings end there. It is also
/// where the parameterization says nothing — every angle names the same place —
/// which is what makes a cone worth building by hand rather than assuming.
#[test]
fn a_cone_built_by_hand_is_a_valid_body() {
    let body = cone();
    body.check();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.characteristic, 2, "{reckoning:?}");
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");
    assert_eq!(body.topology().faces().count(), 3);
    assert_eq!(body.topology().edges().count(), 4);
}

/// The party hat above: apex at the origin, `+y`, forty-five degrees, two tall.
fn cone() -> Body {
    let upright = Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X);
    let surface = Surface::Natural(Natural::Cone(Cone {
        axis: upright,
        half_angle: FRAC_PI_4,
    }));
    let rim = Circle {
        axis: Axis::new(DVec3::new(0.0, TALL, 0.0), DVec3::Y, DVec3::X),
        radius: TALL,
    };
    let lid = Plane {
        origin: DVec3::new(0.0, TALL, 0.0),
        x: DVec3::X,
        // So that `x × y` is `+y`, which is out of a solid sitting below it.
        y: -DVec3::Z,
    };

    let mut body = Body::default();
    let named = Step::default().grew(Grown::Base);
    body.named(named);
    let apex = body.topology_mut().add_vertex(Vertex {
        at: DVec3::ZERO,
        tolerance: EXACT,
    });
    let round = |angle: f64| body_place(&rim, angle);
    let (near, far) = (
        body.topology_mut().add_vertex(Vertex {
            at: round(0.0),
            tolerance: EXACT,
        }),
        body.topology_mut().add_vertex(Vertex {
            at: round(PI),
            tolerance: EXACT,
        }),
    );

    let mut face = |surface, outward| {
        body.topology_mut().add_face(Face {
            surface,
            outward,
            loops: 0..0,
            name: named,
            tolerance: EXACT,
        })
    };
    // Material inside, so every one of them faces away from the axis — and the
    // lid faces up, off the top of the solid below it.
    let (here, there) = (face(surface, true), face(surface, true));
    let base = face(Surface::Natural(Natural::Plane(lid)), true);

    let mut edge = |curve, bounds: [f64; 2], from, to, between, artificial| {
        body.topology_mut().add_edge(Edge {
            curve,
            bounds,
            from,
            to,
            between,
            artificial,
            tolerance: EXACT,
        })
    };
    // The two halves of the rim, and the two rulings the wrap was cut down.
    // A ruling is smooth — the same cone on both sides of it — where an arc of
    // the rim is a genuine crease between the cone and the lid.
    let arcs = [
        edge(
            Curve::Circle(rim),
            [0.0, PI],
            near,
            far,
            [base, here],
            false,
        ),
        edge(
            Curve::Circle(rim),
            [PI, TAU],
            far,
            near,
            [base, there],
            false,
        ),
    ];
    let rulings = [PI, 0.0].map(|angle| {
        let along = round(angle) - DVec3::ZERO;
        let line = Line {
            origin: DVec3::ZERO,
            direction: along.normalize(),
        };
        let end = if angle == 0.0 { near } else { far };
        edge(
            Curve::Line(line),
            [0.0, along.length()],
            apex,
            end,
            [here, there],
            true,
        )
    });

    // Counterclockwise in each face's own parameters. On a lateral half that is
    // up the ruling at the far edge of its turn, back along the rim, and down
    // the near one — the apex being a whole side of the region collapsed to a
    // point, so there is nothing to walk along the bottom.
    let walks = [
        (
            here,
            [(rulings[0], true), (arcs[0], false), (rulings[1], false)],
        ),
        (
            there,
            [(rulings[1], true), (arcs[1], false), (rulings[0], false)],
        ),
    ];
    for (face, walk) in walks {
        let at = body.topology_mut().add_loop(|into| {
            into.extend(walk.map(|(edge, forward)| Coedge { edge, forward }));
        });
        body.topology_mut().face_mut(face).loops = at..at + 1;
    }
    let at = body.topology_mut().add_loop(|into| {
        into.extend(arcs.map(|edge| Coedge {
            edge,
            forward: true,
        }));
    });
    body.topology_mut().face_mut(base).loops = at..at + 1;

    body.sealed(&[here, there, base]);
    body
}

/// How tall the cone stands, which is also its base radius at forty-five
/// degrees.
const TALL: f64 = 2.0;

/// Where `angle` lands on `rim`.
fn body_place(rim: &Circle, angle: f64) -> DVec3 {
    rim.at(angle)
}

/// **A sphere, built by hand and validated as a body** — the pair to the cone
/// above, and the harder shape.
///
/// **Two faces, two edges, two vertices**, and `2 − 2 + 2` is two. No face may
/// wrap, so the ball is split down one great circle into halves that each cover
/// half a turn; the two halves of *that* circle are the two edges, and the poles
/// where they meet are the two vertices.
///
/// **Both edges lie on one circle**, which is the same thing two arcs of a
/// bore's rim are, and both are smooth — a sphere either side of them. **Both
/// vertices are singular points of the parameterization**: every angle round
/// names the same pole, which is what makes this worth building rather than
/// assuming.
#[test]
fn a_sphere_built_by_hand_is_a_valid_body() {
    let body = ball();
    body.check();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.characteristic, 2, "{reckoning:?}");
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");
    assert_eq!(body.topology().faces().count(), 2);
    assert_eq!(body.topology().edges().count(), 2);
    assert_eq!(body.topology().vertices().count(), 2);
}

/// **One mark to a corner, and two to a corner the parameters run out at.**
///
/// [`Face::flatten`] writes a pole *twice* — at the two angles its neighbours
/// round the loop stand at — so a caller holding one mark per traced corner has
/// one mark fewer than the walk it has to read them against.
/// [`Face::doubled`] is that same rule over whatever the caller holds, and it
/// is what keeps the two readable together.
///
/// **Read short, the marks slide and the tail of the loop is lost.** A boolean
/// laying a face out marks each corner with the edge that put it there, and one
/// mark per traced corner read against a walk two corners longer drops the last
/// two corners of every loop that reaches a pole. A ball cannot be cut at all.
///
/// Half a ball is bounded by two meridians meeting at both poles, so its one
/// loop reaches two singular places and comes out two corners longer than the
/// walk that made it.
#[test]
fn a_mark_is_written_twice_where_a_corner_is() {
    let body = ball();
    let topology = body.topology();
    let (_, face) = topology.faces().next().expect("half a ball is a face");
    let mut traced = Vec::new();
    let mut marks = Vec::new();
    for (at, &coedge) in topology.outline_of(face).iter().enumerate() {
        topology.walked(coedge).walk(CHORDED, &mut traced);
        marks.resize(traced.len(), at);
    }

    let mut flattened = Vec::new();
    face.flatten(&traced, None, &mut flattened);
    let mut doubled = Vec::new();
    face.doubled(&traced, &marks, &mut doubled);
    assert_eq!(
        flattened.len(),
        traced.len() + 2,
        "a pole at each end of the two meridians",
    );
    assert_eq!(doubled.len(), flattened.len(), "a mark to every corner");
    assert_eq!(
        doubled.last(),
        marks.last(),
        "the corner the loop closes at kept its own mark",
    );

    // The poles, which are the only corners a half turn up or down.
    let poles: Vec<usize> = (0..flattened.len())
        .filter(|&at| flattened[at].y.abs() > FRAC_PI_2 - 1e-9)
        .collect();
    assert_eq!(poles.len(), 4, "two poles, each written twice");
    for &[here, there] in poles.as_chunks::<2>().0 {
        assert_eq!(there, here + 1, "the two writings of one pole are a pair");
        assert_eq!(doubled[here], doubled[there], "one corner carries one mark");
        assert_ne!(
            flattened[here].x, flattened[there].x,
            "a pole is written at the two angles its neighbours stand at",
        );
    }
}

/// A ball of radius [`ROUND`] about the origin, split down the great circle in
/// the plane `z = 0`.
fn ball() -> Body {
    let surface = Surface::Natural(Natural::Sphere(Sphere {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: ROUND,
    }));
    // The seam, in the plane `z = 0`: its own frame runs `+z` so that the
    // parameter climbs from the south pole through `+x` to the north.
    let seam = Circle {
        axis: Axis::new(DVec3::ZERO, DVec3::Z, DVec3::X),
        radius: ROUND,
    };

    let mut body = Body::default();
    let named = Step::default().grew(Grown::Base);
    body.named(named);
    let mut pole = |up: f64| {
        body.topology_mut().add_vertex(Vertex {
            at: DVec3::new(0.0, up, 0.0),
            tolerance: EXACT,
        })
    };
    let (south, north) = (pole(-ROUND), pole(ROUND));

    let mut face = || {
        body.topology_mut().add_face(Face {
            surface,
            outward: true,
            loops: 0..0,
            name: named,
            tolerance: EXACT,
        })
    };
    let (here, there) = (face(), face());

    let mut meridian = |bounds: [f64; 2], from, to| {
        body.topology_mut().add_edge(Edge {
            curve: Curve::Circle(seam),
            bounds,
            from,
            to,
            between: [here, there],
            // One sphere on both sides of it — see §4.4.
            artificial: true,
            tolerance: EXACT,
        })
    };
    let up = meridian([-FRAC_PI_2, FRAC_PI_2], south, north);
    let down = meridian([FRAC_PI_2, 3.0 * FRAC_PI_2], north, south);

    // Counterclockwise in each half's own parameters: up the meridian at the
    // far edge of its turn and down the near one, the poles being whole sides
    // of the region collapsed to points.
    for (face, walk) in [
        (here, [(down, false), (up, false)]),
        (there, [(up, true), (down, true)]),
    ] {
        let at = body.topology_mut().add_loop(|into| {
            into.extend(walk.map(|(edge, forward)| Coedge { edge, forward }));
        });
        body.topology_mut().face_mut(face).loops = at..at + 1;
    }

    body.sealed(&[here, there]);
    body
}

/// How large the ball above is.
const ROUND: f64 = 3.0;

/// How large the ring the two tests below ask about is: [`MAJOR`] out to the
/// tube's own centre and [`MINOR`] thick.
const MAJOR: f64 = 3.0;
const MINOR: f64 = 1.0;

/// **And the cone meshes to the volume the arithmetic says.**
///
/// `πr²h/3`, which for a base of two and a height of two is `8π/3`. It is the
/// apex that makes this worth asserting: the side of the parameter region that
/// collapses there has to be put back before a triangulator can read the face
/// at all — see [`Face::flatten`](crate::solid::topology::face::Face) — and
/// read short of that the solid came back as nothing meshable.
///
/// Chorded, so it reads under and closes on the true figure as the sagitta
/// falls.
#[test]
fn a_cone_meshes_to_the_volume_its_arithmetic_says() {
    let body = cone();
    let want = PI * TALL * TALL * TALL / 3.0;
    let mut mesher = Mesher::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-2, 1e-3, 1e-4] {
        let off = want - mesher.volume(&body, sagitta);
        assert!(off > 0.0, "a chorded cone read over the true {want}");
        assert!(off < last, "{sagitta} read no nearer than the last: {off}");
        last = off;
    }
    assert!(last < 1e-3, "the cone never converged: {last} short");
}

/// **And the sphere meshes to the volume the arithmetic says.**
///
/// `4πr³/3`, which for a radius of [`ROUND`] is `36π`. It is the meridians that
/// make this worth asserting. They arrive chorded at the width of a cell's
/// diagonal, so a run of the face's own boundary reaches over more than one
/// cell of the grid the face is cut in — and read as a thing to cut down rather
/// than as the bound it is, the refining never settled at any sagitta asked
/// for. See [`Refining`](crate::solid::mesh::refining::Refining).
///
/// Chorded, so it reads under and closes on the true figure as the sagitta
/// falls. Read no finer than a hundredth, the mesh growing as the sagitta does
/// and the arithmetic being the same at every step.
#[test]
fn a_sphere_meshes_to_the_volume_its_arithmetic_says() {
    let body = ball();
    let want = 4.0 / 3.0 * PI * ROUND * ROUND * ROUND;
    let mut mesher = Mesher::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-1, 3e-2, 1e-2] {
        let off = want - mesher.volume(&body, sagitta);
        assert!(off > 0.0, "a chorded sphere read over the true {want}");
        assert!(off < last, "{sagitta} read no nearer than the last: {off}");
        last = off;
    }
    assert!(
        last < want / 200.0,
        "the sphere never converged: {last} short"
    );
}

/// **A torus, built by hand and validated as a body** — the first of the fitted
/// tier to bound a solid rather than merely to exist.
///
/// What one is made of is `Body::ring`'s own note. What this asks is that the
/// four quarters close over each other: an Euler characteristic of nought, the
/// genus of one that implies, and every check a body off the builder passes.
/// Worth building rather than assuming because every other surface here is cut
/// once and this one twice — §4.4's rule about wrapping bites both ways on it.
#[test]
fn a_torus_built_by_hand_is_a_valid_body() {
    let body = Body::ring(MAJOR, MINOR);
    body.check();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.characteristic, 0, "{reckoning:?}");
    assert_eq!(reckoning.genus, 1, "{reckoning:?}");
    assert_eq!(body.topology().faces().count(), 4);
    assert_eq!(body.topology().edges().count(), 8);
    assert_eq!(body.topology().vertices().count(), 4);
    assert!(!body.exact(), "a torus stands on no exact surface");
}

/// **And the torus meshes to the volume the arithmetic says.**
///
/// `2π²Rr²` by Pappus — the tube's own disc of `πr²` carried once round a
/// circle of `2πR` — which for a ring of [`MAJOR`] by [`MINOR`] is `6π²`. It is
/// the second wrap that makes this worth asserting: a face read without it
/// comes back as two pieces of parameter space a whole turn apart, and what a
/// mesher makes of that is not a shell.
///
/// Chorded both ways, and the two errors do not cancel: outside the tube a
/// chord cuts material away and inside it a chord adds some, so what is
/// asserted is that the difference closes rather than which side it falls.
#[test]
fn a_torus_meshes_to_the_volume_its_arithmetic_says() {
    let body = Body::ring(MAJOR, MINOR);
    let want = 2.0 * PI * PI * MAJOR * MINOR * MINOR;
    let mut mesher = Mesher::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-2, 1e-3, 1e-4] {
        // What a chord cuts off goes as two thirds of the sagitta times the
        // area it spans, and a torus has `4π²Rr` of it.
        let slack = (2.0 / 3.0) * sagitta * 4.0 * PI * PI * MAJOR * MINOR;
        let off = (mesher.volume(&body, sagitta) - want).abs();
        assert!(off < slack, "{off} off {want} at a sagitta of {sagitta}");
        assert!(off < last, "{sagitta} read no nearer than the last: {off}");
        last = off;
    }
}

/// **A body says whether it is exact, and the answer is a walk over its own
/// surfaces.**
///
/// `.notes/KERNEL.md` §4.1's claim, asked rather than argued: a body made only
/// of planes, cylinders, cones and spheres is exact and can say so, and one
/// with a fitted surface anywhere in it carries whatever bound its fit was made
/// to.
///
/// **A walk and not a flag**, which is what makes it unforgeable. The block off
/// the builder is exact; put one torus on one of its six faces and it is not,
/// and nothing had to be told. The `Natural` / `Fitted` split is what makes
/// that a `match` on the type rather than a guess about the numbers — a fitted
/// surface is fitted because of what it *is*.
#[test]
fn a_body_says_whether_every_surface_it_stands_on_is_exact() {
    let mut body = block();
    assert!(body.exact(), "a block off the builder is not exact");

    let face = body
        .topology()
        .faces()
        .map(|(at, _)| at)
        .next()
        .expect("a block has faces");
    body.topology_mut().face_mut(face).surface = Surface::Fitted(Fitted::Torus(Torus {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        major: 3.0,
        minor: 1.0,
    }));
    assert!(!body.exact(), "one fitted face left the body exact");
}

/// **A body takes an operation's marched runs and hands back its own room**,
/// which is what keeps a rebuild off the allocator.
///
/// The runs are laid down while the boolean cuts and the body is emptied where
/// the sewing begins, so they cannot be filed straight into it — they change
/// hands once, at the end. What each side walks away with is the other's
/// buffer, so the two never ask for more room than the larger of them has
/// needed.
#[test]
fn a_body_trades_its_marched_room_for_the_runs_laid_down() {
    let walked = [DVec3::X, DVec3::Y, DVec3::NEG_X, DVec3::X];
    let mut laid = Carried::default();
    let run = laid.marched.add(&walked, 1e-3);

    let mut body = Body::default();
    body.topology_mut().trade_curves(&mut laid);
    assert_eq!(body.topology().carried().marched.strayed(run).most, 1e-3);
    // And what the operation walked away with is the body's own, empty.
    assert_eq!(laid.marched.add(&walked, 1e-2), 0, "the numbering ran on");
}
