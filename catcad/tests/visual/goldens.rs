//! This suite's golden directory and how far a frame may drift from one.

use image::RgbaImage;
use palantir::golden::{Goldens, Tolerance};

/// Exact, which is tighter than Palantir's own default and possible for the
/// same reason it is necessary.
///
/// These goldens are not committed — see this crate's `.gitignore` — so a
/// golden is only ever compared against frames from the machine that wrote it.
/// The driver that decides how alpha becomes a sample mask is therefore a
/// constant rather than a variable, and what is left is deterministic:
/// re-rendering the demo twice over gives back the same bytes.
///
/// It has to be exact to be worth anything. A budget measured against the whole
/// frame cannot see a change to one primitive, because no one primitive is much
/// of a frame: correcting how a ring converts its radius to pixels moved every
/// pixel of every rim in the demo and scored 0.22% against the 1% that used to
/// be allowed. A golden that passes when the thing it is watching changes
/// entirely is only a slow way of rendering.
///
/// What this costs is that every deliberate change to what is drawn is a
/// `UPDATE_GOLDEN=1` run, and so is a driver or OS update. That is the trade a
/// golden is for — see the note at the top of `main.rs` on keeping them few.
const TOLERANCE: Tolerance = Tolerance {
    per_channel: 0,
    max_ratio: 0.0,
};

pub(crate) fn assert_matches_golden(name: &str, frame: &RgbaImage) {
    Goldens::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/visual"))
        .tolerance(TOLERANCE)
        .assert_matches(name, frame);
}
