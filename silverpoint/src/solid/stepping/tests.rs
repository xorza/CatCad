//! What a body comes out as, held against the format's own rules.
//!
//! **Counted and cross-checked rather than compared to a golden file.** What
//! matters about an exchange file is that every reference resolves and that the
//! geometry is the body's own — a byte comparison would fail on a comment and
//! pass on a cylinder written as a plane.

use super::*;
use crate::math::plane::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
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
    assert!(
        Stepping::default().write(&block(2.0, 3.0), "block", &mut into),
        "a block of six planes was refused",
    );

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
    assert!(
        Stepping::default().write(&block(1.0, 1.0), "Bob's bracket", &mut into),
        "a block was refused",
    );
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

/// **A curve the kernel walked has no entity to be**, so the body carrying one
/// is refused whole rather than written with a spline fitted where the walk
/// was — see `.notes/KERNEL.md` §1, which is the promise that would break.
///
/// The body is a rod with a flat milled off its own axis and its base rim
/// filleted: the blend's torus meets that flat in a quartic no exact route
/// parameterizes, so each end of the run closes on an arc that was walked.
#[test]
fn a_body_carrying_a_walked_curve_is_refused() {
    use crate::solid::boolean::Boolean;
    use crate::solid::boolean::operation::Operation;
    use crate::solid::rounding::{Bevel, Round, Rounding};

    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 2.0);
    let rod = Extrusion::new(&Arrangement::of(&sketch), &[0], Plane::GROUND, 3.0, SOLID).body();
    let mut tool = Sketch::default();
    tool.outline(&[(1.0, -4.0), (4.0, -4.0), (4.0, 4.0), (1.0, 4.0)]);
    let flat = Extrusion::new(
        &Arrangement::of(&tool),
        &[0],
        Plane {
            origin: DVec3::new(0.0, -1.0, 0.0),
            ..Plane::GROUND
        },
        5.0,
        Step(2),
    )
    .body();
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&rod, &flat, Operation::Cut, &mut cut),
        "milling a flat down a rod was refused",
    );

    let named = |wanted: fn(&Surface) -> bool| {
        cut.topology()
            .faces()
            .find(|(_, face)| wanted(&face.surface))
            .map(|(_, face)| face.name)
            .expect("the flatted rod has the face asked for")
    };
    let along = [[
        named(|surface| {
            matches!(surface, Surface::Natural(Natural::Plane(plane))
                if plane.origin.abs_diff_eq(DVec3::ZERO, 1e-9))
        }),
        named(|surface| matches!(surface, Surface::Natural(Natural::Cylinder(_)))),
    ]];
    let mut rounded = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 0.25, Bevel::Round, Step(3)),
            &cut,
            &mut rounded
        ),
        "a fillet down the broken rim was refused",
    );
    assert!(rounded.strays() > 0.0, "the blend closed on nothing walked");

    let mut into = String::new();
    assert!(
        !Stepping::default().write(&rounded, "rod", &mut into),
        "a body of walked curves was written out",
    );
    assert!(into.is_empty(), "a refusal left half a file behind");
}

/// **A torus goes out as a torus**, which is the whole of why the fitted tier
/// is no bar to this format: STEP carries one natively, so a rim's fillet
/// leaves as the surface it is rather than as a spline fitted to it.
///
/// A rod with its whole base rim filleted. The rim closes, so nothing about it
/// was walked and every curve is written down: three tube faces, and the arcs
/// they are cut apart at.
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
    assert!(
        Stepping::default().write(&rounded, "rod", &mut into),
        "a body standing on a torus was refused",
    );
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
    assert!(
        !into.contains("B_SPLINE"),
        "something was fitted where an analytic entity would do",
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
    assert!(
        Stepping::default().write(&cut, "hollow", &mut into),
        "a body with a cavity was refused",
    );
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
