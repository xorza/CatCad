use super::*;
use crate::names::Names;
use crate::paint;
use aperture::Scene;
use glam::DVec2;
use silverpoint::{Constraint, Plane, PointId, Solver};

/// Where a point of `plane` lands in the world as the drawing draws it — the
/// model's `f64` read out into the `f32` a renderer wants, which is the same
/// crossing `sketch_plane`'s writers make.
fn on(plane: Plane, at: DVec2) -> Vec3 {
    plane.point(at).as_vec3()
}

/// Two free points a fixed span apart, tied to nothing else — the smallest
/// drawing that can actually be dragged, and the shape the demo's linkage has.
#[derive(Debug)]
struct Linkage {
    drawing: Drawing,
    /// The room a drag's solve works in. In production this belongs to whatever
    /// is applying edits; a test doing its own dragging keeps its own.
    solver: Solver,
    grip: PointId,
    swing: PointId,
}

impl Linkage {
    fn new() -> Self {
        let mut sketch = Sketch::default();
        let grip = sketch.add_point(DVec2::new(0.0, 0.0));
        let swing = sketch.add_point(DVec2::new(2.0, 0.0));
        sketch.add_segment(grip, swing);
        sketch.add_constraint(Constraint::Distance {
            a: grip,
            b: swing,
            distance: 2.0,
        });
        let mut solver = Solver::default();
        Self {
            drawing: Drawing::new(&mut solver, sketch, Plane::GROUND),
            solver,
            grip,
            swing,
        }
    }

    /// Where a point has ended up, in the world.
    /// Take `grip` to `world`, as the application's edit path would.
    fn drag_to(&mut self, grip: Grip, world: Vec3) {
        self.drawing.drag_to(&mut self.solver, grip, world);
    }

    fn world_of(&self, point: PointId) -> Vec3 {
        on(
            self.drawing.plane,
            self.drawing.sketch.point(point).position,
        )
    }
}

/// A drag holds the grabbed point exactly where it was sent, and the rest of
/// the drawing moves to suit.
#[test]
fn dragging_a_point_puts_it_where_it_was_sent_and_the_rest_follows() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing.plane;

    // Straight up the plane's own y, four along — a 3-4-5 away from where the
    // partner sits, so where it must swing to is hand-checkable.
    let sent = on(plane, DVec2::new(0.0, 4.0));
    linkage.drag_to(Grip::Point(linkage.grip), sent);

    let outcome = linkage.drawing.outcome();
    assert!(outcome.converged(), "{outcome:?}");
    assert!(
        linkage.world_of(linkage.grip).abs_diff_eq(sent, 1e-5),
        "the held point ended at {:?}",
        linkage.world_of(linkage.grip)
    );
    // The partner kept its span, which is the only thing constraining it.
    let span = linkage.world_of(linkage.swing) - linkage.world_of(linkage.grip);
    assert!((span.length() - 2.0).abs() < 1e-5, "{span:?}");
}

/// A world position off the plane is flattened onto it, because a sketch point
/// has nowhere else to be. Whatever the drag resolves against, the drawing
/// stays a drawing.
#[test]
fn a_drag_off_the_plane_lands_on_it() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing.plane;
    let off = on(plane, DVec2::new(1.0, 3.0)) + plane.normal().as_vec3() * 5.0;

    linkage.drag_to(Grip::Point(linkage.grip), off);

    let landed = linkage.world_of(linkage.grip);
    let above = (landed - plane.origin.as_vec3()).dot(plane.normal().as_vec3());
    assert!(above.abs() < 1e-5, "{landed:?} sits {above} off the plane");
    // And it kept the two coordinates the plane does hold.
    assert!(
        landed.abs_diff_eq(on(plane, DVec2::new(1.0, 3.0)), 1e-5),
        "{landed:?}"
    );
}

/// What a press takes hold of, and what it does not.
#[test]
fn a_grip_reads_both_what_was_hit_and_where_on_it() {
    let mut sketch = Sketch::default();
    let free = sketch.add_point(DVec2::ZERO);
    let pinned = sketch.add_point(DVec2::new(1.0, 0.0));
    let loose = sketch.add_point(DVec2::new(0.0, 1.0));
    let anchored = sketch.add_segment(free, pinned);
    let floating = sketch.add_segment(free, loose);
    let hub = sketch.add_point(DVec2::new(2.0, 2.0));
    let hole = sketch.add_circle(hub, 1.0);
    sketch.fix(pinned);
    let drawing = Drawing::new(&mut Solver::default(), sketch, Plane::GROUND);

    assert_eq!(
        drawing.grip(Entity::Point(free), HitAt::Point),
        Some(Grip::Point(free))
    );

    // `fix` is the user saying where it goes, and a drag is not an argument.
    assert_eq!(drawing.grip(Entity::Point(pinned), HitAt::Point), None);

    // An edge slides only if both its ends can: one pinned end would pivot it
    // rather than translate it, which is not what a grab on an edge means.
    let along = |t| HitAt::Segment { index: 0, t };
    assert_eq!(drawing.grip(Entity::Segment(anchored), along(0.5)), None);
    assert_eq!(
        drawing.grip(Entity::Segment(floating), along(0.25)),
        Some(Grip::Segment {
            id: floating,
            t: 0.25
        })
    );

    // A rim drives the radius, so where round it was grabbed does not matter.
    assert_eq!(
        drawing.grip(Entity::Circle(hole), HitAt::Ring { angle: 1.2 }),
        Some(Grip::Rim(hole))
    );

    // Whatever the grip, the answer is the drawing's own plane — a plane is
    // named by any point of it, so there is nothing per-grip to say.
    let Motion { origin, normal } = drawing.motion();
    assert_eq!(origin, drawing.plane.origin.as_vec3());
    assert_eq!(normal, drawing.plane.normal().as_vec3());
}

/// Dragging an edge slides it whole: both ends travel by the same amount, and
/// the spot that was grabbed lands under the cursor.
#[test]
fn dragging_a_segment_translates_both_of_its_ends() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing.plane;
    let edge = linkage
        .drawing
        .sketch
        .segments()
        .next()
        .expect("the linkage draws one edge")
        .0;

    let was = [
        linkage.world_of(linkage.grip),
        linkage.world_of(linkage.swing),
    ];
    // Grabbed at the midpoint and sent three across, four up.
    let midpoint = was[0].lerp(was[1], 0.5);
    let sent = midpoint + on(plane, DVec2::new(3.0, 4.0)) - plane.origin.as_vec3();
    linkage.drag_to(Grip::Segment { id: edge, t: 0.5 }, sent);

    let now = [
        linkage.world_of(linkage.grip),
        linkage.world_of(linkage.swing),
    ];
    assert!(
        now[0].lerp(now[1], 0.5).abs_diff_eq(sent, 1e-5),
        "the grabbed spot ended at {:?}",
        now[0].lerp(now[1], 0.5)
    );
    // Both ends moved by the same amount, which is what makes it a slide
    // rather than a pivot — and the edge kept its length.
    assert!((now[0] - was[0]).abs_diff_eq(now[1] - was[1], 1e-5));
    assert!(((now[1] - now[0]).length() - (was[1] - was[0]).length()).abs() < 1e-5);
}

/// Dragging a rim resizes the circle without walking it: the radius follows
/// the cursor and the centre stays put.
#[test]
fn dragging_a_rim_drives_the_radius_and_holds_the_centre() {
    let mut sketch = Sketch::default();
    let hub = sketch.add_point(DVec2::new(1.0, 2.0));
    let hole = sketch.add_circle(hub, 1.0);
    let mut solver = Solver::default();
    let mut drawing = Drawing::new(&mut solver, sketch, Plane::GROUND);
    let plane = drawing.plane;

    // Three across and four up from the centre is a radius of five.
    let sent = on(plane, DVec2::new(4.0, 6.0));
    drawing.drag_to(&mut solver, Grip::Rim(hole), sent);

    assert!(drawing.outcome().converged(), "{:?}", drawing.outcome());
    let circle = drawing.sketch.circle(hole);
    assert!((circle.radius - 5.0).abs() < 1e-9, "{}", circle.radius);
    assert_eq!(
        drawing.sketch.point(hub).position,
        DVec2::new(1.0, 2.0),
        "resizing walked the circle"
    );

    // And back down again, so the radius follows rather than only growing.
    drawing.drag_to(
        &mut solver,
        Grip::Rim(hole),
        on(plane, DVec2::new(3.0, 2.0)),
    );
    assert!((drawing.sketch.circle(hole).radius - 2.0).abs() < 1e-9);
}

/// A rewrite renames the drawing from scratch, so the tags have to come out
/// the same — a drag holds one across every frame of itself, and a tag that
/// shifted would let go of the point and grab its neighbour.
#[test]
fn rewriting_a_drawing_gives_its_primitives_the_same_tags() {
    let mut linkage = Linkage::new();
    let mut scene = Scene::default();

    let mut names = Names::default();
    paint::redraw(&linkage.drawing, &mut names, None, &mut scene);
    let before: Vec<Option<Entity>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| names.get(tag)))
        .collect();
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(Option::is_some));

    // Move something, so the rewrite has different geometry to emit.
    let plane = linkage.drawing.plane;
    linkage.drag_to(Grip::Point(linkage.grip), on(plane, DVec2::new(-3.0, 1.0)));
    paint::redraw(&linkage.drawing, &mut names, None, &mut scene);

    let after: Vec<Option<Entity>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| names.get(tag)))
        .collect();
    assert_eq!(before, after, "a rewrite renumbered the drawing");
    // Cleared and refilled rather than appended to.
    assert_eq!(scene.points.len(), 2);
    assert_eq!(scene.curves.len(), 1);
    assert!(scene.rings.is_empty());
}

/// A drawing with one of everything, so a selection of any shape has something
/// to be made of.
///
/// Two points three-four-five apart, the edges between them, and a circle on
/// the far one — every kind a relation can be stated over, with hand-checkable
/// numbers between them.
#[derive(Debug)]
struct Assorted {
    drawing: Drawing,
    /// The room an edit's solve works in, kept beside the drawing for the same
    /// reason [`Linkage`] keeps one.
    solver: Solver,
    a: Entity,
    b: Entity,
    first: Entity,
    second: Entity,
    circle: Entity,
}

impl Assorted {
    fn new() -> Self {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::new(0.0, 0.0));
        let b = sketch.add_point(DVec2::new(3.0, 4.0));
        let c = sketch.add_point(DVec2::new(6.0, 0.0));
        let first = sketch.add_segment(a, b);
        let second = sketch.add_segment(b, c);
        let circle = sketch.add_circle(c, 2.5);
        let mut solver = Solver::default();
        Self {
            drawing: Drawing::new(&mut solver, sketch, Plane::GROUND),
            solver,
            a: Entity::Point(a),
            b: Entity::Point(b),
            first: Entity::Segment(first),
            second: Entity::Segment(second),
            circle: Entity::Circle(circle),
        }
    }
}

/// What each shape of selection admits, and what none of them do.
///
/// The one statement of which picks mean what, so this is where a relation
/// offered to the wrong selection — or quietly not offered to the right one —
/// shows up. Nothing else in the crate knows the mapping.
#[test]
fn a_selection_admits_exactly_the_relations_it_can_bear() {
    let Assorted {
        drawing,
        a,
        b,
        first,
        second,
        circle,
        ..
    } = Assorted::new();
    let mut offers = Vec::new();
    // Named here rather than borrowed from the bar that draws them: what the
    // drawing offers is the drawing's, and a test reading the HUD's wording
    // would fail on a relabelling that changed nothing.
    let kinds = |offers: &[Constraint]| -> Vec<&'static str> {
        offers
            .iter()
            .map(|offer| match offer {
                Constraint::Coincident { .. } => "coincident",
                Constraint::Distance { .. } => "distance",
                Constraint::Horizontal { .. } => "horizontal",
                Constraint::Vertical { .. } => "vertical",
                Constraint::Parallel { .. } => "parallel",
                Constraint::Perpendicular { .. } => "perpendicular",
                Constraint::PointOnSegment { .. } => "on edge",
                Constraint::Radius { .. } => "radius",
                Constraint::PointOnCircle { .. } => "on circle",
            })
            .collect()
    };

    drawing.offers(&[a, b], &mut offers);
    assert_eq!(
        kinds(&offers),
        ["coincident", "distance", "horizontal", "vertical"]
    );
    // The distance offered is the one the drawing already has: 3-4-5.
    let Constraint::Distance { distance, .. } = offers[1] else {
        panic!("{offers:?}");
    };
    assert!((distance - 5.0).abs() < 1e-9, "{distance}");

    drawing.offers(&[first, second], &mut offers);
    assert_eq!(kinds(&offers), ["parallel", "perpendicular"]);

    // Either way round is the same relation — which was picked first says
    // nothing about which is held to which.
    for pair in [[a, second], [second, a]] {
        drawing.offers(&pair, &mut offers);
        assert_eq!(kinds(&offers), ["on edge"], "{pair:?}");
    }
    for pair in [[a, circle], [circle, a]] {
        drawing.offers(&pair, &mut offers);
        assert_eq!(kinds(&offers), ["on circle"], "{pair:?}");
    }

    // A radius takes the size the circle already is, so asking for one locks
    // what is there rather than demanding a number nobody can type yet.
    drawing.offers(&[circle], &mut offers);
    assert_eq!(kinds(&offers), ["radius"]);
    let Constraint::Radius { radius, .. } = offers[0] else {
        panic!("{offers:?}");
    };
    assert_eq!(radius, 2.5);

    // And the selections that bear nothing: too few, too many, and a pair with
    // no relation between them.
    for picked in [
        &[][..],
        &[a][..],
        &[first][..],
        &[a, b, circle][..],
        &[first, circle][..],
    ] {
        drawing.offers(picked, &mut offers);
        assert!(offers.is_empty(), "{picked:?} offered {:?}", kinds(&offers));
    }
}

/// Stating a relation moves the drawing onto it, and taking geometry away takes
/// what was built on it.
#[test]
fn constraining_settles_the_drawing_and_deleting_cascades() {
    let Assorted {
        mut drawing,
        mut solver,
        a,
        b,
        first,
        circle,
        ..
    } = Assorted::new();
    let (Entity::Point(pa), Entity::Point(pb)) = (a, b) else {
        panic!("the fixture picks two points");
    };

    // The two points sit 4 apart in y; asked to be level, they meet.
    let mut offers = Vec::new();
    drawing.offers(&[a, b], &mut offers);
    let level = offers[2];
    assert!(matches!(level, Constraint::Horizontal { .. }));
    drawing.constrain(&mut solver, level);
    assert!(drawing.outcome().converged(), "{:?}", drawing.outcome());
    let apart = drawing.sketch().point(pa).position.y - drawing.sketch().point(pb).position.y;
    assert!(apart.abs() < 1e-9, "{apart}");

    // The constraint is a thing the drawing holds, and taking it away leaves
    // the geometry where the solve had put it.
    let stated = drawing
        .sketch()
        .constraints()
        .map(|(id, _)| id)
        .last()
        .expect("the relation was stated");
    assert!(drawing.holds(stated));
    drawing.remove(&mut solver, Entity::Constraint(stated));
    assert!(!drawing.holds(stated));
    assert!(drawing.holds(a) && drawing.holds(b));

    // Removing a point takes the edges it ends with it, and leaves the rest.
    drawing.remove(&mut solver, a);
    assert!(!drawing.holds(a));
    assert!(!drawing.holds(first), "the edge outlived its endpoint");
    assert!(drawing.holds(b) && drawing.holds(circle));
}
