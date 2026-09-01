//! A run across a wave of an angle that wraps.

use crate::inline::Inline;
use std::f64::consts::{PI, TAU};

/// The angles where `round·cos ψ + up·sin ψ` comes to `to`, in no order.
///
/// **One solve for every sinusoid**, which is what a cosine and a sine of one
/// angle added come to: a single wave of size `hypot(round, up)` shifted to
/// `atan2(up, round)`, so the angles it takes a value at are that shift and one
/// `acos` either side of it.
///
/// **Two of them, or the one a graze leaves.** Where the value stands at the
/// very top or the very bottom of the wave, `acos` comes to nought or `π` and
/// the two angles either side of the bearing are one and the same angle. Handed
/// back twice it would make a stretch of no width, and a caller cutting a turn
/// at these angles then lays a place on the tangency itself rather than inside
/// anything — see [`seeded`](crate::solid::meeting::seeding::seeded), which is
/// such a caller.
///
/// Nothing where the value stands further off than the wave ever reaches, and
/// nothing for a wave of no size at all — which takes its one value everywhere
/// or nowhere, and neither is an angle to hand back.
pub(crate) fn angles(round: f64, up: f64, to: f64) -> Inline<f64, 2> {
    let mut found = Inline::none();
    let size = round.hypot(up);
    if size == 0.0 || to.abs() > size {
        return found;
    }
    let (turn, share) = (up.atan2(round), (to / size).acos());
    found.push(turn + share);
    if share > 0.0 && share < PI {
        found.push(turn - share);
    }
    found
}

/// Where along the run from the angle `from` to the angle `to` the sine of the
/// angle less `phase` comes to `sine`, in no order.
///
/// **Two answers at most, and the turn is the whole of the difficulty.** A sine
/// takes a value twice a turn — see [`angles`], which is the solve — and each of
/// those stands at every turn of the angle, where the run covers one stretch of
/// it. What comes back is the turn of each that the run's own span holds, and
/// nothing for the one it does not.
///
/// **Where a cut in a cylinder's parameters is fenced.** Both curved cuts a
/// cylinder can carry — the boolean's `Ripple` and `Bow` — turn where the sine
/// of their own angle reaches a value they solve for, and a fence laid at one
/// turn of it and not another is a root walked past.
///
/// Nothing for a run that stands at one angle, which has no span to hold a turn
/// in, and nothing for a value no sine reaches. Half open in how far along, so
/// a run's own far end is left to the run that starts there — and **half open
/// the same way whichever way the run goes**: how far along decides on its own,
/// so a run walked from a greater angle to a lesser holds its own near end
/// exactly as a forward one does.
///
/// **The angle alone**, where a caller holds a run of a surface's two
/// parameters: only the one the sine is taken of decides, so the other would be
/// a number handed over and never read.
pub(crate) fn met(sine: f64, phase: f64, from: f64, to: f64) -> Inline<f64, 2> {
    let mut found = Inline::none();
    let run = to - from;
    if run == 0.0 {
        return found;
    }
    let lo = from.min(to);
    for turn in angles(0.0, 1.0, sine) {
        let over = ((lo - phase - turn) / TAU).ceil();
        let angle = phase + turn + TAU * over;
        let along = (angle - from) / run;
        if (0.0..1.0).contains(&along) {
            found.push(along);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::number::tolerance::DRIFTING;
    use std::f64::consts::FRAC_PI_2;

    fn wave(round: f64, up: f64, angle: f64) -> f64 {
        round * angle.cos() + up * angle.sin()
    }

    /// Least first, so an answer given in no order can be held against one
    /// written down.
    fn sorted<const N: usize>(mut found: Inline<f64, N>) -> Inline<f64, N> {
        found.all_mut().sort_by(f64::total_cmp);
        found
    }

    /// **A crossing answers two angles and a graze answers one.**
    ///
    /// `cos ψ` takes nought at `±π/2` and takes one at nought alone, which is
    /// the whole of the difference: a value inside the wave is met on the way
    /// up and on the way down, and a value at the very top or the very bottom
    /// is met where the two meet each other. The bottom is the same case a half
    /// turn along, where `acos` comes to `π` and the angles either side of the
    /// bearing are one turn apart rather than two places.
    ///
    /// **Shifted, so the bearing is read rather than assumed.** `3·cos + 4·sin`
    /// is a wave of five bearing `atan2(4, 3)`, and its graze stands at that
    /// bearing however far it is from nought.
    #[test]
    fn a_graze_is_one_angle_where_a_crossing_is_two() {
        assert_eq!(
            sorted(angles(1.0, 0.0, 0.0)).all(),
            [-FRAC_PI_2, FRAC_PI_2],
            "cos is nought a quarter turn either side of nought",
        );
        assert_eq!(
            sorted(angles(1.0, 0.0, 1.0)).all(),
            [0.0],
            "cos is one once"
        );
        assert_eq!(
            sorted(angles(1.0, 0.0, -1.0)).all(),
            [PI],
            "cos is less one once"
        );

        let bearing = 4.0_f64.atan2(3.0);
        assert_eq!(
            sorted(angles(3.0, 4.0, 5.0)).all(),
            [bearing],
            "the top is one"
        );
        assert_eq!(
            sorted(angles(3.0, 4.0, -5.0)).all(),
            [bearing + PI],
            "the bottom is one",
        );

        // Inside the wave, where the two are two — and each of them is where
        // the wave really takes the value, which no count would say.
        let both = angles(3.0, 4.0, 4.0);
        assert_eq!(both.all().len(), 2, "a value under the top is met twice");
        assert_ne!(both.all()[0], both.all()[1], "twice at one angle");
        for &angle in both.all() {
            let at = wave(3.0, 4.0, angle);
            assert!((at - 4.0).abs() <= DRIFTING * 4.0, "{at} rather than 4");
        }

        assert!(angles(3.0, 4.0, 5.001).all().is_empty(), "past the wave");
        assert!(angles(0.0, 0.0, 0.0).all().is_empty(), "no wave at all");
    }

    /// **A run holds its own near end and leaves its far one, whichever way
    /// round it goes.**
    ///
    /// `sin` is nought at nought and at `π`, so the run from nought to `π`
    /// holds the first and leaves the second — and the run from `π` back to
    /// nought holds `π`, which is now the near end, and leaves nought. Both
    /// answer one root at nought of the way along, and a reader that fenced the
    /// far end alone would drop the second of them.
    ///
    /// **Read the other way round as a whole**: every place `x` of a forward
    /// run is the place `1 − x` of the same run reversed, near end and all. Two
    /// roots apart rather than one, so the pairing is a claim about the run and
    /// not about a single answer.
    ///
    /// **The phase is read**, which the run from nought to `π` shows on its
    /// own: shifted a quarter turn, the root inside it moves from its start to
    /// its middle.
    ///
    /// A graze is one root here as it is one angle above — see [`angles`] —
    /// where a doubled one would be a fence laid twice at one place.
    #[test]
    fn a_run_holds_its_near_end_whichever_way_it_runs() {
        fn held(named: &str, found: Inline<f64, 2>, want: &[f64]) {
            let found = sorted(found);
            let all = found.all();
            assert_eq!(all.len(), want.len(), "{named}: {all:?} against {want:?}");
            for (&at, &to) in all.iter().zip(want) {
                assert!(
                    (at - to).abs() <= DRIFTING,
                    "{named}: {at} rather than {to}"
                );
            }
        }

        held("forward", met(0.0, 0.0, 0.0, PI), &[0.0]);
        held("reversed", met(0.0, 0.0, PI, 0.0), &[0.0]);
        held("phased", met(0.0, FRAC_PI_2, 0.0, PI), &[0.5]);
        held("grazed", met(1.0, 0.0, 0.0, PI), &[0.5]);

        // `sin ψ = 1/2` at `π/6` and at `5π/6`, which are a twelfth and five
        // twelfths of the way round a whole turn.
        held("round", met(0.5, 0.0, 0.0, TAU), &[1.0 / 12.0, 5.0 / 12.0]);
        held(
            "round back",
            met(0.5, 0.0, TAU, 0.0),
            &[7.0 / 12.0, 11.0 / 12.0],
        );

        assert!(met(0.0, 0.0, 1.0, 1.0).all().is_empty(), "no run at all");
        assert!(met(1.5, 0.0, 0.0, TAU).all().is_empty(), "no sine reaches");
    }
}
