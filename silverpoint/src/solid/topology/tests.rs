use crate::math::plane::Plane;
use crate::number::tolerance::EXACT;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::extrusion::Extrusion;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::mesh::Mesher;
use crate::solid::named::Step;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::Edge;
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::lump::Lump;
use crate::solid::topology::shell::Shell;
use crate::solid::topology::vertex::Vertex;
use glam::DVec3;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// A two-by-two block three deep — the simplest thing the checker has an
/// opinion about, and the fixture every mutation below breaks.
fn block() -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, 0, Plane::GROUND, 3.0, Step::default()).body()
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

/// Move one corner of `body` off its curves by `much` of what it stands for.
fn nudged(body: &mut Body, much: f64) {
    let corner = body
        .topology()
        .edges()
        .next()
        .expect("a block has edges")
        .1
        .from;
    let stood = body.topology().vertex(corner).tolerance;
    body.topology_mut().vertex_mut(corner).at.x += stood * much;
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
    let corner = body
        .topology()
        .edges()
        .next()
        .expect("a block has edges")
        .1
        .from;
    body.topology_mut().vertex_mut(corner).tolerance = 0.0;
    body.check();
}

/// **A cone, built by hand and validated as a body** — which nothing in the
/// kernel constructs, so until now the surface was tested and the *solid* was
/// not. One of the two `.notes/KERNEL.md` M0 owes.
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
    let surface = Surface::Cone(Cone {
        axis: upright,
        half_angle: FRAC_PI_4,
    });
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
    let base = face(Surface::Plane(lid), true);

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

    sealed(&mut body, &[here, there, base]);
    body
}

/// Gather `faces` into one shell and that shell into one lump, which is how a
/// closed body ends however it was built.
fn sealed(body: &mut Body, faces: &[FaceId]) {
    let from = body.topology().faces_shelled();
    for &face in faces {
        body.topology_mut().add_shelled(face);
    }
    let to = body.topology().faces_shelled();
    let shell = body.topology_mut().add_shell(Shell { faces: from..to });
    body.topology_mut().add_lump(Lump {
        outer: shell,
        voids: Vec::new(),
    });
}

/// How tall the cone stands, which is also its base radius at forty-five
/// degrees.
const TALL: f64 = 2.0;

/// Where `angle` lands on `rim`.
fn body_place(rim: &Circle, angle: f64) -> DVec3 {
    rim.at(angle)
}

/// **A sphere, built by hand and validated as a body** — the other of the two
/// `.notes/KERNEL.md` M0 owes, and the harder shape.
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

/// A ball of radius [`ROUND`] about the origin, split down the great circle in
/// the plane `z = 0`.
fn ball() -> Body {
    let surface = Surface::Sphere(Sphere {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: ROUND,
    });
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

    sealed(&mut body, &[here, there]);
    body
}

/// How large the ball above is.
const ROUND: f64 = 3.0;

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
