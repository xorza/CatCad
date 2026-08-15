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

/// The three corners of one triangle.
fn spans(fill: &Fill, at: usize) -> [DVec2; 3] {
    fill.triangles[at].map(|corner| fill.corners[corner as usize])
}

/// Twice the signed area of one triangle — positive counterclockwise.
fn turned(fill: &Fill, at: usize) -> f64 {
    let [a, b, c] = spans(fill, at);
    (b - a).perp_dot(c - a)
}

/// Everything the triangles cover, signed. Equal to the polygon's own area
/// exactly when they tile it without gap or overlap and all wind one way —
/// which is the whole of what a triangulation has to promise.
fn covered(fill: &Fill) -> f64 {
    (0..fill.triangles.len())
        .map(|at| turned(fill, at) / 2.0)
        .sum()
}

/// Every triangle turns counterclockwise, and none of them is a sliver.
fn all_wound_forward(fill: &Fill) -> bool {
    (0..fill.triangles.len()).all(|at| turned(fill, at) > 0.0)
}

/// Where the middle of a triangle falls.
fn middle(fill: &Fill, at: usize) -> DVec2 {
    let [a, b, c] = spans(fill, at);
    (a + b + c) / 3.0
}

/// A square comes out as two triangles that cover it exactly, whichever way
/// round it was given.
#[test]
fn a_square_fills_both_ways_round() {
    let square = corners(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let fill = polygon(&square, &[]);
    assert_eq!(fill.triangles.len(), 2);
    assert!((covered(&fill) - 4.0).abs() < 1e-12, "{}", covered(&fill));
    assert!(all_wound_forward(&fill));

    // The same square wound the other way fills to the same area, still
    // counterclockwise: which way the caller happened to walk it is not
    // something it should have to know.
    let mut backwards = square.clone();
    backwards.reverse();
    let flipped = polygon(&backwards, &[]);
    assert_eq!(flipped.triangles.len(), 2);
    assert!((covered(&flipped) - 4.0).abs() < 1e-12);
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
    assert!((covered(&fill) - 5.0).abs() < 1e-12, "{}", covered(&fill));
    assert!(all_wound_forward(&fill));
    // Six corners, so four triangles — a simple loop always cuts to two fewer
    // triangles than it has corners.
    assert_eq!(fill.triangles.len(), 4);

    // Nothing sits in the bite. A fan from corner 0 would put a triangle
    // straight across it, which is the failure this catches.
    for at in 0..fill.triangles.len() {
        let at = middle(&fill, at);
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
    assert!((covered(&fill) - 15.0).abs() < 1e-12, "{}", covered(&fill));
    assert!(all_wound_forward(&fill));
    // Four corners outside and four in, plus the bridge walked out and back:
    // ten around the loop, so eight triangles.
    assert_eq!(fill.triangles.len(), 8);

    // And none of them lies over the hole.
    for at in 0..fill.triangles.len() {
        let at = middle(&fill, at);
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
    assert!((covered(&fill) - 20.0).abs() < 1e-12, "{}", covered(&fill));
    assert!(all_wound_forward(&fill));

    for at in 0..fill.triangles.len() {
        let at = middle(&fill, at);
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
    assert!((covered(&one) - 15.0).abs() < 1e-12, "{}", covered(&one));
    assert!(
        (covered(&other) - 15.0).abs() < 1e-12,
        "{}",
        covered(&other)
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
    let area = covered(&fill);
    let exact = std::f64::consts::PI * (4.0 - 1.0);
    assert!(
        area < exact && area > exact * 0.997,
        "{area} against {exact}"
    );

    // Nothing was laid across the middle.
    for triangle in 0..fill.triangles.len() {
        assert!(
            middle(&fill, triangle).length() > 1.0,
            "a triangle reached into the hole"
        );
    }
}

/// Outlines with no triangle in them fill to nothing rather than to a
/// degenerate one.
#[test]
fn an_outline_with_no_area_fills_to_nothing() {
    assert!(polygon(&[], &[]).triangles.is_empty());
    assert!(polygon(&corners(&[(0.0, 0.0)]), &[]).triangles.is_empty());
    assert!(
        polygon(&corners(&[(0.0, 0.0), (1.0, 1.0)]), &[])
            .triangles
            .is_empty()
    );
    // Three corners in a line enclose nothing, so there is nothing to cut.
    let flat = polygon(&corners(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]), &[]);
    assert!(covered(&flat).abs() < 1e-12, "{}", covered(&flat));
}
