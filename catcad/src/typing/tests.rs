use super::*;
use glam::DVec2;
use palantir::{KeyPress, Modifiers, TextChunk};
use silverpoint::{Constraint, Sketch};

use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature};

/// A dimension to open a field over.
///
/// Which one is never read below — every test here is about the line being
/// typed rather than about the drawing — but it is a real handle out of a real
/// sketch all the same, because a [`Part`] has no other way to be made and a
/// stand-in would be pinning nothing.
fn dimension() -> Part {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let mut sketch = Sketch::default();
    let from = sketch.add_point(DVec2::ZERO);
    let to = sketch.add_point(DVec2::X);
    let span = sketch.add_constraint(Constraint::Distance {
        a: from,
        b: to,
        distance: 1.0,
    });
    let at = timeline.add(Feature::Sketch { on: ground, sketch });
    Part::Entity {
        sketch: at,
        entity: span.into(),
    }
}

/// One key going down, with no modifiers held.
fn press(key: Key) -> KeyboardEvent {
    KeyboardEvent::Down(KeyPress {
        key,
        mods: Modifiers::NONE,
        repeat: false,
        physical: key,
    })
}

/// The same, with a modifier.
fn chord(key: Key, mods: Modifiers) -> KeyboardEvent {
    KeyboardEvent::Down(KeyPress {
        key,
        mods,
        repeat: false,
        physical: key,
    })
}

/// A character arriving the way one really does from a window: as a key going
/// down, with the logical key already post-shift.
///
/// *Not* [`committed`] beside it. Which of the two a character comes through is
/// the thing worth keeping straight here — a field that took only the other one
/// would look right in every test written with it and take nothing at all from
/// a keyboard.
fn typed(ch: char) -> KeyboardEvent {
    press(Key::Char(ch))
}

/// The other path: what an input method hands over once it has composed.
fn committed(text: &str) -> KeyboardEvent {
    KeyboardEvent::Text(TextChunk::new(text).expect("short enough to be inline"))
}

/// A field opens showing what the dimension says, picked out whole, so the
/// first character typed replaces it.
///
/// The selection is the half that matters. Retyping a dimension is far more
/// common than amending one, and a field that opened with the caret at the end
/// would make "40" out of "125.4" and "40" rather than out of "40".
#[test]
fn a_field_opens_on_the_value_and_replaces_it_when_typed_over() {
    let mut typing = Typing::on(dimension(), 125.4);
    assert_eq!(typing.field().content(), "125.40", "two decimal places");
    assert_eq!(typing.field().selected(), "125.40", "not picked out whole");
    assert_eq!(typing.value(), Some(125.4));

    assert_eq!(typing.take(&typed('4')), None);
    assert_eq!(typing.take(&typed('0')), None);
    assert_eq!(typing.field().content(), "40");
    assert_eq!(typing.value(), Some(40.0));

    // The other way a character arrives, which an input method uses and a
    // paste will. Both have to insert: a field that took only the composed one
    // would take nothing at all from a keyboard, and one that took only the key
    // would drop everything an IME produced.
    assert_eq!(typing.take(&committed(".5")), None);
    assert_eq!(typing.field().content(), "40.5");
    assert_eq!(typing.value(), Some(40.5));
}

/// Enter commits, Escape cancels, and neither types a character.
///
/// The second half is what tells committed text from a key going down: Enter
/// produces a `Down` and no `Text`, so a field reading only the text stream
/// would never see it and one reading both would put a newline in.
#[test]
fn enter_commits_and_escape_cancels() {
    let mut typing = Typing::on(dimension(), 10.0);
    assert_eq!(typing.take(&press(Key::Enter)), Some(Done::Commit));
    assert_eq!(typing.field().content(), "10.00", "Enter typed something");

    let mut typing = Typing::on(dimension(), 10.0);
    assert_eq!(typing.take(&press(Key::Escape)), Some(Done::Cancel));
    assert_eq!(typing.field().content(), "10.00", "Escape typed something");
}

/// Every key that edits the line counts as an edit, and every one that does not
/// leaves the count where it was.
///
/// The count is what the picture watches to know the field is worth writing
/// again — see [`Typing::revision`]. One left uncounted is a keystroke that
/// does not appear on screen until some unrelated thing redraws, which reads as
/// a dropped key.
#[test]
fn every_edit_is_counted_and_nothing_else_is() {
    let mut typing = Typing::on(dimension(), 12.0);
    let mut counted = typing.revision();
    let mut edits = |typing: &mut Typing, event, expected: &str| {
        typing.take(&event);
        assert!(typing.revision() > counted, "{expected}: uncounted");
        counted = typing.revision();
        assert_eq!(typing.field().content(), expected);
    };
    // Typing over the selection, then each way of moving and deleting.
    edits(&mut typing, typed('8'), "8");
    edits(&mut typing, typed('5'), "85");
    edits(&mut typing, press(Key::ArrowLeft), "85");
    edits(&mut typing, press(Key::Delete), "8");
    edits(&mut typing, press(Key::End), "8");
    edits(&mut typing, press(Key::Backspace), "");
    edits(&mut typing, typed('7'), "7");
    edits(&mut typing, press(Key::Home), "7");
    edits(&mut typing, press(Key::ArrowRight), "7");

    // And the ones that are not edits. Enter and Escape are answered rather
    // than typed, a key the field has no use for is ignored, and a command
    // chord belongs to the application.
    let at = typing.revision();
    for event in [
        press(Key::Enter),
        press(Key::Escape),
        press(Key::Tab),
        chord(
            Key::Char('s'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        ),
    ] {
        typing.take(&event);
    }
    assert_eq!(typing.revision(), at, "something uncountable was counted");
    assert_eq!(typing.field().content(), "7");
}

/// Shift with an arrow takes the run it passes over; without it, the caret
/// moves alone.
#[test]
fn shift_with_an_arrow_picks_out_what_it_passes() {
    let mut typing = Typing::on(dimension(), 42.0);
    typing.take(&press(Key::End));
    assert_eq!(typing.field().selected(), "");

    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    typing.take(&chord(Key::ArrowLeft, shift));
    typing.take(&chord(Key::ArrowLeft, shift));
    assert_eq!(typing.field().selected(), "00");

    // And the whole line, which is the chord every field binds.
    typing.take(&chord(
        Key::Char('a'),
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    ));
    assert_eq!(typing.field().selected(), "42.00");
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
    let mut typing = Typing::on(dimension(), 1.0);
    for (content, value) in [
        ("2.5", Some(2.5)),
        // Surrounding space is a typo, not a refusal.
        ("  7 ", Some(7.0)),
        ("-3", Some(-3.0)),
        ("", None),
        ("1.", Some(1.0)),
        (".", None),
        ("12mm", None),
        ("2+3", None),
    ] {
        typing.take(&chord(
            Key::Char('a'),
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        ));
        typing.take(&press(Key::Backspace));
        for ch in content.chars() {
            typing.take(&typed(ch));
        }
        assert_eq!(typing.field().content(), content);
        assert_eq!(typing.value(), value, "{content:?}");
    }
}
