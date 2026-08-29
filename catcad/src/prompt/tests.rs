use super::*;
use crate::prompt::Form;
use crate::prompt::marked::internals::EVERY;
use glam::DVec2;
use silverpoint::{Along, Constraint, Dimension, Operation, Sketch};

use crate::profile::Profile;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature, World};

/// A dimension to open a form over.
///
/// Which one is never read below — every test here is about the draft rather
/// than about the drawing — but it is a real handle out of a real sketch all the
/// same, because a [`Part`] has no other way to be made and a stand-in would be
/// pinning nothing.
fn dimension() -> Part {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
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
        Form::default(),
        Asking::Extrude {
            profile: Profile::of(sketch, std::iter::empty()),
            operation: Operation::Join,
        },
        [Asks {
            label: "Depth",
            unit: "",
            seed: Seed::Offered(0.0),
        }],
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
    let prompt = Prompt::on(
        Form::default(),
        Asking::Dimension { part },
        [Asks {
            label: "",
            unit: "",
            seed: Seed::Stated(125.4),
        }],
    );
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
        Form::default(),
        Asking::Dimension { part: dimension() },
        [Asks {
            label: "",
            unit: "",
            seed: Seed::Stated(12.0),
        }],
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

/// **What a form means and what it draws come apart on exactly one draft**, and
/// each has to be asked by name.
///
/// [`Prompt::says`] is read at the commit and [`Prompt::shows`] by the drawing
/// — through [`Prompt::carrying`], which is what the depth arrow travels
/// against. They agree everywhere except a draft that is not a number:
///
/// - somebody typed one, so the draft speaks to both;
/// - nobody has typed, so the offer the form opened on speaks to both — the
///   number is on screen as the placeholder, and a form refusing to commit what
///   it is showing would be a form arguing with itself;
/// - somebody is *part way* through typing one. There is nothing to commit,
///   because the only number to hand is one nobody typed and the field is
///   showing something else. There is still something to draw, because a solid
///   blinking out between the two keystrokes of `-2` would be worse than one
///   that waits where the pointer left it.
///
/// The third is the whole reason there are two readings. A single one has to be
/// wrong about a commit or wrong about a frame.
#[test]
fn what_a_form_means_and_what_it_draws_come_apart_on_a_draft_mid_word() {
    let mut grown = grown();
    for (draft, says, shows) in [
        ("", Some(0.0), Some(0.0)),
        ("3.5", Some(3.5), Some(3.5)),
        // Signed, which is how a solid is flipped to the other side of its
        // plane — and `-` on its own is the draft the split is about.
        ("-2", Some(-2.0), Some(-2.0)),
        ("1.", Some(1.0), Some(1.0)),
        ("-", None, Some(0.0)),
        (".", None, Some(0.0)),
        ("40mm", None, Some(0.0)),
    ] {
        grown.fields[0].draft.clear();
        grown.fields[0].draft.push_str(draft);
        assert_eq!(grown.says(0), says, "committed, at {draft:?}");
        assert_eq!(grown.shows(0), shows, "drawn, at {draft:?}");
        assert_eq!(
            grown.carrying().map(|carrying| carrying.depth),
            shows,
            "the depth the arrow travels against is not the one drawn, at {draft:?}"
        );
    }

    // **And the offer is not read back where somebody has typed.** The pointer
    // moving over the drawing writes a suggestion whatever the field holds, so
    // a draft that means nothing must not quietly come to mean that instead —
    // which is the failure a fallback would reintroduce.
    grown.suggest(0, 7.0);
    grown.fields[0].draft.clear();
    grown.fields[0].draft.push_str("abc");
    assert_eq!(
        grown.says(0),
        None,
        "a commit read the pointer's own number"
    );
    assert_eq!(grown.shows(0), Some(7.0), "the drawing lost its depth");
}

/// **A form is dismissed by its chips or by clicking away, never by both.**
///
/// The one rule that differs between the two kinds, and it has to: an extrude's
/// depth is dragged by an arrow in the drawing, so a form that threw itself
/// away when the pointer went to that arrow would be one you could never drag
/// against. A dimension has no such handle, and clicking away *is* how you say
/// you are done with it.
#[test]
fn a_form_with_answers_is_not_dismissed_by_losing_focus() {
    let typed = Prompt::on(
        Form::default(),
        Asking::Dimension { part: dimension() },
        [Asks {
            label: "",
            unit: "",
            seed: Seed::Stated(1.0),
        }],
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

/// **Every mark on the form carries a word, and no two of them collide.**
///
/// What is left for this to say now that the marks are artwork rather than
/// characters. That the artwork *exists* is the icon table's own check —
/// `every_source_sits_at_its_own_glyph_and_paints_in_one_colour`, which walks
/// all of it rather than the eight rows a form draws.
///
/// The collisions are this table's alone, and both are silent. A form records a
/// chip under its glyph, so two of them sharing one would be two controls
/// sharing an id — the second would never be pressed. Two rows under one word
/// would be a tooltip saying the same thing twice, and two forms under one mark
/// would be two forms that look alike.
#[test]
fn every_mark_on_the_form_is_drawn_and_named() {
    for mark in EVERY {
        assert!(
            !mark.word.trim().is_empty(),
            "{mark:?} carries no word, so nothing on the form says what it is",
        );
    }

    for (at, one) in EVERY.iter().enumerate() {
        for two in &EVERY[at + 1..] {
            assert_ne!(one.glyph, two.glyph, "{one:?} and {two:?} share a mark");
            assert_ne!(one.word, two.word, "{one:?} and {two:?} share a word");
        }
    }

    // And the three the row is laid out from are three, which is what says the
    // pairing carries the operation rather than dropping it.
    let [joins, cuts, shares] =
        [Operation::Join, Operation::Cut, Operation::Intersect].map(marked::doing);
    assert_ne!(joins, cuts, "a join and a cut draw as one chip");
    assert_ne!(cuts, shares, "a cut and an intersect draw as one chip");
    assert_ne!(joins, shares, "a join and an intersect draw as one chip");
}

/// **What a form is about is what it stands over, asked once for both.**
///
/// A prune closes a form left over geometry an undo took away; the drawing
/// leaves out the mark the field is drawn in place of. One answer serves both
/// only while no form is about a part it does not also mark, so this is what
/// says the question never needs splitting in two again.
#[test]
fn a_form_is_about_exactly_the_mark_it_stands_over() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    let at = timeline.add(Feature::Sketch { on: ground, sketch });

    // A dimension being retyped stands over its own mark.
    let part = dimension();
    let typed = Prompt::on(
        Form::default(),
        Asking::Dimension { part },
        [Asks {
            label: "",
            unit: "",
            seed: Seed::Stated(1.0),
        }],
    );
    assert_eq!(typed.marks(), Some(part));

    // A circle still being drawn names nothing the document holds at all, which
    // is the point of it — there is no circle until the form commits.
    let drawing = Prompt::on(
        Form::default(),
        Asking::Circle {
            sketch: at,
            center: crate::drawing::anchor::Anchor::On(middle),
        },
        [Asks {
            label: "Radius",
            unit: "",
            seed: Seed::Offered(0.0),
        }],
    );
    assert_eq!(drawing.marks(), None);
}
