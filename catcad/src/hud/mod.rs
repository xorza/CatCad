//! What floats over the viewport: the tools across the top, and the readout in
//! the corner.

use aperture::Projection;
use palantir::{
    Align, Button, ButtonTheme, Configure, DragValue, InternedStr, Palette, Panel, Text, TextWrap,
    Ui,
};

use crate::intent::change::Change;
use crate::intent::{Choice, Intent, Intents, Opening, Step};
use crate::model::{Model, Models};
use crate::paint::DECIMALS;
use crate::part::Part;
use crate::selection::Selection;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;
use crate::wording;
use silverpoint::{Constraint, ConstraintId, Entity};

mod bar;

use crate::hud::bar::{filing_buttons, floating, projection_toggle, stacked, tidy_button};

/// Logical pixels of breathing room inside a floating panel, and between the
/// things standing on one.
///
/// Shared rather than written twice, because the panels are pinned to different
/// corners of the same view and nothing but these two numbers lines them up: one
/// padded unlike its neighbour reads as a mistake rather than as a choice.
pub(super) const PADDING: f32 = 12.0;
pub(super) const GAP: f32 = 8.0;

/// Everything drawn over the viewport, and the look a tool button wears while
/// its tool is in hand.
///
/// A value rather than a pair of bare `show`s, for the theme alone: the armed
/// look is a handful of backgrounds derived once and read every frame, and the
/// record pass is gated at zero allocations.
///
/// Shows and does not act — the whole of it. Every control reads app state and
/// asks for what it wants as an [`Intent`]: one that
/// turned the camera itself would be one that had to be handed a camera, and
/// one that armed a tool itself would arm it and put it straight back down on a
/// replayed pass.
#[derive(Debug)]
pub(crate) struct Hud {
    armed: ButtonTheme,
    /// What the current selection admits, refilled every frame. Kept for its
    /// room rather than its contents: the record pass allocates nothing, and a
    /// bar rebuilt sixty times a second would otherwise ask the heap for a list
    /// each time.
    offers: Vec<Constraint>,
    /// The number the dimension field is showing, re-seeded from the drawing
    /// every frame and written over by the widget while it is being scrubbed.
    /// Scratch: what a dimension *is* lives in the sketch, and this is only
    /// what one gesture has made of it so far.
    draft: f64,
}

impl Hud {
    /// Show the whole of it, putting whatever a control asks for in `intents`.
    ///
    /// `status` arrives already in the pass's text arena, so nothing here copies
    /// it — and it has to be lowered in the pass that minted it, which is the
    /// same pass that is calling.
    /// **Everything is pinned to the left edge**, and that is a rule rather
    /// than a taste. A panel centred over the view is centred in whatever the
    /// view's own container came out as, and the container is floored by the
    /// widest thing standing on it — so the readout growing a long enough line
    /// widens the container past the window and carries every centred thing
    /// sideways with it. A left-aligned panel sits at the edge whatever the
    /// container measures.
    ///
    /// What that cost was worth catching: a document saved to a long path made
    /// the status line wide enough to slide the tool bar out from under the
    /// pointer, so a click on Point armed nothing.
    pub(crate) fn show(&mut self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        // The tools and the readout in one column rather than two panels at one
        // corner: left-aligned, they would otherwise be drawn over each other.
        floating(Panel::vstack(), "chrome", Align::TOP_LEFT).show(ui, |ui| {
            self.tools(ui, shown.tool, intents);
            self.readout(ui, shown, intents);
        });
        self.constraints(ui, shown, intents);
    }

    /// What can be asked of what is picked out, along the bottom.
    ///
    /// Shown only when there is something to offer, rather than a fixed bar of
    /// mostly-dead buttons. A selection admits at most four relations and
    /// usually none, so a bar sized to every constraint there is would be
    /// mostly grey the whole time — and what the user wants to know is what
    /// *this* selection can do.
    fn constraints(&mut self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        let Shown {
            models, selection, ..
        } = shown;
        // Nothing at all where no sketch is open. What this bar offers is what
        // can be *said about* a drawing — a relation between two of its
        // entities, a dimension retyped, a region grown — and none of that is
        // asked of a document you are only looking at.
        let Some(model) = models.open() else {
            return;
        };
        let sketch = model.of();
        model.offers(selection.picked(), &mut self.offers);
        let dimension = dimension_picked(model, selection);
        let region = region_picked(selection);
        if self.offers.is_empty() && dimension.is_none() && region.is_none() {
            return;
        }
        // Seeded from the drawing every frame rather than remembered, which is
        // what makes the field a *view* of the dimension: an undo, a drag that
        // moved it, or picking a different one all show up here without anything
        // having to notice. The widget writes its draft over the seed and says
        // so, and that is the only frame anything is asked for.
        if let Some(resizable) = dimension {
            self.draft = resizable.value;
        }
        floating(Panel::hstack(), "constraints", Align::BOTTOM_LEFT).show(ui, |ui| {
            if let Some(resizable) = dimension {
                let edited = DragValue::new(&mut self.draft)
                    .auto_id()
                    .speed(DIMENSION_SPEED)
                    .decimals(DECIMALS)
                    .show(ui);
                if edited.changed {
                    intents.push(Change::Resize {
                        sketch,
                        constraint: resizable.constraint,
                        to: self.draft,
                    });
                }
                // One gesture, one step to take back. `Resize` coalesces, so
                // the run of them a scrub sends is one open step — and this is
                // what closes it, the same signal a drag's release gives.
                if edited.committed {
                    intents.push(Step::Release);
                }
            }
            // Before the relations, because it is the one thing here that
            // builds rather than states: a relation says something about the
            // drawing, and this puts a step on the end of the document.
            if let Some(growable) = region {
                let pressed = Button::new()
                    .id_salt("Extrude")
                    .label("Extrude")
                    .show(ui)
                    .left
                    .clicked();
                if pressed {
                    // Asks rather than builds. The solid appears at no depth at
                    // all and the form beside it decides how far it goes, so
                    // what reaches the timeline is one step carrying the depth
                    // that was settled on — and a form cancelled leaves the
                    // document never having heard of it.
                    intents.push(Choice::Ask(Some(Opening::Extrude {
                        sketch: growable.sketch,
                        region: growable.region,
                    })));
                }
            }
            for &constraint in &self.offers {
                let label = wording::named(constraint).word;
                let pressed = Button::new()
                    .id_salt(label)
                    .label(label)
                    .show(ui)
                    .left
                    .clicked();
                if pressed {
                    // **Two answers, and which one a button gives follows
                    // from what it is short of.** A relation says something the
                    // drawing can work out for itself — that two edges are
                    // parallel, that a point sits on one — so pressing it
                    // states it outright. A dimension already knows its number,
                    // because the drawing measured it, and is short of
                    // somewhere to put it, so it goes into the pointer's hands.
                    //
                    // Which is why a bar button and the tool are not two ways
                    // of doing one thing: the bar is what says *which* of three
                    // readings a pair is measured by, which a pointer can only
                    // guess at, and the tool is what says where the figure
                    // goes, which a button cannot say at all.
                    intents.push(match constraint {
                        // A radius used to be asked for with a form instead,
                        // on the grounds that the bar's other offers were
                        // relations needing no number. They have not been for
                        // some time, and the form was also the one door that
                        // minted a dimension without a placement.
                        constraint if let Some(placing) = Dimensioning::placing(constraint) => {
                            Intent::from(Choice::Hold(Tool::Dimension(placing)))
                        }
                        constraint => Change::Constrain { sketch, constraint }.into(),
                    });
                }
            }
        });
    }

    /// What the solve made of the drawing, what the camera is doing, and the
    /// commands that are about neither — pinned to the top-left corner.
    ///
    /// Controls among the readout because both of these ask something of the
    /// whole drawing rather than of what is picked out, which is the line the
    /// constraint bar sits on the far side of.
    fn readout(&self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        let Shown {
            status,
            projection,
            models,
            ..
        } = shown;
        stacked(Panel::vstack(), "readout").show(ui, |ui| {
            projection_toggle(ui, projection, intents);
            // Cut off rather than allowed to run on, and this is load-bearing
            // rather than tidy. A run of text reports its whole natural width as
            // the *least* it will accept, a panel is at least as wide as what
            // stands on it, and the view is a `FILL` beside this — so a status
            // line long enough would widen the panel over the view, floor the
            // whole overlay past the window, and stretch the viewport with it.
            // A stretched viewport is a different projection, so the drawing
            // would be picked where it is not drawn.
            //
            // What made that reachable is a document saved to a long path: the
            // line says where it was written, and a temporary directory is
            // sixty characters before the name.
            Text::new(status)
                .auto_id()
                .text_wrap(TextWrap::Ellipsis)
                .show(ui);
            // Only where there is a drawing to clean up. A button that could
            // not act is worse than none, because it still takes the press —
            // and palantir's has no dark state to wear instead, which is the
            // same reason the constraint bar shows nothing rather than a row of
            // grey.
            if let Some(sketch) = models.open().map(Model::of) {
                tidy_button(ui, sketch, intents);
            }
            filing_buttons(ui, intents);
        });
    }

    /// The tools, in a bar across the top.
    fn tools(&self, ui: &mut Ui, tool: Tool, intents: &mut Intents) {
        // A row inside the column rather than a panel floating on the view, so
        // it takes the column's padding rather than adding its own.
        stacked(Panel::hstack(), "tools").show(ui, |ui| {
            self.tool(ui, tool, Tool::Point, "Point", intents);
            self.tool(ui, tool, Tool::Line { from: None }, "Line", intents);
            self.tool(ui, tool, Tool::Circle { center: None }, "Circle", intents);
            self.tool(
                ui,
                tool,
                Tool::Dimension(Dimensioning::Empty),
                "Dimension",
                intents,
            );
        });
    }

    /// One tool button, which asks for `arms` and shows whether it is in hand.
    ///
    /// Salted with the label rather than `auto_id`, which would give every tool
    /// on the bar the one id of this call site.
    fn tool(&self, ui: &mut Ui, tool: Tool, arms: Tool, label: &str, intents: &mut Intents) {
        let mut button = Button::new().id_salt(label).label(label);
        // The same tool, however far through it is — a line half drawn is still
        // the line tool, and a button that went dark between its two clicks
        // would be saying the opposite.
        if tool.is(arms) {
            button = button.style(&self.armed);
        }
        if button.show(ui).left.clicked() {
            intents.push(Choice::Hold(tool.toggled(arms)));
        }
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
    pub(crate) tool: Tool,
    /// Already in the pass's own text arena, so nothing here copies it — and it
    /// has to be lowered in the pass that minted it.
    pub(crate) status: InternedStr,
    pub(crate) projection: Projection,
    /// Every sketch the document holds, and which of them is open.
    ///
    /// The model rather than the drawing, because a control here reads what is
    /// *picked out* — and a part names the sketch it belongs to as well as the
    /// thing within it. Only a model can tell the two apart, and one of another
    /// sketch would otherwise resolve here as whatever sits at that slot.
    ///
    /// It is also what every control that asks for a change names, since the
    /// sketch it is of is the one open — see
    /// [`Session::editing`](crate::session::Session).
    pub(crate) models: Models<'a>,
    pub(crate) selection: &'a Selection,
}

/// Sketch units per pixel of scrub. A hundredth, so a drag reads a dimension
/// out at the same precision the drawing prints it to and a slow pull can land
/// on a round number.
const DIMENSION_SPEED: f64 = 0.01;

/// A dimension the bar can scrub, and the number it states as it stands.
///
/// The value comes along because the field is a *view* of the dimension rather
/// than a draft beside it — read in the same breath as the handle, so the two
/// cannot be a frame apart.
#[derive(Debug, Clone, Copy)]
struct Resizable {
    constraint: ConstraintId,
    value: f64,
}

/// The one dimension picked out, if what is picked is exactly that.
///
/// One rather than any, because the field edits a value and two values have no
/// single answer. A selection holding a dimension *and* something else is
/// someone part-way through picking a pair, so the field stays away.
fn dimension_picked(model: Model<'_>, selection: &Selection) -> Option<Resizable> {
    let [only] = *selection.picked() else {
        return None;
    };
    let Some(Entity::Constraint(id)) = model.entity(only) else {
        return None;
    };
    model
        .drawing()
        .holds(id)
        .then(|| {
            model
                .sketch()
                .constraint(id)
                .value()
                .map(|value| Resizable {
                    constraint: id,
                    value,
                })
        })
        .flatten()
}

/// A region the bar can grow a solid off.
///
/// Both halves, because a region is only a region of the arrangement it was
/// walked out of — see [`Part::Region`] — so the sketch it belongs to travels
/// with it rather than being looked up again at the press.
#[derive(Debug, Clone, Copy)]
struct Growable {
    sketch: FeatureId,
    region: usize,
}

/// The one region picked out, if what is picked is exactly that.
///
/// One rather than any, for the reason the dimension above is one: the button
/// grows a solid off *a* region, and two would be two solids from one press —
/// which is a thing the user should ask for twice if they want it.
///
/// Not checked against the drawing, unlike the dimension: a part that no longer
/// exists has already been pruned out of the selection by the time anything here
/// reads it — see [`Session::prune`](crate::session::Session::prune).
fn region_picked(selection: &Selection) -> Option<Growable> {
    match *selection.picked() {
        [Part::Region { sketch, at }] => Some(Growable { sketch, region: at }),
        _ => None,
    }
}

impl Default for Hud {
    fn default() -> Self {
        // Off the stock palette, which is the one the app runs on: nothing here
        // restyles palantir, so this is `Theme::default`'s own button with two
        // of its states rewritten.
        let mut armed = ButtonTheme::from_palette(&Palette::DEFAULT);
        // An armed tool reads as held down — it stays pressed until it is put
        // down, so the button wears its pressed look at rest and under the
        // pointer alike.
        armed.looks.normal = armed.looks.active.clone();
        armed.looks.hovered = armed.looks.active.clone();
        Self {
            armed,
            offers: Vec::new(),
            draft: 0.0,
        }
    }
}

/// Where a harness has to click to reach the bar, which the bar itself never
/// needs to know.
///
/// A press arrives at the application as a cursor, and nothing in it can turn
/// "the Line button" into one — palantir places the row and a widget's rect is
/// the layout engine's answer, a frame late. So a harness driving the real
/// button drives it by position, and the positions belong beside the row that
/// decides them rather than being written out once per harness. They were
/// written out twice, and the copy that was not exercised by an assertion drifted
/// a bar's width when the row stopped being centred.
///
/// Gated on `bench` rather than on `internals` beside it: the two callers are
/// the unit tests and the allocation bench, and the wider gate would leave this
/// dead in every build that turned `internals` on for the *renderer* reach-in
/// and nothing else — see [`CatCad::internals`](crate::internals).
#[cfg(any(test, feature = "bench"))]
pub(crate) mod internals {
    use glam::Vec2;

    /// The middle of each button on the tool row, which is the top of the column
    /// down the left edge — measured by sweeping and reading back which widget a
    /// click at each pixel would land on.
    ///
    /// Hand-written numbers, and safe ones only where the caller checks: every
    /// press through these is followed by an assertion about what ended up in
    /// hand, so a layout that moved a button fails there rather than quietly
    /// testing the gap between two.
    ///
    /// The same numbers at every surface size, which is what makes them numbers
    /// at all: the column is pinned to the top left, where a centred bar would
    /// move with the width of the widest thing on the view — see
    /// [`Hud::show`](crate::hud::Hud::show).
    pub(crate) const LINE_BUTTON: Vec2 = Vec2::new(112.0, 26.0);
    /// The rest are `test`-only, narrower again: the bench reaches for the Line
    /// button alone.
    #[cfg(test)]
    pub(crate) const POINT_BUTTON: Vec2 = Vec2::new(45.0, 26.0);
    #[cfg(test)]
    pub(crate) const CIRCLE_BUTTON: Vec2 = Vec2::new(187.0, 26.0);

    /// The clean-up command, further down the same column: it asks something of
    /// the whole drawing rather than of what is picked out, so it is not a tool.
    #[cfg(test)]
    pub(crate) const TIDY_BUTTON: Vec2 = Vec2::new(58.0, 140.0);

    /// The Extrude command, on the bar along the bottom that shows what can be
    /// asked of what is picked out.
    ///
    /// With one region picked it is the only thing on that bar, and the bar
    /// hugs what it holds against the left edge. The one position here that is
    /// *not* the same at every surface size — the bar is pinned to the bottom,
    /// so its y is measured down from `SIZE`.
    #[cfg(test)]
    pub(crate) const EXTRUDE_BUTTON: Vec2 = Vec2::new(55.0, 570.0);
}
