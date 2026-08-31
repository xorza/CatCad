//! Which curved pairs the kernel puts together.
//!
//! **Every pair it answers, in one place.** Which curved pairs the kernel can
//! put together is a fact about it rather than about any one test, and it moves
//! whenever a routine is written or a reduction is found. So the answered ones
//! are written down here, in two tables, and a pair that stops working is a
//! diff in a table rather than a test nobody linked to another. What is refused
//! is in `.notes/ISSUES.md`, and `.notes/KERNEL.md` §9.4 is where the frontier
//! is argued.
//!
//! **An answered row is checked rather than counted.** What a cut leaves and
//! what an intersection keeps are complements, so the two add back to the body
//! they were taken from — which needs no closed form for any pair here, and
//! moves for a face kept that should not be, a face wound backwards or a lump
//! gathered into the wrong shell. Every answer has already been through the
//! validity check on its way out of the sewing.

use super::{ball, block, rod};
use crate::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::{Boolean, Operation};
use crate::solid::build::revolving::{Revolution, Sector};
use crate::solid::geometry::axis::Axis;
use crate::solid::mesh::Mesher;
use crate::solid::named::Step;
use crate::solid::topology::body::Body;
use glam::{DVec2, DVec3};

/// The step the first operand of every row is grown by, and the one the second
/// is.
///
/// Two, and that is the point: a name tells one feature's faces from another's,
/// and every row below is one feature against one other — see
/// [`Named`](crate::solid::named::Named). Two rods built alike and both calling
/// their wall the same thing is exactly the collision this is for.
const FIRST: Step = Step(1);
const SECOND: Step = Step(2);

/// How finely every body here is meshed to read its volume back.
const SAGITTA: f64 = 4e-3;

/// How far the two sides of a cut may fail to add back to the whole, as a
/// fraction of the whole.
///
/// **Not the chording error, which cancels.** Both sides carry the same chords
/// over the faces they share with the body they came from and over the cut
/// between them, so what a chord gives up on one side it takes back on the
/// other. What is left is the two triangulations of one surface disagreeing
/// where they meet its boundary.
///
/// **A fraction, because what is left is a surface effect on a volume.** It
/// grows with the body where an absolute bound would have to be set by the
/// largest row and would then say nothing about the smallest.
///
/// **And it goes as [`SAGITTA`], measured rather than derived.** The widest row
/// reads `5.8e-4` here, and `1.3e-4`, `1.3e-5` and `1.3e-6` at a quarter of
/// this sagitta and each tenth below that — proportional over three decades,
/// which is what says the reading is the chording and not a body that fails to
/// close.
const CLOSES: f64 = 2e-3;

/// A plane facing `normal`, through `origin`.
fn facing(origin: DVec3, normal: DVec3) -> Plane {
    Axis::about(origin, normal.normalize()).plane()
}

/// The four-by-four-by-four block the plane rows are taken against.
fn cube() -> Body {
    block(
        Plane::GROUND,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
        FIRST,
    )
}

/// A rod of `radius` about the axis running `way` from `at`, `deep` long.
fn shaft(at: DVec3, way: DVec3, radius: f64, deep: f64, by: Step) -> Body {
    rod(facing(at, way), DVec2::ZERO, radius, deep, by).body
}

/// A cone `across` at its base and `high`, its apex `high` up `way` from `at`,
/// spun from a right triangle.
///
/// **The sketch plane holds the axis rather than standing square to it**,
/// unlike [`shaft`]: a revolve draws its profile beside the line it turns
/// about, so the sketch's own `+y` has to be the direction the cone points.
fn cone(at: DVec3, way: DVec3, across: f64, high: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (across, 0.0), (0.0, high)]);
    let found = Arrangement::of(&sketch);
    let axis = Axis::about(at, way.normalize());
    let plane = Plane {
        origin: at,
        x: axis.reference,
        y: axis.direction,
    };
    Revolution::new(
        &found,
        &[0],
        plane,
        DVec2::ZERO,
        DVec2::Y,
        Sector::WHOLE,
        by,
    )
    .body()
}

/// Put `one` and `two` together both ways round, and say whether the kernel
/// would.
///
/// Where it would, the two answers are held to adding back to `one` — see
/// [`CLOSES`]. Where it would not, both operations are held to refusing: a pair
/// answered one way and refused the other would be a body whose complement does
/// not exist.
fn answers(boolean: &mut Boolean, mesher: &mut Mesher, one: &Body, two: &Body, what: &str) -> bool {
    let (mut kept, mut took) = (Body::default(), Body::default());
    let cut = boolean.combine(one, two, Operation::Cut, &mut kept);
    let met = boolean.combine(one, two, Operation::Intersect, &mut took);
    assert_eq!(
        cut, met,
        "{what}: cut and intersect disagree about refusing"
    );
    if !cut {
        return false;
    }
    let whole = mesher.volume(one, SAGITTA);
    let apart = mesher.volume(&kept, SAGITTA) + mesher.volume(&took, SAGITTA);
    assert!(
        (whole - apart).abs() < CLOSES * whole,
        "{what}: the two sides come to {apart} where the whole is {whole}",
    );
    true
}

/// **A plane against each of the other four**, which is the row of §7.3's table
/// with no gap left in it.
#[test]
fn a_plane_against_each_curved_surface_adds_back_to_the_whole() {
    let up = DVec3::Y;
    let cases: [(&str, Body, Body); 4] = [
        (
            "a rod bored through a block",
            cube(),
            shaft(DVec3::new(2.0, -1.0, 2.0), up, 1.0, 6.0, SECOND),
        ),
        (
            "a cone stood in a block",
            cube(),
            cone(DVec3::new(2.0, 1.0, 2.0), up, 1.5, 4.0, SECOND),
        ),
        (
            "a ball sunk in a block",
            cube(),
            ball(DVec3::new(2.0, 4.0, 2.0), 1.5, SECOND),
        ),
        (
            "a plane through a cone's apex",
            cone(DVec3::ZERO, up, 2.0, 4.0, FIRST),
            block(
                facing(DVec3::new(0.0, -1.0, 0.0), DVec3::X),
                &[(-9.0, -9.0), (9.0, -9.0), (9.0, 9.0), (-9.0, 9.0)],
                9.0,
                SECOND,
            ),
        ),
    ];
    held(cases);
}

/// **Every pair of curved surfaces the kernel puts together, held to its own
/// complement.**
///
/// What is not here is refused, and for one of two reasons: the pencil search
/// finds no ruled member at all, or it finds one and something after it turns
/// the pair away. Both are in `.notes/ISSUES.md`.
#[test]
fn every_curved_pair_the_kernel_answers_adds_back_to_the_whole() {
    let up = DVec3::Y;
    let cases: [(&str, Body, Body); 8] = [
        (
            // Stopping short of the apex, which a hole taken right up to one
            // would end in a knife edge at.
            "a rod bored up a cone's axis",
            cone(DVec3::ZERO, up, 2.0, 4.0, FIRST),
            shaft(DVec3::new(0.0, -1.0, 0.0), up, 0.5, 3.0, SECOND),
        ),
        // A rod against a rod, which is three quartics and the one placement
        // where two of them never meet at all.
        (
            "two rods crossing square, nested",
            shaft(DVec3::new(-4.0, 0.0, 0.0), DVec3::X, 2.0, 8.0, FIRST),
            shaft(DVec3::new(0.0, -4.0, 0.0), up, 1.0, 8.0, SECOND),
        ),
        (
            // Unequal radii on axes that meet at a lean, whose meeting is the
            // quartic that turned the member search away from a chart it could
            // not walk — see `Filed::resolves`.
            "two rods meeting at a lean",
            shaft(DVec3::new(-4.0, 0.0, 0.0), DVec3::X, 2.0, 8.0, FIRST),
            shaft(
                DVec3::new(-2.0, -3.46, 0.0),
                DVec3::new(0.5, 0.866, 0.0),
                1.0,
                8.0,
                SECOND,
            ),
        ),
        (
            // Cross-sections that overlap rather than nest, whose meeting is
            // one loop doubling back where a nested pair's is two — and no
            // graph over either cylinder's angle holds it.
            "two rods crossing square, overlapping",
            shaft(DVec3::new(-4.0, 0.0, 0.0), DVec3::X, 2.0, 8.0, FIRST),
            shaft(DVec3::new(0.0, -4.0, 1.5), up, 1.0, 8.0, SECOND),
        ),
        (
            "a rod bored coaxially through a rod",
            shaft(DVec3::new(0.0, -4.0, 0.0), up, 2.0, 8.0, FIRST),
            shaft(DVec3::new(0.0, -5.0, 0.0), up, 1.0, 10.0, SECOND),
        ),
        (
            "a ball on a cone's axis",
            cone(DVec3::ZERO, up, 2.0, 4.0, FIRST),
            ball(DVec3::new(0.0, 2.0, 0.0), 1.5, SECOND),
        ),
        (
            "a ball off a cone's axis",
            cone(DVec3::ZERO, up, 2.0, 4.0, FIRST),
            ball(DVec3::new(1.0, 2.0, 0.0), 1.5, SECOND),
        ),
        (
            "a ball on a rod's axis",
            shaft(DVec3::new(0.0, -4.0, 0.0), up, 2.0, 8.0, FIRST),
            ball(DVec3::ZERO, 3.0, SECOND),
        ),
    ];
    held(cases);
}

/// Put every row of `cases` together both ways round, and hold each to its own
/// complement.
fn held<const ROWS: usize>(cases: [(&str, Body, Body); ROWS]) {
    let (mut boolean, mut mesher) = (Boolean::default(), Mesher::default());
    for (what, one, two) in cases {
        assert!(
            !one.is_empty() && !two.is_empty(),
            "{what}: an operand is empty",
        );
        assert!(
            answers(&mut boolean, &mut mesher, &one, &two, what),
            "{what} was refused",
        );
    }
}
