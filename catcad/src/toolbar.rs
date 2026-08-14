//! The tools, in a bar across the top of the viewport.

use palantir::{Align, Background, Button, ButtonTheme, Configure, Palette, Panel, Sizing, Ui};

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
    /// Show the tools, arming and disarming `tool` as they are pressed.
    ///
    /// Writes `tool` where the overlay next door raises an intent, and the
    /// difference is what is being written: the camera the overlay's button
    /// turns is the document's, so it goes through the one place a document is
    /// written, where the tool in hand is nobody's but this frame's.
    ///
    /// A press is read once however many times palantir replays the record
    /// pass, because a click is an edge rather than a latch and the input
    /// queues are drained between passes — which is what keeps a settling frame
    /// from arming a tool and putting it straight back down.
    pub(crate) fn show(&self, ui: &mut Ui, tool: &mut Tool) {
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
                self.tool(ui, tool, Tool::Point, "Point");
            });
    }

    /// One button, which arms `arms` and shows whether it is armed.
    ///
    /// Salted with the label rather than `auto_id`, which would give every tool
    /// on the bar the one id of this call site.
    fn tool(&self, ui: &mut Ui, tool: &mut Tool, arms: Tool, label: &str) {
        let mut button = Button::new().id_salt(label).label(label);
        if *tool == arms {
            button = button.style(&self.armed);
        }
        if button.show(ui).left.clicked() {
            *tool = tool.toggled(arms);
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
