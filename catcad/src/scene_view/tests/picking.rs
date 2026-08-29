//! What a press finds, what a move lights, and what one part takes from
//! another.

use crate::drawing::Grip;
use crate::intent::Opening;
use crate::internals::HARNESS_SIZE;
use crate::part::Part;
use crate::scene_view::click::dimension;
use crate::scene_view::gesture::{Gesture, label};
use crate::scene_view::tests::harness::RaisedView;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;
use glam::{DVec2, Vec2};
use palantir::Modifiers;
use silverpoint::Entity;

/// The pointer moving *within* the view has to wake a frame, and what it lands
/// on has to reach `hovered`.
///
/// Palantir drops a `PointerMoved` that crosses no widget boundary and latches
/// no press, so a view filling the window sees none of them unless it watches
/// for them — and a highlight computed on the way in then sits stale on screen
/// until an unrelated event forces a frame. That is the whole of what this
/// pins: the move inside, not the one that enters.
#[test]
fn a_move_inside_the_view_wakes_a_frame_and_lights_what_it_lands_on() {
    let mut raised = RaisedView::new();
    // Arranges the view, so there is something for the pointer to be over.
    raised.frame();

    let cursor = raised
        .over_draggable()
        .expect("the demo draws something to grab");

    // Entering the view changes the hover target, which wakes a frame by
    // itself — so the one that proves anything is the next, wholly inside.
    raised.harness.move_to(cursor);
    raised.frame();
    let delta = raised.harness.move_to(cursor + Vec2::splat(2.0));
    assert!(
        delta.requests_repaint,
        "a move inside the view left the frame asleep, so the highlight would go stale"
    );

    // And the frame that move asks for is the one that lights the primitive.
    raised.harness.move_to(cursor);
    raised.frame();
    assert!(
        raised.view.hovered().is_some(),
        "aimed at the drawing and lit nothing"
    );

    // Off the drawing entirely, nothing stays lit.
    raised.harness.move_to(Vec2::new(
        HARNESS_SIZE.x as f32 - 1.0,
        HARNESS_SIZE.y as f32 - 1.0,
    ));
    raised.frame();
    assert_eq!(raised.view.hovered(), None);
}

/// A click picks out exactly what it landed on, a shift-click adds to what is
/// picked out, and a tool in hand puts itself down rather than drawing over
/// something already there.
///
/// One rule and its two qualifiers, which is why they are one test: what a
/// click selects is whatever is under it, so a click on empty space selects
/// nothing and clears — and shift changes "instead of" to "as well as" without
/// changing what was found. The tool is the exception that proves it: the only
/// click that does *not* select is the one spent putting something down.
#[test]
fn a_click_picks_out_what_it_landed_on_and_shift_adds_to_it() {
    let mut raised = RaisedView::new();
    raised.frame();
    let empty = raised.cursor_on(raised.empty_spot());
    let over_point = raised
        .over(|grip| matches!(grip, Grip::Point(_)))
        .expect("the demo draws a point that can be grabbed");
    let over_rim = raised
        .over(|grip| matches!(grip, Grip::Rim(_)))
        .expect("the demo draws a circle");

    // Nothing is picked out until something is clicked.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(raised.session.selection().count(), 0);

    raised.harness.click_at(over_point);
    raised.frame();
    let point = raised.named_at(over_point).expect("a point is there");
    assert!(raised.session.selection().contains(raised.part(point)));
    assert_eq!(raised.session.selection().count(), 1);

    // Shift adds, leaving what was already picked out where it was.
    raised.harness.set_modifiers(Modifiers {
        shift: true,
        ..Modifiers::NONE
    });
    raised.harness.click_at(over_rim);
    raised.frame();
    let rim = raised.named_at(over_rim).expect("a circle is there");
    assert!(
        raised.session.selection().contains(raised.part(point)),
        "shift dropped the first"
    );
    assert!(raised.session.selection().contains(raised.part(rim)));
    assert_eq!(raised.session.selection().count(), 2);

    // A shift-click on empty space adds nothing and clears nothing.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(
        raised.session.selection().count(),
        2,
        "shift on nothing changed it"
    );

    // A plain click starts over with what it landed on.
    raised.harness.set_modifiers(Modifiers::NONE);
    raised.harness.click_at(over_rim);
    raised.frame();
    assert!(raised.session.selection().contains(raised.part(rim)));
    assert!(
        !raised.session.selection().contains(raised.part(point)),
        "the first survived"
    );
    assert_eq!(raised.session.selection().count(), 1);

    // And on nothing, it clears.
    raised.harness.click_at(empty);
    raised.frame();
    assert_eq!(raised.session.selection().count(), 0);

    // A tool in hand takes the click instead: nothing is picked out by it, and
    // the tool stays in hand. A point already there is the one click that
    // builds nothing — there is a point there.
    raised.hold(Tool::Point);
    let before = raised.markers();
    raised.harness.click_at(over_point);
    raised.frame();
    assert_eq!(
        raised.session.tool(),
        Tool::Point,
        "the tool went out of hand"
    );
    assert_eq!(raised.markers(), before, "it laid a point over a point");
    assert_eq!(
        raised.session.selection().count(),
        0,
        "a click the tool took picked something out"
    );
}

/// A region is hovered and picked out like anything else, and so is a face of
/// the solid grown off one.
///
/// The three things "selectable like the rest" has to mean: the cursor over one
/// reports it, a click picks it out, and it is named by something that survives
/// the drawing being laid out again — which for a region is where it falls among
/// the faces, since it has no handle of its own.
///
/// A solid's face is checked with it because the two are the same claim about
/// the two ends of one feature: the region an extrude was grown *from* and the
/// faces it grew. It is also the whole of what says a solid is drawn at all —
/// nothing can be hovered that was never written into the scene, and nothing can
/// be named that was written without a tag.
#[test]
fn a_region_and_a_solids_face_are_hovered_and_picked_out_like_any_other_part() {
    let mut raised = RaisedView::new();
    raised.frame();

    let on_ground = |raised: &RaisedView, x: f64, y: f64| {
        raised.cursor_on(raised.drawing().plane().point(DVec2::new(x, y)).as_vec3())
    };

    // Inside the demo's rectangle and clear of everything: the frame runs to
    // 8 by 5, the hub's cylinder stands in the middle of it out to a radius of
    // 1.5 about (4, 2.5), and the arm is off below zero. This leaves better than
    // a unit to the nearest of them.
    let inside = on_ground(&raised, 1.4, 2.5);
    raised.harness.move_to(inside);
    raised.frame();
    let hovered = raised.view.hovered();
    assert!(
        matches!(hovered, Some(Part::Region { .. })),
        "the cursor over a region reported {hovered:?}"
    );

    // And over the solid grown off the hub, which stands proud of the plane —
    // so the cursor finds its far end rather than the region it was grown from.
    let solid = on_ground(&raised, 1.2, 4.2);
    raised.harness.move_to(solid);
    raised.frame();
    let over = raised.view.hovered();
    assert!(
        matches!(over, Some(Part::Solid { .. })),
        "the cursor over the extruded hub reported {over:?}"
    );
    raised.harness.click_at(solid);
    raised.frame();
    assert_eq!(
        raised.session.selection().picked(),
        [over.expect("the hover found one")],
        "clicking the solid picked out something else"
    );

    raised.harness.move_to(inside);
    raised.frame();

    // A click picks it out, and what is picked is the same face the hover was.
    raised.harness.click_at(inside);
    raised.frame();
    assert_eq!(
        raised.session.selection().picked(),
        [hovered.expect("the hover found one")],
        "the click picked out something else"
    );

    // And its name survives the drawing being laid out again. Dragging the arm
    // moves geometry without changing what crosses what, so the face is still
    // the face it was — a name that did not survive would be one dropped by the
    // prune every frame of a drag.
    //
    // Asked of the hover rather than of the selection, because taking hold of
    // the arm picks the arm out: what is checked here is that the *name* still
    // resolves to the same face, which is what a position-in-the-walk has to do
    // and a handle would get for free.
    let wrist = raised.cursor_on(raised.wrist());
    raised.harness.press_at(wrist);
    raised.frame();
    raised.harness.drag_to(wrist + Vec2::new(20.0, 12.0));
    raised.frame();
    raised.harness.release();
    raised.frame();
    raised.harness.move_to(inside);
    raised.frame();
    assert_eq!(
        raised.view.hovered(),
        hovered,
        "the face came back as a different face after the drawing moved"
    );
}

/// A click on the drawing over a face takes the drawing, not the face.
///
/// The rule the surface rank exists for: every stroke and marker bounding a
/// face lies *within* it, so a face that ranked with them would swallow every
/// click meant for its own boundary.
#[test]
fn what_is_drawn_on_a_face_takes_the_click_over_it() {
    let mut raised = RaisedView::new();
    raised.frame();

    // A point of the demo's frame, which sits on the rectangle's corner — so
    // the face and the marker are both under this cursor.
    let corner = raised.cursor_on(
        raised
            .drawing()
            .plane()
            .point(DVec2::new(8.0, 5.0))
            .as_vec3(),
    );
    raised.harness.move_to(corner);
    raised.frame();
    assert!(
        matches!(
            raised.view.hovered(),
            Some(Part::Entity {
                entity: Entity::Point(_) | Entity::Segment(_),
                ..
            })
        ),
        "a face took a cursor over the drawing: {:?}",
        raised.view.hovered()
    );
}

/// **A double-click and a press mean something over a dimension and nothing
/// over anything else.**
///
/// What decides whether either gesture finds a number at all — the half of each
/// that can be asked without a painted frame. A relation states no number —
/// perpendicular, parallel, equal — so there is nothing to type into one and
/// nothing to drag, and neither is there for a point or an edge.
///
/// Both in one sweep, because they are one question asked of one fixture: which
/// of the demo's relations has a number, and does each gesture agree. Apart,
/// they were the same walk of the same constraints written twice, and the way
/// that goes wrong is one of them being taught about a new kind of dimension and
/// the other not.
///
/// The other half of each, that the gesture reaches the mark, needs the mark
/// measured, and only a paint measures one — see
/// [`Text::extent`](aperture::Text).
#[test]
fn a_dimension_is_the_only_relation_a_double_click_or_a_press_finds() {
    let raised = RaisedView::new();
    let sketch = raised.editing();
    let drawing = raised.document.drawn(sketch);

    let mut dimensions = 0;
    let mut relations = 0;
    for (id, constraint) in drawing.sketch().constraints() {
        let part = Part::Entity {
            sketch,
            entity: id.into(),
        };
        let opened = dimension(part, &raised.document);
        let held = label(part, Some(drawing), Some(sketch));
        match constraint.value() {
            Some(states) => {
                dimensions += 1;
                assert_eq!(
                    opened.expect("a dimension has a number to type into"),
                    Opening::Dimension { part, from: states },
                    "the form would open on the wrong dimension or value"
                );
                assert_eq!(held, Some(id), "a number could not be taken hold of");
            }
            None => {
                relations += 1;
                assert!(opened.is_none(), "a relation offered a number to type");
                assert_eq!(held, None, "a symbol offered itself to be dragged");
            }
        }
    }
    assert!(
        dimensions > 0 && relations > 0,
        "the demo states only one kind, so this asked half a question"
    );

    // And nothing that is not a constraint at all.
    let (point, _) = drawing
        .sketch()
        .points()
        .next()
        .expect("the demo draws points");
    let marker = Part::Entity {
        sketch,
        entity: point.into(),
    };
    assert!(dimension(marker, &raised.document).is_none());
    assert_eq!(label(marker, Some(drawing), Some(sketch)), None);

    // A press refuses a number of a sketch you are not in, where the
    // double-click above does not — and the difference is what each gesture
    // *does*. Moving one is an edit, and an edit lands where you are; opening a
    // form over one only reads it.
    let elsewhere = raised
        .document
        .models(&raised.build, Some(sketch))
        .iter()
        .map(|model| model.of())
        .find(|&at| at != sketch)
        .expect("the demo draws two sketches");
    let (borrowed, _) = drawing
        .sketch()
        .constraints()
        .find(|(_, constraint)| constraint.value().is_some())
        .expect("the demo states a dimension");
    assert_eq!(
        label(
            Part::Entity {
                sketch: elsewhere,
                entity: borrowed.into(),
            },
            Some(drawing),
            Some(sketch),
        ),
        None,
        "a number of a sketch nobody is in offered itself to be dragged"
    );
}

/// Hovering a plane's square reports that plane and lights it without taking
/// its colour away.
///
/// The look every other part takes replaces the colour outright, which for a
/// plane erases the one thing it is saying: which of the three the world comes
/// with it is. They are told apart by hue and nothing else — there is no shape
/// to tell a Front from a Side — so a highlight that spent that colour would
/// leave three identical squares crossing at the origin.
#[test]
fn hovering_a_plane_lights_it_without_recolouring_it() {
    let mut raised = RaisedView::new();
    raised.frame();

    // Aimed at geometry that was actually drawn rather than at coordinates
    // worked out here: a point along one edge of the square, a little in from
    // the corner so a neighbouring stroke cannot claim it.
    // Found by what it names rather than by the width it is stroked at, which is
    // also the tag the hover below is weighed against: the same batch carries a
    // dimension's rule and the arrow that grows a solid.
    let (on_edge, drawn) = {
        let pane = raised.view.pane();
        let square = pane
            .scene
            .gizmos
            .iter()
            .find(|gizmo| {
                let named = gizmo.tag.and_then(|tag| raised.view.part(tag));
                matches!(named, Some(Part::Step(_)))
            })
            .expect("a plane shows a square that answers for it");
        let corners = &square.points;
        (
            corners[0].lerp(corners[1], 0.4),
            square.tag.expect("found by the part its tag names"),
        )
    };

    raised.harness.move_to(raised.cursor_on(on_edge));
    raised.frame();
    let hovered = raised.view.hovered();
    assert!(
        matches!(hovered, Some(Part::Step(_))),
        "the cursor on a plane's square reported {hovered:?}"
    );

    // The square it landed on, and nothing else: one plane is one stroke now,
    // where a datum used to be four pieces that had to light together.
    let lit: Vec<_> = raised.view.lit().iter().map(|lit| lit.tag).collect();
    assert_eq!(lit, [drawn], "hovering a plane lit {} strokes", lit.len());
    // And it keeps its own colour, brightened. `Tint::Ink` here would be the
    // hover's yellow, which is also how it would look if the three planes had
    // stopped being told apart.
    for entry in raised.view.lit() {
        assert!(
            matches!(entry.look.tint, aperture::Tint::Lift(by) if by > 1.0),
            "a plane was lit with {:?}, which spends the colour it is made of",
            entry.look.tint,
        );
    }
}

/// A click leaves nothing in hand, so the pointer goes on lighting what it
/// crosses afterwards.
///
/// The gesture a press settles is read for as long as the button is down, and
/// what it holds is what gets lit — deliberately, so a drag lights the thing in
/// hand rather than whatever it passes over. But a press that never travels is
/// a *click*, and a click latches no drag, so the `Drag::Stopped` that ends a
/// gesture never arrives for one. Left unended, the first click on anything
/// grabbable pinned the highlight to it and every later hover answered with the
/// part still notionally in hand.
#[test]
fn a_click_ends_the_gesture_so_hover_goes_on_working_after_it() {
    let mut raised = RaisedView::new();
    raised.frame();
    let over_point = raised
        .over(|grip| matches!(grip, Grip::Point(_)))
        .expect("the demo draws a point that can be grabbed");
    let over_rim = raised
        .over(|grip| matches!(grip, Grip::Rim(_)))
        .expect("the demo draws a circle");

    // Pressed and released across two frames, the way a pointer does it.
    // `click_at` queues both into one, where the press edge and the release
    // collapse and no gesture is ever settled — so it cannot see this.
    raised.harness.press_at(over_point);
    raised.frame();
    raised.harness.release();
    raised.frame();
    let point = raised.named_at(over_point).expect("a point is there");
    assert_eq!(
        raised.view.hovered(),
        Some(raised.part(point)),
        "the click left the pointer over what it landed on",
    );

    // Moving off it and onto something else lights the something else.
    raised.harness.move_to(over_rim);
    raised.frame();
    let rim = raised.named_at(over_rim).expect("a circle is there");
    assert_eq!(
        raised.view.hovered(),
        Some(raised.part(rim)),
        "the clicked part stayed lit, so the click never let go of it",
    );

    // And moving off the drawing lights nothing, rather than the part the
    // gesture is still holding.
    raised.harness.move_to(Vec2::new(
        HARNESS_SIZE.x as f32 - 1.0,
        HARNESS_SIZE.y as f32 - 1.0,
    ));
    raised.frame();
    assert_eq!(
        raised.view.hovered(),
        None,
        "something stayed lit off the drawing"
    );
}

/// A tool still in hand refuses every grab, dimensions included — which is why
/// stating one leaves no number on the drawing movable until it is put down.
///
/// Placing a dimension re-arms the tool rather than putting it away, so the
/// press that would take hold of a *number* — the one thing a press can find
/// that is not geometry — settles as an orbit like every other press does while
/// a tool is up. Pinned because the two rules are written apart: the re-arm is
/// the dimension click's and the refusal is [`Gesture::grab`]'s, and neither
/// mentions the other.
#[test]
fn a_tool_in_hand_refuses_to_take_hold_of_a_dimension() {
    let mut raised = RaisedView::new();
    raised.frame();
    let sketch = raised.editing();
    let (id, _) = raised
        .document
        .drawn(sketch)
        .sketch()
        .constraints()
        .find(|(_, c)| c.value().is_some())
        .expect("the demo states a dimension");
    let part = Part::Entity {
        sketch,
        entity: id.into(),
    };
    let drawing = raised.document.drawn(sketch);

    // With the pointer, that number is a handle.
    assert_eq!(
        label(part, Some(drawing), Some(sketch)),
        Some(id),
        "a dimension the demo stated is not a handle at all",
    );

    // Stating one leaves the tool up, and a tool that is up grabs nothing.
    raised.hold(Tool::Dimension(Dimensioning::Empty));
    assert_ne!(
        raised.session.tool(),
        Tool::Pointer,
        "the fixture put the tool down, so this asks nothing",
    );
    assert!(
        !matches!(
            Gesture::grab(
                Some(crate::scene_view::aimed::Aimed::at(glam::Vec2::ZERO)),
                Some(raised.lens()),
                &raised.view.picture,
                &raised.document,
                &raised.session,
            ),
            Gesture::Move(_)
        ),
        "a tool in hand took hold of something, so the refusal has moved",
    );
}
