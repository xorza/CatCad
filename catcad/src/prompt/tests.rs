use super::*;
use glam::DVec2;
use silverpoint::{Along, Constraint, Dimension, Sketch};

use crate::profile::Profile;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature};

/// A dimension to open a form over.
///
/// Which one is never read below — every test here is about the draft rather
/// than about the drawing — but it is a real handle out of a real sketch all the
/// same, because a [`Part`] has no other way to be made and a stand-in would be
/// pinning nothing.
fn dimension() -> Part {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let mut sketch = Sketch::default();
    let from = sketch.add_point(DVec2::ZERO);
    let to = sketch.add_point(DVec2::X);
    let span = sketch.add_constraint(Constraint::Distance {
        a: from,
        b: to,
        along: Along::Shortest,
        dimension: Dimension::new(1.0),
    });
    let at = timeline.add(Feature::Sketch { on: ground, sketch });
    Part::Entity {
        sketch: at,
        entity: span.into(),
    }
}

/// A form growing a solid, seeded the way one is opened.
///
/// A real name out of a real sketch, though which region it names is never
/// resolved by anything below — resolving one wants an arrangement, and what
/// these tests ask a form is what it *says*. Beside [`dimension`] because the
/// two are the pair every test here needs: the kind of form that is dismissed
/// by clicking away, and the kind that carries its own answers.
fn grown() -> Prompt {
    let Part::Entity { sketch, .. } = dimension() else {
        unreachable!("the fixture is a dimension of a sketch");
    };
    Prompt::on(
        Asking::Extrude {
            profile: Profile::new(sketch, Vec::new()),
        },
        &[("Depth", Seed::Offered(0.0))],
    )
}

/// A form opens showing what the dimension says, to the places a dimension is
/// read out to.
///
/// **Nothing here is about editing**, and that is the whole shape of this type:
/// what a keystroke does to a line, where the caret goes, what a click picks
/// out — all of it belongs to the palantir field this is shown through, and is
/// pinned there. What is left to say is that the draft starts where the
/// dimension is and reads back as a number.
#[test]
fn a_form_opens_on_the_value_the_dimension_states() {
    let part = dimension();
    let prompt = Prompt::on(Asking::Dimension { part }, &[("", Seed::Stated(125.4))]);
    assert_eq!(prompt.marks(), Some(part));
    assert_eq!(
        prompt.fields[0].draft, "125.40",
        "opened on some other value"
    );
    assert_eq!(prompt.value(0), Some(125.4));
}

/// A draft that is not a number has no value to commit, and says so without
/// refusing anything.
///
/// What a half-typed field looks like — "1." on the way to "1.5" — and what an
/// expression will look like before it parses. The caller reads `None` as "not
/// yet" and leaves the field open, which is why this is an `Option` rather than
/// an error.
#[test]
fn a_draft_that_is_not_a_number_has_no_value() {
    let mut prompt = Prompt::on(
        Asking::Dimension { part: dimension() },
        &[("", Seed::Stated(12.0))],
    );
    for (draft, value) in [
        ("40", Some(40.0)),
        ("  40.5 ", Some(40.5)),
        ("-3", Some(-3.0)),
        ("1.", Some(1.0)),
        ("", None),
        (".", None),
        ("4 0", None),
        ("40mm", None),
    ] {
        prompt.fields[0].draft.clear();
        prompt.fields[0].draft.push_str(draft);
        assert_eq!(prompt.value(0), value, "{draft:?}");
    }
}

/// **A form hands over the depth it shows, never one of its own.**
///
/// [`Prompt::carrying`] is read at the press that grabs the depth arrow and
/// [`Prompt::says`] at the commit, on different schedules — so a `carrying`
/// that substituted a value where the draft said nothing would have the arrow
/// travelling against a depth the solid was never drawn at.
///
/// Which is also what the sweep pins about the *placeholder*: where the draft
/// does not read as a number the seed speaks, and it has to speak to both. A
/// field opened on an offer is never short of an answer, so there is no draft
/// at all for which this reports nothing — see [`Seed::Offered`].
#[test]
fn a_form_hands_over_the_depth_it_shows() {
    let mut grown = grown();
    for (draft, says) in [
        // Nobody has typed, so the placeholder the form opened on speaks — a
        // solid at no depth rather than no solid. See [`Seed::Offered`].
        ("", Some(0.0)),
        ("3.5", Some(3.5)),
        // Signed, which is how a solid is flipped to the other side of its
        // plane.
        ("-2", Some(-2.0)),
        ("1.", Some(1.0)),
        // Nothing a number can be read out of, so the placeholder speaks again
        // rather than the solid vanishing between two keystrokes.
        (".", Some(0.0)),
    ] {
        grown.fields[0].draft.clear();
        grown.fields[0].draft.push_str(draft);
        assert_eq!(grown.says(0), says, "{draft:?}");
        assert_eq!(
            grown.carrying().map(|carrying| carrying.depth),
            says,
            "the depth a press reads is not the one the form says, at {draft:?}"
        );
    }
}

/// **A form is dismissed by its buttons or by clicking away, never by both.**
///
/// The one rule that differs between the two kinds, and it has to: an extrude's
/// depth is dragged by an arrow in the drawing, so a form that threw itself
/// away when the pointer went to that arrow would be one you could never drag
/// against. A dimension has no such handle, and clicking away *is* how you say
/// you are done with it.
#[test]
fn a_form_with_answers_is_not_dismissed_by_losing_focus() {
    let typed = Prompt::on(
        Asking::Dimension { part: dimension() },
        &[("", Seed::Stated(1.0))],
    );
    assert!(typed.blurs(), "a dimension form has no other way out");
    assert_eq!(
        typed.resolve(Said {
            lost_focus: true,
            ..Said::default()
        }),
        Some(Done::Cancel)
    );

    let grown = grown();
    assert!(!grown.blurs(), "an extrude form carries its own answers");
    assert_eq!(
        grown.resolve(Said {
            lost_focus: true,
            ..Said::default()
        }),
        None,
        "clicking towards the drag arrow threw the form away"
    );
    // Escape still cancels it, and Enter still commits — losing focus is the
    // only signal the two kinds read differently.
    assert_eq!(
        grown.resolve(Said {
            cancelled: true,
            ..Said::default()
        }),
        Some(Done::Cancel)
    );
    assert_eq!(
        grown.resolve(Said {
            submitted: true,
            ..Said::default()
        }),
        Some(Done::Commit)
    );
}

/// **Both answers have a glyph to draw them.**
///
/// The failure this guards is silent and total, and it is the one the
/// constraint marks are guarded against for the same reason: a character the
/// fonts lack rasterizes to nothing, so the button is laid out, painted and
/// clickable, and reads as a blank block of colour. Nothing else notices.
///
/// The size a button sets its label at rather than the mark's, because that is
/// what these are drawn as — a glyph missing at one size and present at another
/// is not a thing, but reading it off the wrong style would still be asking a
/// question nobody has.
#[test]
fn both_answers_have_a_glyph_to_draw_them() {
    let shaper = palantir::TextShaper::new();
    let mut glyphs = shaper.glyphs();
    let mut placed = Vec::new();

    for answer in [look::CONFIRM, look::CANCEL] {
        glyphs.line(answer, crate::paint::mark_font(), 1.0, &mut placed);
        let [glyph] = placed[..] else {
            panic!("{answer:?} shaped to {} glyphs", placed.len());
        };
        let image = glyphs
            .rasterize(glyph.raster_key)
            .unwrap_or_else(|| panic!("{answer:?} has no glyph"));
        assert!(
            image.placement.width > 0 && image.placement.height > 0,
            "{answer:?} rasterized to nothing, so the button draws blank",
        );
    }
}

/// **What a form is about, and what it stands over, are the same answer but
/// once.**
///
/// [`Prompt::holds`] is what a prune reads — a form open over geometry an undo
/// took away has to close rather than commit onto a handle naming nothing — and
/// [`Prompt::marks`] is what the drawing reads, leaving out the mark the field
/// is drawn in place of. For every form they agree, and a **radius** is the one
/// they do not: it is about a circle the document can lose, and it stands over
/// no mark, because that circle has no radius to draw until this form gives it
/// one.
///
/// The pair are one match and one arm now rather than two matches, so this is
/// what says the arm is still there — both ways round, since collapsing them
/// either way leaves every other form answering as it did.
#[test]
fn a_radius_form_is_about_a_circle_it_stands_over_no_mark_of() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    let hub = sketch.add_circle(middle, 2.0);
    let at = timeline.add(Feature::Sketch { on: ground, sketch });

    let radius = Prompt::on(
        Asking::Radius {
            sketch: at,
            circle: hub,
        },
        &[("Radius", Seed::Stated(2.0))],
    );
    assert_eq!(
        radius.holds(),
        Some(Part::Entity {
            sketch: at,
            entity: hub.into(),
        }),
        "a radius form that is about nothing would outlive its own circle"
    );
    assert_eq!(
        radius.marks(),
        None,
        "the drawing left out a mark for a radius the circle does not have yet"
    );

    // And the form they agree about, which is what says the difference above is
    // the radius rather than the two questions being unrelated.
    let part = dimension();
    let typed = Prompt::on(Asking::Dimension { part }, &[("", Seed::Stated(1.0))]);
    assert_eq!(typed.holds(), Some(part));
    assert_eq!(typed.marks(), Some(part));

    // A circle still being drawn names nothing the document holds at all, which
    // is the point of it — there is no circle until the form commits.
    let drawing = Prompt::on(
        Asking::Circle {
            sketch: at,
            center: crate::drawing::anchor::Anchor::On(middle),
        },
        &[("Radius", Seed::Offered(0.0))],
    );
    assert_eq!(drawing.holds(), None);
    assert_eq!(drawing.marks(), None);
}
