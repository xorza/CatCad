use super::*;
use crate::build::Build;
use crate::model::{Model, Models};
use crate::paint;
use crate::paint::layout::Layout;
use crate::part::Part;
use crate::timeline::Timeline;
use aperture::Scene;
use glam::DVec2;
use silverpoint::{Along, Constraint, Dimension, PointId};

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
    timeline: Timeline,
    /// The room a drag's solve works in and what it leaves behind. In
    /// production this belongs to whatever is applying edits; a test doing its
    /// own dragging keeps its own.
    build: Build,
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
            along: Along::Shortest,
            dimension: Dimension::new(2.0),
        });
        let mut build = Build::default();
        let mut timeline = Timeline::of(sketch);
        timeline.edit(timeline.first_sketch()).opened(&mut build);
        Self {
            timeline,
            build,
            grip,
            swing,
        }
    }

    /// The sketch and its plane, as a reader of the drawing wants them.
    fn drawing(&self) -> Drawing<'_> {
        self.timeline.drawing(self.timeline.first_sketch())
    }

    /// The two halves as a reader of the model wants them.
    fn model(&self) -> Model<'_> {
        self.models().open()
    }

    /// Every sketch it holds, which for a fixture of one is that one — open,
    /// so it draws in the colours of what it has left to decide.
    fn models(&self) -> Models<'_> {
        Models::new(&self.timeline, &self.build, self.timeline.first_sketch())
    }

    /// Take `grip` to `world`, as the application's edit path would.
    fn drag_to(&mut self, grip: Grip, world: Vec3) {
        let at = self.timeline.first_sketch();
        self.timeline.edit(at).drag_to(&mut self.build, grip, world);
    }

    fn world_of(&self, point: PointId) -> Vec3 {
        on(
            self.drawing().plane(),
            self.drawing().sketch().point(point).position,
        )
    }
}

/// A drag holds the grabbed point exactly where it was sent, and the rest of
/// the drawing moves to suit.
#[test]
fn dragging_a_point_puts_it_where_it_was_sent_and_the_rest_follows() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing().plane();

    // Straight up the plane's own y, four along — a 3-4-5 away from where the
    // partner sits, so where it must swing to is hand-checkable.
    let sent = on(plane, DVec2::new(0.0, 4.0));
    linkage.drag_to(Grip::Point(linkage.grip), sent);

    let model = linkage.model();
    let outcome = model.outcome();
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
    let plane = linkage.drawing().plane();
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
    let timeline = Timeline::of(sketch);
    let drawing = timeline.drawing(timeline.first_sketch());

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
    // named by any point of it, so there is nothing per-grip to say. A drawing
    // never answers with a line: what travels along one is a datum, which is not
    // drawn on anything.
    assert_eq!(
        drawing.motion(),
        Motion::Plane {
            origin: drawing.plane().origin.as_vec3(),
            normal: drawing.plane().normal().as_vec3(),
        }
    );
}

/// Dragging an edge slides it whole: both ends travel by the same amount, and
/// the spot that was grabbed lands under the cursor.
#[test]
fn dragging_a_segment_translates_both_of_its_ends() {
    let mut linkage = Linkage::new();
    let plane = linkage.drawing().plane();
    let edge = linkage
        .drawing()
        .sketch()
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
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();
    let plane = timeline.plane_of(at);

    // Three across and four up from the centre is a radius of five.
    let sent = on(plane, DVec2::new(4.0, 6.0));
    timeline.edit(at).drag_to(&mut build, Grip::Rim(hole), sent);

    assert!(
        build.settled(at).outcome().converged(),
        "{:?}",
        build.settled(at).outcome()
    );
    let circle = timeline.drawing(at).sketch().circle(hole);
    assert!((circle.radius - 5.0).abs() < 1e-9, "{}", circle.radius);
    assert_eq!(
        timeline.drawing(at).sketch().point(hub).position,
        DVec2::new(1.0, 2.0),
        "resizing walked the circle"
    );

    // And back down again, so the radius follows rather than only growing.
    timeline
        .edit(at)
        .drag_to(&mut build, Grip::Rim(hole), on(plane, DVec2::new(3.0, 2.0)));
    assert!((timeline.drawing(at).sketch().circle(hole).radius - 2.0).abs() < 1e-9);
}

/// A rewrite renames the drawing from scratch, so the tags have to come out
/// the same — a drag holds one across every frame of itself, and a tag that
/// shifted would let go of the point and grab its neighbour.
#[test]
fn rewriting_a_drawing_gives_its_primitives_the_same_tags() {
    let mut linkage = Linkage::new();
    let mut scene = Scene::default();

    let mut layout = Layout::default();
    paint::redraw(linkage.models(), &mut layout, None, None, None, &mut scene);
    let before: Vec<Option<Part>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| layout.names().get(tag)))
        .collect();
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(Option::is_some));

    // Move something, so the rewrite has different geometry to emit.
    let plane = linkage.drawing().plane();
    linkage.drag_to(Grip::Point(linkage.grip), on(plane, DVec2::new(-3.0, 1.0)));
    paint::redraw(linkage.models(), &mut layout, None, None, None, &mut scene);

    let after: Vec<Option<Part>> = scene
        .points
        .iter()
        .map(|point| point.tag.and_then(|tag| layout.names().get(tag)))
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
    timeline: Timeline,
    /// The room an edit's solve works in, kept beside the drawing for the same
    /// reason [`Linkage`] keeps one.
    build: Build,
    a: Entity,
    b: Entity,
    first: Entity,
    second: Entity,
    circle: Entity,
    /// A second circle, a different size from the first — so a relation
    /// between the two has something to do.
    other: Entity,
}

impl Assorted {
    /// Every sketch it holds, which for a fixture of one is that one.
    fn models(&self) -> Models<'_> {
        Models::new(&self.timeline, &self.build, self.timeline.first_sketch())
    }

    /// The two halves as a reader of the model wants them.
    fn model(&self) -> Model<'_> {
        self.models().open()
    }

    fn new() -> Self {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::new(0.0, 0.0));
        let b = sketch.add_point(DVec2::new(3.0, 4.0));
        let c = sketch.add_point(DVec2::new(6.0, 0.0));
        let first = sketch.add_segment(a, b);
        let second = sketch.add_segment(b, c);
        let circle = sketch.add_circle(c, 2.5);
        let other = sketch.add_circle(a, 1.0);
        let mut build = Build::default();
        let mut timeline = Timeline::of(sketch);
        timeline.edit(timeline.first_sketch()).opened(&mut build);
        Self {
            timeline,
            build,
            a: Entity::Point(a),
            b: Entity::Point(b),
            first: Entity::Segment(first),
            second: Entity::Segment(second),
            circle: Entity::Circle(circle),
            other: Entity::Circle(other),
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
    let assorted = Assorted::new();
    let model = assorted.model();
    let Assorted {
        a,
        b,
        first,
        second,
        circle,
        other,
        ..
    } = assorted;
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
                Constraint::Standoff { .. } => "standoff",
                Constraint::Spacing { .. } => "spacing",
                Constraint::Radius { .. } => "radius",
                Constraint::PointOnCircle { .. } => "on circle",
                // Told apart here where the bar calls both "Equal", because
                // this is about which relation the drawing reached for and the
                // two are not interchangeable.
                Constraint::EqualLength { .. } => "equal length",
                Constraint::EqualRadius { .. } => "equal radius",
                Constraint::Tangent { .. } => "tangent",
            })
            .collect()
    };

    model.offers(&[model.part(a), model.part(b)], &mut offers);
    assert_eq!(
        kinds(&offers),
        ["coincident", "distance", "horizontal", "vertical"]
    );
    // The distance offered is the one the drawing already has: 3-4-5.
    let Constraint::Distance { dimension, .. } = offers[1] else {
        panic!("{offers:?}");
    };
    assert!((dimension.value - 5.0).abs() < 1e-9, "{dimension:?}");

    model.offers(&[model.part(first), model.part(second)], &mut offers);
    assert_eq!(
        kinds(&offers),
        ["parallel", "perpendicular", "equal length"]
    );

    // Either way round is the same relation — which was picked first says
    // nothing about which is held to which.
    for pair in [[a, second], [second, a]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["on edge"], "{pair:?}");
    }
    for pair in [[a, circle], [circle, a]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["on circle"], "{pair:?}");
    }
    for pair in [[first, circle], [circle, first]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["tangent"], "{pair:?}");
    }
    for pair in [[circle, other], [other, circle]] {
        model.offers(&pair.map(|entity| model.part(entity)), &mut offers);
        assert_eq!(kinds(&offers), ["equal radius"], "{pair:?}");
    }

    // A radius takes the size the circle already is, so asking for one locks
    // what is there rather than demanding a number nobody can type yet.
    model.offers(&[model.part(circle)], &mut offers);
    assert_eq!(kinds(&offers), ["radius"]);
    let Constraint::Radius { dimension, .. } = offers[0] else {
        panic!("{offers:?}");
    };
    assert_eq!(dimension.value, 2.5);

    // And the selections that bear nothing, which are now only the wrong
    // *size*: every pair of geometry kinds above admits something, so a pair
    // that admits nothing would have to be one holding a constraint — and a
    // constraint is a statement rather than a place, so nothing can be stated
    // over one.
    // And a face among them, which admits nothing of its own and takes the
    // pair it is half of down with it: a relation is stated about geometry,
    // and a face is what geometry encloses.
    let face = model.region(0);
    for picked in [
        &[][..],
        &[model.part(a)][..],
        &[model.part(first)][..],
        &[model.part(a), model.part(b), model.part(circle)][..],
        &[face][..],
        &[face, model.part(a)][..],
        &[model.part(a), face][..],
    ] {
        model.offers(picked, &mut offers);
        assert!(offers.is_empty(), "{picked:?} offered {:?}", kinds(&offers));
    }
}

/// Stating a relation moves the drawing onto it, and taking geometry away takes
/// what was built on it.
#[test]
fn constraining_settles_the_drawing_and_deleting_cascades() {
    let Assorted {
        mut timeline,
        mut build,
        a,
        b,
        first,
        circle,
        ..
    } = Assorted::new();
    let at = timeline.first_sketch();
    let (Entity::Point(pa), Entity::Point(pb)) = (a, b) else {
        panic!("the fixture picks two points");
    };

    // The two points sit 4 apart in y; asked to be level, they meet.
    let mut offers = Vec::new();
    let model = Models::new(&timeline, &build, at).open();
    model.offers(&[model.part(a), model.part(b)], &mut offers);
    let level = offers[2];
    assert!(matches!(level, Constraint::Horizontal { .. }));
    timeline.edit(at).constrain(&mut build, level);
    assert!(
        build.settled(at).outcome().converged(),
        "{:?}",
        build.settled(at).outcome()
    );
    let apart = timeline.drawing(at).sketch().point(pa).position.y
        - timeline.drawing(at).sketch().point(pb).position.y;
    assert!(apart.abs() < 1e-9, "{apart}");

    // The constraint is a thing the drawing holds, and taking it away leaves
    // the geometry where the solve had put it.
    let stated = timeline
        .drawing(at)
        .sketch()
        .constraints()
        .map(|(id, _)| id)
        .last()
        .expect("the relation was stated");
    assert!(timeline.drawing(at).holds(stated));
    timeline
        .edit(at)
        .remove(&mut build, Entity::Constraint(stated));
    assert!(!timeline.drawing(at).holds(stated));
    assert!(timeline.drawing(at).holds(a) && timeline.drawing(at).holds(b));

    // Removing a point takes the edges it ends with it, and leaves the rest.
    timeline.edit(at).remove(&mut build, a);
    assert!(!timeline.drawing(at).holds(a));
    assert!(
        !timeline.drawing(at).holds(first),
        "the edge outlived its endpoint"
    );
    assert!(timeline.drawing(at).holds(b) && timeline.drawing(at).holds(circle));
}

/// An edge drawn onto a point already there gets its own point and a
/// coincidence, so the join holds while it is stated and comes apart when it is
/// deleted.
///
/// The whole of why a join is a constraint rather than a shared handle. Sharing
/// holds the two together more exactly and holds them together *for good*:
/// there is nothing to delete, so a corner once drawn can never be pulled
/// apart, and nothing on the drawing says why dragging one edge moves another.
#[test]
fn an_edge_started_on_a_point_is_tied_to_it_and_can_be_untied() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(0.0, 0.0));
    let b = sketch.add_point(DVec2::new(2.0, 0.0));
    sketch.add_segment(a, b);
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();
    let ground = timeline.plane_of(at);

    // A second edge begun on the first one's far end.
    timeline.edit(at).add_segment(
        &mut build,
        Anchor::On(b),
        Anchor::At(on(ground, DVec2::new(2.0, 2.0))),
    );

    // Two new points, not one: the corner is two points that agree, and the
    // agreement is written down.
    assert_eq!(timeline.drawing(at).sketch().points().count(), 4);
    assert_eq!(timeline.drawing(at).sketch().segments().count(), 2);
    let ends: Vec<PointId> = timeline
        .drawing(at)
        .sketch
        .segments()
        .flat_map(|(_, edge)| [edge.a, edge.b])
        .collect();
    assert_eq!(
        ends.iter().filter(|&&id| id == b).count(),
        1,
        "the new edge took the point that was already there: {ends:?}"
    );
    let (tie, coincidence) = timeline
        .drawing(at)
        .sketch
        .constraints()
        .find(|(_, c)| matches!(c, Constraint::Coincident { .. }))
        .expect("the join is stated");
    let Constraint::Coincident { a: corner, .. } = coincidence else {
        unreachable!("found by the match above")
    };
    assert_ne!(corner, b, "the coincidence ties a point to itself");

    // A cleanup leaves the corner alone. Both of its points end an edge, so
    // neither is spare however exactly they sit on each other — which is what
    // stops the command from quietly undoing every join in the drawing.
    let before = (
        timeline.drawing(at).sketch().points().count(),
        timeline.drawing(at).sketch().segments().count(),
        timeline.drawing(at).sketch().constraints().count(),
    );
    timeline.edit(at).remove_duplicates(&mut build);
    assert_eq!(
        (
            timeline.drawing(at).sketch().points().count(),
            timeline.drawing(at).sketch().segments().count(),
            timeline.drawing(at).sketch().constraints().count(),
        ),
        before,
        "a cleanup pulled a join apart"
    );

    // Stated, the join holds: taking `b` somewhere brings the corner with it,
    // which is the behaviour sharing a handle used to give for free.
    timeline
        .edit(at)
        .drag_to(&mut build, Grip::Point(b), on(ground, DVec2::new(2.5, 0.5)));
    let moved = timeline.drawing(at).sketch().point(b).position;
    assert!(
        timeline
            .drawing(at)
            .sketch
            .point(corner)
            .position
            .abs_diff_eq(moved, 1e-6),
        "the corner came apart under a drag: {:?} against {moved:?}",
        timeline.drawing(at).sketch().point(corner).position
    );

    // Deleted, it does not. This is the whole point: the second edge is now
    // free of the first and stays where it was left.
    let parted = timeline.drawing(at).sketch().point(corner).position;
    timeline
        .edit(at)
        .remove(&mut build, Entity::Constraint(tie));
    timeline.edit(at).drag_to(
        &mut build,
        Grip::Point(b),
        on(ground, DVec2::new(0.5, -1.5)),
    );
    assert!(
        timeline
            .drawing(at)
            .sketch
            .point(corner)
            .position
            .abs_diff_eq(parted, 1e-9),
        "the untied corner followed the drag anyway"
    );
    assert!(
        !timeline
            .drawing(at)
            .sketch()
            .point(b)
            .position
            .abs_diff_eq(parted, 1e-3),
        "the drag went nowhere, so this proves nothing"
    );
}
