//! What the app decides, from the sketch it opens with to the frames it
//! records.
//!
//! Every test here drives the whole application through
//! [`Raised`](harness::Raised) — a real `CatCad` recording into a headless
//! harness — so what is asked is what a user would have done and what is read
//! is what the app made of it. The view on its own, without an application
//! around it, is asked next door in
//! [`scene_view`](crate::scene_view) instead.

mod editing;
mod fields;
mod filing;
mod harness;
mod opening;
mod tools;
mod undo;
