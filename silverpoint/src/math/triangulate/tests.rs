use super::*;
use crate::math::winding::swept;

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

/// The outline and holes of the smallest shape found to tile wrongly.
fn notched_with_two_holes() -> (Vec<DVec2>, Vec<Vec<DVec2>>) {
    let outline = corners(&[
        (43.97114719357511, 0.0),
        (7.402488982271089, 22.782518474856925),
        (-22.381680528676952, 16.26124275231332),
        (-12.236256715578277, -8.890160887458812),
        (4.494613153849542, -13.83299690957409),
    ]);
    let hole = |at: DVec2| -> Vec<DVec2> {
        (0..6)
            .map(|i| {
                let round = -std::f64::consts::TAU * i as f64 / 6.0;
                at + DVec2::new(1.1 * round.cos(), 1.1 * round.sin())
            })
            .collect()
    };
    let holes = vec![
        hole(DVec2::new(2.3947585844075016, 1.0124876900024913)),
        hole(DVec2::new(-2.3947585844075016, -1.0124876900024913)),
    ];
    (outline, holes)
}

/// **Two holes bridged into a notched outline tile it, like everything else.**
///
/// They did not. Triangles came out wound backwards and overlapping, and the
/// area still came out exact — the overlap making up for what was reversed,
/// which is why no area check saw it. Not slivers either: on this outline, of
/// area about 1400, the worst was some eighty-five units of triangle.
///
/// It takes two holes *and* a notched outline. Neither alone does it, which is
/// why [`two_holes_are_both_punched_out`] passes over a rectangle and this needs
/// a shape of its own.
#[test]
fn two_holes_in_a_notched_outline_are_tiled_like_anything_else() {
    let (outline, holes) = notched_with_two_holes();
    let fill = polygon(&outline, &holes);
    assert!(
        all_wound_forward(&fill),
        "a triangle came out wound backwards, so the tiling overlaps itself"
    );
    // The other two the sweep below asks of this class of shape, asked of the
    // one shape named: the area is what the overlap kept exact while the
    // winding went wrong, so on its own it says nothing, and it is here to
    // catch the opposite mistake rather than this one.
    let want =
        swept(&outline).abs() / 2.0 - holes.iter().map(|h| swept(h).abs() / 2.0).sum::<f64>();
    assert!(
        (fill.covered() - want).abs() < 1e-9,
        "tiled {} of the {want} it encloses",
        fill.covered()
    );
    for at in 0..fill.triangles.len() {
        let middle = fill.middle(at);
        for hole in &holes {
            assert!(!encloses(hole, middle), "a triangle was laid over a hole");
        }
    }
}

/// Whether `at` falls inside `loop_`, by crossings — which does not care which
/// way round the loop is wound, unlike everything else here.
fn encloses(loop_: &[DVec2], at: DVec2) -> bool {
    let mut crossings = 0;
    for i in 0..loop_.len() {
        let (a, b) = (loop_[i], loop_[(i + 1) % loop_.len()]);
        if (a.y > at.y) != (b.y > at.y) {
            let x = a.x + (at.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if x > at.x {
                crossings += 1;
            }
        }
    }
    crossings % 2 == 1
}

/// **A notched outline is tiled exactly however many holes are bridged into
/// it.**
///
/// The sweep the shape above came out of. What it holds is the whole of what a
/// tiling promises and what area alone cannot say: every triangle wound the way
/// the outline is, none of them over a hole, and the lot covering exactly what
/// the outline encloses less what the holes take out. The overlap this was
/// written for kept the area exact while reversing triangles, so the area is the
/// weakest of the three and is here to catch the opposite mistake.
#[test]
fn a_notched_outline_is_tiled_whatever_is_punched_out_of_it() {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    for seed in 0..600usize {
        let sides = 5 + seed % 21;
        // Radii swinging hard between neighbours, so the outline is notched
        // rather than merely not round — a convex one has no reflex corner for
        // a bridge to be drawn to, and reflex corners are the whole question.
        let outline: Vec<DVec2> = (0..sides)
            .map(|i| {
                let turn = std::f64::consts::TAU * i as f64 / sides as f64;
                let out = 7.0 + 43.0 * next();
                DVec2::new(out * turn.cos(), out * turn.sin())
            })
            .collect();
        let holes = seed % 4;
        let punched: Vec<Vec<DVec2>> = (0..holes)
            .map(|h| {
                let turn = std::f64::consts::TAU * h as f64 / holes.max(1) as f64 + 0.4;
                let at = DVec2::new(2.6 * turn.cos(), 2.6 * turn.sin());
                (0..6)
                    .map(|i| {
                        let round = -std::f64::consts::TAU * i as f64 / 6.0;
                        at + DVec2::new(1.1 * round.cos(), 1.1 * round.sin())
                    })
                    .collect()
            })
            .collect();

        let fill = polygon(&outline, &punched);
        assert!(
            all_wound_forward(&fill),
            "seed {seed}: {sides} sides and {holes} holes wound a triangle backwards"
        );
        let want =
            swept(&outline).abs() / 2.0 - punched.iter().map(|h| swept(h).abs() / 2.0).sum::<f64>();
        assert!(
            (fill.covered() - want).abs() < 1e-9,
            "seed {seed}: tiled {} of the {want} it encloses",
            fill.covered()
        );
        for at in 0..fill.triangles.len() {
            let middle = fill.middle(at);
            for hole in &punched {
                assert!(
                    !encloses(hole, middle),
                    "seed {seed}: a triangle was laid over a hole"
                );
            }
        }
    }
}

/// **A contour that is no simple loop is cut rather than tripping over**, and
/// what comes back covers what the contour encloses.
///
/// No ear can be taken from one — every corner's triangle holds another corner
/// — so every cut leaves through [`clip`]'s fallback, which is the path that can
/// put a corner *back* into the loop. Which the bookkeeping in [`retest`] said
/// could not happen, and asserted; a bowtie is the shortest thing that does it.
///
/// Three shapes, because they are three ways of not being a loop and each fails
/// somewhere else: a corner sitting on an edge, a corner written twice, and a
/// spur run out and back. A boolean reaches them by cutting a face tangent to
/// its own boundary.
///
/// Held against the shoelace over the same corners, which is what the polygon
/// encloses whatever it does with itself — and against a hand-computed figure
/// beside it, so that a fill and a winding rule agreeing on the wrong answer
/// still fails.
#[test]
fn a_contour_that_is_no_simple_loop_is_still_cut() {
    let cases = [
        // A square with a spike from its far side run down onto its base, which
        // takes the upper half out and leaves two triangles of four apiece.
        (
            "a spike onto an edge",
            &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (2.0, 0.0), (0.0, 4.0)][..],
            8.0,
        ),
        // A square of two by two whose second corner is written twice.
        (
            "a corner written twice",
            &[(0.0, 0.0), (2.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
            4.0,
        ),
        // The same square with a spur run down to its base and back, which
        // shuts in nothing and leaves the four the square covers.
        (
            "a spur out and back",
            &[
                (0.0, 0.0),
                (2.0, 0.0),
                (2.0, 2.0),
                (1.0, 2.0),
                (1.0, 0.0),
                (1.0, 2.0),
                (0.0, 2.0),
            ],
            4.0,
        ),
    ];
    for (name, shape, want) in cases {
        let places = corners(shape);
        let encloses = swept(&places) / 2.0;
        assert!(
            (encloses - want).abs() < 1e-12,
            "{name} encloses {encloses} rather than the {want} it was written to",
        );
        let fill = polygon(&places, &[]);
        assert!(
            (fill.covered() - want).abs() < 1e-12,
            "{name} filled to {} rather than {want}",
            fill.covered(),
        );
    }
}

/// **A contour that crosses itself is answered rather than refused**, and what
/// it is answered with is every lobe of it, each counted once.
///
/// A bowtie is the shortest one: two lobes of a unit each, meeting at their
/// middle, wound against each other. The shoelace over it comes to nought
/// because the two cancel — and the fill comes to two, because the fallback
/// takes each lobe as it finds it and nothing here recovers the sign of the one
/// that runs backwards.
///
/// Worth pinning rather than leaving as whatever it happens to do. Two says the
/// area is neither lost nor doubled, which is what makes the answer a *drawing*
/// of the contour rather than a mess; nought would say the crossing had been
/// resolved, and it has not. Resolving one wants the boolean's own machinery,
/// and nothing yet hands a self-crossing contour here on purpose.
#[test]
fn a_contour_that_crosses_itself_is_drawn_lobe_by_lobe() {
    let bowtie = corners(&[(0.0, 0.0), (2.0, 2.0), (2.0, 0.0), (0.0, 2.0)]);
    // The two lobes are `(0,0) (1,1) (0,2)` and `(1,1) (2,2) (2,0)`, a unit
    // each, and they cancel.
    assert!(swept(&bowtie).abs() < 1e-12);

    let fill = polygon(&bowtie, &[]);
    assert!(
        (fill.covered() - 2.0).abs() < 1e-12,
        "the bowtie filled to {} rather than to both of its lobes",
        fill.covered(),
    );
}
