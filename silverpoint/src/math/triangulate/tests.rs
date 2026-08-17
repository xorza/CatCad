use super::*;

/// One polygon cut, through a cutter stood up for the call.
///
/// Every test here asks about one shape, so nothing is saved by keeping the
/// cutter — what it saves is measured where it matters, in the arrangement's
/// own sweep and in the application's allocation gates.
fn polygon(around: &[DVec2], holes: &[Vec<DVec2>]) -> Fill {
    let mut punched = Loops::default();
    for hole in holes {
        punched.push(hole);
    }
    let mut fill = Fill::default();
    Cutter::default().polygon(around, &punched, &mut fill);
    fill
}

fn corners(of: &[(f64, f64)]) -> Vec<DVec2> {
    of.iter().map(|&(x, y)| DVec2::new(x, y)).collect()
}

/// Every triangle turns counterclockwise.
///
/// Winding alone. How *large* each one is, is what `covered` reads, and a
/// sliver that survived clipping still turns the right way.
fn all_wound_forward(fill: &Fill) -> bool {
    (0..fill.triangles.len()).all(|at| fill.sweep_of(at) > 0.0)
}

/// A square comes out as two triangles that cover it exactly, whichever way
/// round it was given.
#[test]
fn a_square_fills_both_ways_round() {
    let square = corners(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let fill = polygon(&square, &[]);
    assert_eq!(fill.triangles.len(), 2);
    assert!((fill.covered() - 4.0).abs() < 1e-12, "{}", fill.covered());
    assert!(all_wound_forward(&fill));

    // The same square wound the other way fills to the same area, still
    // counterclockwise: which way the caller happened to walk it is not
    // something it should have to know.
    let mut backwards = square.clone();
    backwards.reverse();
    let flipped = polygon(&backwards, &[]);
    assert_eq!(flipped.triangles.len(), 2);
    assert!((flipped.covered() - 4.0).abs() < 1e-12);
    assert!(all_wound_forward(&flipped));
}

/// A concave outline is covered exactly, and nothing is laid over the notch cut
/// out of it — which is the thing a fan from one corner would get wrong.
#[test]
fn a_concave_outline_leaves_its_notch_empty() {
    // An L, three across and three up with a two-by-two bite out of the top
    // right: 9 − 4 = 5.
    let ell = corners(&[
        (0.0, 0.0),
        (3.0, 0.0),
        (3.0, 1.0),
        (1.0, 1.0),
        (1.0, 3.0),
        (0.0, 3.0),
    ]);
    let fill = polygon(&ell, &[]);
    assert!((fill.covered() - 5.0).abs() < 1e-12, "{}", fill.covered());
    assert!(all_wound_forward(&fill));
    // Six corners, so four triangles — a simple loop always cuts to two fewer
    // triangles than it has corners.
    assert_eq!(fill.triangles.len(), 4);

    // Nothing sits in the bite. A fan from corner 0 would put a triangle
    // straight across it, which is the failure this catches.
    for at in 0..fill.triangles.len() {
        let at = fill.middle(at);
        assert!(
            !(at.x > 1.0 && at.y > 1.0),
            "a triangle covered the notch at {at:?}"
        );
    }
}

/// A hole is bridged into the outline and comes out empty.
#[test]
fn a_hole_is_punched_out_of_what_surrounds_it() {
    let outer = corners(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
    let hole = corners(&[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)]);
    let fill = polygon(&outer, &[hole]);

    // Sixteen less the one taken out of it.
    assert!((fill.covered() - 15.0).abs() < 1e-12, "{}", fill.covered());
    assert!(all_wound_forward(&fill));
    // Four corners outside and four in, plus the bridge walked out and back:
    // ten around the loop, so eight triangles.
    assert_eq!(fill.triangles.len(), 8);

    // And none of them lies over the hole.
    for at in 0..fill.triangles.len() {
        let at = fill.middle(at);
        let within = at.x > 1.0 && at.x < 2.0 && at.y > 1.0 && at.y < 2.0;
        assert!(!within, "a triangle covered the hole at {at:?}");
    }
}

/// Two holes are bridged one after the other, and the second reaches past the
/// first rather than through it.
#[test]
fn two_holes_are_both_punched_out() {
    let outer = corners(&[(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)]);
    let near = corners(&[(1.0, 1.0), (2.0, 1.0), (2.0, 3.0), (1.0, 3.0)]);
    let far = corners(&[(4.0, 1.0), (5.0, 1.0), (5.0, 3.0), (4.0, 3.0)]);
    let fill = polygon(&outer, &[near, far]);

    // Twenty-four, less two of two apiece.
    assert!((fill.covered() - 20.0).abs() < 1e-12, "{}", fill.covered());
    assert!(all_wound_forward(&fill));

    for at in 0..fill.triangles.len() {
        let at = fill.middle(at);
        let in_hole =
            |left: f64, right: f64| at.x > left && at.x < right && at.y > 1.0 && at.y < 3.0;
        assert!(!in_hole(1.0, 2.0) && !in_hole(4.0, 5.0), "covered {at:?}");
    }
}

/// A hole given the same way round as the outline is still a hole.
///
/// Winding is normalised on the way in for both, so a caller that walked every
/// loop the same way — which is what a face walk hands over before anything
/// classifies it — gets the fill it meant rather than a doubled outline.
#[test]
fn a_hole_wound_like_its_outline_is_still_cut_out() {
    let outer = corners(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
    // Counterclockwise, the same as the outline above.
    let hole = corners(&[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)]);
    let mut backwards = hole.clone();
    backwards.reverse();

    let one = polygon(&outer, &[hole]);
    let other = polygon(&outer, &[backwards]);
    assert!((one.covered() - 15.0).abs() < 1e-12, "{}", one.covered());
    assert!(
        (other.covered() - 15.0).abs() < 1e-12,
        "{}",
        other.covered()
    );
}

/// A ring flattened into corners — the shape a circle actually arrives as.
#[test]
fn a_flattened_ring_fills_to_the_area_it_encloses() {
    let steps = 64;
    let at = |turn: f64, radius: f64| {
        let angle = std::f64::consts::TAU * turn;
        DVec2::new(angle.cos(), angle.sin()) * radius
    };
    let outer: Vec<DVec2> = (0..steps)
        .map(|step| at(step as f64 / steps as f64, 2.0))
        .collect();
    let inner: Vec<DVec2> = (0..steps)
        .map(|step| at(step as f64 / steps as f64, 1.0))
        .collect();

    // The donut: a ring inside a ring, which is the case that has no crossings
    // at all and so exists only because a hole can be bridged.
    let fill = polygon(&outer, &[inner]);
    assert!(all_wound_forward(&fill));
    // A 64-gon covers a shade under its circle, by the same fraction at both
    // radii, so the difference lands just under 3π.
    let area = fill.covered();
    let exact = std::f64::consts::PI * (4.0 - 1.0);
    assert!(
        area < exact && area > exact * 0.997,
        "{area} against {exact}"
    );

    // Nothing was laid across the middle.
    for triangle in 0..fill.triangles.len() {
        assert!(
            fill.middle(triangle).length() > 1.0,
            "a triangle reached into the hole"
        );
    }
}

/// Outlines with no triangle in them cover nothing, and the one that is nothing
/// *but* a degenerate triangle comes back empty rather than carrying it.
///
/// The emptiness is the half worth asserting. Every triangle a fill hands back
/// is one the caller flattens, indexes and rasterizes, so a sliver in the list
/// is work done to draw no pixels — and the last three corners are the one
/// place one could reach the caller, being the only triple `ear` never tests.
/// Reading the *area* instead would pass either way: a degenerate triangle
/// covers nothing, which is exactly what makes it invisible here.
#[test]
fn an_outline_with_no_area_fills_to_nothing() {
    assert!(polygon(&[], &[]).triangles.is_empty());
    assert!(polygon(&corners(&[(0.0, 0.0)]), &[]).triangles.is_empty());
    assert!(
        polygon(&corners(&[(0.0, 0.0), (1.0, 1.0)]), &[])
            .triangles
            .is_empty()
    );

    // Three corners in a line enclose nothing, so there is nothing to cut and
    // nothing comes back — not one triangle of no area.
    let flat = polygon(&corners(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]), &[]);
    assert!(flat.triangles.is_empty(), "{:?}", flat.triangles);

    // A longer run of them is the stated limit rather than an oversight: no ear
    // can be cut from a contour with no area anywhere, so every corner leaves
    // through the fallback, which emits to keep a self-crossing contour from
    // going undrawn. What holds whatever the length is that nothing is covered.
    for run in [
        &[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)][..],
        &[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0)][..],
    ] {
        let along = polygon(&corners(run), &[]);
        assert!(
            along.covered().abs() < 1e-12,
            "{} corners in a line covered {}",
            run.len(),
            along.covered()
        );
    }
}
