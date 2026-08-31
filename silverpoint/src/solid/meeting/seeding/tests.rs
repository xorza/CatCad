use super::*;
use crate::loops::Loops;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::meeting::marching::Marching;

/// The ring every surface below is held against: three out to the tube's
/// own centre, one thick, about the world's `+Y` through the origin.
fn ring() -> Torus {
    Torus {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        major: 3.0,
        minor: 1.0,
    }
}

/// The plane through `origin` facing `normal`, framed however.
fn facing(origin: DVec3, normal: DVec3) -> Surface {
    Surface::Natural(Natural::Plane(
        Axis::about(origin, normal.normalize()).plane(),
    ))
}

/// A rod of `radius` running the ring's own way, `off` from its axis.
fn beside(off: f64, radius: f64) -> Surface {
    Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::new(DVec3::X * off, DVec3::Y, DVec3::X),
        radius,
    }))
}

/// A drill of `radius` through `off`, running `way` — which leans on the
/// ring's own axis rather than standing parallel to it.
fn leaning(off: DVec3, way: DVec3, radius: f64) -> Surface {
    Surface::Natural(Natural::Cylinder(Cylinder {
        axis: Axis::about(off, way.normalize()),
        radius,
    }))
}

/// Assert that a fine sweep of the curve finds nothing the seeds missed.
///
/// **What says every piece was found.** The places of the curve at a fine
/// sweep of `v` are had without asking where the *pieces* are, which is the
/// one thing [`Reading::ends`] decides and the one thing this does not use.
/// So every place the sweep turns up has to stand on some loop that was
/// walked, and a piece nobody was seeded on shows up as a place far from
/// all of them.
///
/// Every seed is held against both surfaces on the way, to the last bits
/// the machine keeps: a seed here is a place of the torus worked out
/// outright rather than one walked onto.
fn every_piece_is_reached(surface: &Surface, what: &str) {
    let torus = ring();
    let round = Surface::Fitted(Fitted::Torus(torus));
    let reading = Reading::of(surface, &torus).expect("the pair has a reading");
    let mut found = Vec::new();
    assert!(
        seeded(surface, &torus, &mut found),
        "{what}: the pair has a reading"
    );
    assert!(!found.is_empty(), "{what}: nothing was seeded");
    let mut marching = Marching::default();
    let mut walked = Vec::new();
    for &seed in &found {
        for (named, on) in [("the ring", &round), ("the other", surface)] {
            let off = on.off(seed);
            assert!(off < 1e-12, "{what}: {seed:?} stands {off} off {named}");
        }
        marching
            .walk(&round, surface, seed, 1e-4)
            .unwrap_or_else(|| panic!("{what}: a seeded walk did not close"));
        walked.extend_from_slice(marching.walked());
    }
    for step in 0..512 {
        let v = TAU * step as f64 / 512.0;
        for turn in reading.turns(v) {
            let at = torus.at(DVec2::new(turn, v));
            let near = walked
                .iter()
                .map(|on| on.distance(at))
                .fold(f64::INFINITY, f64::min);
            // A chord of the walk above is `√(8·ρ·sagitta)` long, which
            // for a curve of this ring's own size is a few hundredths — so
            // half of one is well under this, and a piece nobody walked is
            // a whole loop away rather than a chord.
            assert!(near < 0.05, "{what}: {at:?} stands {near} off every loop");
        }
    }
}

/// **Every piece of the curve a leaning plane cuts is seeded**, and a plane
/// that both leans and stands off the middle is where that stops being
/// free.
///
/// Four planes, and the last is the one that needs the ends solved as
/// written: it leans, it stands off the middle, *and* it cuts two stretches
/// rather than one. Through the middle the equation is symmetric enough
/// that a sign the wrong way round answers with the same set, and with one
/// stretch a wrong end still leaves a midpoint on the curve to seed from.
/// Only two stretches off the middle tells the two apart.
#[test]
fn every_piece_of_a_leaning_plane_is_seeded() {
    for (what, origin, lean) in [
        ("steeply", DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0)),
        ("gently", DVec3::ZERO, DVec3::new(0.2, 1.0, 0.0)),
        (
            "gently and raised",
            DVec3::Y * 0.5,
            DVec3::new(0.2, 1.0, 0.0),
        ),
        (
            "nearly square and raised",
            DVec3::Y * 0.2,
            DVec3::new(0.15, 1.0, 0.0),
        ),
    ] {
        every_piece_is_reached(&facing(origin, lean), what);
    }
}

/// **And every piece a rod bored the ring's own way cuts**, which is the
/// bolt hole through a flange.
///
/// A rod parallel to the axis meets the ring where
/// `2·out·off·cos(u − phase) = out² + off² − across²`, and the ends of that
/// turn on how far out the tube reaches and on nothing else. Three rods:
/// one straight through the tube's own middle, one biting the ring's outer
/// edge, and a wide one that swallows the axis and cuts the ring from
/// inside.
#[test]
fn every_piece_of_a_rod_bored_the_rings_own_way_is_seeded() {
    for (what, off, radius) in [
        ("through the tube", 3.0, 0.3),
        ("biting the outer edge", 3.8, 0.5),
        ("swallowing the axis", 0.5, 2.6),
    ] {
        every_piece_is_reached(&beside(off, radius), what);
    }
}

/// **And every piece a drill that leans cuts**, which is the case the
/// second harmonic is written for — see [`Leaning`].
///
/// Five drills. The first leans in the plane its axis shares with the
/// ring's, which is the symmetric case; the second leans out of it, so that
/// all five harmonics carry something. The third runs straight across the
/// ring through the hole, cutting four pieces and crossing the angle the
/// half-angle chart cannot name; the fourth does the same raised off the
/// ring's own plane. The last leans steeply enough to enter one wall and
/// leave through the other, which is one piece where the rest are two or
/// four.
#[test]
fn every_piece_of_a_drill_that_leans_is_seeded() {
    for (what, off, way, radius) in [
        (
            "leaning in the plane of the axes",
            DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(0.3, 1.0, 0.0),
            0.3,
        ),
        (
            "leaning out of it",
            DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(0.3, 1.0, 0.4),
            0.3,
        ),
        ("straight across the ring", DVec3::ZERO, DVec3::X, 0.3),
        (
            "across and raised",
            DVec3::new(0.0, 0.4, 0.0),
            DVec3::X,
            0.3,
        ),
        (
            "leaning steeply through both walls",
            DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            0.4,
        ),
    ] {
        every_piece_is_reached(&leaning(off, way, radius), what);
    }
}

/// **And the small loop a search finds by luck is found by arithmetic.**
///
/// A plane parallel to the axis, a twentieth inside the ring's outer
/// equator, cuts one small closed piece near it. A 512×512 subdivision
/// misses one of these once it stands half a cell off a node — see
/// `.notes/KERNEL.md` §7.3. The ends of its stretch are two `acos` here,
/// and where it is comes out with them.
///
/// **Held to the ellipse it closes on.** For a plane `d` inside the outer
/// equator the piece is an ellipse of semi-axes `√(2·minor·d)` along the
/// axis and `√(2(major + minor)d)` across it, in the limit of small `d` —
/// which for a twentieth of a ring of three by one is `0.316` by `0.632`.
#[test]
fn a_small_loop_just_inside_the_equator_is_seeded_by_arithmetic() {
    let torus = ring();
    let inside = 0.05;
    let out = torus.major + torus.minor - inside;
    let grazing = facing(DVec3::X * out, DVec3::X);
    let mut found = Vec::new();
    assert!(
        seeded(&grazing, &torus, &mut found),
        "a plane inside the equator has a reading"
    );
    assert_eq!(found.len(), 1, "{found:?}");

    let round = Surface::Fitted(Fitted::Torus(torus));
    let mut marching = Marching::default();
    marching
        .walk(&round, &grazing, found[0], 1e-6)
        .expect("the small walk did not close");
    let reach = |of: fn(DVec3) -> f64| {
        marching
            .walked()
            .iter()
            .map(|&at| of(at))
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let (up, across) = (reach(|at| at.y), reach(|at| at.z));
    let (tall, wide) = (
        (2.0 * torus.minor * inside).sqrt(),
        (2.0 * (torus.major + torus.minor) * inside).sqrt(),
    );
    assert!((up - tall).abs() < tall / 50.0, "{up} rather than {tall}");
    assert!(
        (across - wide).abs() < wide / 50.0,
        "{across} rather than {wide}"
    );
}

/// **A rod through the tube's middle cuts two pieces**, and the angles they
/// end at are read off the arithmetic rather than off the walk.
///
/// With the rod's axis three out — the tube's own centre circle — the tube
/// reaches it between `out = 3 ± 0.3`, which is `cos v = ±0.3`. Two
/// stretches, each a closed piece: the hole the rod makes going in and the
/// one it makes coming out.
#[test]
fn a_rod_through_the_tube_ends_where_the_arithmetic_says() {
    let torus = ring();
    let rod = beside(3.0, 0.3);
    let reading = Reading::of(&rod, &torus).expect("a parallel rod reads");
    let mut want = [
        0.3f64.acos(),
        TAU - 0.3f64.acos(),
        (-0.3f64).acos(),
        TAU - (-0.3f64).acos(),
    ];
    want.sort_by(f64::total_cmp);
    let ends = reading.ends();
    assert_eq!(ends.all().len(), 4, "{:?}", ends.all());
    for (got, want) in ends.all().iter().zip(want) {
        assert!((got - want).abs() < 1e-12, "{:?} misses {want}", ends.all());
    }
    let mut found = Vec::new();
    assert!(
        seeded(&rod, &torus, &mut found),
        "a parallel rod has a reading"
    );
    assert_eq!(found.len(), 2, "one piece each way");
}

/// **A drill straight across the ring cuts four pieces**, and the seeding
/// offers a place on each of them.
///
/// The ring's own hole is two out and its outer equator four, so a drill
/// along the world's `+X` through the origin passes through the tube twice
/// on each side. Four loops, and no two of them within a tenth of each
/// other — so a seed on each is a seed that walks a loop none of the others
/// walked.
#[test]
fn a_drill_straight_across_the_ring_cuts_four_pieces() {
    let torus = ring();
    let round = Surface::Fitted(Fitted::Torus(torus));
    let drill = leaning(DVec3::ZERO, DVec3::X, 0.3);
    let mut found = Vec::new();
    assert!(
        seeded(&drill, &torus, &mut found),
        "a leaning drill has a reading"
    );

    let mut marching = Marching::default();
    let mut loops = Loops::<DVec3>::default();
    for &seed in &found {
        if loops
            .iter()
            .any(|had| had.iter().any(|&at| at.distance(seed) < 0.05))
        {
            continue;
        }
        marching
            .walk(&round, &drill, seed, 1e-4)
            .expect("a seeded walk did not close");
        loops.push(marching.walked());
    }
    assert_eq!(loops.len(), 4, "{} loops", loops.len());
    // Each loop rings one of the four crossings of the tube: two out and
    // four out, on either side of the axis.
    let mut middles: Vec<f64> = loops
        .iter()
        .map(|had| had.iter().map(|at| at.x).sum::<f64>() / had.len() as f64)
        .collect();
    middles.sort_by(f64::total_cmp);
    for (got, want) in middles.iter().zip([-4.0, -2.0, 2.0, 4.0]) {
        assert!(
            (got - want).abs() < 0.1,
            "{middles:?} against the crossings"
        );
    }
}
