//! This suite's golden directory and how far a frame may drift from one.

use image::RgbaImage;
use palantir::golden::{Goldens, Tolerance};

/// Looser than Palantir's default, and for a reason that is about what CatCad
/// draws rather than about how careful anyone is being.
///
/// Two of the renderer's four passes ask for alpha-to-coverage, and WebGPU
/// leaves the mapping from alpha to a sample mask implementation-defined —
/// only "0 covers nothing" and "1 covers everything" are promised. Every
/// antialiased marker edge and circle rim is therefore the driver's to decide,
/// and a scene of strokes and rims is very nearly all edge. Palantir's 0.1%
/// suits flat axis-aligned UI; one rim here is already more pixels than that.
const TOLERANCE: Tolerance = Tolerance {
    per_channel: 2,
    max_ratio: 0.01,
};

pub(crate) fn assert_matches_golden(name: &str, frame: &RgbaImage) {
    Goldens::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/visual"))
        .tolerance(TOLERANCE)
        .assert_matches(name, frame);
}
