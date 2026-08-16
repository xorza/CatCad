//! A dimension opened for typing, and the field it is typed into.

use aperture::{Camera, Viewport};
use glam::{Vec2, Vec3};
use palantir::{
    Align, Configure, Palette, Panel, Sizing, StatefulLook, TextEdit, TextEditTheme, TextRun,
    TextStyle, TextWrap, Ui, WidgetId,
};
use silverpoint::Entity;

use crate::intent::{Change, Choice, Intents, Step};
use crate::paint::{DECIMALS, mark_font, mark_lift};
use crate::part::Part;

/// A dimension being retyped: which one, and what has been typed so far.
///
/// **Session state, and scratch.** Nothing here is written down by saving, none
/// of it is a step to take back, and a draft abandoned is a draft that never
/// happened — the dimension is not touched until Enter. That puts it on the same
/// footing as the rubber band a half-drawn line follows, which is likewise kept
/// rather than raised as an intent.
///
/// **A string and a [`Part`], and nothing else.** Where the caret is, what is
/// picked out, what a keystroke does to either, and every rule about clicking
/// into a line belong to the [`TextEdit`] this is shown through. Palantir owns
/// the pointer and the keyboard, and a field that is one of its widgets sits
/// *inside* the routing that decides who gets a press — so none of it has to be
/// arbitrated here. What this holds is the two things palantir cannot know:
/// which dimension is being retyped, and where in the world to put the box.
#[derive(Debug)]
pub(crate) struct Typing {
    /// The dimension being retyped.
    ///
    /// What the drawing skips when it lays its marks out — the field is drawn
    /// where the mark would be, and both at once would be one on top of the
    /// other — and what a commit names.
    part: Part,
    /// What has been typed.
    ///
    /// The buffer [`TextEdit`] borrows, so the widget writes it and this reads
    /// it. Seeded from what the dimension says when the field opens and never
    /// re-seeded: a draft is what the *user* has made of the value, and a
    /// dimension that moved under it says nothing about what they meant to type.
    draft: String,
    /// Whether the field has been drawn yet.
    ///
    /// What decides the one frame it asks for focus. A field opens *between*
    /// the asking half of a frame and the next one — the double-click raises
    /// the intent, the session lands it afterwards — so the frame it is shown
    /// on is never the frame it was opened on, and nothing outside it is in a
    /// position to say which one that was.
    shown: bool,
    /// What the field is set in.
    ///
    /// Built once per opening rather than per frame, on the same terms as the
    /// toolbar's armed button: it is the stock theme with one axis rewritten,
    /// and rebuilding it sixty times a second to say the same thing would be
    /// sixty copies of a bundle that never changes.
    look: TextEditTheme,
}

/// What the field is recorded under.
///
/// **Named rather than derived from the call site**, and the reason is when
/// focus has to be asked for. A field that has just opened *is* the thing being
/// typed into, so it has to come up focused — and focus is read at the top of
/// the widget's own pass. An `auto_id` is only knowable from the response, which
/// is a pass too late: the field would paint once with no caret and nothing
/// picked out, and — since changing focus asks for no frame of its own — stay
/// that way until some unrelated event woke one. Named here, the focus is set
/// before the field reads it and the first frame is already the focused one.
///
/// A verbatim id and not a scoped one, which is honest about what it is: a
/// session has one dimension open for typing at a time, so the field is a
/// singleton and a stable name is what it deserves.
pub(crate) fn field_id() -> WidgetId {
    WidgetId::from_hash("catcad.typing.field")
}

/// The stock field, set in the face a dimension's mark is set in.
///
/// **Mono**, and for the reason [`mark_font`] gives rather than for a matching
/// one: a value being retyped is read digit by digit, and a proportional face
/// sets `1` narrower than `8`, so a number shifts under the caret as it is
/// typed. The mark it stands over is mono already, so this is also what keeps a
/// number the same shape whether it is being edited or merely shown.
///
/// The size and weight come from the same call for the same reason. Nothing
/// else is touched: the box, the caret and the wash are palantir's, and a field
/// in the drawing has no cause to look unlike a field anywhere else.
fn field_look() -> TextEditTheme {
    let font = mark_font();
    let text = TextStyle {
        font_size_px: font.size_px,
        family: font.family,
        weight: font.weight,
        // The mark's leading, not the stock one. A field is the same string in
        // the same face as the mark it stands over, so a different line box
        // would set the glyphs at a different height inside it — the number
        // would drop as it became editable — and would make the box taller than
        // the line it holds for no reason anyone could see.
        line_height_mult: font.line_height_px / font.size_px,
        ..TextStyle::default()
    };
    let mut theme = TextEditTheme::from_palette(&Palette::DEFAULT);
    // Destructured rather than swept, so a fifth state added to a look is a
    // compile error here rather than one state quietly left in the wrong face.
    let StatefulLook {
        normal,
        hovered,
        active,
        disabled,
    } = &mut theme.looks;
    for state in [normal, hovered, active, disabled] {
        state.text = Some(text.clone());
    }
    theme
}

/// What a frame of the open field asked of the session, beyond editing the
/// draft.
///
/// Private, and a measure of how little of a field is this crate's: the widget
/// reports `submitted`, `cancelled` and `lost_focus`, and turning those three
/// into an answer about a *dimension* is the whole of what is left to do.
///
/// A verdict handed *out* of the closure rather than intents raised inside it,
/// because what a commit needs — the draft, read back after the widget has
/// written it — is only reachable through [`Typing::value`], and the field is
/// shown with the draft borrowed away from the rest of the type.
#[derive(Debug)]
enum Done {
    /// Put what the field says on the dimension.
    Commit,
    /// Leave the dimension as it was.
    Cancel,
}

impl Typing {
    /// Open `part` at `value`.
    ///
    /// Nothing is said here about the caret or what is picked out. The field
    /// takes focus the frame it opens and
    /// [`select_all_on_focus`](TextEdit::select_all_on_focus) picks the value
    /// out whole, which is what a dimension wants: it is retyped far more often
    /// than amended, so the first digit typed should replace the number rather
    /// than land in the middle of it.
    pub(crate) fn on(part: Part, value: f64) -> Self {
        Self {
            part,
            draft: format!("{value:.*}", DECIMALS),
            shown: false,
            look: field_look(),
        }
    }

    /// The dimension being retyped.
    pub(crate) fn part(&self) -> Part {
        self.part
    }

    /// What the field says as a number, or `None` where it says something that
    /// is not one.
    ///
    /// A `Result` would be the wrong shape: text that is not a number is not a
    /// failure to report but a draft that is not finished, and what a commit
    /// does with one is refuse to commit. Untrusted input in the sense the
    /// error-handling rules mean it — a person typing — but the only thing that
    /// can be said about it is whether it parses.
    ///
    /// Where a formula goes. Today the field holds a literal and this is
    /// [`str::parse`]; when it holds an expression this is where it is
    /// evaluated, and every caller already treats `None` as "not yet".
    pub(crate) fn value(&self) -> Option<f64> {
        self.draft.trim().parse().ok()
    }

    /// Show the field over the dimension, and put what it asked for in
    /// `intents`.
    ///
    /// Shows and does not act, like every other control the application draws:
    /// what the field says reaches the document as a [`Change::Resize`], and
    /// closing it is a [`Choice::Type`].
    ///
    /// **Placed by projecting the mark**, so the box stands over the number it
    /// is about however the camera is turned. `None` from
    /// [`Camera::screen_of`] is a dimension the projection does not draw —
    /// behind the eye, or outside an orthographic slab — and there the field
    /// simply is not shown for the frame. It is not *closed*: the camera turning
    /// back brings it round again, and a dimension swinging off screen is not
    /// somebody asking to stop typing into it.
    ///
    /// **Focused on the frame it first appears**, which is what puts it straight
    /// into typing rather than leaving a box nothing is aimed at — and, with
    /// `select_all_on_focus`, what makes the first digit typed replace the value
    /// rather than land in the middle of it. Asked for before the field is
    /// shown, so it costs no second frame; see [`field_id`].
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        at: Vec3,
        camera: &Camera,
        viewport: Viewport,
        intents: &mut Intents,
    ) {
        let Some(screen) = camera.screen_of(at, viewport) else {
            return;
        };
        // After the projection, so a dimension that was never drawn has not
        // used up the one frame that takes focus.
        let opening = !std::mem::replace(&mut self.shown, true);
        // Measured before the field is shown, because where its corner goes
        // depends on how wide its number comes out — see [`Typing::placed_over`].
        // The same shaper the widget itself will use, asked the same question,
        // so the two cannot answer differently.
        let run = self.run();
        let width = ui.probe_text(run).size().w;
        let origin = self.placed_over(screen, width);
        // Borrowed apart before the closure, which would otherwise take the
        // whole of `self` and hand the field a buffer and a theme off the same
        // borrow.
        let Self { draft, look, .. } = self;
        let id = field_id();
        // A `Canvas` filling the view, so the field's position is measured from
        // the same corner the projection answers in. It senses nothing, so every
        // press it does not contain falls through to the viewport beneath —
        // which is what makes clicking away from the field a click on the
        // drawing as well as a blur.
        let done = Panel::canvas()
            .id_salt("typing")
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                // **Before the field is shown, not after**, so that the pass
                // recording it is the pass that reads the focus — see
                // [`field_id`]. Only on the frame it opens: after that focus is
                // palantir's, and losing it is how clicking away cancels.
                if opening {
                    ui.request_focus(Some(id));
                }
                let field = TextEdit::new(draft)
                    .id(id)
                    .style(look)
                    .select_all_on_focus()
                    .text_align(Align::CENTER)
                    .size((Sizing::HUG, Sizing::HUG))
                    .position(origin)
                    .show(ui);
                // **Cancel is tested first**, which is what a commit-on-blur
                // caller owes itself: Escape blurs as well as cancelling, so
                // asking about focus first could not tell a cancel from a click
                // away — and here the two mean the same thing, which is why they
                // share an arm.
                if field.cancelled || field.lost_focus {
                    Some(Done::Cancel)
                } else if field.submitted {
                    Some(Done::Commit)
                } else {
                    None
                }
            })
            .inner;
        // Outside the closure, because the value is read off the draft the field
        // has only just finished writing.
        match done {
            // A draft that is not a number is not finished, so Enter on one is
            // not a refusal to report but a key that does nothing — see
            // [`Typing::value`]. The field stays open, which is what says so.
            Some(Done::Commit) => {
                if let Some(to) = self.value() {
                    self.commit(to, intents);
                }
            }
            Some(Done::Cancel) => intents.push(Choice::Type(None)),
            None => {}
        }
    }

    /// Where to put the field's own corner so that its glyphs land exactly
    /// where the mark's did.
    ///
    /// **What keeps a number from jumping as it becomes editable.** The mark
    /// hangs its text off the dimension by [`MARK_ANCHOR`]; the field hangs its
    /// text off its own corner by everything it puts in between — the ring it
    /// is drawn with, the padding inside that, and the caret's room at the
    /// trailing edge. So the corner is the mark's text origin with all of that
    /// backed off.
    ///
    /// Read off the theme this type authored rather than written out again, so
    /// a box restyled in [`field_look`] moves the field with it instead of
    /// leaving this behind.
    ///
    /// The text's own width cancels: the field centres its text in the same
    /// inner rect the corner is measured from, and the mark centres its text on
    /// the dimension, so both land half a run to the left of the same point
    /// whatever the run measures. Only the caret's room breaks the symmetry,
    /// being reserved on one side.
    fn placed_over(&self, screen: Vec2, width: f32) -> Vec2 {
        // Off `normal`, and safe to be: palantir pins the ring's width across
        // every state so that focus changes its colour without moving the inner
        // rect — which is the same shift this call exists to avoid, one state
        // further along.
        let ring = self
            .look
            .looks
            .normal
            .background
            .as_ref()
            .map_or(0.0, |bg| bg.stroke.width);
        let inset = Vec2::new(self.look.padding.horiz(), self.look.padding.vert()) * 0.5
            + Vec2::splat(ring);
        // The mark centres its number on the dimension, so the field's own text
        // has to start half a run to the left of the same point — and the box
        // starts a further inset out, plus the half-caret the field keeps its
        // hugged text clear of the trailing edge by.
        Vec2::new(
            screen.x - width * 0.5 - inset.x - self.look.caret_width * 0.5,
            screen.y - mark_lift() - inset.y,
        )
    }

    /// The number as the field will shape it.
    ///
    /// Built off the same theme the field is styled with, so what this measures
    /// is what that draws. Unbounded and single-line: a dimension is one run
    /// that the box is sized to rather than the other way about.
    fn run(&self) -> TextRun<'_> {
        let font = mark_font();
        TextRun {
            text: &self.draft,
            font_size_px: font.size_px,
            line_height_px: font.line_height_px,
            wrap: TextWrap::Scroll,
            align: Align::CENTER,
            family: font.family,
            weight: font.weight,
            max_width_px: None,
        }
    }

    /// Ask for the dimension to be restated at `to`, and for the field to close.
    fn commit(&self, to: f64, intents: &mut Intents) {
        let (Some(sketch), Some(Entity::Constraint(constraint))) =
            (self.part.sketch(), self.part.entity())
        else {
            unreachable!("a field is only ever opened over a dimension");
        };
        intents.push(Change::Resize {
            sketch,
            constraint,
            to,
        });
        // One gesture, one step to take back — the same signal a scrub's release
        // gives, because `Resize` coalesces and this is what closes the run of
        // them.
        intents.push(Step::Release);
        intents.push(Choice::Type(None));
    }
}

#[cfg(test)]
mod tests;
