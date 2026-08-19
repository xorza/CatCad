//! What floats over the viewport: the tools across the top, and the readout in
//! the corner.

use aperture::Projection;
use palantir::{
    Align, Button, ButtonTheme, Configure, DragValue, InternedStr, Palette, Panel, Sizing, Text,
    TextWrap, Ui,
};

use crate::intent::change::Change;
use crate::intent::{Choice, Intent, Intents, Opening, Step};
use crate::model::{Broken, Model, Models};
use crate::paint::DECIMALS;
use crate::part::Part;
use crate::selection::Selection;
use crate::timeline::FeatureId;
use crate::timeline::feature::{Datum, Feature};
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

/// Everything drawn over the viewport, and the look a button wears while what it
/// stands for is held.
///
/// A value rather than a pair of bare `show`s, for the theme alone: the held-down
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
    pressed: ButtonTheme,
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
            self.tools(ui, shown, intents);
            self.readout(ui, shown, intents);
        });
        self.tree(ui, shown, intents);
        self.constraints(ui, shown, intents);
    }

    /// The recipe, down the right edge: a row per step, in the order they build.
    ///
    /// **What two of the three kinds of step could not be pointed at without.** A
    /// plane has a square in the view, but a sketch *step* and an extrude have
    /// nothing on screen that is the step rather than something it produced — a
    /// sketch's geometry is what is drawn *in* it, and a solid's face was grown
    /// off a region. So until there was a list of them, neither could be picked
    /// out at all, and so neither could be deleted.
    ///
    /// The right edge, which is the one side the overlay does not already use:
    /// the tools and the readout are a column at the top left, and the
    /// constraint bar is pinned bottom left.
    ///
    /// **Named by kind and ordinal**, worked out here and stored nowhere. The
    /// three the world comes with have names of their own — see
    /// [`World::named`](crate::timeline::feature::World) — and the rest are
    /// counted as they are met. Steps a user can *name* are their own item; a
    /// tree reading "Sketch, Sketch, Sketch" is what makes them one.
    ///
    /// Interned into the pass's own arena rather than formatted into a `String`,
    /// like the status line beside it: this is a row per step per frame, and the
    /// record pass is gated at zero allocations.
    fn tree(&self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        let Shown {
            models, selection, ..
        } = shown;
        floating(Panel::vstack(), "tree", Align::TOP_RIGHT).show(ui, |ui| {
            let (mut planes, mut sketches, mut solids) = (0, 0, 0);
            for (at, feature) in models.steps() {
                // The same words the status line uses for the same states, so
                // the two say it the same way — see
                // [`Status`](crate::status::Status).
                let broken = match models.broken_at(at) {
                    Some(Broken::Profile) => " · lost",
                    Some(Broken::Unmerged) => " · apart",
                    None => "",
                };
                let (named, nth) = match feature {
                    Feature::Plane(Datum::World(world)) => (world.named(), None),
                    Feature::Plane(Datum::Offset { .. }) => {
                        planes += 1;
                        ("Plane", Some(planes))
                    }
                    Feature::Sketch { .. } => {
                        sketches += 1;
                        ("Sketch", Some(sketches))
                    }
                    Feature::Extrude { .. } => {
                        solids += 1;
                        ("Extrude", Some(solids))
                    }
                };
                let label = match nth {
                    Some(nth) => ui.fmt(format_args!("{named} {nth}{broken}")),
                    None => ui.fmt(format_args!("{named}{broken}")),
                };
                let mut row = Button::new()
                    // By the handle, which is what makes a row's identity
                    // survive a step above it going: salted by position, every
                    // row below a delete would take its neighbour's id and the
                    // pointer would find itself over a different step.
                    .id_salt(at)
                    .label(label)
                    .text_align(Align::LEFT)
                    .size((Sizing::FILL, Sizing::HUG));
                if selection.contains(Part::Step(at)) {
                    row = row.style(&self.pressed);
                }
                if row.show(ui).left.clicked() {
                    // Picked out and nothing else. What follows from picking a
                    // step is decided where everything else about a selection is
                    // — a sketch opens, a plane does not — see
                    // [`Models::opens`](crate::model::Models).
                    intents.push(Choice::Select(Some(Part::Step(at))));
                }
                // **The bar, drawn between the rows it divides.** One marker
                // rather than a mark on every row below it: what is rolled back
                // is a *tail*, so where it starts is the whole of what there is
                // to show, and a suffix repeated down the list would be saying
                // it once per step.
                if models.rolled() == Some(at) {
                    Text::new(ui.intern("── rolled back ──"))
                        .id_salt(at)
                        .show(ui);
                }
            }
        });
    }

    /// What can be asked of what is picked out, along the bottom.
    ///
    /// Shown only when there is something to offer, rather than a fixed bar of
    /// mostly-dead buttons. A selection admits at most four relations and
    /// usually none, so a bar sized to every constraint there is would be
    /// mostly grey the whole time — and what the user wants to know is what
    /// *this* selection can do.
    ///
    /// **Almost everything here wants a sketch open**, and for one reason: what
    /// it offers is what can be *said about* a drawing — a relation between two
    /// of its entities, a dimension retyped, a region grown — and none of that is
    /// asked of a document you are only looking at. Starting a sketch is the one
    /// offer that is not, so it is the one read before that gate: a plane picked
    /// out is a thing to *begin* on, which is exactly what there is to do when
    /// there is no drawing yet.
    fn constraints(&mut self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        let Shown {
            models, selection, ..
        } = shown;
        let startable = plane_picked(selection);
        let open = models.open();
        match open {
            Some(model) => model.offers(selection.picked(), &mut self.offers),
            // Cleared rather than left, because it is kept between frames: what
            // the last open sketch admitted is not what a document being looked
            // at admits, and the walk below reads this list whether or not
            // anything refilled it.
            None => self.offers.clear(),
        }
        let dimension = open.and_then(|model| dimension_picked(model, selection));
        let region = open.and_then(|_| region_picked(selection));
        if self.offers.is_empty() && dimension.is_none() && region.is_none() && startable.is_none()
        {
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
            // First, because it is the one thing here that can be asked of a
            // document nobody is drawing in — so where it stands alone it stands
            // at the near end of the bar rather than after a gap.
            if let Some(on) = startable
                && Button::new()
                    .id_salt("Sketch")
                    .label("Sketch")
                    .show(ui)
                    .left
                    .clicked()
            {
                // Builds rather than asks, unlike Extrude beside it: an extrude
                // is short of a depth and has a form to collect one, where a
                // sketch is born empty and has nothing left to settle. What
                // follows the step is being taken into it, which is the
                // application's — see [`Session::entered`](crate::session::Session).
                intents.push(Change::AddSketch { on });
            }
            // Everything else here is something to *say about* a drawing, so
            // all of it wants one open — and one binding says which sketch, in
            // place of each of them asking again.
            if let Some(sketch) = open.map(Model::of) {
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

    /// The tools, in a bar across the top — and the way out of the sketch they
    /// draw in.
    ///
    /// **Nothing but the way out where no sketch is open.** Every tool here
    /// draws, and drawing wants somewhere to draw; palantir's button has no dark
    /// state to wear, so a tool that could not be used would still take the
    /// press. What stands in their place is nothing at all, which is also what
    /// says the document is being looked at rather than worked in.
    fn tools(&self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        let Shown { tool, models, .. } = shown;
        if models.open().is_none() {
            return;
        }
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
            // Last, and apart from the tools it stands beside: the four above
            // say what a click will *do*, and this says where clicking stops.
            // Named for what it finishes rather than for closing, because that
            // is the word a modeller reaches for and because "close" is what a
            // document does.
            if Button::new()
                .id_salt("finish")
                .label("Finish sketch")
                .show(ui)
                .left
                .clicked()
            {
                intents.push(Choice::Close);
            }
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
            button = button.style(&self.pressed);
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

/// The one plane picked out, if what is picked is exactly that.
///
/// One rather than any, for the reason the two above are one: the button starts
/// *a* sketch, and two would be two sketches from one press — which is a thing
/// the user should ask for twice if they want it.
///
/// A plane and nothing beside it, so this says nothing while a pair is being
/// picked for a relation. That also keeps the two halves of the bar from ever
/// arguing: a selection admits relations or it admits a sketch, never both.
fn plane_picked(selection: &Selection) -> Option<FeatureId> {
    match *selection.picked() {
        [Part::Step(at)] => Some(at),
        _ => None,
    }
}

impl Default for Hud {
    fn default() -> Self {
        // Off the stock palette, which is the one the app runs on: nothing here
        // restyles palantir, so this is `Theme::default`'s own button with two
        // of its states rewritten.
        let mut pressed = ButtonTheme::from_palette(&Palette::DEFAULT);
        // A tool in hand and a step picked out both read as held down — each
        // stays that way until something puts it down — so the button wears its
        // pressed look at rest and under the pointer alike. One theme for the
        // two, because it is one thing being said about them.
        pressed.looks.normal = pressed.looks.active.clone();
        pressed.looks.hovered = pressed.looks.active.clone();
        Self {
            pressed,
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

    /// The Sketch command, on the same bar and measured the same way.
    ///
    /// With one plane picked out it is the only thing on that bar — a plane
    /// admits no relation and is no region — so it stands where Extrude stands
    /// when a region is picked, at the near end of a bar hugging the left edge.
    #[cfg(test)]
    pub(crate) const SKETCH_BUTTON: Vec2 = Vec2::new(50.0, 570.0);

    /// The middle of the feature tree's first row, and how far apart the rows
    /// sit — both measured the same way as the buttons above.
    ///
    /// A pitch rather than a position apiece, because the rows are a list: what
    /// a test wants is the row of a step it found by walking the recipe, and
    /// writing out one constant per row would be writing down how many steps the
    /// demo has in a file that is not about that.
    ///
    /// Pinned to the top *right*, so the x is measured in from `SIZE` where the
    /// column's are measured out from zero. Well inside the panel rather than at
    /// its middle: how wide it is follows from the longest label, and a test
    /// aimed at the edge would be a test about the wording.
    #[cfg(test)]
    pub(crate) const TREE_ROW: Vec2 = Vec2::new(760.0, 28.0);
    #[cfg(test)]
    pub(crate) const TREE_PITCH: f32 = 41.25;
}
