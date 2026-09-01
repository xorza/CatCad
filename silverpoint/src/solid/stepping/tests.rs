//! What a body comes out as, held against the format's own rules.
//!
//! **Counted and cross-checked rather than compared to a golden file.** What
//! matters about an exchange file is that every reference resolves and that the
//! geometry is the body's own — a byte comparison would fail on a comment and
//! pass on a cylinder written as a plane.

use super::*;
use crate::math::plane::Plane;
use crate::number::tolerance::CHORDED;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::Boolean;
use crate::solid::boolean::operation::Operation;
use crate::solid::build::builder::Extrusion;
use crate::solid::named::Step;
use glam::{DVec2, DVec3};
use std::collections::HashSet;

const SOLID: Step = Step(1);

/// A block of side `wide`, carried `deep` off the ground.
fn block(wide: f64, deep: f64) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (wide, 0.0), (wide, wide), (0.0, wide)]);
    Extrusion::new(&Arrangement::of(&sketch), &[0], Plane::GROUND, deep, SOLID).body()
}

/// How many times `what` appears as an entity kind.
fn count(of: &str, what: &str) -> usize {
    of.lines()
        .filter(|line| line.contains(&format!("= {what}(")) || line.contains(&format!("({what}(")))
        .count()
}

/// **A block writes out as the six planes it is**, and every entity the file
/// names is one the file defines.
///
/// Twelve edges, each walked by two faces, so twenty-four oriented edges over
/// twelve `EDGE_CURVE`s — an edge written once and pointed at twice is the
/// whole of what an exchange file's numbering is for. Eight corners, six loops,
/// one shell and one solid.
#[test]
fn a_block_writes_the_six_planes_it_stands_on() {
    let mut into = String::new();
    Stepping::default().write(&block(2.0, 3.0), "block", CHORDED, &mut into);

    assert!(into.starts_with("ISO-10303-21;\n"), "{into}");
    assert!(into.ends_with("ENDSEC;\nEND-ISO-10303-21;\n"), "{into}");
    assert_eq!(count(&into, "ADVANCED_FACE"), 6);
    assert_eq!(count(&into, "PLANE"), 6);
    assert_eq!(count(&into, "EDGE_CURVE"), 12);
    assert_eq!(count(&into, "LINE"), 12, "one line per edge, written once");
    assert_eq!(
        count(&into, "ORIENTED_EDGE"),
        24,
        "every edge is walked twice"
    );
    assert_eq!(count(&into, "VERTEX_POINT"), 8);
    assert_eq!(count(&into, "EDGE_LOOP"), 6);
    assert_eq!(count(&into, "FACE_OUTER_BOUND"), 6);
    assert_eq!(count(&into, "FACE_BOUND"), 0, "a block punches no holes");
    assert_eq!(count(&into, "CLOSED_SHELL"), 1);
    assert_eq!(count(&into, "MANIFOLD_SOLID_BREP"), 1);
    assert_eq!(
        count(&into, "BREP_WITH_VOIDS"),
        0,
        "a block shuts nothing in"
    );
    assert_eq!(count(&into, "SHAPE_DEFINITION_REPRESENTATION"), 1);

    // Every `#n` the file names is one it defines, and every number is handed
    // out once. A file that referred forward to nothing would open empty.
    let mut defined = HashSet::new();
    for line in into.lines() {
        let Some(rest) = line.strip_prefix('#') else {
            continue;
        };
        let Some((number, _)) = rest.split_once(' ') else {
            continue;
        };
        assert!(
            defined.insert(number.parse::<u32>().expect("an entity number")),
            "entity #{number} was handed out twice",
        );
    }
    for line in into.lines() {
        let body = line.split_once(" = ").map_or("", |(_, body)| body);
        for at in body.split('#').skip(1) {
            let number: u32 = at
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .expect("a reference is a number");
            assert!(
                defined.contains(&number),
                "#{number} is named and not defined"
            );
        }
    }
}

/// **A quote in a name is doubled**, which is Part 21's own escape and the one
/// thing a document called after a person can trip.
#[test]
fn a_name_carrying_a_quote_is_written_the_way_the_format_reads_one() {
    let mut into = String::new();
    Stepping::default().write(&block(1.0, 1.0), "Bob's bracket", CHORDED, &mut into);
    assert!(
        into.contains("FILE_NAME('Bob''s bracket',"),
        "the quote was not doubled: {}",
        into.lines().nth(3).unwrap_or_default(),
    );
}

/// **Every real carries a decimal point**, which is the format's own rule and
/// the one thing the machine's shortest spelling of a small number drops.
#[test]
fn every_real_is_written_the_way_the_format_reads_one() {
    let mut held = String::new();
    for (of, want) in [
        (1.0, "1.0"),
        (-0.5, "-0.5"),
        (0.0, "0.0"),
        (1e-7, "1.0E-7"),
        (1.5e-7, "1.5E-7"),
        (2e20, "2.0E20"),
    ] {
        held.clear();
        real(&mut held, of);
        assert_eq!(held, want, "{of} was written {held}");
    }
}

/// **A walked curve goes out as the polyline it is**, which is the one fit this
/// export will make: a marched curve *is* a run of chords laid to a sagitta the
/// body already declares, so writing it through those very places claims
/// nothing the body did not.
///
/// The body is a rod with a flat milled off its own axis and its base rim
/// filleted: the blend's torus meets that flat in a quartic no exact route
/// parameterizes, so each end of the run closes on an arc that was walked.
///
/// **And the file says how far it strays.** What a reader is told to weld by is
/// the body's own bound rather than the machine's floor, which is what makes
/// the polyline honest rather than merely small.
///
/// **A run is written once**, however many edges lie on it: a torus is four
/// faces, so a curve across it is cut into several — and a polyline of hundreds
/// of places repeated per edge would be a file many times the size for nothing.
#[test]
fn a_walked_curve_goes_out_as_the_polyline_it_is() {
    let halved = halved();
    assert!(halved.strays() > 0.0, "the cut left nothing walked");

    let mut into = String::new();
    Stepping::default().write(&halved, "ring", CHORDED, &mut into);
    // Two ovals, and a run apiece. The torus is four faces (§4.4), so each oval
    // is several edges — and a run of hundreds of places is written once and
    // named by every edge on it, exactly as a vertex is.
    let splines = count(&into, "B_SPLINE_CURVE_WITH_KNOTS");
    assert_eq!(splines, 2, "one per curve the plane cut, not one per edge");
    assert!(
        count(&into, "EDGE_CURVE") > splines,
        "more curves than edges, so nothing was written twice to no purpose",
    );
    assert!(
        into.contains("B_SPLINE_CURVE_WITH_KNOTS('',1,("),
        "the polyline was written at some other degree",
    );
    assert!(
        into.contains(".POLYLINE_FORM."),
        "the curve claims a form it was not fitted to",
    );

    // The file's own accuracy is what the body says it strays, not the floor a
    // body of written-down curves would carry.
    let mut floor = String::new();
    real(&mut floor, halved.strays());
    assert!(
        into.contains(&format!("LENGTH_MEASURE({floor})")),
        "the file claims an accuracy the body did not: {}",
        into.lines()
            .find(|line| line.contains("UNCERTAINTY"))
            .unwrap_or_default(),
    );
}

/// **A curve the format cannot say goes out chorded, and the file says so.**
///
/// The two are the quartic a general pair of quadrics meets in and the saddle a
/// cross drilling leaves. Both are written down *exactly* here, so a chording
/// costs an error the body did not carry — and the whole of what makes it
/// honest is that the file's own accuracy declares it.
///
/// A bar drilled across by a narrower hole, which leaves the saddle.
#[test]
fn a_curve_the_format_cannot_say_goes_out_chorded_at_a_declared_slack() {
    let cut = drilled();
    assert!(cut.exact(), "a bar and a bore are quadrics");
    assert_eq!(cut.strays(), 0.0, "an exact body is walked nowhere");

    let mut into = String::new();
    Stepping::default().write(&cut, "bar", CHORDED, &mut into);
    // A cross drilling leaves two saddles, and the bore's wall is split — §4.4
    // — so each is cut into two edges. One polyline apiece all the same.
    let saddles = cut
        .topology()
        .edges()
        .filter(|(_, edge)| matches!(edge.curve, Curve::Saddle(_)))
        .count();
    assert_eq!(saddles, 4, "the cross drilling left something else");
    assert_eq!(
        count(&into, "B_SPLINE_CURVE_WITH_KNOTS"),
        2,
        "one per curve the drilling left, not one per edge on it",
    );

    // The body strays nought and the file does not: what a chording cost is
    // what a reader is told to weld by.
    let mut slack = String::new();
    real(&mut slack, CHORDED);
    assert!(
        into.contains(&format!("LENGTH_MEASURE({slack})")),
        "the file claims an accuracy the chording did not cost: {}",
        into.lines()
            .find(|line| line.contains("UNCERTAINTY"))
            .unwrap_or_default(),
    );
}

/// **A body of nothing but analytic curves claims no slack it never spent.**
#[test]
fn a_body_of_written_down_curves_declares_only_the_floor() {
    let mut into = String::new();
    Stepping::default().write(&block(2.0, 3.0), "block", CHORDED, &mut into);
    let mut floor = String::new();
    real(&mut floor, WELD);
    assert!(
        into.contains(&format!("LENGTH_MEASURE({floor})")),
        "a block of six planes claims more than the machine's own floor: {}",
        into.lines()
            .find(|line| line.contains("UNCERTAINTY"))
            .unwrap_or_default(),
    );
}

/// A bar drilled across by a narrower hole, which is what leaves a saddle.
fn drilled() -> Body {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 1.0);
    let bar = Extrusion::new(&Arrangement::of(&sketch), &[0], Plane::GROUND, 4.0, SOLID).body();
    let mut across = Sketch::default();
    let at = across.add_point(DVec2::ZERO);
    across.add_circle(at, 0.5);
    let bore = Extrusion::new(
        &Arrangement::of(&across),
        &[0],
        Plane {
            origin: DVec3::new(0.0, 2.0, 2.0),
            ..Plane::FRONT
        },
        -4.0,
        Step(2),
    )
    .body();
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&bar, &bore, Operation::Cut, &mut cut),
        "cross drilling a bar was refused",
    );
    assert!(
        cut.topology()
            .edges()
            .any(|(_, edge)| matches!(edge.curve, Curve::Saddle(_))),
        "the cross drilling left no saddle to chord",
    );
    cut
}

/// Half a ring, cut by a plane that leans.
///
/// **The cheapest body with a walked curve in it.** A torus against a plane is
/// a pair with a fitted half, so what they meet in is marched rather than
/// written down — and a plane that leans is the one that neither the axis nor a
/// square crossing answers exactly.
fn halved() -> Body {
    let ring = Body::ring(3.0, 1.0);
    let mut sketch = Sketch::default();
    sketch.outline(&[(-10.0, -10.0), (10.0, -10.0), (10.0, 10.0), (-10.0, 10.0)]);
    let leaning = Extrusion::new(
        &Arrangement::of(&sketch),
        &[0],
        Plane {
            origin: DVec3::ZERO,
            x: DVec3::new(1.0, -1.0, 0.0).normalize(),
            y: DVec3::NEG_Z,
        },
        20.0,
        Step(2),
    )
    .body();
    let mut into = Body::default();
    assert!(
        Boolean::default().combine(&ring, &leaning, Operation::Intersect, &mut into),
        "a ring halved by a leaning plane was refused",
    );
    into
}

/// **A torus goes out as a torus**, which is the whole of why the fitted tier
/// is no bar to this format: STEP carries one natively, so a rim's fillet
/// leaves as the surface it is rather than as a spline fitted to it.
///
/// A rod with its whole base rim filleted. The rim closes, so nothing about it
/// was walked and every curve is written down.
#[test]
fn a_filleted_rim_goes_out_as_the_torus_it_is() {
    use crate::solid::rounding::{Bevel, Round, Rounding};

    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 1.0);
    let rod = Extrusion::new(&Arrangement::of(&sketch), &[0], Plane::GROUND, 4.0, SOLID).body();
    let names: Vec<_> = rod.names().collect();
    let along = [[names[0], names[2]]];
    let mut rounded = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 0.25, Bevel::Round, Step(2)),
            &rod,
            &mut rounded
        ),
        "a fillet down the rim of a rod was refused",
    );

    let mut into = String::new();
    Stepping::default().write(&rounded, "rod", CHORDED, &mut into);
    assert_eq!(
        count(&into, "TOROIDAL_SURFACE"),
        2,
        "one per piece of the run"
    );
    assert_eq!(
        count(&into, "CYLINDRICAL_SURFACE"),
        2,
        "the rod's own two walls"
    );
    assert_eq!(
        count(&into, "B_SPLINE_CURVE_WITH_KNOTS"),
        0,
        "a rim that closes has nothing walked about it",
    );
}

/// **A cavity is a second shell, turned over**, which is what `BREP_WITH_VOIDS`
/// says: the same closed surface read with its material on the other side.
#[test]
fn a_body_with_a_cavity_writes_the_shell_it_shuts_in() {
    use crate::solid::boolean::Boolean;
    use crate::solid::boolean::operation::Operation;

    let mut inside = Sketch::default();
    inside.outline(&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)]);
    let swallowed = Extrusion::new(
        &Arrangement::of(&inside),
        &[0],
        Plane {
            origin: DVec3::new(0.0, 1.0, 0.0),
            ..Plane::GROUND
        },
        2.0,
        Step(2),
    )
    .body();
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&block(4.0, 4.0), &swallowed, Operation::Cut, &mut cut),
        "swallowing a block was refused",
    );

    let mut into = String::new();
    Stepping::default().write(&cut, "hollow", CHORDED, &mut into);
    assert_eq!(count(&into, "BREP_WITH_VOIDS"), 1);
    assert_eq!(count(&into, "ORIENTED_CLOSED_SHELL"), 1);
    assert_eq!(
        count(&into, "CLOSED_SHELL"),
        2,
        "the outside and the cavity"
    );
    assert_eq!(
        count(&into, "MANIFOLD_SOLID_BREP"),
        0,
        "a hollow is not a plain solid"
    );
}
