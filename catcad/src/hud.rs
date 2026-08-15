//! What floats over the viewport: the tools across the top, and the readout in
//! the corner.

use aperture::Projection;
use palantir::{
    Align, Background, Button, ButtonTheme, Configure, DragValue, InternedStr, Palette, Panel,
    Sizing, Text, Ui,
};

use crate::drawing::Drawing;
use crate::intent::{Change, Choice, Intents, Step};
use crate::paint::DECIMALS;
use crate::part::Part;
use crate::selection::Selection;
use crate::tool::Tool;
use silverpoint::{Constraint, ConstraintId, Entity};

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
    /// The number the field is showing, re-seeded from the drawing every frame
    /// and written over by the widget when it is being edited. Scratch: what a
    /// dimension *is* lives in the sketch, and this is only what one gesture has
    /// made of it so far.
    editing: f64,
}

impl Hud {
    /// Show the whole of it, putting whatever a control asks for in `intents`.
    ///
    /// `status` arrives already in the pass's text arena, so nothing here copies
    /// it — and it has to be lowered in the pass that minted it, which is the
    /// same pass that is calling.
    pub(crate) fn show(&mut self, ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
        self.readout(ui, shown.status, shown.projection, intents);
        self.constraints(ui, shown.drawing, shown.selection, intents);
        // Last, so the bar is the topmost thing in the zstack and takes its own
        // presses rather than the readout or the view beneath it.
        self.tools(ui, shown.tool, intents);
    }

    /// What can be asked of what is picked out, along the bottom.
    ///
    /// Shown only when there is something to offer, rather than a fixed bar of
    /// mostly-dead buttons. A selection admits at most four relations and
    /// usually none, so a bar sized to every constraint there is would be
    /// mostly grey the whole time — and what the user wants to know is what
    /// *this* selection can do.
    fn constraints(
        &mut self,
        ui: &mut Ui,
        drawing: Drawing<'_>,
        selection: &Selection,
        intents: &mut Intents,
    ) {
        drawing.offers(selection.picked(), &mut self.offers);
        let dimension = dimension_picked(drawing, selection);
        if self.offers.is_empty() && dimension.is_none() {
            return;
        }
        // Seeded from the drawing every frame rather than remembered, which is
        // what makes the field a *view* of the dimension: an undo, a drag that
        // moved it, or picking a different one all show up here without anything
        // having to notice. The widget writes its draft over the seed and says
        // so, and that is the only frame anything is asked for.
        if let Some((_, value)) = dimension {
            self.editing = value;
        }
        floating(Panel::hstack(), "constraints", Align::BOTTOM).show(ui, |ui| {
            if let Some((id, _)) = dimension {
                let edited = DragValue::new(&mut self.editing)
                    .auto_id()
                    .speed(DIMENSION_SPEED)
                    .decimals(DECIMALS)
                    .show(ui);
                if edited.changed {
                    intents.push(Change::Resize {
                        constraint: id,
                        to: self.editing,
                    });
                }
                // One gesture, one step to take back. `Resize` coalesces, so
                // the run of them a scrub sends is one open step — and this is
                // what closes it, the same signal a drag's release gives.
                if edited.committed {
                    intents.push(Step::Release);
                }
            }
            for &constraint in &self.offers {
                let label = label(constraint);
                if Button::new()
                    .id_salt(label)
                    .label(label)
                    .show(ui)
                    .left
                    .clicked()
                {
                    intents.push(Change::Constrain(constraint));
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
    fn readout(
        &self,
        ui: &mut Ui,
        status: InternedStr,
        projection: Projection,
        intents: &mut Intents,
    ) {
        floating(Panel::vstack(), "readout", Align::TOP_LEFT).show(ui, |ui| {
            projection_toggle(ui, projection, intents);
            Text::new(status).auto_id().show(ui);
            tidy_button(ui, intents);
        });
    }

    /// The tools, in a bar across the top.
    fn tools(&self, ui: &mut Ui, tool: Tool, intents: &mut Intents) {
        floating(Panel::hstack(), "tools", Align::TOP).show(ui, |ui| {
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
    panel
        .id_salt(salt)
        // A panel's own background would put a slab of theme colour over the
        // drawing; these sit *on* the view, and whatever stands on them carries
        // its own edges.
        .background(Background::NONE)
        .size((Sizing::HUG, Sizing::HUG))
        .align(align)
        .padding(PADDING)
        .gap(GAP)
}

/// Everything the overlay reads to draw itself.
///
/// Gathered rather than passed one by one, because they arrive together and
/// mean one thing between them: this is the frame's state as the controls see
/// it. What is *not* here is the inbox — the overlay reads all of this and
/// writes none of it, and keeping the two apart at the signature is what says
/// so.
#[derive(Debug)]
pub(crate) struct Shown<'a> {
    pub(crate) tool: Tool,
    /// Already in the pass's own text arena, so nothing here copies it — and it
    /// has to be lowered in the pass that minted it.
    pub(crate) status: InternedStr,
    pub(crate) projection: Projection,
    pub(crate) drawing: Drawing<'a>,
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
fn dimension_picked(drawing: Drawing<'_>, selection: &Selection) -> Option<(ConstraintId, f64)> {
    let [
        Part::Entity {
            entity: Entity::Constraint(id),
            ..
        },
    ] = *selection.picked()
    else {
        return None;
    };
    drawing
        .holds(id)
        .then(|| drawing.sketch().constraint(id).value().map(|at| (id, at)))
        .flatten()
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
        Constraint::Distance { .. } => "Distance",
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
fn tidy_button(ui: &mut Ui, intents: &mut Intents) {
    if Button::new()
        .auto_id()
        .label("Clean up")
        .show(ui)
        .left
        .clicked()
    {
        intents.push(Change::Tidy);
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
            editing: 0.0,
        }
    }
}
