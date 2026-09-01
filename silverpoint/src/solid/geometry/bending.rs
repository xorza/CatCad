//! How fast and how hard the angle a crossing cylinder is reached at moves.

/// Bounds on the first two derivatives, with the parameter, of an angle that
/// reads `asin` of a cosine.
///
/// **What both curves a cylinder crossing a cylinder leaves are chorded by.**
/// The curve is a [`Saddle`](super::saddle::Saddle) in the world and a `Bow` in
/// the wider cylinder's own two parameters, and each is walked at a chord count
/// taken against how hard it bends — see
/// [`arc::chords`](crate::math::arc::chords). What the two share is the angle
/// the narrower cylinder is reached at. How the *place* moves with it is each
/// curve's own business, and is where the two part company.
///
/// **Finite because the cross-sections are nested**, which is what keeps the
/// sine below one. The pair that is not — a drilling that runs out of the far
/// side, which is an open bow — reaches no such angle and is bounded another
/// way.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bending {
    /// A bound on how fast the angle moves.
    pub(crate) turn: f64,
    /// A bound on how hard it turns.
    pub(crate) bend: f64,
}

impl Bending {
    /// The bounds for a cylinder of radius `across` whose axis passes that of
    /// one of radius `reach` by `off`, square to both.
    ///
    /// With `q = across/reach` and `s = (across + |off|)/reach`, the two come
    /// to `q/√(1 − s²)` and `q/√(1 − s²) + s·q²/(1 − s²)^{3/2}`.
    pub(crate) fn of(across: f64, off: f64, reach: f64) -> Self {
        let quick = across / reach;
        let most = (across + off.abs()) / reach;
        let leaning = (1.0 - most * most).sqrt();
        let turn = quick / leaning;
        Self {
            turn,
            bend: turn + most * quick * quick / (leaning * leaning * leaning),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// **The two bounds really bound the angle, and neither is idle.**
    ///
    /// The angle is `asin((across·cos t + off)/reach)`, which is what a cut
    /// round a crossing cylinder is read at. Its two derivatives are taken by
    /// central differences over a whole turn and held against what
    /// [`Bending::of`] answers. No reading may pass either bound — and the
    /// worst reading has to come within a factor of four of it, or a bound of
    /// infinity would pass the first half of this and say nothing.
    ///
    /// **Four rather than one, because the bound takes the worst of each
    /// factor separately** and the two do not fall at one parameter: the angle
    /// moves quickest a quarter turn along, where the sine it reads is
    /// smallest, and reads its largest sine a quarter turn away from there.
    /// Measured, the overstatement runs from three hundredths where the
    /// drilling is narrow to three and a third where the two cross-sections
    /// come near to touching.
    ///
    /// **A step of a ten-thousandth**, which is where the two errors of a
    /// second difference cross: the truncation goes as the step squared and the
    /// rounding as the machine over it squared, and at `1e-4` both are near
    /// `1e-8`. The slack allowed is a ten-thousandth, which is four decades of
    /// room over that and four under the bounds themselves.
    ///
    /// Three pairs, so the answer is read off the geometry rather than being
    /// one number that happens to fit.
    #[test]
    fn the_bounds_hold_the_angle_and_are_not_idle() {
        let step = 1e-4;
        let slack = 1e-4;
        for (named, across, off, reach) in [
            ("narrow", 0.25, 0.0, 1.0),
            ("wide", 0.8, 0.0, 1.0),
            ("wide and off", 0.5, 0.4, 1.0),
        ] {
            let of = Bending::of(across, off, reach);
            let angle = |t: f64| ((across * t.cos() + off) / reach).asin();
            let (mut quickest, mut hardest) = (0.0_f64, 0.0_f64);
            for at in 0..1000 {
                let t = TAU * at as f64 / 1000.0;
                let (back, here, ahead) = (angle(t - step), angle(t), angle(t + step));
                quickest = quickest.max(((ahead - back) / (2.0 * step)).abs());
                hardest = hardest.max(((ahead - 2.0 * here + back) / (step * step)).abs());
            }
            assert!(
                quickest <= of.turn + slack,
                "{named}: it moves at {quickest}, past a bound of {}",
                of.turn,
            );
            assert!(
                hardest <= of.bend + slack,
                "{named}: it turns at {hardest}, past a bound of {}",
                of.bend,
            );
            assert!(
                quickest * 4.0 >= of.turn,
                "{named}: it moves at {quickest} against an idle bound of {}",
                of.turn,
            );
            assert!(
                hardest * 4.0 >= of.bend,
                "{named}: it turns at {hardest} against an idle bound of {}",
                of.bend,
            );
        }

        // A wider drilling turns harder, which is what says the radii decide
        // the answer rather than the shape of the formula.
        let narrow = Bending::of(0.25, 0.0, 1.0);
        let wide = Bending::of(0.8, 0.0, 1.0);
        assert!(
            wide.turn > narrow.turn && wide.bend > narrow.bend,
            "{wide:?} against {narrow:?}",
        );
    }
}
