//! CatCad application entry point.

use palantir::{
    App, Configure, HostHandle, Panel, Sizing, Text, Ui, WindowToken, WinitHost, WinitHostError,
};

#[derive(Debug)]
struct CatCad {}

impl CatCad {
    fn new(_ui: &mut Ui, _handle: HostHandle<Self>) -> Self {
        CatCad {}
    }
}

impl App for CatCad {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
        Panel::vstack()
            .auto_id()
            .gap(8.0)
            .size((Sizing::HUG, Sizing::HUG))
            .show(ui, |ui| {
                Text::new("CatCad").auto_id().show(ui);
            });
    }
}

fn main() -> Result<(), WinitHostError> {
    WinitHost::builder(WindowToken(0))
        .title("CatCad")
        .build(CatCad::new)?
        .run()
}
