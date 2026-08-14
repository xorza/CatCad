//! The tools, in a bar across the top of the viewport.

use palantir::{Align, Background, Button, ButtonTheme, Configure, Palette, Panel, Sizing, Ui};

use crate::intent::{Intent, Intents};
use crate::tool::Tool;

/// The bar of tools, and the look a button wears while its tool is in hand.
///
/// A value rather than a bare `show`, for the theme alone: the armed look is a
/// handful of backgrounds derived once and read every frame, and the record
/// pass is gated at zero allocations.
#[derive(Debug)]
pub(crate) struct Toolbar {
    armed: ButtonTheme,
}

impl Toolbar {
    /// Show the tools, putting what a press asks for in `intents`.
    ///
    /// Shows and does not act, like the overlay next door: `tool` is read to
    /// know which button to draw as held, and what a press asks for leaves as
    /// an [`Intent::Hold`] naming the tool it wants — which is what makes a
    /// replayed pass harmless, where flipping the tool here would arm it and
    /// put it straight back down.
    pub(crate) fn show(&self, ui: &mut Ui, tool: Tool, intents: &mut Intents) {
        Panel::hstack()
            .auto_id()
            // Chrome would put a slab of theme colour over the drawing; the bar
            // floats on the view, and the buttons carry their own edges.
            .background(Background::NONE)
            .size((Sizing::HUG, Sizing::HUG))
            .align(Align::TOP)
            .padding(12.0)
            .gap(8.0)
            .show(ui, |ui| {
                self.tool(ui, tool, Tool::Point, "Point", intents);
                self.tool(ui, tool, Tool::Line { from: None }, "Line", intents);
                self.tool(ui, tool, Tool::Circle { center: None }, "Circle", intents);
            });
    }

    /// One button, which asks for `arms` and shows whether it is in hand.
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
            intents.push(Intent::Hold(tool.toggled(arms)));
        }
    }
}

impl Default for Toolbar {
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
        Self { armed }
    }
}
