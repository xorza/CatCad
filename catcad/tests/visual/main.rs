//! Rendering the real app to a texture, and the checks that buys.
//!
//! Two kinds of check live under here. Most measure a property of the frame —
//! how wide a stroke came out, where a marker landed, how round a rim stayed —
//! and say what has to hold whatever else changes. The golden tests say only
//! "this is what it looked like", which catches what nobody thought to measure
//! at the cost of failing on every deliberate change. Keep the second kind few.
//!
//! **The frame is the view and not the window.** [`CatCad`](catcad::CatCad)
//! records its drawing under a HUD, and none of that chrome is what any of this
//! is about: a golden holding a status line to the byte fails when a tool is
//! renamed, and a sweep counting pixels of a colour finds buttons. So the
//! harness paints the app to build the scene and then paints that scene again
//! through a bare pane — see `harness::shown`.
//!
//! The alternative was driving a window and screenshotting the compositor,
//! which makes every measurement depend on where the window landed and on
//! nothing else having stolen focus. Palantir renders headlessly on the same
//! path a window uses, so a frame here is the frame a user would see, minus
//! the window.
//!
//! | module | asks |
//! |---|---|
//! | `harness` | getting a frame out of the app at all |
//! | `ink` | reading one: what a stroke deposited, what is lit, where a run landed |
//! | `overlays` | how wide a stroke is drawn and how round a rim stays |
//! | `depth` | what hides what, and what shows through |
//! | `lettering` | where type is drawn, and what hides it |
//! | `projection` | where a point lands, and what parallel rays do to a width |
//! | `retained` | what the renderer keeps between frames |
//! | `picking` | what a press on the drawing finds, and what a drag does with it |
//! | `goldens` | the whole picture, unchanged |

mod depth;
mod goldens;
mod harness;
mod ink;
mod lettering;
mod overlays;
mod picking;
mod projection;
mod retained;
