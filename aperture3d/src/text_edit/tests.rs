use super::*;
use crate::batch::Batch;
use crate::camera::{Camera, Projection};
use crate::viewport::Viewport;
use glam::UVec2;
use palantir::TextShaper;

/// Looking straight down −Z from 5 away with a 90° fov, so a 100×100 viewport
/// puts the origin dead centre — the same fixture every other picking test in
/// the crate aims through.
fn head_on() -> Camera {
    Camera {
        target: Vec3::ZERO,
        distance: 5.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: std::f32::consts::FRAC_PI_2,
        near_ratio: 1.0 / 5.0,
        projection: Projection::Perspective,
    }
}

const CENTRE: Vec2 = Vec2::new(50.0, 50.0);

fn aim_at(cursor: Vec2, radius: f32) -> Aim {
    Aim::new(
        &head_on(),
        cursor,
        Viewport::new(UVec2::new(100, 100)),
        radius,
    )
}

/// A field of round numbers: ten pixels a character, four either side, and a
/// line twenty tall. Anchored at the world origin, so its box hangs off screen
/// centre.
///
/// Every measurement below is hand-computed against these, which is the point
/// of standing in for the shaper: a real face would hide an off-by-one inside
/// plausible fractions.
fn ten_px(content: &str) -> TextEdit {
    TextEdit {
        position: Vec3::ZERO,
        font: GlyphFont {
            line_height_px: 20.0,
            ..GlyphFont::new(20.0)
        },
        padding: Vec2::new(4.0, 3.0),
        min_width: 20.0,
        content: content.to_string(),
        caret: content.len(),
        mark: content.len(),
        ..TextEdit::default()
    }
    .measured(10.0, 20.0)
    .tagged(Tag::new(7))
}

/// Typing puts characters in, a selection is replaced whole, and both leave the
/// caret after what was written.
///
/// One test for the three ways text arrives — a keystroke, a paste, and an
/// input method's commit — because all three are [`TextEdit::insert`] and the
/// replace-the-selection rule is the one that would otherwise be written thrice.
#[test]
fn inserting_writes_over_whatever_is_picked_out() {
    let mut field = ten_px("125");
    assert_eq!(field.caret(), 3, "a new field carets at the end");
    assert_eq!(field.selection(), 3..3);

    field.insert(".4");
    assert_eq!(field.content(), "125.4");
    assert_eq!(field.caret(), 5);
    assert!(field.selected().is_empty(), "insertion left a selection");

    // Back over the ".4" and type across it.
    field.seek(Seek::Left, Selecting::Extend);
    field.seek(Seek::Left, Selecting::Extend);
    assert_eq!(field.selected(), ".4");
    field.insert("0");
    assert_eq!(field.content(), "1250");
    assert_eq!(field.caret(), 4);

    // The whole line, replaced — what typing over a seeded value does.
    field.select_all();
    assert_eq!(field.selection(), 0..4);
    field.insert("7");
    assert_eq!(field.content(), "7");
    assert_eq!(field.caret(), 1);

    // Inserting nothing over a selection is how the selection is dropped
    // without anything taking its place.
    field.select_all();
    field.insert("");
    assert_eq!(field.content(), "");
    assert_eq!(field.caret(), 0);
}

/// Both deletes take a character where nothing is picked out and the run where
/// something is, and neither runs off the end of the line.
#[test]
fn deleting_takes_a_character_or_the_run_picked_out() {
    let mut field = ten_px("125.4");
    field.delete_back();
    assert_eq!(field.content(), "125.");
    assert_eq!(field.caret(), 4);

    field.seek(Seek::Start, Selecting::Drop);
    field.delete_forward();
    assert_eq!(field.content(), "25.");
    assert_eq!(field.caret(), 0);

    // At either end there is nothing to take, and nothing happens — no panic,
    // and no caret off the line.
    field.delete_back();
    assert_eq!(field.content(), "25.");
    assert_eq!(field.caret(), 0);
    field.seek(Seek::End, Selecting::Drop);
    field.delete_forward();
    assert_eq!(field.content(), "25.");
    assert_eq!(field.caret(), 3);

    // A run picked out goes whole, whichever key asked.
    for delete in [TextEdit::delete_back, TextEdit::delete_forward] {
        let mut field = ten_px("125.4");
        field.seek(Seek::Byte(1), Selecting::Drop);
        field.seek(Seek::Byte(4), Selecting::Extend);
        assert_eq!(field.selected(), "25.");
        delete(&mut field);
        assert_eq!(field.content(), "14");
        assert_eq!(field.caret(), 1, "the caret is left where the run began");
        assert!(field.selected().is_empty());
    }
}

/// An arrow key with a run picked out collapses to the end it was moving
/// toward, and one without steps a character.
///
/// The collapse is the case worth pinning: stepping *past* the selection's end
/// is the plausible wrong answer, and it is what treating the caret as the only
/// state would give.
#[test]
fn a_step_collapses_a_selection_rather_than_passing_it() {
    let mut field = ten_px("125.4");
    field.seek(Seek::Byte(1), Selecting::Drop);
    field.seek(Seek::Byte(4), Selecting::Extend);
    assert_eq!(field.selection(), 1..4);

    field.seek(Seek::Left, Selecting::Drop);
    assert_eq!(field.caret(), 1, "left of a selection is its own start");
    assert_eq!(field.selection(), 1..1);

    field.seek(Seek::Byte(1), Selecting::Drop);
    field.seek(Seek::Byte(4), Selecting::Extend);
    field.seek(Seek::Right, Selecting::Drop);
    assert_eq!(field.caret(), 4, "right of a selection is its own end");

    // With shift it is an ordinary step, from wherever the caret is — which is
    // the moving end, not the low end.
    field.seek(Seek::Byte(1), Selecting::Drop);
    field.seek(Seek::Byte(4), Selecting::Extend);
    field.seek(Seek::Left, Selecting::Extend);
    assert_eq!(field.selection(), 1..3);
    assert_eq!(field.caret(), 3);

    // And a step off either end moves nothing.
    field.seek(Seek::Start, Selecting::Drop);
    field.seek(Seek::Left, Selecting::Drop);
    assert_eq!(field.caret(), 0);
    field.seek(Seek::End, Selecting::Drop);
    field.seek(Seek::Right, Selecting::Drop);
    assert_eq!(field.caret(), 5);
}

/// The caret steps whole characters and lands only on boundaries, however many
/// bytes one takes.
///
/// `°` is two bytes and `≤` is three, so a caret that counted bytes would stop
/// inside both — and slicing the content there is a panic rather than a wrong
/// answer, which is why this is pinned rather than left to the type system.
#[test]
fn a_caret_steps_characters_and_never_lands_inside_one() {
    // Bytes: "12.5" is 0..4, "°" is 4..6, "≤" is 6..9, "7" is 9..10.
    let mut field = ten_px("12.5°≤7");
    assert_eq!(field.content().len(), 10);

    field.seek(Seek::Start, Selecting::Drop);
    let mut stops = vec![0];
    while field.caret() < field.content().len() {
        field.seek(Seek::Right, Selecting::Drop);
        stops.push(field.caret());
    }
    assert_eq!(stops, [0, 1, 2, 3, 4, 6, 9, 10]);

    // And back the same way, which is what says the two directions agree.
    let mut back = vec![field.caret()];
    while field.caret() > 0 {
        field.seek(Seek::Left, Selecting::Drop);
        back.push(field.caret());
    }
    back.reverse();
    assert_eq!(back, stops);

    // A byte offset from anywhere — a click, a caller's own arithmetic — is
    // moved back to the boundary it falls in rather than trusted.
    for (asked, lands) in [(0, 0), (5, 4), (7, 6), (8, 6), (9, 9), (99, 10)] {
        field.seek(Seek::Byte(asked), Selecting::Drop);
        assert_eq!(field.caret(), lands, "byte {asked}");
    }
}

/// Re-seeding a field keeps the caret where it still fits and drops it back
/// where it does not.
///
/// What makes a field a *view* of the value it shows: the model writes it every
/// frame, and a caret sent home on each of those would make it impossible to
/// type in.
#[test]
fn re_seeding_keeps_the_caret_wherever_it_still_fits() {
    let mut field = ten_px("125.4");
    field.seek(Seek::Byte(2), Selecting::Drop);

    // The same text is not a write at all.
    field.set_content("125.4");
    assert_eq!(field.caret(), 2);

    // Longer, and the caret is untouched.
    field.set_content("1250.75");
    assert_eq!(field.content(), "1250.75");
    assert_eq!(field.caret(), 2);

    // Shorter than the caret, and it comes back to the end.
    field.set_content("1");
    assert_eq!(field.caret(), 1);

    // And onto a boundary, never inside a character: "°" is bytes 1..3, so a
    // caret at 2 has nowhere to be but 1.
    let mut field = ten_px("125.4");
    field.seek(Seek::Byte(2), Selecting::Drop);
    field.set_content("1°");
    assert_eq!(field.caret(), 1);
    // Sliced rather than merely compared, because landing inside a character
    // is a panic rather than a wrong number.
    assert_eq!(&field.content()[..field.caret()], "1");
}

/// The surround hangs off the text rather than the other way about, and the
/// caret and the wash are placed along the text.
///
/// Hand-computed against the ten-pixel characters the fixture stands in with:
/// "125" measures 30 by 20, so the text sits at the anchor and the surround
/// reaches 4 left and right of it and 3 above and below — 38 by 26, starting at
/// (−4, −3).
///
/// The sign is the point. Padding reaching *outward* is what keeps the text
/// where a label would have put it; padding measured inward from a box would
/// put these glyphs 4 across and 3 down of that.
#[test]
fn the_surround_hangs_off_the_text_without_moving_it() {
    let mut field = ten_px("125");
    assert_eq!(field.extent(), Vec2::new(30.0, 20.0), "the text alone");
    assert_eq!(field.surround(), Vec2::new(38.0, 26.0), "and the padding");

    field.seek(Seek::Byte(2), Selecting::Drop);
    let parts = field.parts();
    assert_eq!(parts.text, Vec2::ZERO, "the text moved off the anchor");
    assert_eq!(parts.surround, Rect::new(-4.0, -3.0, 38.0, 26.0));
    assert_eq!(parts.caret, Rect::new(20.0, 0.0, CARET_WIDTH, 20.0));
    assert_eq!(parts.wash, None, "nothing is picked out");

    // Picking out "25" washes from boundary 1 to boundary 3 — 10 across to 30 —
    // behind text that has not moved.
    field.seek(Seek::Byte(1), Selecting::Drop);
    field.seek(Seek::Byte(3), Selecting::Extend);
    let picked = field.parts();
    assert_eq!(picked.wash, Some(Rect::new(10.0, 0.0, 20.0, 20.0)));
    assert_eq!(picked.caret, Rect::new(30.0, 0.0, CARET_WIDTH, 20.0));
    assert_eq!(picked.text, parts.text, "picking a run out moved the text");

    // Emptied, the floor under the width is what is left, and it is taken up on
    // the right — so the text origin and the caret stay exactly where the first
    // character would have started.
    field.select_all();
    field.insert("");
    let empty = field.measured(10.0, 20.0);
    assert_eq!(empty.extent(), Vec2::new(0.0, 20.0));
    assert_eq!(empty.surround(), Vec2::new(28.0, 26.0), "the floor is 20");
    assert_eq!(empty.parts().text, Vec2::ZERO);
    assert_eq!(empty.parts().caret, Rect::new(0.0, 0.0, CARET_WIDTH, 20.0));
    assert_eq!(empty.parts().surround, Rect::new(-4.0, -3.0, 28.0, 26.0));
}

/// The anchor fraction moves the text off the position, and the surround
/// follows it.
#[test]
fn the_anchor_fraction_moves_the_box_off_the_position() {
    let field = ten_px("125").anchored(Vec2::splat(0.5));
    // The *text* is 30 by 20, so centring puts its top-left 15 left and 10 up
    // of the anchor — and the surround is that, out by the padding.
    let parts = field.parts();
    assert_eq!(parts.text, Vec2::new(-15.0, -10.0));
    assert_eq!(parts.surround, Rect::new(-19.0, -13.0, 38.0, 26.0));
    assert_eq!(field.position, Vec3::ZERO, "the anchor itself did not move");

    // And the box a click has to land in moves with it: centred, it spans
    // x 31..69 where anchored at the text's corner it spans 46..84.
    let left = aim_at(Vec2::new(35.0, 50.0), 0.0);
    assert!(field.pick(&left).is_some());
    assert!(ten_px("125").pick(&left).is_none());
}

/// A click anywhere in the box is a hit, and answers the boundary nearest where
/// it fell.
///
/// The nearest boundary rather than the character under the cursor is what a
/// caret does, and the half-way point is where the two answers differ — so the
/// cases either side of one are what this pins.
#[test]
fn a_click_lands_in_the_box_and_answers_the_nearest_boundary() {
    // Anchored at the *text's* top-left, so the text starts at screen centre
    // and the surround reaches back over the padding: x 46..84, y 47..73.
    let field = ten_px("125");

    let hit = field.pick(&aim_at(CENTRE, 0.0)).expect("its own corner");
    assert_eq!(hit.at, HitAt::Field);
    assert_eq!(hit.tag, Tag::new(7));
    assert_eq!(hit.screen, 0.0, "inside is no distance away");
    assert_eq!(hit.world, Vec3::ZERO);

    // Five past the right edge, level with the box: the gap to that edge, and
    // refused once the reach no longer covers it.
    let beside = Vec2::new(89.0, 60.0);
    assert_eq!(
        field.pick(&aim_at(beside, 10.0)).expect("in reach").screen,
        5.0
    );
    assert!(field.pick(&aim_at(beside, 4.0)).is_none());

    // Along the text: x 50 is boundary 0, and each character is 10 wide.
    for (x, byte) in [
        (30.0, 0), // left of the box entirely
        (50.0, 0), // the text's own left edge
        (54.0, 0), // inside the first character, nearer its start
        (56.0, 1), // past its middle, so the boundary after it
        (70.0, 2),
        (80.0, 3),
        (99.0, 3), // right of everything
    ] {
        let at = field.byte_at(&aim_at(Vec2::new(x, 60.0), 0.0));
        assert_eq!(at, Some(byte), "x {x}");
    }

    // Scenery answers no hit however well it was laid out — but it still says
    // where a cursor fell, because that is a question about the box and not
    // about whether anything was named.
    let mut scenery = field;
    scenery.tag = None;
    assert!(scenery.pick(&aim_at(CENTRE, 0.0)).is_none());
    assert_eq!(scenery.byte_at(&aim_at(CENTRE, 0.0)), Some(0));
}

/// A field behind the camera is not drawn, so neither picked nor clicked into.
#[test]
fn a_field_the_projection_drops_is_not_picked() {
    // Ten behind the eye, which sits five out along +Z looking at the origin.
    let mut behind = ten_px("125");
    behind.position = Vec3::new(0.0, 0.0, 15.0);
    assert!(behind.pick(&aim_at(CENTRE, 50.0)).is_none());
    assert_eq!(behind.byte_at(&aim_at(CENTRE, 50.0)), None);
}

/// A field beats every other kind of target, and an unfocused one beats them
/// just the same.
///
/// The rung is the point: a field is drawn opaque over whatever it covers, so a
/// marker underneath is a marker you cannot see — and a click that landed on
/// the box can have been meant for nothing else.
#[test]
fn a_field_outranks_every_other_kind_of_target() {
    let field = HitAt::Field.rank();
    let marker = HitAt::Point.rank();
    let label = HitAt::Text.rank();
    let edge = HitAt::Segment { index: 0, t: 0.5 }.rank();

    assert!(field < marker, "a field should beat a marker");
    assert!(marker < label, "a marker should still beat a label");
    assert!(label < edge, "a label should still beat an edge");
}

/// Laying a batch out fills in what a caret needs, and leaves the batch clean.
///
/// The clean half is the one that matters, and it is the same rule a label's
/// extent answers by: a renderer re-flattens whatever is marked, so a pass that
/// marked what it measured would ask to be run again on every frame forever.
#[test]
fn laying_a_batch_out_fills_the_stops_without_marking_it() {
    let mut fields = Batch::default();
    fields.push(TextEdit::new(Vec3::ZERO, "125.4", 16.0));
    fields.push(TextEdit::new(Vec3::ZERO, "", 16.0));
    // Pushing marked it, which is the mark a renderer takes when it flattens.
    assert!(fields.take_dirty());

    let shaper = TextShaper::new();
    measure_all(&fields, &mut shaper.glyphs());
    assert!(
        !fields.take_dirty(),
        "laying out asked to be laid out again"
    );

    // One stop per boundary, climbing left to right, starting at nothing.
    let stops = fields[0].stops.borrow().clone();
    assert_eq!(stops.len(), 6, "{stops:?}");
    assert_eq!(stops[0], Stop { byte: 0, x: 0.0 });
    for pair in stops.windows(2) {
        assert!(pair[0].byte < pair[1].byte, "{stops:?}");
        assert!(pair[0].x < pair[1].x, "{stops:?}");
    }
    // The last stop is exactly how far the text reaches, because the two come
    // of measuring the same string — and a caret at the end of the line sitting
    // anywhere but the end of the text is the failure that would hide here.
    let text = fields[0].extent();
    assert_eq!(stops.last().expect("a stop apiece").x, text.x);
    assert!(text.x > 0.0 && text.y > 0.0, "{text:?}");
    // The surround is that, out by the padding, and past its own floor.
    let surround = fields[0].surround();
    let padding = fields[0].padding;
    assert_eq!(surround, text + padding * 2.0);
    assert!(
        text.x > fields[0].min_width,
        "the floor bound a measured field"
    );

    // An empty field has the one boundary every field has, reaches nowhere, and
    // falls back on the floor and the font for a surround to stand up in.
    assert_eq!(*fields[1].stops.borrow(), [Stop { byte: 0, x: 0.0 }]);
    assert_eq!(
        fields[1].extent(),
        Vec2::new(0.0, fields[1].font.line_height_px)
    );
    assert_eq!(
        fields[1].surround(),
        Vec2::new(
            fields[1].min_width + fields[1].padding.x * 2.0,
            fields[1].font.line_height_px + fields[1].padding.y * 2.0,
        )
    );
}

/// A field laid out by a real shaper carets exactly where it was measured, and
/// a click at that spot answers the boundary it was placed at.
///
/// What ties the halves together: the stops a shaper hands back are both where
/// the caret is drawn and what a click is tested against, so the two cannot
/// come apart — clicking where the caret is leaves it there.
#[test]
fn a_click_at_the_caret_answers_the_boundary_it_is_at() {
    let mut fields = Batch::default();
    fields.push(TextEdit::new(Vec3::ZERO, "125.4", 16.0).tagged(Tag::new(7)));
    let shaper = TextShaper::new();
    measure_all(&fields, &mut shaper.glyphs());

    let field = &mut fields[0];
    for byte in [0, 1, 3, 5] {
        field.seek(Seek::Byte(byte), Selecting::Drop);
        let caret = field.parts().caret;
        // The caret is drawn from the anchor, which the field hangs at screen
        // centre, so where it lands on screen is that plus its own corner.
        let on_screen = CENTRE + caret.min + Vec2::new(CARET_WIDTH * 0.5, 1.0);
        let at = field.byte_at(&aim_at(on_screen, 0.0));
        assert_eq!(at, Some(byte), "caret at {byte} landed at {at:?}");
    }
}
