use super::*;
use crate::build::Build;
use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
use aperture::Motion;
use glam::{DVec2, DVec3, Vec3};
use silverpoint::{Plane, Sketch};

/// A step names an earlier step and never itself, and a handle stays dead once
/// its step is gone.
///
/// The two rules that make a timeline a recipe rather than a graph: the walk
/// that resolves a plane only ever goes backwards, so it terminates, and a
/// handle cannot come back naming something else because a position is never
/// handed out twice.
#[test]
fn a_step_is_built_only_on_earlier_ones_and_handles_are_never_reused() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: Sketch::default(),
    });
    assert!(timeline.holds(ground) && timeline.holds(drawn));
    assert_ne!(ground, drawn, "two steps took the same handle");

    // A third step takes a third handle rather than either of the first two,
    // so nothing minted earlier can be confused for it.
    let next = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    assert!(next != ground && next != drawn);

    // And a handle from one timeline names nothing in a shorter one — which is
    // what a caller holding a stale handle across an undo would be doing.
    assert!(!Timeline::default().holds(ground));
}

/// A sketch is stored in its plane's coordinates, and the plane is worked out
/// rather than kept.
///
/// The whole of why a plane can be moved: the sketch below is drawn at (3, 4)
/// on its plane, and where that lands in the world is the plane's answer and
/// nobody else's. Nothing in the sketch records it, so there is no second copy
/// for a move to leave behind.
#[test]
fn a_sketch_lands_where_its_plane_says_and_stores_none_of_it() {
    let mut sketch = Sketch::default();
    let corner = sketch.add_point(DVec2::new(3.0, 4.0));
    let timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();

    // The ground's own axes are world +X and −Z, so (3, 4) on it is (3, 0, −4).
    assert_eq!(timeline.plane_of(at), Plane::GROUND);
    assert_eq!(
        timeline.drawing(at).at(Anchor::On(corner)),
        Vec3::new(3.0, 0.0, -4.0)
    );
    // And the sketch itself holds the flat pair it was given, unchanged.
    assert_eq!(
        timeline.drawing(at).sketch().point(corner).position,
        DVec2::new(3.0, 4.0)
    );
}

/// A datum answers where it may travel and what offset puts it somewhere, and
/// the two are measured from the same place.
///
/// What a drag on one reads. Both answers start at the plane it is *measured
/// off* rather than at the world or at the datum itself, which is what makes
/// them agree: how far along the line a point stands is then exactly the number
/// that would put the plane there, with nothing to convert between.
#[test]
fn a_datum_travels_on_its_base_and_measures_its_offset_from_the_same_place() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let shelf = timeline.add(Feature::Plane(Datum::Offset {
        from: ground,
        by: 2.0,
    }));

    // The ground is where the world is rather than somewhere a plane was put,
    // so there is nothing to take hold of.
    assert_eq!(timeline.movable(ground), None);

    // The ground's normal is +Y, so the shelf travels straight up — through
    // wherever it was grabbed, which is what keeps the corner in hand under the
    // cursor rather than the base's origin at a depth of its own.
    let movable = timeline.movable(shelf).expect("a datum can be moved");
    let grabbed = Vec3::new(7.0, 2.0, -3.0);
    assert_eq!(
        movable.along.travel(grabbed),
        Motion::Line {
            origin: grabbed,
            along: Vec3::Y,
        }
    );
    // Where the shelf already is reads back as the offset it already has, and
    // everything across the line falls out: a point five up and well off to
    // the side is still five.
    assert_eq!(movable.along.offset_at(Vec3::new(7.0, 2.0, -3.0)), 2.0);
    assert_eq!(movable.along.offset_at(Vec3::new(-1.0, 5.5, 8.0)), 5.5);

    // A datum measured off another measures from *that* one. The loft stands
    // 1.5 above the shelf, which is 3.5 above the world — and its own offset is
    // the 1.5, because that is the number the timeline holds for it.
    let loft = timeline.add(Feature::Plane(Datum::Offset {
        from: shelf,
        by: 1.5,
    }));
    let movable = timeline.movable(loft).expect("a datum can be moved");
    assert_eq!(
        movable.along.travel(grabbed),
        Motion::Line {
            origin: grabbed,
            along: Vec3::Y,
        }
    );
    // Which line it travels on says nothing about where the measuring starts:
    // the loft is measured off the shelf, so a point at the world's 3.5 reads
    // back as the 1.5 the timeline holds.
    assert_eq!(movable.along.offset_at(Vec3::new(4.0, 3.5, 1.0)), 1.5);
    assert_eq!(timeline.plane(loft).origin.y, 3.5);
    assert_eq!(movable.along.offset_at(Vec3::new(0.0, 3.5, 0.0)), 1.5);
}

/// Asking a sketch how far it is offset is a caller that has mistaken one kind
/// of step for the other, and is told so rather than answered.
#[test]
#[should_panic(expected = "names a sketch rather than a plane")]
fn a_sketch_is_not_a_plane_that_can_be_moved() {
    let timeline = Timeline::of(Sketch::default());
    timeline.movable(timeline.first_sketch());
}

/// A started timeline holds the three world planes, and none of them moves.
///
/// Where every document begins, and three claims about it. That the three are
/// *found* rather than sitting at fixed positions is what lets a file hold them
/// anywhere — see [`Timeline::world`]. That each resolves to silverpoint's own
/// constant is what says the two crates agree about which way is up. And that
/// none is movable is what keeps a drag off the world: there is no offset on one
/// to restate.
#[test]
fn a_started_timeline_holds_three_world_planes_and_moves_none_of_them() {
    let mut timeline = Timeline::started();
    // Written out rather than walked off `started`, so a fourth world plane
    // left out of that list fails here rather than passing quietly.
    for (world, frame) in [
        (World::Ground, Plane::GROUND),
        (World::Front, Plane::FRONT),
        (World::Side, Plane::SIDE),
    ] {
        let at = timeline
            .world(world)
            .unwrap_or_else(|| panic!("a started timeline is missing {world:?}"));
        assert_eq!(timeline.plane(at), frame, "{world:?} stands elsewhere");
        assert!(timeline.movable(at).is_none(), "{world:?} can be moved");
    }
    assert_eq!(timeline.steps().count(), 3, "a started timeline holds more");
    // Nothing drawn on any of them, which is what an empty document is.
    assert_eq!(timeline.sketches().count(), 0);

    // A datum measured off one runs along *that* one's normal, which is what
    // makes which world plane a drawing starts from a real choice rather than
    // three names for the same frame.
    let front = timeline.world(World::Front).expect("the front");
    let out = timeline.add(Feature::Plane(Datum::Offset {
        from: front,
        by: 2.0,
    }));
    assert_eq!(
        timeline.plane(out).origin,
        DVec3::new(0.0, 0.0, 2.0),
        "a datum off the front does not stand out along +Z",
    );
}

/// **Moving a world plane says which kind of plane was named.**
///
/// The one refusal that cannot be worded as a kind: a world plane *is* a plane,
/// so a caller told it had named "a plane rather than a plane" would learn
/// nothing — and this is the caller most in need of telling, having mistaken a
/// plane for the world. See
/// [`Feature::kind`](crate::timeline::feature::Feature), which is what tells the
/// two apart, and [`wrong_kind`](crate::timeline::wrong_kind), which every
/// refusal here comes through.
#[test]
#[should_panic(expected = "names a world plane rather than a plane that can be moved")]
fn a_world_plane_is_a_plane_and_not_one_that_moves() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    timeline.offset(ground, 1.0);
}

/// And carrying a plane says which kind of step it found.
///
/// A plane held off the ground rather than the ground itself, which is what
/// makes this the other half of the test above: the same slip about the two
/// kinds of plane is reported in the two ways that tell them apart.
#[test]
#[should_panic(expected = "names a plane rather than an extrude")]
fn a_plane_is_not_a_solid_that_can_be_carried() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let shelf = timeline.add(Feature::Plane(Datum::Offset {
        from: ground,
        by: 1.0,
    }));
    timeline.carry(shelf, 1.0);
}

/// Editing reaches the sketch the timeline holds, and the report follows it.
///
/// What `Document::apply` does, one level down: an edit goes through the pair
/// the timeline hands out, so there is no way to reach a sketch without the
/// plane it lies on or the build that records the solve.
#[test]
fn an_edit_through_the_timeline_reaches_the_sketch_it_names() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(2.0, 0.0));
    sketch.add_segment(a, b);

    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();
    let was = build.revision();

    timeline.edit(at).drag_to(
        &mut build,
        Grip::Point(b),
        Plane::GROUND.point(DVec2::new(5.0, 0.0)).as_vec3(),
    );

    // Reached rather than written: a drag pulls toward the cursor through the
    // constraints, so it arrives to the solver's tolerance.
    let landed = timeline.drawing(at).sketch().point(b).position;
    assert!(
        (landed - DVec2::new(5.0, 0.0)).length() < 1e-7,
        "{landed:?}"
    );
    assert_ne!(build.revision(), was, "the edit went unrecorded");
}
