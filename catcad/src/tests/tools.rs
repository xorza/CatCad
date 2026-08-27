//! Drawing with the tools on the bar: where a click puts geometry and what it
//! ties itself to.

use crate::CatCad;
use crate::prompt::Prompt;
use crate::tests::harness::Raised;
use glam::DVec2;
use palantir::Key;
use silverpoint::Entity;
use silverpoint::PointId;

use crate::hud::internals;
use crate::intent::Choice;
use crate::part::Part;
use crate::prompt::Asking;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;

/// The line and circle tools take two clicks, reach the document only on the
/// second, and tie themselves to a point the first click landed on.
///
/// The tie is the half that matters. An edge drawn onto a point already there
/// is what makes a sketch a sketch rather than a heap of unrelated coordinates
/// — drag that point and both edges follow — and it is *stated* rather than
/// shared, so the second line brings its own corner point and a coincidence
/// saying the two are one. That is what this pins: four new points across two
/// edges, no handle held in common, and a relation to show for it. Taking one
/// apart again is `an_edge_started_on_a_point_is_tied_to_it_and_can_be_untied`,
/// which drives the drawing directly and can drag.
///
/// And that nothing lands until the shape is finished: a line abandoned after
/// one click leaves no stray point behind, which is what lets the whole edge be
/// one step to take back.
#[test]
fn a_line_takes_two_clicks_and_ties_itself_to_the_point_it_started_on() {
    let mut raised = Raised::new();
    let at_rest = raised.drawing().sketch().points().count();
    let edges = raised.drawing().sketch().segments().count();

    // Three spots on bare plane, left of the demo's frame.
    let plane = raised.drawing().plane();
    let corner = [
        plane.point(DVec2::new(-1.5, 1.0)).as_vec3(),
        plane.point(DVec2::new(-1.5, 3.5)).as_vec3(),
        plane.point(DVec2::new(-4.0, 3.5)).as_vec3(),
    ];
    let at = corner.map(|world| raised.cursor_on(world));

    // One click starts the line and puts nothing in the document.
    raised.press(internals::tool("Line"));
    raised.frame();
    raised.harness.click_at(at[0]);
    raised.frame();
    assert_eq!(
        raised.drawing().sketch().points().count(),
        at_rest,
        "the first click of a line reached the document"
    );
    assert!(
        raised.app.session.tool().started().is_some(),
        "the first click was not remembered"
    );

    // The second finishes it: two points and the edge between them, and the
    // tool starts over ready for another.
    raised.harness.click_at(at[1]);
    raised.frame();
    let sketch = raised.drawing().sketch();
    assert_eq!(sketch.points().count(), at_rest + 2);
    assert_eq!(sketch.segments().count(), edges + 1);
    assert!(
        raised.app.session.tool().started().is_none(),
        "the tool did not start over"
    );
    assert!(
        raised.app.session.tool().is(Tool::Line { from: None }),
        "it left the hand"
    );

    // A second line begun on the first one's far end brings its own corner, so
    // this one costs two new points and a coincidence tying one of them to the
    // point it was started on.
    let relations = raised.drawing().sketch().constraints().count();
    raised.harness.click_at(at[1]);
    raised.frame();
    raised.harness.click_at(at[2]);
    raised.frame();
    let sketch = raised.drawing().sketch();
    assert_eq!(
        sketch.points().count(),
        at_rest + 4,
        "the second line took the point it started on instead of tying to it"
    );
    assert_eq!(sketch.segments().count(), edges + 2);
    assert_eq!(
        sketch.constraints().count(),
        relations + 1,
        "the join was not written down"
    );

    // The two edges name four points between them and hold none in common:
    // what joins them is the relation, not the handle. Counted rather than
    // sorted, because a handle carries no order — only whether it is the same
    // handle, which is the whole of the question here.
    let ends: Vec<PointId> = sketch
        .segments()
        .skip(edges)
        .flat_map(|(_, edge)| [edge.a, edge.b])
        .collect();
    let distinct = ends
        .iter()
        .enumerate()
        .filter(|(seen, id)| !ends[..*seen].contains(id))
        .count();
    assert_eq!(distinct, 4, "the two edges share a point: {ends:?}");

    // Ctrl+Z takes back a whole edge, both its points with it.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    let sketch = raised.drawing().sketch();
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "half an edge came back"
    );
    assert_eq!(sketch.points().count(), at_rest + 2);
}

/// The circle tool takes its centre from the first click and its size from the
/// second, and makes a point only at the centre.
///
/// A radius is a number rather than a place, which is the whole of why this is
/// not two points: the second click says how far, and the sketch is left with
/// nothing out there to drag.
#[test]
fn a_circle_takes_its_centre_from_one_click_and_its_size_from_the_next() {
    let mut raised = Raised::new();
    let at_rest = raised.drawing().sketch().points().count();
    let rings = raised.drawing().sketch().circles().count();

    // Centre and rim two units apart on the plane, so the radius is known.
    let plane = raised.drawing().plane();
    let middle = plane.point(DVec2::new(-3.0, 2.5)).as_vec3();
    let rim = plane.point(DVec2::new(-1.0, 2.5)).as_vec3();
    let (at_middle, at_rim) = (raised.cursor_on(middle), raised.cursor_on(rim));

    raised.press(internals::tool("Circle"));
    raised.frame();
    raised.harness.click_at(at_middle);
    raised.frame();
    assert_eq!(
        raised.drawing().sketch().circles().count(),
        rings,
        "the first click of a circle reached the document"
    );

    // Out to the rim before clicking there, which is what drawing a circle
    // *is*: the band follows the pointer, and the form asking for a radius
    // stands clear of the band rather than of the centre — so where it goes is
    // only settled once there is a circle for it to keep off.
    raised.harness.move_to(at_rim);
    raised.frame();
    raised.harness.click_at(at_rim);
    raised.frame();
    let sketch = raised.drawing().sketch();
    assert_eq!(sketch.circles().count(), rings + 1);
    // One point, at the centre. Nothing was made out on the rim.
    assert_eq!(sketch.points().count(), at_rest + 1);

    let (_, circle) = sketch.circles().last().expect("a circle was just added");
    assert!(
        (circle.radius - 2.0).abs() < 1e-2,
        "two units apart on the plane made a radius of {}",
        circle.radius
    );
    assert!(
        (sketch.point(circle.center).position - DVec2::new(-3.0, 2.5)).length() < 1e-2,
        "the centre did not land where it was clicked"
    );
}

/// **A circle's radius can be typed instead of clicked, and the form asking for
/// it stands from the moment there is a centre.**
///
/// The one form that stands where there is nothing yet to name. Every other
/// restates something already drawn; this one *makes* the circle, which is the
/// only way a tool can offer a form at all — what a change makes has no handle
/// until the change lands, and the session applies before the history does.
#[test]
fn a_circle_takes_a_typed_radius_instead_of_a_second_click() {
    let mut raised = Raised::new();
    let rings = |app: &CatCad| app.document.drawn(app.editing()).sketch().circles().count();
    let before = rings(&raised.app);

    let plane = raised.drawing().plane();
    let middle = plane.point(DVec2::new(-3.0, 2.5)).as_vec3();
    let at_middle = raised.cursor_on(middle);

    raised.press(internals::tool("Circle"));
    raised.frame();
    raised.harness.click_at(at_middle);
    raised.frame();
    assert!(
        matches!(
            raised.app.session.prompt().map(Prompt::about),
            Some(Asking::Circle { .. })
        ),
        "putting the centre down opened no form"
    );
    assert_eq!(
        before,
        rings(&raised.app),
        "the centre click reached the document"
    );

    // Typed rather than clicked, and settled. The tool goes back to its first
    // click, which is where a second one would have left it.
    raised.harness.type_text("2");
    raised.frame();
    raised.harness.key(Key::Enter);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "Enter left the form open"
    );
    assert_eq!(
        rings(&raised.app),
        before + 1,
        "the typed radius drew no circle"
    );
    assert_eq!(
        raised.app.session.tool(),
        Tool::Circle { center: None },
        "the tool did not go back to its first click"
    );

    // At the size that was typed, measured off the centre it was struck from.
    let drawing = raised.drawing();
    let (_, drawn) = drawing
        .sketch()
        .circles()
        .last()
        .expect("the circle that was just drawn");
    assert!(
        (drawn.radius.abs() - 2.0).abs() < 1e-6,
        "the circle came out at {} rather than the 2 that was typed",
        drawn.radius
    );
}

/// **The pointer offers a radius until somebody types one, and then it stops.**
///
/// Two views of one number, and the rule for which of them is speaking. The
/// pointer *suggests* — the field shows what the band is measuring, and the
/// draft stays empty so the first keystroke lands in a field with nothing to
/// fight. From that keystroke the keyboard has it: the band snaps to what was
/// typed and stops following the cursor.
///
/// Which is driving needs no flag to say so. **The draft being non-empty is the
/// keyboard having it**, so backspacing the last character hands the pointer
/// back — which is what anyone would expect and what a flag would have had to
/// be told to do.
#[test]
fn the_pointer_offers_a_radius_until_one_is_typed_and_then_lets_go() {
    let mut raised = Raised::new();

    let plane = raised.drawing().plane();
    let middle = plane.point(DVec2::new(-3.0, 2.5)).as_vec3();
    let at_middle = raised.cursor_on(middle);
    raised.press(internals::tool("Circle"));
    raised.frame();
    raised.harness.click_at(at_middle);
    raised.frame();

    // Two units out, so what the band measures is known by hand.
    let out = raised.cursor_on(plane.point(DVec2::new(-1.0, 2.5)).as_vec3());
    raised.harness.move_to(out);
    raised.frame();
    let banded = |app: &CatCad| app.view.banded().map(|to| middle.distance(to));
    assert!(
        (banded(&raised.app).expect("the band follows the pointer") - 2.0).abs() < 1e-3,
        "the band measured {:?} rather than the two units it was carried",
        banded(&raised.app)
    );
    let open = raised.app.session.prompt().expect("the form is open");
    assert_eq!(
        open.value(0),
        None,
        "nobody has typed, so nobody is driving"
    );
    assert!(
        (open.says(0).expect("the pointer offers one") - 2.0).abs() < 1e-3,
        "the field is not showing what the band is measuring"
    );

    // Typed, and the band lets go of the pointer: it holds the typed radius
    // even as the cursor carries on somewhere else entirely.
    raised.harness.type_text("5");
    raised.frame();
    assert_eq!(
        raised.app.session.prompt().and_then(|open| open.value(0)),
        Some(5.0)
    );
    let elsewhere = raised.cursor_on(plane.point(DVec2::new(3.0, 2.5)).as_vec3());
    raised.harness.move_to(elsewhere);
    raised.frame();
    assert!(
        (banded(&raised.app).expect("the band is still drawn") - 5.0).abs() < 1e-3,
        "the band went back to following the cursor at {:?}",
        banded(&raised.app)
    );
}

/// **The bar's Radius offer puts a dimension in hand rather than a form.**
///
/// It used to open a field instead, on the grounds that a radius was a number
/// the drawing could not work out for itself. That is true of every dimension
/// on the bar and so singles out none of them — a distance the drawing already
/// measures is no more what you meant than a radius is — and they are all
/// placed the same way now: stated at what the drawing measures, retyped
/// afterwards by the field that opens on any dimension's mark.
///
/// The two halves are what this pins. Pressing the offer reaches the document
/// with nothing, and hands the tool a circle already picked; the click that
/// puts the number down is the click that states it, at the size the circle was
/// — a radius that *moved* the circle would be the offer locking a number
/// nobody chose, which is the failure the form existed to avoid.
#[test]
fn the_radius_offer_hands_the_dimension_tool_a_circle_to_place() {
    let mut raised = Raised::new();

    let sketch = raised.app.editing();
    // One the drawing does not already hold to a size — a circle whose radius
    // is stated admits no second one, and the demo draws one of each.
    let drawing = raised.app.document.drawn(sketch);
    let held_to = |circle| {
        drawing.sketch().constraints().any(|(_, held)| {
            matches!(held, silverpoint::Constraint::Radius { circle: at, .. } if at == circle)
        })
    };
    let (circle, was) = drawing
        .sketch()
        .circles()
        .find(|&(id, _)| !held_to(id))
        .map(|(id, held)| (id, held.radius))
        .expect("the demo draws a circle with no radius stated");
    raised.choose(Choice::Select(Some(Part::Entity {
        sketch,
        entity: circle.into(),
    })));
    raised.frame();

    let radii = |app: &CatCad| {
        app.document
            .drawn(sketch)
            .sketch()
            .constraints()
            .filter(|(_, held)| matches!(held, silverpoint::Constraint::Radius { .. }))
            .count()
    };
    let before = radii(&raised.app);

    // The one thing a single circle admits — see
    // [`wording`](crate::wording) for the word the chip is named by.
    raised.press(internals::relation("Radius"));
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "the Radius offer opened a form rather than handing the tool a circle"
    );
    assert_eq!(
        raised.app.session.tool(),
        Tool::Dimension(Dimensioning::Placing {
            first: Entity::Circle(circle),
            second: None,
            // A radius is read one way, so there is nothing for the button to
            // name and nothing left for the pointer to decide but where the
            // number goes.
            along: None,
        }),
        "the offer put something other than this circle's radius in hand"
    );
    assert_eq!(
        radii(&raised.app),
        before,
        "pressing Radius stated one before it was placed"
    );

    // Clear of the circle, which for a radius says only where the number sits.
    let plane = raised.app.document.drawn(sketch).plane();
    let out = raised.cursor_on(plane.point(DVec2::new(8.0, 8.0)).as_vec3());
    raised.harness.move_to(out);
    raised.frame();
    raised.harness.click_at(out);
    raised.frame();
    assert_eq!(radii(&raised.app), before + 1, "the click stated no radius");
    assert_eq!(
        raised.app.session.tool(),
        Tool::Dimension(Dimensioning::Empty),
        "the tool kept the circle it had already stated"
    );

    // At the size it already was, asked of both ends: what the relation states
    // and where the circle settled. The circle alone would pass for a radius
    // stated at zero that never converged, and the number alone for one the
    // solver ignored.
    let drawn = raised.app.document.drawn(sketch);
    // Exactly one, because this circle is the one the demo left unheld and the
    // count above says a single radius was added.
    let (_, stated) = drawn
        .sketch()
        .constraints()
        .find(|(_, held)| {
            matches!(held, silverpoint::Constraint::Radius { circle: at, .. } if *at == circle)
        })
        .expect("a radius was just stated about this circle");
    let says = stated.value().expect("a radius carries a number");
    let now = drawn.sketch().circle(circle).radius;
    assert!(
        (says.abs() - was.abs()).abs() < 1e-6,
        "the offer stated {says} for a circle measuring {was}"
    );
    assert!(
        (now.abs() - was.abs()).abs() < 1e-6,
        "stating the radius moved the circle from {was} to {now}"
    );
}
