//! What floats over the viewport: five surfaces, one per edge and corner.

use aperture::Camera;
use palantir::{Align, Background, Configure, InternedStr, Panel, Sizing, Spacing, Ui, WidgetId};
use std::hash::Hash;

use crate::hud::cube::{Cube, Gizmo};
use crate::intent::{Choice, Intents};
use crate::look::Theme;
use crate::look::icons::Icons;
use crate::model::models::Models;
use crate::notation::Notation;
use crate::selection::Selection;
use crate::status::Solved;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use silverpoint::Constraint;

mod camera;
pub(crate) mod cube;
mod papers;
mod rail;
mod readout;
mod recipe;
mod relations;

/// A control's identity, stated rather than taken from the line it is written
/// on.
///
/// **What lets a harness find a control by name.** A press arrives at the
/// application as a cursor, and nothing in one can turn "the Line chip" into a
/// position — palantir places the row, and a widget's rect is the layout
/// engine's answer, a frame late. With a stated id, a test asks the frame where
/// the chip ended up instead of carrying a number that a changed row silently
/// invalidates.
///
/// Salted with the crate and the role as well as the key, so two surfaces are
/// free to use one word for two different controls.
fn control(role: &'static str, key: impl Hash) -> WidgetId {
    WidgetId::from_hash(("catcad.hud", role, key))
}

/// Everything drawn over the viewport, and the scratch two of its surfaces
/// share.
///
/// **Shows and does not act** — the whole of it. Every control reads app state
/// and asks for what it wants as an [`Intent`](crate::intent::Intent): one that
/// turned the camera itself would be one that had to be handed a camera, and
/// one that armed a tool itself would arm it and put it straight back down on a
/// replayed pass.
#[derive(Debug, Default)]
pub(crate) struct Hud {
    /// What the current selection admits, refilled every frame. Kept for its
    /// room rather than its contents: the record pass allocates nothing, and a
    /// bar rebuilt sixty times a second would otherwise ask the heap for a list
    /// each time.
    offers: Vec<Constraint>,
    /// What is picked out, sorted into what the bar can be asked about — kept
    /// across frames for its room, like the offers above it.
    picked: relations::Picked,
    /// What is picked out, as this frame has it.
    ///
    /// **The session's answer with everything asked for since laid over it**,
    /// and what every control here is handed in place of the session's own —
    /// see [`Hud::following`].
    ///
    /// Kept across frames for its room, like the sort beside it.
    picking: Selection,
    /// Every step a removal would take, while one is being offered and pointed
    /// at. Empty every other frame, and kept for its room like the two above.
    doomed: Vec<FeatureId>,
    /// The number the dimension field is showing, re-seeded from the drawing
    /// every frame and written over by the widget while it is being scrubbed.
    /// Scratch: what a dimension *is* lives in the sketch, and this is only what
    /// one gesture has made of it so far.
    draft: f64,
    /// The number the reach field is showing.
    ///
    /// **Remembered where the draft above is re-seeded**, and the difference is
    /// what each field is a view of: a dimension is in the drawing, and a blend
    /// still being offered is not in anything yet. So this keeps what it was
    /// last scrubbed to and a second blend opens at the first one's reach, and
    /// a blend already in the recipe overwrites it while it is picked out. See
    /// [`Reach`](relations::Reach), which is what says the field opens on a
    /// number the kernel will take.
    reach: relations::Reach,
    /// The view the orientation cube is on its way to, if it is on its way
    /// anywhere. The same kind of thing as the draft above: one gesture's
    /// worth of intent, where the camera itself is the document's.
    cube: Cube,
}

impl Hud {
    /// Show the whole of it, putting whatever a control asks for in `intents`.
    ///
    /// **Every surface states a width or is built out of chips**, and that is a
    /// rule rather than a taste. A surface is measured by the widest thing
    /// standing on it, the application root is floored by the widest surface,
    /// and the viewport fills what is left — so one run of text with nothing to
    /// stop it stretches the view past the window. A stretched view is a
    /// different projection, and the drawing is then picked where it is not
    /// drawn.
    ///
    /// What that cost was worth catching: a document saved to a long path made
    /// the status line wide enough to slide the tool bar out from under the
    /// pointer, so a click on Line armed nothing.
    pub(crate) fn show(&mut self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        self.following(shown.selection, intents);
        // **Read once, before either drawer.** The bar offers the removal and
        // the card wears what it would take, and the two are drawn a call
        // apart — so a card that asked for itself could show a cascade the bar
        // was no longer offering.
        //
        // Off the chip's own response, which is a frame behind like every
        // other: the wear and the chip's own highlight then arrive together.
        self.doomed.clear();
        if let Some(step) = self.picked.removable(shown.models)
            && ui
                .response_for(relations::relation_id(relations::REMOVE))
                .hovered
        {
            shown.models.doomed_at(step, &mut self.doomed);
        }
        let chrome = &shown.theme.chrome;
        let showing = shown.picking(&self.picking);
        // The document commands and the tools in one column rather than two
        // surfaces at one corner: pinned to the same edge, they would otherwise
        // be drawn over each other. The column carries the margin, so the two
        // pills on it take the gap between siblings and nothing else.
        Panel::vstack()
            .id_salt("left")
            .align(Align::TOP_LEFT)
            .margin(Spacing::all(chrome.inset))
            .size((Sizing::HUG, Sizing::HUG))
            .gap(chrome.gap)
            .background(Background::NONE)
            .show(ui, |ui| {
                papers::show(ui, showing, intents);
                rail::show(ui, showing, intents);
            });
        self.following(shown.selection, intents);
        recipe::show(ui, shown.picking(&self.picking), &self.doomed, intents);
        self.following(shown.selection, intents);
        camera::show(ui, &mut self.cube, shown.picking(&self.picking), intents);
        self.following(shown.selection, intents);
        readout::show(ui, shown.picking(&self.picking));
        relations::show(
            ui,
            shown.picking(&self.picking),
            &mut self.offers,
            &self.picked,
            &mut self.draft,
            &mut self.reach,
            intents,
        );
    }

    /// Take up everything asked for so far, so that whatever is drawn next is
    /// drawn from it.
    ///
    /// **Called before every control that can ask for anything.** A control
    /// raises an [`Intent`](crate::intent::Intent), and the session answers it
    /// at the frame's *second* apply — which is after every control has been
    /// drawn. So `session` is what was picked before this frame's click, and a
    /// control handed it is drawn as though the click had not happened: a
    /// recipe row that does not come up picked, and a bar that has not heard
    /// what the row picked.
    ///
    /// Three controls share the call before them rather than take one each:
    /// the left column's pair, which is drawn inside one closure, and the
    /// readout, which stands between the cube and the bar. None of the three
    /// picks anything.
    ///
    /// Laid over from the top each time rather than followed on from where the
    /// last call stopped, and that costs nothing a frame can measure: a frame
    /// asks for one thing or two. Every [`Choice`] here names the answer it
    /// wants rather than a move, so replaying the queue arrives where the
    /// session will arrive, by the steps the session will take.
    fn following(&mut self, session: &Selection, intents: &Intents) {
        self.picking.copy_from(session);
        for choice in intents.choices() {
            // Every arm spelled rather than a rest, so a [`Choice`] added
            // later cannot start saying what is picked without this being
            // asked to answer it as well. What the session does with these two
            // is the same call, and the two would otherwise part company in
            // silence.
            match *choice {
                Choice::Select(what) => self.picking.select(what),
                Choice::Include(what) => self.picking.include(what),
                Choice::Ask(_)
                | Choice::Set { .. }
                | Choice::Suggest { .. }
                | Choice::Close
                | Choice::Hold(_) => {}
            }
        }
        self.picked.sort(&self.picking);
    }

    /// Where the orientation gizmo landed and what the pointer is on it, for
    /// the picture that draws its pane.
    ///
    /// **Read after the overlay has recorded**, which is what makes the answer
    /// this frame's: the cube resolves the hover as it senses, and where its box
    /// sits is what the last arrangement put it.
    pub(crate) fn gizmo(&self) -> Gizmo<'_> {
        self.cube.gizmo()
    }
}

/// Everything the overlay reads to draw itself.
///
/// Gathered rather than passed one by one, because they arrive together and
/// mean one thing between them: this is the frame's state as the controls see
/// it. What is *not* here is the inbox — the overlay reads all of this and
/// writes none of it, and keeping the two apart at the signature is what says
/// so.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Shown<'a> {
    /// The artwork every surface draws its controls with.
    ///
    /// A resource among the state, because it is read exactly the way the rest
    /// of this is: taken up by the frame, handed down, and written by nobody.
    pub(crate) icons: &'a Icons,
    /// Every colour, weight and metric the overlay draws with.
    ///
    /// Beside the artwork rather than folded in with it, because the two are
    /// owned differently: a theme is data the application decides, and an icon
    /// set owns what the *host* has parsed and rasterized.
    pub(crate) theme: &'a Theme,
    /// What a number in the document means — see [`Notation`].
    ///
    /// Beside the theme rather than read off the models: how a drawing is
    /// *drawn* and how its numbers are *said* are both things the surfaces are
    /// handed, and neither is anything a sketch holds.
    pub(crate) notation: Notation,
    pub(crate) tool: Tool,
    /// Whatever the drawing has to say beyond how the solve went — see
    /// [`Status::rest`](crate::status::Status::rest).
    ///
    /// The tail rather than the whole line, because the readout draws the solve
    /// itself as fields: handed the sentence it would say the verdict, the
    /// degrees of freedom and the iterations twice over.
    ///
    /// Already in the pass's own text arena, so nothing here copies it — and it
    /// has to be lowered in the pass that minted it.
    pub(crate) rest: InternedStr,
    /// What the last solve made of the open sketch, where one is open.
    ///
    /// Beside the tail above rather than read out of it, because the two are
    /// read differently: that is a sentence and this is what the fields report.
    pub(crate) solved: Option<Solved>,
    /// Where the document is being looked at from.
    ///
    /// The whole camera rather than the projection alone, because the cube is
    /// drawn *in* it: which faces are turned toward the eye is what the gizmo
    /// is a picture of.
    pub(crate) camera: Camera,
    /// Every sketch the document holds, and which of them is open.
    ///
    /// The model rather than the drawing, because a control here reads what is
    /// *picked out* — and a part names the sketch it belongs to as well as the
    /// thing within it.
    pub(crate) models: Models<'a>,
    pub(crate) selection: &'a Selection,
}

impl<'a> Shown<'a> {
    /// This, with what the frame has picked in place of what the session still
    /// holds — see [`Hud::following`].
    fn picking(self, picked: &'a Selection) -> Self {
        Self {
            selection: picked,
            ..self
        }
    }
}

/// The ids a harness clicks the overlay through.
///
/// **Names rather than positions.** These used to be hand-measured pixel
/// centres — the middle of each chip, swept for and written down — and every
/// one of them moved whenever a row did. An id is stated by the control itself,
/// so a test resolves it against the frame that drew it and a layout change
/// stops being a test change. See [`control`].
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use palantir::WidgetId;

    use crate::hud::rail;
    #[cfg(test)]
    use crate::hud::{recipe, relations};
    #[cfg(test)]
    use crate::timeline::FeatureId;

    /// One tool on the rail, by the label it is named and captioned with.
    pub(crate) fn tool(label: &str) -> WidgetId {
        rail::tool_id(label)
    }

    /// One command on the relation bar, by its label.
    #[cfg(test)]
    pub(crate) fn relation(label: &str) -> WidgetId {
        relations::relation_id(label)
    }

    /// One row of the recipe, by the step it stands for.
    ///
    /// By the handle rather than by position, which is the same reason the row
    /// itself is: a test walks the recipe for the step it means and asks for
    /// that step's row, so a delete above it changes nothing here.
    #[cfg(test)]
    pub(crate) fn step(at: FeatureId) -> WidgetId {
        recipe::step_id(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::Part;

    /// **The overlay draws from the session's answer with this frame's asking
    /// laid over it.**
    ///
    /// The session does not move until the frame's second apply, which is
    /// after every control has been drawn — so a control drawn from it is
    /// drawn as though this frame's click had not happened.
    ///
    /// The session is empty throughout, so every answer below is the inbox's
    /// alone.
    #[test]
    fn the_overlay_reads_what_this_frame_asked_for_and_not_the_session() {
        let mut hud = Hud::default();
        let session = Selection::default();
        let mut intents = Intents::default();

        hud.following(&session, &intents);
        assert!(hud.picking.picked().is_empty(), "nothing was asked for");

        intents.push(Choice::Select(Some(Part::Growing)));
        hud.following(&session, &intents);
        assert_eq!(hud.picking.picked(), [Part::Growing]);

        intents.push(Choice::Include(Part::Turning));
        hud.following(&session, &intents);
        assert_eq!(hud.picking.picked(), [Part::Growing, Part::Turning]);

        // Laid over from the top each time, so a second reading of the same
        // inbox is the same answer — which is what lets every control take one
        // without the overlay tracking how far it has read.
        hud.following(&session, &intents);
        assert_eq!(hud.picking.picked(), [Part::Growing, Part::Turning]);

        // A later `Select` replaces rather than adds, exactly as the session
        // will answer it — the two follow one rule, spelled in
        // [`Selection::select`].
        intents.push(Choice::Select(Some(Part::Turning)));
        hud.following(&session, &intents);
        assert_eq!(hud.picking.picked(), [Part::Turning]);

        // What everything else in the inbox comes to here: nothing.
        intents.push(Choice::Close);
        hud.following(&session, &intents);
        assert_eq!(hud.picking.picked(), [Part::Turning]);
    }
}
