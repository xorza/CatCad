//! What the view makes of the pointer, without an application around it.
//!
//! Every test here drives a [`RaisedView`](harness::RaisedView) — the document,
//! the history, the build and the session with a `SceneView` over them and a
//! harness to record frames into. What the whole *app* makes of the same
//! gestures is asked at the crate root instead.

mod camera;
mod dragging;
mod harness;
mod intents;
mod picking;
mod tools;
