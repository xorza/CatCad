//! Which way round a closed run of corners goes, and how much it shuts in.

use glam::DVec2;

/// Twice the area a closed run of corners shuts in, positive counterclockwise.
///
/// **Twice**, because that is where the shoelace naturally stops and both
/// readers want something different from it: one asks only the sign, and the
/// other holds it against a bound it can halve as easily as this can double.
/// Doubling it here and halving it there would be two roundings for no gain.
///
/// The run is closed whether or not its last corner repeats its first — the
/// walk wraps — so a caller need not decide which convention it is using.
pub(crate) fn swept(walk: &[DVec2]) -> f64 {
    let mut total = 0.0;
    for (at, &here) in walk.iter().enumerate() {
        total += here.perp_dot(walk[(at + 1) % walk.len()]);
    }
    total
}
