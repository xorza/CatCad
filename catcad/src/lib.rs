//! A parametric CAD application: constrained sketches, solved, and drawn.
//!
//! The binary beside this is a `WinitHost` call and nothing else, so the whole
//! of the app is reachable from a test — which is what lets the visual suite
//! raise the real thing rather than a stand-in for it.

mod build;
mod cat_cad;
mod control;
mod demo;
mod dialog;
mod document;
mod drawing;
mod filing;
mod history;
mod hud;
mod intent;
mod lens;
mod look;
mod marked;
mod model;
mod notation;
mod paint;
mod part;
mod preview;
mod profile;
mod prompt;
mod scene_view;
mod selection;
mod session;
mod status;
mod timeline;
mod tool;
mod wording;

pub use cat_cad::CatCad;

#[cfg(test)]
mod tests;
