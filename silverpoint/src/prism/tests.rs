use super::*;
use crate::prism::skinner::{Patch, Skinner};
use crate::sketch::Sketch;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use glam::{DVec2, DVec3};
use std::f64::consts::PI;

/// How fine the walls and caps below are cut.
///
/// Fine enough that a flattened circle lands within a rounding of the true one
/// at the sizes here, so an arc's answer can be checked against the area it
/// really encloses rather than against whatever the flattening happened to give.
const FINE: f64 = 1e-5;

/// One arrangement of `sketch`.
fn arranged(sketch: &Sketch) -> Arrangement {
    let mut found = Arrangement::default();
    found.rebuild(sketch);
    found
}

/// A closed run of segments through the given corners.
fn outline(sketch: &mut Sketch, corners: &[(f64, f64)]) {
    let placed: Vec<_> = corners
        .iter()
        .map(|&(x, y)| sketch.add_point(DVec2::new(x, y)))
        .collect();
    for at in 0..placed.len() {
        sketch.add_segment(placed[at], placed[(at + 1) % placed.len()]);
    }
}

/// How much space `prism` shuts in, read off the triangles alone.
///
/// The divergence theorem over a closed surface: a sixth of the sum of
/// `a · (b × c)` across every triangle. It is the one number that asks
/// everything at once — a wall wound the wrong way subtracts where it should
/// add, a cap facing the wrong direction does the same, a hole left unpunched
/// counts as solid, and a surface with a gap in it is not a volume at all. And
/// it is *signed*, so a solid built inside out comes out negative rather than
/// merely wrong by a bit.
///
/// Independent of where the origin sits, which is what lets a prism on a plane
/// away from it be checked against the same arithmetic.
fn volume(prism: &Prism<'_>) -> f64 {
    let mut skinner = Skinner::default();
    let mut patch = Patch::default();
    let mut total = 0.0;
    for grown in prism.grown() {
        skinner.cut(prism, grown, FINE, &mut patch);
        let corner = |at: u32| patch.corners[at as usize];
        for &[a, b, c] in &patch.triangles {
            total += corner(a).dot(corner(b).cross(corner(c)));
        }
    }
    total / 6.0
}

/// **A prism is a closed solid the right way out, whichever way it grows.**
///
/// A two-by-two square carried three deep is twelve, and the sign is the half
/// worth having: every face of it — two caps and four walls — has to be wound
/// counterclockwise seen from *outside*, and any one of them turned over would
/// take its own contribution off instead of adding it. Six faces, because a
/// square is bounded by four curves.
///
/// Then the same prism grown the other way. A negative distance is the same
/// solid on the other side of the plane, so it encloses the same twelve and is
/// no more inside out — which is the whole reason the winding follows the sign
/// rather than being fixed one way.
#[test]
fn a_prism_closes_the_right_way_out_whichever_way_it_grows() {
    let mut sketch = Sketch::default();
    outline(
        &mut sketch,
        &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
    );
    let found = arranged(&sketch);
    assert_eq!(found.faces().len(), 1);

    let up = Prism::new(&found, 0, Plane::GROUND, 3.0);
    let faces: Vec<Grown> = up.grown().collect();
    assert_eq!(faces.len(), 6, "{faces:?}");
    assert_eq!(faces[0], Grown::Base);
    assert_eq!(faces[1], Grown::Far);
    assert!(
        (volume(&up) - 12.0).abs() < 1e-9,
        "grown along the normal it encloses {}",
        volume(&up)
    );

    let down = Prism::new(&found, 0, Plane::GROUND, -3.0);
    assert!(
        (volume(&down) - 12.0).abs() < 1e-9,
        "grown against the normal it encloses {}, so it is inside out",
        volume(&down)
    );

    // And off the world's own origin, since a volume read this way must not
    // depend on where that is.
    let aside = Plane {
        origin: DVec3::new(-4.0, 7.0, 2.5),
        ..Plane::GROUND
    };
    let moved = Prism::new(&found, 0, aside, 3.0);
    assert!(
        (volume(&moved) - 12.0).abs() < 1e-9,
        "moved off the origin it encloses {}",
        volume(&moved)
    );
}

/// **A hole is carried through, and the wall it raises faces inward.**
///
/// The case a cap alone cannot check. A ring is two curves and so four faces —
/// two ends, the outside and the bore — and the bore's wall has to be wound the
/// other way round from the outside's, or the solid would come out as though the
/// hole were filled. The volume says both at once: π(3² − 1²) two deep is 16π,
/// and a bore left facing outward would report 20π instead.
#[test]
fn a_hole_is_carried_through_and_its_wall_faces_inward() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    let outer = sketch.add_circle(middle, 3.0);
    let bore = sketch.add_circle(middle, 1.0);

    let found = arranged(&sketch);
    let ring = found
        .faces()
        .iter()
        .position(|face| face.holes() == 1)
        .expect("the larger circle has the smaller cut from it");
    let prism = Prism::new(&found, ring, Plane::GROUND, 2.0);

    let faces: Vec<Grown> = prism.grown().collect();
    assert_eq!(faces.len(), 4, "{faces:?}");
    // The outside is walked counterclockwise and the bore the other way, which
    // is what tells the two walls apart by name as well as by winding.
    assert!(faces.contains(&Grown::Side(Bound {
        of: Entity::Circle(outer),
        along: true,
    })));
    assert!(faces.contains(&Grown::Side(Bound {
        of: Entity::Circle(bore),
        along: false,
    })));

    // Flattening cuts corners off both circumferences, so the answer lands a
    // shade under the true 16π — and nowhere near the 20π a filled bore is.
    let stated = PI * (3.0 * 3.0 - 1.0 * 1.0) * 2.0;
    let found = volume(&prism);
    assert!(
        found < stated && found > stated * 0.9999,
        "{found} against {stated}"
    );
}

/// **A wall swept off an arc is round rather than faceted.**
///
/// What a normal per corner buys, and the reason [`Edge::outward`] is asked per
/// parameter rather than per edge. Every corner of a cylinder's wall has to
/// carry the radial direction at that corner: hand a quad one normal and the
/// wall reads as a run of flats, however finely it is cut.
///
/// Unit length is checked with it, because a normal that pointed the right way
/// and was not unit would shade as though the surface were tilted.
///
/// [`Edge::outward`]: crate::sketch::arrangement::edge::Edge::outward
#[test]
fn a_wall_swept_off_an_arc_carries_the_curves_own_normal() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(1.0, -2.0));
    let rim = sketch.add_circle(middle, 3.0);
    let found = arranged(&sketch);
    let prism = Prism::new(&found, 0, Plane::GROUND, 1.0);

    let mut skinner = Skinner::default();
    let mut patch = Patch::default();
    skinner.cut(
        &prism,
        Grown::Side(Bound {
            of: Entity::Circle(rim),
            along: true,
        }),
        FINE,
        &mut patch,
    );
    assert!(!patch.corners.is_empty(), "the wall came out empty");

    let axis = Plane::GROUND.point(DVec2::new(1.0, -2.0));
    let mut seen = 0;
    for (corner, normal) in patch.corners.iter().zip(&patch.normals) {
        assert!(
            (normal.length() - 1.0).abs() < 1e-12,
            "a normal {} long",
            normal.length()
        );
        // Straight out from the axis the cylinder stands on — which is the
        // corner's own position with however far it has been raised taken off.
        let out = *corner - axis;
        let flat = out - Plane::GROUND.normal() * out.dot(Plane::GROUND.normal());
        assert!(
            normal.dot(flat.normalize()) > 1.0 - 1e-9,
            "a corner {flat} away faces {normal}, which is not straight out"
        );
        seen += 1;
    }
    // Enough of them that this is a curve rather than a couple of flats: at this
    // sagitta a circle of radius three wants hundreds of pieces.
    assert!(seen > 100, "only {seen} corners round the whole rim");
}

/// **A prism of no depth is well-formed, and encloses nothing.**
///
/// Where an extrude *starts*: a solid is grown the moment it is asked for and
/// the depth is typed or dragged afterwards, so the first thing built is always
/// this one. It has to survive being asked for before it is worth looking at.
///
/// Nothing here is a special case in the code, and the point of the test is to
/// say so. The walls come out as quads whose head and foot coincide — zero-area
/// triangles the rasterizer discards — and their normals are read off the
/// *edge's* outward direction rather than off the sweep, so there is no
/// direction of travel to normalize and no zero to divide by. The two caps land
/// coincident and wound against each other, which is exactly what encloses
/// nothing.
///
/// The face count is the half that would break quietly. A prism that dropped its
/// walls when they had no height would still measure zero volume and still draw
/// correctly, and then a solid dragged up off zero would have to grow faces it
/// had never had — where `Grown` names them durably so a selection survives the
/// drag.
#[test]
fn a_prism_of_no_depth_is_well_formed_and_encloses_nothing() {
    let mut sketch = Sketch::default();
    outline(
        &mut sketch,
        &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
    );
    let found = arranged(&sketch);
    let flat = Prism::new(&found, 0, Plane::GROUND, 0.0);

    // Six faces, the same six a prism of any depth has: two caps and a wall per
    // bounding curve. None of them is skipped for being flat.
    let faces: Vec<Grown> = flat.grown().collect();
    assert_eq!(faces.len(), 6, "{faces:?}");
    assert_eq!(faces[0], Grown::Base);
    assert_eq!(faces[1], Grown::Far);

    // Coincident caps wound against each other enclose nothing at all.
    assert_eq!(volume(&flat), 0.0);

    // And every number of it is a number. A sweep of no length is where a
    // normalize would divide by zero, and every corner and normal reaching the
    // caller finite is what says none happens.
    let mut skinner = Skinner::default();
    let mut patch = Patch::default();
    for grown in flat.grown() {
        skinner.cut(&flat, grown, FINE, &mut patch);
        assert!(
            !patch.corners.is_empty(),
            "{grown:?} came out with no corners"
        );
        for corner in &patch.corners {
            assert!(corner.is_finite(), "{grown:?} put a corner at {corner:?}");
        }
        for normal in &patch.normals {
            assert!(normal.is_finite(), "{grown:?} gave a normal of {normal:?}");
            // Read off the edge rather than off the sweep, so it is still a
            // direction even where nothing was swept.
            assert!(
                (normal.length() - 1.0).abs() < 1e-9,
                "{grown:?} gave a normal of length {}",
                normal.length()
            );
        }
    }
}
