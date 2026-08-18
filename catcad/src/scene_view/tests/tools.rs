//! Drawing with a tool held: where a click puts geometry, and what a
//! half-finished shape shows while it is being placed.

use crate::drawing::Grip;
use crate::intent::Intents;
use crate::intent::change::Change;
use crate::preview::Preview;
use crate::scene_view::tests::harness::{RaisedView, open_markers};
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;
use glam::{DVec2, Vec2};
use silverpoint::Entity;
use silverpoint::Measurement;

/// With the point tool in hand, a click puts a point on the sketch plane where
/// it landed — and a press that travels turns the view instead of taking hold
/// of what it started on.
///
/// The two halves are one decision. The tool is in hand for the whole gesture,
/// so what it must not do is exactly what the select tool exists to do — and
/// the press below starts on empty space, where a tool that took hold of things
/// would still find nothing, only to travel across the drawing.
#[test]
fn the_point_tool_places_where_it_is_clicked_and_takes_hold_of_nothing() {
    let mut raised = RaisedView::new();
    raised.frame();
    // Empty plane, because a click on something already drawn puts the tool
    // down instead of drawing over it — which is its own test. The spot lies on
    // the sketch plane, so the ray through the pixel showing it meets the plane
    // exactly there, and where the new point belongs is known rather than read
    // back off the thing that placed it.
    let empty = raised.empty_spot();
    let cursor = raised.cursor_on(empty);
    let before = raised.markers();

    raised.hold(Tool::Point);
    raised.harness.move_to(cursor);
    raised.frame();
    raised.harness.click_at(cursor);
    raised.frame();

    let placed = raised.markers();
    assert_eq!(
        placed.len(),
        before.len() + 1,
        "the click placed nothing at all"
    );
    // Somewhere among them rather than last: the scene draws every sketch the
    // document holds, so what comes last is whatever the last sketch drew.
    assert!(
        placed.iter().any(|at| at.abs_diff_eq(empty, 1e-3)),
        "nothing was placed under the cursor at {empty:?}, only {placed:?}"
    );
    // A placement adds; it does not edit what it lands on. The point goes down
    // free and unconstrained, so nothing the solver already settled moves —
    // exactly, since the solve it runs starts from where the last one left off.
    //
    // Every one of them still there rather than the first n of them: the new
    // marker lands among its own sketch's, which is in the middle of a scene
    // drawing two.
    assert!(
        before.iter().all(|was| placed.contains(was)),
        "placing a point moved geometry that was already settled"
    );

    // Still in hand afterwards, so a row of points is a row of clicks.
    assert_eq!(raised.session.tool(), Tool::Point);

    // And a press that travels orbits: the drawing stays put, and the click
    // palantir suppresses in favour of the drag places nothing on release.
    let camera = raised.camera();
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(50.0, 0.0));
    raised.frame();
    raised.harness.release();
    raised.frame();

    assert_ne!(raised.camera(), camera, "an armed press has to still orbit");
    assert_eq!(
        raised.markers(),
        placed,
        "an armed press dragged the drawing, or its release placed a second point"
    );

    // The right button puts it down, and a click afterwards places nothing —
    // which is the half worth checking, since a tool that stopped *looking*
    // armed and went on placing would pass every assertion above.
    raised.harness.right_click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.session.tool(),
        Tool::Pointer,
        "the right button left it in hand"
    );

    raised.harness.click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.markers(),
        placed,
        "a cancelled tool went on placing points"
    );
}

/// A point put down on an edge is held to it, and one put down on a rim is held
/// to that.
///
/// The whole of what a click on something already drawn buys. A click reaches
/// six pixels, so where it lands is *near* the edge and not on it — what makes
/// the point belong to the edge is the constraint, and what proves the
/// constraint is that the solve pulled the point onto the line. Measured
/// against the edge's own two ends, so the answer is the geometry's rather than
/// the picker's.
#[test]
fn a_point_clicked_onto_an_edge_is_held_to_it() {
    let mut raised = RaisedView::new();
    raised.frame();
    let free = raised
        .document
        .models(&raised.build, raised.session.editing())
        .open()
        .outcome()
        .degrees_of_freedom();

    let over_edge = raised
        .over(|grip| matches!(grip, Grip::Segment { .. }))
        .expect("the demo draws an edge");
    let Some(Entity::Segment(edge)) = raised.named_at(over_edge) else {
        panic!("the sweep found something that is not an edge");
    };

    raised.hold(Tool::Point);
    raised.harness.click_at(over_edge);
    raised.frame();

    let sketch = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch();
    let (placed, at) = sketch.points().last().expect("a point was just added");
    let at = at.position;
    // On the edge's infinite line, which is what `PointOnSegment` says: the
    // cross product of the edge's direction with the way to the point is zero.
    let held = sketch.segment(edge);
    let (a, b) = (sketch.point(held.a).position, sketch.point(held.b).position);
    let across = (b - a).perp_dot(at - a) / (b - a).length();
    assert!(
        across.abs() < 1e-6,
        "the point sits {across} off the edge it was put on"
    );

    // Two parameters added and one equation with them, so the drawing has one
    // more degree of freedom than it had — the point may slide along the edge
    // and do nothing else.
    assert_eq!(
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .outcome()
            .degrees_of_freedom(),
        free + 1,
        "a point on an edge should be free along it and nowhere else"
    );
    assert!(
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .outcome()
            .converged(),
        "the solve that puts the point on the edge did not converge"
    );

    // And it slides. A cursor is never exactly on the line, so a drag that
    // demanded the point be exactly where the pointer is could never move it at
    // all — what makes this work is the pull `Solver::drag` reaches with,
    // which lets the point settle back onto the edge as near the cursor as it
    // can get.
    let plane = raised.document.drawing_at(raised.session.editing()).plane();
    // Along the edge on screen, so the drag unarguably asks the point to travel
    // rather than nudging it across a line it is already on.
    let ends = [a, b].map(|end| raised.cursor_on(plane.point(end).as_vec3()));
    let along = (ends[1] - ends[0]).normalize();
    let grab = raised.cursor_on(plane.point(at).as_vec3());

    // The tool goes down first: a press with one in hand turns the view rather
    // than taking hold of anything.
    raised.hold(Tool::Pointer);
    // And the pointer has to arrive a frame before it presses: what a press
    // finds is the hit index the last frame left behind.
    raised.harness.move_to(grab);
    raised.frame();
    raised.harness.press_at(grab);
    raised.frame();
    raised.harness.drag_to(grab + along * 60.0);
    raised.frame();
    raised.harness.release();
    raised.frame();

    let sketch = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch();
    let now = sketch.point(placed).position;
    assert!(
        (now - at).length() > 1e-3,
        "the drag never moved the point, so it proves nothing"
    );
    let held = sketch.segment(edge);
    let (a, b) = (sketch.point(held.a).position, sketch.point(held.b).position);
    let across = (b - a).perp_dot(now - a) / (b - a).length();
    assert!(
        across.abs() < 1e-6,
        "the drag took the point {across} off the edge it was held to"
    );
}

/// Clicking *near* an edge puts the new point on the edge, and leaves the edge
/// exactly where it was.
///
/// Which of the two moves is the whole of it. A click reaches six pixels, so
/// one that lit an edge landed a little off it, and a constraint tying them is
/// exact — so something must give. Left to the solve, the answer is whichever
/// geometry *can* move: aimed at the demo's arm, which is free, the arm came up
/// to meet the cursor. That is backwards. Clicking a thing is a statement about
/// what is being drawn, not an invitation to move what was drawn already.
///
/// The arm rather than the frame, because the frame is determined and could not
/// move whatever the solve wanted — it would pass this while the bug stood.
#[test]
fn a_point_clicked_near_an_edge_moves_itself_onto_it_and_not_the_edge() {
    let mut raised = RaisedView::new();
    raised.frame();

    // The far bar of the arm, which is free at both ends.
    let (edge, bar) = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch()
        .segments()
        .last()
        .expect("the demo draws edges");
    let plane = raised.document.drawing_at(raised.session.editing()).plane();
    let ends = [bar.a, bar.b].map(|id| {
        raised
            .document
            .drawing_at(raised.session.editing())
            .sketch()
            .point(id)
            .position
    });
    let was: Vec<DVec2> = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch()
        .points()
        .map(|(_, at)| at.position)
        .collect();

    // Three pixels off the middle of it, square to it on screen: near enough to
    // light, and nowhere near on it.
    let on_screen = ends.map(|end| raised.cursor_on(plane.point(end).as_vec3()));
    let across = (on_screen[1] - on_screen[0]).normalize().perp();
    let cursor = (on_screen[0] + on_screen[1]) / 2.0 + across * 3.0;
    assert_eq!(
        raised.named_at(cursor),
        Some(Entity::Segment(edge)),
        "the cursor did not land near the bar it was aimed at"
    );

    raised.hold(Tool::Point);
    raised.harness.click_at(cursor);
    raised.frame();

    // The bar has not budged — nor has anything else that was already drawn.
    let sketch = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch();
    let now: Vec<DVec2> = sketch.points().map(|(_, at)| at.position).collect();
    for (index, (before, after)) in was.iter().zip(&now).enumerate() {
        assert!(
            (*after - *before).length() < 1e-9,
            "point {index} moved {} to meet a click that was about the new point",
            (*after - *before).length()
        );
    }

    // And the new point is on the bar's line, which is what it was clicked onto.
    let placed = *now.last().expect("a point was just added");
    let (a, b) = (sketch.point(bar.a).position, sketch.point(bar.b).position);
    let off = (b - a).perp_dot(placed - a) / (b - a).length();
    assert!(
        off.abs() < 1e-9,
        "the new point sits {off} off the edge it was clicked onto"
    );
}

/// A half-drawn line is a stroke on screen and nothing in the document, hanging
/// from where it started to wherever the cursor is.
///
/// The band is the only thing this view draws that the drawing did not write, so
/// what it has to prove is that it is *both*: one more stroke in the scene than
/// the sketch has edges, ending under the cursor, and gone the moment the tool
/// is put down — with the sketch untouched throughout.
#[test]
fn a_half_drawn_line_hangs_from_its_start_to_the_cursor() {
    let mut raised = RaisedView::new();
    raised.frame();
    let edges = raised
        .document
        .drawing_at(raised.session.editing())
        .sketch()
        .segments()
        .count();
    let strokes = raised.strokes();

    let from = raised.empty_spot();
    let start = raised.cursor_on(from);
    raised.hold(Tool::Line { from: None });
    raised.harness.click_at(start);
    raised.frame();
    assert_eq!(
        raised
            .document
            .drawing_at(raised.session.editing())
            .sketch()
            .segments()
            .count(),
        edges,
        "the first click of a line reached the document"
    );

    // Away from where it started, so the band has somewhere to reach.
    let to = raised
        .document
        .drawing_at(raised.session.editing())
        .plane()
        .point(DVec2::new(-4.0, 0.5))
        .as_vec3();
    raised.harness.move_to(raised.cursor_on(to));
    raised.frame();

    assert_eq!(
        raised.strokes(),
        strokes + 1,
        "the band was not drawn, or was drawn into the document"
    );
    // The stroke it added runs from the click to the cursor. It is written
    // after everything the drawing wrote, so it is the last one.
    let renderer = raised.view.renderer().borrow();
    let band = renderer.scene().curves.last().expect("a band was drawn");
    assert!(
        band.points[0].abs_diff_eq(from, 1e-3) && band.points[1].abs_diff_eq(to, 1e-3),
        "the band runs {:?}, not from {from:?} to {to:?}",
        band.points
    );
    // Untagged, so it cannot be hovered, grabbed or picked out — it is not
    // there yet.
    assert_eq!(band.tag, None);
    drop(renderer);

    // Put the tool down and it goes, leaving the drawing exactly as it was.
    raised.harness.right_click_at(raised.cursor_on(to));
    raised.frame();
    assert_eq!(raised.session.tool(), Tool::Pointer);
    assert_eq!(raised.strokes(), strokes, "the band outlived the tool");
    assert_eq!(
        raised
            .document
            .drawing_at(raised.session.editing())
            .sketch()
            .segments()
            .count(),
        edges
    );

    // A circle bands the same way, as a rim rather than a stroke: its size is
    // how far the cursor is from where the first click landed, so a cursor two
    // and a half units out is a band of that radius.
    let rims = raised.view.renderer().borrow().scene().rings.len();
    raised.hold(Tool::Circle { center: None });
    raised.harness.click_at(raised.cursor_on(from));
    raised.frame();
    let out = raised
        .document
        .drawing_at(raised.session.editing())
        .plane()
        .point(DVec2::new(-1.5 + 2.5, 2.5))
        .as_vec3();
    raised.harness.move_to(raised.cursor_on(out));
    raised.frame();

    let renderer = raised.view.renderer().borrow();
    assert_eq!(renderer.scene().rings.len(), rims + 1, "no rim was banded");
    let band = renderer.scene().rings.last().expect("a band was drawn");
    assert!(
        (band.radius - 2.5).abs() < 1e-2,
        "the band came out {} across rather than 2.5",
        band.radius
    );
    assert_eq!(band.tag, None);
}

/// The dimension tool picks two things, follows the pointer, and states what it
/// was showing.
///
/// The whole gesture end to end, and the one claim worth making about it: what
/// the preview showed is what the click stated. They are one value read twice —
/// see [`Dimensioning::proposed`](crate::tool::dimensioning::Dimensioning) — and
/// a tool where they were two is one that looks right until the number lands
/// somewhere else.
///
/// Points rather than a dimension's own mark, which is what lets this be driven
/// through the harness at all: a marker is picked from its anchor and a run of
/// text from a box only a paint measures.
#[test]
fn the_dimension_tool_states_the_distance_its_preview_was_showing() {
    let mut raised = RaisedView::new();
    raised.frame();
    let sketch = raised.session.editing();
    let relations = |raised: &RaisedView| {
        raised
            .document
            .drawing_at(sketch)
            .sketch()
            .constraints()
            .count()
    };
    let stated = relations(&raised);

    // Two points of the open sketch, far enough apart that every reading of
    // them measures something.
    let places = {
        let drawing = raised.document.drawing_at(sketch);
        let mut points = drawing.sketch().points();
        let (_, first) = points.next().expect("the demo draws points");
        let (_, second) = points
            .find(|&(_, point)| {
                // Off the first in *both* axes, so no reading of the pair
                // measures nothing and the tool has all three to choose between.
                let apart = (point.position - first.position).abs();
                apart.x > 1.0 && apart.y > 1.0
            })
            .expect("the demo draws two points apart in both axes");
        [first, second].map(|point| drawing.plane().point(point.position).as_vec3())
    };

    raised.hold(Tool::Dimension(Dimensioning::Empty));
    for (nth, at) in places.into_iter().enumerate() {
        raised.harness.click_at(raised.cursor_on(at));
        raised.frame();
        // Picked out as it is picked up, which between the first click and the
        // second is the only thing on screen that has changed — see the tool's
        // arm in [`SceneView::poll`]. The count is what tells "added to" from
        // "replaced", and both are wanted here: the first click starts over and
        // the second joins it.
        assert_eq!(
            raised.session.selection().picked().len(),
            nth + 1,
            "after {} click(s) the tool showed nothing for what it had picked",
            nth + 1
        );
    }
    assert_eq!(
        relations(&raised),
        stated,
        "picking what to measure reached the document"
    );

    // Out to one side of the pair, which is where a vertical dimension is
    // stood — so the reading the pointer asks for is the one the number lands
    // as, rather than whichever the tool would have chosen anyway.
    let midpoint = places[0].midpoint(places[1]);
    let plane = raised.document.drawing_at(sketch).plane();
    let out = midpoint + (plane.x * 6.0).as_vec3();
    raised.harness.move_to(raised.cursor_on(out));
    raised.frame();

    // The preview is the constraint it would state, so it can simply be read.
    let Some(Preview::Dimension(shown)) = raised.view.preview() else {
        panic!("the tool showed no dimension once it had a pair");
    };

    raised.harness.click_at(raised.cursor_on(out));
    raised.frame();
    assert_eq!(
        relations(&raised),
        stated + 1,
        "the click stated no dimension"
    );

    // The very constraint the preview was showing, down to which way it is read
    // and where its number went.
    let (_, landed) = raised
        .document
        .drawing_at(sketch)
        .sketch()
        .constraints()
        .last()
        .expect("a relation was just stated");
    assert_eq!(landed, shown, "the click stated something else");
    assert!(
        matches!(
            landed,
            // Qualified, because `Along` in this module is the timeline's — the
            // line a plane travels — and this one is the sketch's, which is the
            // way a distance is read.
            silverpoint::Constraint::Distance {
                along: silverpoint::Along::Vertical,
                ..
            }
        ),
        "dragging out to the side read the pair the wrong way: {landed:?}"
    );

    // And the tool is ready for another rather than still holding the pair —
    // holding nothing, either, since what was picked has now been said.
    assert_eq!(
        raised.session.tool(),
        Tool::Dimension(Dimensioning::Empty),
        "the tool kept what it had already stated"
    );
    assert!(
        raised.session.selection().picked().is_empty(),
        "the pair stayed picked out after the dimension was stated"
    );
}

/// Placing a number moves it and leaves the drawing under it alone.
///
/// The one edit to a sketch that changes no geometry, so the whole of what it
/// has to get right is the pair: the number goes where the gesture took it, and
/// everything the constraints decided stays exactly where it was. A placement
/// that reached the solver would show up here as the drawing settling somewhere
/// else for a change that said nothing about it.
///
/// Where the number lands is checked against the place asked for rather than
/// against a number worked out by hand, because that *is* the claim: a placement
/// is stored in the dimension's own frame and read back through it, and a change
/// that wrote one frame and read another would land somewhere plausible and
/// wrong.
#[test]
fn placing_a_number_moves_it_and_settles_nothing() {
    let mut raised = RaisedView::new();
    let sketch = raised.session.editing();
    let (constraint, _) = raised
        .document
        .drawing_at(sketch)
        .sketch()
        .constraints()
        .find(|(_, constraint)| constraint.value().is_some())
        .expect("the demo states a dimension");

    let drawn = open_markers(&raised);
    let stated: Vec<Option<f64>> = raised
        .document
        .drawing_at(sketch)
        .sketch()
        .constraints()
        .map(|(_, constraint)| constraint.value())
        .collect();
    let solved = raised.build.settled(sketch).outcome().iterations();

    // Somewhere the number is plainly not, and off both of the frame's axes so a
    // placement that dropped a component would land short of it.
    let plane = raised.document.drawing_at(sketch).plane();
    let put = plane.point(DVec2::new(-3.25, 4.75)).as_vec3();
    let mut intents = Intents::default();
    intents.push(Change::Place {
        sketch,
        constraint,
        at: put,
    });
    raised
        .history
        .apply(&mut raised.document, &mut raised.build, &intents);

    // The number is where it was put, read back through the drawing rather than
    // through anything this test worked out.
    let drawing = raised.document.drawing_at(sketch);
    let label = Measurement::of(drawing.sketch(), drawing.sketch().constraint(constraint))
        .expect("a dimension has a measurement")
        .label;
    assert!(
        drawing
            .plane()
            .point(label)
            .as_vec3()
            .abs_diff_eq(put, 1e-6),
        "the number was placed at {put:?} and reads at {:?}",
        drawing.plane().point(label)
    );

    // And nothing else moved: not the geometry, not what any dimension states,
    // and not the solve — placing a number is not a question the constraints
    // have anything to say about.
    assert_eq!(
        open_markers(&raised),
        drawn,
        "placing a number moved the drawing it is about"
    );
    assert_eq!(
        raised
            .document
            .drawing_at(sketch)
            .sketch()
            .constraints()
            .map(|(_, constraint)| constraint.value())
            .collect::<Vec<_>>(),
        stated,
        "placing a number restated one"
    );
    assert_eq!(
        raised.build.settled(sketch).outcome().iterations(),
        solved,
        "placing a number ran the solver"
    );
}
