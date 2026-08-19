use crate::math::plane::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::extrusion::Extrusion;
use crate::solid::named::Step;
use crate::solid::topology::body::Body;

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
