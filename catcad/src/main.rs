//! CatCad application entry point.

use catcad::CatCad;
use palantir::{WindowToken, WinitHost, WinitHostError};
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), WinitHostError> {
    logging();
    WinitHost::builder(WindowToken(0))
        .title("CatCad")
        .build(CatCad::new)?
        .run()
}

/// Send what the program has to say to the terminal it was started from.
///
/// **The binary's, and only the binary's.** A library that installed one would
/// be taking the decision from whoever links it, which is why the subscriber
/// crate is reached from here and from nowhere under `lib.rs`.
///
/// `RUST_LOG` selects, by target and by level: `RUST_LOG=catcad.overlay=trace`
/// asks the overlay alone, `catcad=debug` asks all of it, and
/// `catcad=debug,palantir.repaint=trace` asks this program's own decisions
/// alongside the frames the window decided to draw. Warnings and above without
/// it, so a run nobody asked anything of still says when something was refused.
///
/// **Nothing below `info` is in a release build** unless it was built with the
/// defaults off — see the `quiet` feature, which is where that is argued. So a
/// filter naming `trace` against an ordinary release build selects nothing, and
/// that is the trade rather than an oversight.
fn logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        // The wall clock says nothing a frame counter does not say better, and
        // a line per frame is easier to read without one.
        .without_time()
        .with_target(true)
        .init();
}
