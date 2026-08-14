//! What floats over the viewport: the tools across the top, and the readout in
//! the corner.

use aperture::Projection;
use palantir::{
    Align, Background, Button, ButtonTheme, Configure, InternedStr, Palette, Panel, Sizing, Text,
    Ui,
};

use crate::drawing::Drawing;
use crate::intent::{Change, Choice, Intents};
use crate::selection::Selection;
use crate::tool::Tool;
use silverpoint::Constraint;

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
        drawing: &Drawing,
        selection: &Selection,
        intents: &mut Intents,
    ) {
        drawing.offers(selection.picked(), &mut self.offers);
        if self.offers.is_empty() {
            return;
        }
        floating(Panel::hstack(), "constraints", Align::BOTTOM).show(ui, |ui| {
            for &constraint in &self.offers {
                let label = noun(constraint);
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

    /// What the solve made of the drawing, and what the camera is doing, pinned
    /// to the top-left corner.
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
    pub(crate) drawing: &'a Drawing,
    pub(crate) selection: &'a Selection,
}

/// What to call a relation on the button that states it.
///
/// The user's word rather than the solver's, like
/// [`noun`](crate::noun) for an entity: a `PointOnSegment` is "on edge" to
/// whoever is drawing, and reads as a sentence about what was picked.
fn noun(constraint: Constraint) -> &'static str {
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
        }
    }
}
