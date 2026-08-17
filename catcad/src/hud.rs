//! What floats over the viewport: the tools across the top, and the readout in
//! the corner.

use aperture::Projection;
use palantir::{
    Align, Background, Button, ButtonTheme, Configure, DragValue, InternedStr, Palette, Panel,
    Sizing, Text, TextWrap, Ui,
};

use crate::intent::{Change, Choice, Errand, Intent, Intents, Opening, Step};
use crate::model::{Model, Models};
use crate::paint::DECIMALS;
use crate::part::Part;
use crate::selection::Selection;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use silverpoint::{Along, Constraint, ConstraintId, Entity};

/// Logical pixels of breathing room inside a floating panel, and between the
/// things standing on one.
///
/// Shared rather than written twice, because the panels are pinned to different
/// corners of the same view and nothing but these two numbers lines them up: one
/// padded unlike its neighbour reads as a mistake rather than as a choice.
const PADDING: f32 = 12.0;
const GAP: f32 = 8.0;

/// Everything drawn over the viewport, and the look a tool button wears while
/// its tool is in hand.
///
/// A value rather than a pair of bare `show`s, for the theme alone: the armed
/// look is a handful of backgrounds derived once and read every frame, and the
/// record pass is gated at zero allocations.
///
/// Shows and does not act — the whole of it. Every control reads app state and
/// asks for what it wants as an [`Intent`](crate::intent::Intent): one that
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
        let model = models.open();
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
        if let Some((_, value)) = dimension {
            self.draft = value;
        }
        floating(Panel::hstack(), "constraints", Align::BOTTOM_LEFT).show(ui, |ui| {
            if let Some((id, _)) = dimension {
                let edited = DragValue::new(&mut self.draft)
                    .auto_id()
                    .speed(DIMENSION_SPEED)
                    .decimals(DECIMALS)
                    .show(ui);
                if edited.changed {
                    intents.push(Change::Resize {
                        sketch,
                        constraint: id,
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
            if let Some((sketch, region)) = region {
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
                    intents.push(Choice::Ask(Some(Opening::Extrude { sketch, region })));
                }
            }
            for &constraint in &self.offers {
                let label = label(constraint);
                let pressed = Button::new()
                    .id_salt(label)
                    .label(label)
                    .show(ui)
                    .left
                    .clicked();
                if pressed {
                    // A radius *asks* where every other relation states. The
                    // rest say something the drawing can work out for itself —
                    // that two edges are parallel, that a point sits on one —
                    // and a radius is a number, which until now could only be
                    // the size the circle happened to be. See
                    // [`Opening::Radius`].
                    intents.push(match constraint {
                        Constraint::Radius { circle, dimension } => {
                            Intent::from(Choice::Ask(Some(Opening::Radius {
                                sketch,
                                circle,
                                from: dimension.value,
                            })))
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
        let sketch = models.open().of();
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
            tidy_button(ui, sketch, intents);
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

/// A panel that floats on the view rather than boxing part of it off, pinned to
/// `align`.
///
/// Salted rather than left to `auto_id`, for the reason [`Hud::tool`] is:
/// `auto_id` reads the line it is written on, so one called here would hand
/// every panel built from this recipe the same id.
fn floating(panel: Panel, salt: &str, align: Align) -> Panel {
    stacked(panel, salt).align(align).padding(PADDING)
}

/// A group standing inside one of those, which is the same panel without a
/// corner to pin itself to or padding of its own — the one it is in has both.
fn stacked(panel: Panel, salt: &str) -> Panel {
    panel
        .id_salt(salt)
        // A panel's own background would put a slab of theme colour over the
        // drawing; these sit *on* the view, and whatever stands on them carries
        // its own edges.
        .background(Background::NONE)
        .size((Sizing::HUG, Sizing::HUG))
        .gap(GAP)
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

/// The one dimension picked out, if what is picked is exactly that.
///
/// One rather than any, because the field edits a value and two values have no
/// single answer. A selection holding a dimension *and* something else is
/// someone part-way through picking a pair, so the field stays away.
fn dimension_picked(model: Model<'_>, selection: &Selection) -> Option<(ConstraintId, f64)> {
    let [only] = *selection.picked() else {
        return None;
    };
    let Some(Entity::Constraint(id)) = model.entity(only) else {
        return None;
    };
    model
        .drawing()
        .holds(id)
        .then(|| model.sketch().constraint(id).value().map(|at| (id, at)))
        .flatten()
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
fn region_picked(selection: &Selection) -> Option<(FeatureId, usize)> {
    match *selection.picked() {
        [Part::Region { sketch, at }] => Some((sketch, at)),
        _ => None,
    }
}

/// What the button that states a relation is captioned.
///
/// The user's word rather than the solver's: a `PointOnSegment` is "on edge" to
/// whoever is drawing. A caption rather than a noun — which is why this is not
/// [`noun`](crate::noun), whose answers are lowercase because they are read
/// inside a sentence in the status line.
fn label(constraint: Constraint) -> &'static str {
    match constraint {
        Constraint::Coincident { .. } => "Coincident",
        // Which way a distance is read is part of what the button asks for, so
        // it is part of what the button says. "Distance" alone for the aligned
        // one, because that is the plain case and the other two are the ones
        // that need naming.
        Constraint::Distance {
            along: Along::Shortest,
            ..
        } => "Distance",
        Constraint::Distance {
            along: Along::Horizontal,
            ..
        } => "Horizontal distance",
        Constraint::Distance {
            along: Along::Vertical,
            ..
        } => "Vertical distance",
        // Both are a distance to whoever is drawing, and which one is meant is
        // plain from what is picked out — a point and an edge, or two edges.
        // The same argument "Equal" is one word for two relations below.
        Constraint::Standoff { .. } | Constraint::Spacing { .. } => "Distance",
        Constraint::Horizontal { .. } => "Horizontal",
        Constraint::Vertical { .. } => "Vertical",
        Constraint::Parallel { .. } => "Parallel",
        Constraint::Perpendicular { .. } => "Perpendicular",
        Constraint::PointOnSegment { .. } => "On edge",
        Constraint::Radius { .. } => "Radius",
        Constraint::PointOnCircle { .. } => "On circle",
        // One word for both, the way a modeller offers it: which of the two a
        // press means is settled by what is picked out, and a selection admits
        // only ever one of them — see [`Drawing::offers`].
        Constraint::EqualLength { .. } | Constraint::EqualRadius { .. } => "Equal",
        Constraint::Tangent { .. } => "Tangent",
    }
}

/// Flips the camera between the two projections.
///
/// Labelled with the projection it is on rather than the one it would switch
/// to: the button has to answer "which am I looking at?" every frame, and only
/// answers "what happens if I press this?" once.
fn projection_toggle(ui: &mut Ui, projection: Projection, intents: &mut Intents) {
    let label = match projection {
        Projection::Perspective => "Perspective",
        Projection::Orthographic => "Orthographic",
    };
    if Button::new().auto_id().label(label).show(ui).left.clicked() {
        intents.push(Change::Project(projection.toggled()));
    }
}

/// Asks for the drawing's spare geometry to be taken out.
///
/// Beside the readout rather than on the constraint bar, because it is not
/// about what is picked out — it asks a question of the whole drawing, and the
/// bar below appears and vanishes with the selection.
///
/// Always live, rather than shown only when it would do something. Answering
/// "is there anything to clean up?" means running the whole search, and the
/// record pass allocates nothing — so the choice is between a search a frame
/// and a button that is sometimes a no-op, and a no-op costs nothing.
fn tidy_button(ui: &mut Ui, sketch: FeatureId, intents: &mut Intents) {
    let pressed = Button::new()
        .auto_id()
        .label("Clean up")
        .show(ui)
        .left
        .clicked();
    if pressed {
        intents.push(Change::Tidy { sketch });
    }
}

/// Puts the document away, and fetches one back.
///
/// Beside the readout with the cleanup, and for the same reason: neither is
/// about what is picked out. Both are here rather than on a menu bar because
/// there is no menu bar — and two buttons that say what they do beat a File
/// menu holding two buttons.
///
/// Neither is ever dark. Whether saving would ask for a path is [`Filing`]'s to
/// know and the answer changes nothing about whether the command is available,
/// so a button that greyed itself out would be answering a question nobody
/// asked.
///
/// [`Filing`]: crate::filing::Filing
fn filing_buttons(ui: &mut Ui, intents: &mut Intents) {
    Panel::hstack()
        .id_salt("filing")
        .background(Background::NONE)
        .size((Sizing::HUG, Sizing::HUG))
        .gap(GAP)
        .show(ui, |ui| {
            for (label, errand) in [("Open", Errand::Open), ("Save", Errand::Save)] {
                if Button::new()
                    .id_salt(label)
                    .label(label)
                    .show(ui)
                    .left
                    .clicked()
                {
                    intents.push(errand);
                }
            }
        });
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
