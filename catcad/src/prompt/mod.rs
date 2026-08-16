//! A value asked for against the drawing: where the form stands, and what
//! committing it asks the document for.
//!
//! **Session state, and scratch.** Nothing here is written down by saving, none
//! of it is a step to take back, and a form abandoned is a form that never
//! happened — the document is not touched until the value is committed. That
//! puts it on the same footing as the rubber band a half-drawn line follows.
//!
//! **What is left after palantir.** Where the caret is, what is picked out,
//! what a keystroke does to either, and every rule about clicking into a line
//! belong to the [`TextEdit`] a field is shown through — and *fitting the form
//! on screen* belongs to [`Popup`], which resolves a body against an anchor and
//! flips or shifts it to stay on the surface. What is left is the two things
//! neither can know: where in the world the form is about, and what pressing
//! Enter means.

use aperture::{Camera, Viewport};
use glam::{Vec2, Vec3};
use palantir::{
    Align, Button, ButtonTheme, ClickOutside, Configure, Panel, Popup, Rect, Sizing, Text,
    TextEdit, TextEditTheme, TextRun, TextWrap, Ui, WidgetId,
};
use silverpoint::Entity;
use std::fmt::Write;

use crate::intent::{Change, Choice, Intents, Step};
use crate::paint::{DECIMALS, Growing, mark_font, mark_lift};
use crate::part::Part;
use crate::timeline::FeatureId;

pub(crate) mod look;

/// What a form is about, and so what committing it asks for.
///
/// One arm per operation, which is the same shape [`Tool`](crate::tool::Tool)
/// and [`Change`] already take. It is what turns "the user pressed Enter" into
/// something the document understands, and there is no way to know that without
/// knowing which operation is in hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Asking {
    /// A sketch dimension being restated.
    Dimension { part: Part },
    /// A solid being grown off a region, *before* it reaches the timeline.
    ///
    /// Held here rather than created at zero and carried, because a
    /// [`Prism`](silverpoint::Prism) is a reading rather than a thing the
    /// document holds — an arrangement, a region, a plane and a distance, all
    /// four of which this has without a step existing. So the solid can be
    /// drawn while it is still being decided, the document is not touched until
    /// confirm, and cancelling is dropping this rather than unpicking a step
    /// that was already taken.
    Extrude { sketch: FeatureId, region: usize },
}

/// Where a form stands relative to what it is about.
///
/// **The distinction the whole module turns on.** A dimension's field stands
/// *over* its mark — the drawing leaves the mark out and the field takes its
/// place, and a number that shifted as it became editable would be exactly the
/// jump the alignment below exists to prevent. Everything else stands *beside*
/// what it is about, because the point of an extrude's form is that you can
/// still see the face.
///
/// Collapsing the two would either cover the geometry a form is asking about or
/// make an editable number jump, so the two placements are named rather than
/// approximated by one.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Stands {
    /// Exactly where the drawing would have put what the form replaces.
    Over(Vec2),
    /// Clear of a footprint, so what the form is about stays visible under it.
    Beside(Rect),
}

/// The screen rectangle `at` covers, or `None` where the projection draws none
/// of it.
///
/// Points the projection drops are skipped and the rest still answer: a face
/// half off screen still has somewhere to put a form. All of them dropped is
/// `None`, and the form is not shown for the frame — not *closed*, because a
/// camera turning back brings it round again and geometry swinging out of view
/// is nobody asking to stop typing.
pub(crate) fn footprint(
    at: impl Iterator<Item = Vec3>,
    camera: &Camera,
    viewport: Viewport,
) -> Option<Rect> {
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    let mut drawn = false;
    for corner in at.filter_map(|corner| camera.screen_of(corner, viewport)) {
        low = low.min(corner);
        high = high.max(corner);
        drawn = true;
    }
    drawn.then(|| Rect::new(low.x, low.y, high.x - low.x, high.y - low.y))
}

/// One value being typed, and what to call it where two are asked for at once.
#[derive(Debug)]
struct Field {
    /// Empty where the form asks for one thing and standing over it says which
    /// — a dimension's field is the number, and a word beside it would be a
    /// word the drawing never had.
    label: &'static str,
    /// The buffer [`TextEdit`] borrows, so the widget writes it and this reads
    /// it. Seeded when the form opens and never re-seeded: a draft is what the
    /// *user* has made of the value, and geometry that moved under it says
    /// nothing about what they meant to type.
    draft: String,
}

/// A form open against the drawing: what it is about, and what has been typed.
#[derive(Debug)]
pub(crate) struct Prompt {
    about: Asking,
    /// One per value asked for. A rectangle wants two and everything built so
    /// far wants one, which is why this is a list rather than a field.
    fields: Vec<Field>,
    /// Whether the form has been drawn yet.
    ///
    /// What decides the one frame it asks for focus. A form opens *between* the
    /// asking half of a frame and the next one, so the frame it is shown on is
    /// never the frame it was opened on, and nothing outside it is in a
    /// position to say which one that was.
    shown: bool,
    /// What its fields are set in. Built once per opening rather than per frame
    /// — it is the stock theme with one axis rewritten, and rebuilding it sixty
    /// times a second to say the same thing would be sixty copies of a bundle
    /// that never changes.
    look: TextEditTheme,
    /// The two answers, on the same terms. Built for every form rather than
    /// only the ones that show them: two bundles per *opening* is nothing, and
    /// an `Option` here would be a second thing saying what
    /// [`Prompt::answered`] already says.
    goes: ButtonTheme,
    stops: ButtonTheme,
}

/// What a field made of the frame.
///
/// Named rather than three bools in a row, which is what it was: they are all
/// one type, so any two could change places and still compile — and what the
/// three come to is a *cancel* or a *commit*, which is exactly the kind of
/// mistake nothing downstream would look wrong for.
///
/// Accumulated across a row with [`Said::and`], because Enter in *any* field
/// commits the form: a second value is another thing to say about one
/// operation, not a second operation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Said {
    cancelled: bool,
    lost_focus: bool,
    submitted: bool,
}

impl Said {
    /// What a [`TextEdit`] reported.
    fn of(shown: &palantir::TextEditResponse) -> Self {
        Self {
            cancelled: shown.cancelled,
            lost_focus: shown.lost_focus,
            submitted: shown.submitted,
        }
    }

    /// This and `other` together, which is what a row of fields says.
    fn and(self, other: Self) -> Self {
        Self {
            cancelled: self.cancelled || other.cancelled,
            lost_focus: self.lost_focus || other.lost_focus,
            submitted: self.submitted || other.submitted,
        }
    }
}

/// What a frame of the open form asked of the session, beyond editing a draft.
///
/// Private, and a measure of how little of a form is this crate's: the widgets
/// report `submitted`, `cancelled` and `lost_focus`, and turning those into an
/// answer about a *dimension* or an *extrude* is the whole of what is left.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Done {
    /// Put what the form says on the document.
    Commit,
    /// Leave the document as it was.
    Cancel,
}

impl Prompt {
    /// Open a form for `about`, seeded with `values`.
    pub(crate) fn on(about: Asking, values: &[(&'static str, f64)]) -> Self {
        Self {
            about,
            fields: values
                .iter()
                .map(|&(label, value)| Field {
                    label,
                    draft: format!("{value:.*}", DECIMALS),
                })
                .collect(),
            shown: false,
            look: look::field(),
            goes: look::confirm(),
            stops: look::cancel(),
        }
    }

    /// What the form is about.
    pub(crate) fn about(&self) -> Asking {
        self.about
    }

    /// The dimension being restated, where that is what this is about.
    ///
    /// What the drawing skips when it lays its marks out — the field is drawn
    /// where the mark would be, and both at once would be one on top of the
    /// other.
    pub(crate) fn marks(&self) -> Option<Part> {
        match self.about {
            Asking::Dimension { part } => Some(part),
            Asking::Extrude { .. } => None,
        }
    }

    /// The solid this form is deciding, as the drawing should show it now.
    ///
    /// `None` for a form about anything else, and for one whose depth does not
    /// read as a number yet — a half-typed "1." is not a depth, and a solid
    /// that flickered away between the digits of one would be worse than a
    /// solid that waits.
    pub(crate) fn growing(&self) -> Option<Growing> {
        let Asking::Extrude { sketch, region } = self.about else {
            return None;
        };
        Some(Growing {
            sketch,
            region,
            distance: self.value(0)?,
        })
    }

    /// What the `nth` field says as a number, or `None` where it says something
    /// that is not one.
    ///
    /// A `Result` would be the wrong shape: text that is not a number is not a
    /// failure to report but a draft that is not finished, and what a commit
    /// does with one is refuse to commit. Untrusted input in the sense the
    /// error-handling rules mean it — a person typing — but the only thing that
    /// can be said about it is whether it parses.
    ///
    /// Where a formula goes. Today a field holds a literal and this is
    /// [`str::parse`]; when it holds an expression this is where it is
    /// evaluated, and every caller already treats `None` as "not yet".
    pub(crate) fn value(&self, nth: usize) -> Option<f64> {
        self.fields.get(nth)?.draft.trim().parse().ok()
    }

    /// Put `to` in the `nth` field, as the drawing's own handle for it moved.
    ///
    /// Written over the draft rather than beside it, so what the field shows and
    /// what the gesture said are one number: a drag and the keyboard are two
    /// ways of saying the same thing, and a form that kept them apart would have
    /// to decide which one Enter meant.
    pub(crate) fn write(&mut self, nth: usize, to: f64) {
        let Some(field) = self.fields.get_mut(nth) else {
            return;
        };
        field.draft.clear();
        let _ = write!(field.draft, "{to:.*}", DECIMALS);
    }

    /// Show the form, and put what it asked for in `intents`.
    ///
    /// Shows and does not act, like every other control the application draws:
    /// what a field says reaches the document as a [`Change`], and closing the
    /// form is a [`Choice::Ask`].
    pub(crate) fn show(&mut self, ui: &mut Ui, stands: Stands, intents: &mut Intents) {
        // Taken after the caller has established there is somewhere to stand,
        // so a form the projection never drew has not used up the one frame
        // that takes focus.
        let opening = !std::mem::replace(&mut self.shown, true);
        let done = match stands {
            Stands::Over(screen) => self.over(ui, screen, opening),
            Stands::Beside(anchor) => self.beside(ui, anchor, opening),
        };
        // Outside the bodies, because a value is read off a draft the widget
        // has only just finished writing.
        match done {
            // A draft that is not a number is not finished, so Enter on one is
            // not a refusal to report but a key that does nothing. The form
            // stays open, which is what says so.
            Some(Done::Commit) => self.commit(intents),
            Some(Done::Cancel) => intents.push(Choice::Ask(None)),
            None => {}
        }
    }

    /// Whether losing focus is this form's cancel.
    ///
    /// It is where clicking away is the only way out, and it is *not* where the
    /// form carries its own buttons: an extrude's depth is dragged by an arrow
    /// in the drawing, and a form that threw itself away when the pointer went
    /// to that arrow would be one you could never drag.
    fn blurs(&self) -> bool {
        match self.about {
            Asking::Dimension { .. } => true,
            Asking::Extrude { .. } => false,
        }
    }

    /// What the widget's three signals come to, given how this form is
    /// dismissed.
    ///
    /// **Cancel is tested before submit**, which is what a commit-on-blur form
    /// owes itself: Escape blurs as well as cancelling, so asking about focus
    /// first could not tell a cancel from a click away — and where it blurs the
    /// two mean the same thing, which is why they share an arm.
    fn resolve(&self, said: Said) -> Option<Done> {
        if said.cancelled || (said.lost_focus && self.blurs()) {
            Some(Done::Cancel)
        } else if said.submitted {
            Some(Done::Commit)
        } else {
            None
        }
    }

    /// Ask the document for what the form says, and for the form to close.
    fn commit(&self, intents: &mut Intents) {
        match self.about {
            Asking::Dimension { part } => {
                let Some(to) = self.value(0) else {
                    return;
                };
                let (Some(sketch), Some(Entity::Constraint(constraint))) =
                    (part.sketch(), part.entity())
                else {
                    unreachable!("a dimension form is only ever opened over one");
                };
                intents.push(Change::Resize {
                    sketch,
                    constraint,
                    to,
                });
                // One gesture, one step to take back — the same signal a
                // scrub's release gives, because `Resize` coalesces and this is
                // what closes the run of them.
                intents.push(Step::Release);
            }
            Asking::Extrude { sketch, region } => {
                let Some(distance) = self.value(0) else {
                    return;
                };
                // The one step the whole operation costs. The solid has been on
                // screen since the form opened, drawn from this form's own
                // reading — see [`Asking::Extrude`] — so what reaches the
                // timeline is the depth that was settled on rather than a zero
                // that was then carried.
                intents.push(Change::Extrude {
                    sketch,
                    region,
                    distance,
                });
            }
        }
        intents.push(Choice::Ask(None));
    }

    /// What a field is recorded under.
    ///
    /// **Named rather than derived from the call site**, and the reason is when
    /// focus has to be asked for. A form that has just opened *is* the thing
    /// being typed into, so it has to come up focused — and focus is read at
    /// the top of the widget's own pass. An `auto_id` is only knowable from the
    /// response, which is a pass too late: the field would paint once with no
    /// caret and nothing picked out, and — since changing focus asks for no
    /// frame of its own — stay that way until some unrelated event woke one.
    ///
    /// A session has one form open at a time, so a name and the field's place
    /// in it are between them enough to tell any two apart.
    fn field_id(nth: usize) -> WidgetId {
        WidgetId::from_hash(("catcad.prompt.field", nth))
    }

    /// The number as a field will shape it.
    ///
    /// Built off the same face the field is styled with, so what this measures
    /// is what that draws. Unbounded and single-line: a dimension is one run
    /// that the box is sized to rather than the other way about.
    fn run(&self, nth: usize) -> TextRun<'_> {
        let font = mark_font();
        TextRun {
            text: &self.fields[nth].draft,
            font_size_px: font.size_px,
            line_height_px: font.line_height_px,
            wrap: TextWrap::Scroll,
            align: Align::CENTER,
            family: font.family,
            weight: font.weight,
            max_width_px: None,
        }
    }
}

impl Prompt {
    /// Whether the form carries its own confirm and cancel.
    ///
    /// The same question [`Prompt::blurs`] answers from the other side, and
    /// deliberately its opposite: a form is dismissed by its buttons or by
    /// clicking away, never by both and never by neither.
    fn answered(&self) -> bool {
        !self.blurs()
    }

    /// One field standing exactly where the drawing would have put the mark it
    /// replaces.
    ///
    /// A `Canvas` filling the view, so the field's position is measured from
    /// the same corner the projection answers in. It senses nothing, so every
    /// press it does not contain falls through to the viewport beneath — which
    /// is what makes clicking away from the field a click on the drawing as
    /// well as a blur.
    fn over(&mut self, ui: &mut Ui, screen: Vec2, opening: bool) -> Option<Done> {
        // Measured before the field is shown, because where its corner goes
        // depends on how wide its number comes out. The same shaper the widget
        // itself will use, asked the same question, so the two cannot answer
        // differently.
        let width = ui.probe_text(self.run(0)).size().w;
        let origin = self.placed_over(screen, width);
        let Self { fields, look, .. } = self;
        let id = Self::field_id(0);
        let said = Panel::canvas()
            .id_salt("prompt.over")
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                // **Before the field is shown, not after**, so that the pass
                // recording it is the pass that reads the focus — see
                // [`Prompt::field_id`]. Only on the frame it opens: after that
                // focus is palantir's, and losing it is how clicking away
                // cancels.
                if opening {
                    ui.request_focus(Some(id));
                }
                let shown = TextEdit::new(&mut fields[0].draft)
                    .id(id)
                    .style(look)
                    .select_all_on_focus()
                    .text_align(Align::CENTER)
                    .size((Sizing::HUG, Sizing::HUG))
                    .position(origin)
                    .show(ui);
                Said::of(&shown)
            })
            .inner;
        self.resolve(said)
    }

    /// Every field and both answers, in a row standing clear of `anchor`.
    ///
    /// [`Popup`] does the placing: it resolves the row against the anchor from
    /// its own measured size and flips or shifts it to stay on the surface, so
    /// a face near the bottom of the view gets its form above instead. Nothing
    /// here measures anything.
    ///
    /// [`ClickOutside::PassThrough`] because this annotates rather than
    /// interrupts. A modal popup installs a click-eater over the whole surface
    /// and claims the keyboard for every layer below, which for a form whose
    /// value is *also* dragged by an arrow in the drawing would mean a form you
    /// could never drag against.
    fn beside(&mut self, ui: &mut Ui, anchor: Rect, opening: bool) -> Option<Done> {
        let answerable = self.answered();
        let Self {
            fields,
            look,
            goes,
            stops,
            ..
        } = self;
        let mut said = Said::default();
        let mut answered = None;
        Popup::below(anchor)
            .id_salt("prompt.beside")
            .click_outside(ClickOutside::PassThrough)
            .show(ui, |ui, _| {
                if opening {
                    ui.request_focus(Some(Self::field_id(0)));
                }
                Panel::hstack().id_salt("row").show(ui, |ui| {
                    for (nth, field) in fields.iter_mut().enumerate() {
                        if !field.label.is_empty() {
                            Text::new(field.label).id_salt(field.label).show(ui);
                        }
                        let shown = TextEdit::new(&mut field.draft)
                            .id(Self::field_id(nth))
                            .style(look)
                            .select_all_on_focus()
                            .text_align(Align::CENTER)
                            .size((Sizing::HUG, Sizing::HUG))
                            .show(ui);
                        said = said.and(Said::of(&shown));
                    }
                    // Only where this is how the form is dismissed. A form that
                    // blurs shut has no use for them, and two buttons that were
                    // not the way out would be two buttons lying about it.
                    if answerable {
                        for (glyph, theme, answer) in [
                            (look::CONFIRM, &*goes, Done::Commit),
                            (look::CANCEL, &*stops, Done::Cancel),
                        ] {
                            let pressed = Button::new()
                                .id_salt(glyph)
                                .label(glyph)
                                .style(theme)
                                .show(ui)
                                .left
                                .clicked();
                            if pressed {
                                answered = Some(answer);
                            }
                        }
                    }
                });
            });
        // The buttons first: a press on one is an answer whatever the fields
        // made of the same frame, and a field that lost focus *to* the button
        // must not cancel out from under it.
        answered.or_else(|| self.resolve(said))
    }

    /// Where to put a field's own corner so that its glyphs land exactly where
    /// the mark's did.
    ///
    /// **What keeps a number from jumping as it becomes editable.** The mark
    /// hangs its text off the dimension by its own anchor; the field hangs its
    /// text off its corner by everything it puts in between — the ring it is
    /// drawn with, the padding inside that, and the caret's room at the
    /// trailing edge. So the corner is the mark's text origin with all of that
    /// backed off.
    ///
    /// Read off the theme this module authored rather than written out again,
    /// so a box restyled in [`look::field`] moves the field with it instead of
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
        Vec2::new(
            screen.x - width * 0.5 - inset.x - self.look.caret_width * 0.5,
            screen.y - mark_lift() - inset.y,
        )
    }
}

/// What a harness measuring the open form reaches past it for.
///
/// A field's id is the form's own business — it is how focus is asked for
/// before the widget reads it — and no part of the application needs to know
/// one. A test does: it finds where the box was laid out to check it stands
/// where the mark did.
///
/// Gated on `test` alone rather than on `internals` beside it: the one caller
/// is a unit test, and the wider gate would leave this dead in every build that
/// turned the feature on without turning tests on.
#[cfg(test)]
pub(crate) mod internals {
    use palantir::WidgetId;

    use crate::prompt::Prompt;

    impl Prompt {
        /// What the `nth` field is recorded under.
        pub(crate) fn nth_field_id(nth: usize) -> WidgetId {
            Self::field_id(nth)
        }
    }
}

#[cfg(test)]
mod tests;
