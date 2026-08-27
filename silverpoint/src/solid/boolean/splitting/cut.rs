//! A cut across a face, and what a region is asked about one.

use crate::math::arc;
use crate::math::quadratic;
use crate::number::tolerance::PLACED;
use crate::solid::boolean::splitting::corner::{Came, Corner};
use crate::solid::boolean::splitting::oval::Oval;
use crate::solid::boolean::splitting::ripple::Ripple;
use glam::DVec2;
use std::f64::consts::TAU;

/// How finely a closed cut is flattened, as a fraction of its longer half.
///
/// **A classification tolerance and not a geometry one**, which is what lets it
/// be this coarse. What the corners are for is saying which region a place
/// falls in and how much one covers, and the body's own curve comes from the
/// meeting rather than from them — so what this has to be fine enough for is a
/// sample point landing on the right side of a hole, not for a face. Taken off
/// the tolerance ladder instead it would be seventy thousand chords to a
/// circle, for an answer no better.
pub(super) const ROUNDED: f64 = 1e-3;

/// A cut across a surface's own parameters, with a side to keep.
///
/// The side kept is always the *left* of the way the cut runs, which is what
/// makes cutting both ways one operation asked twice — see [`Cut::turned`].
///
/// Two shapes, because two are what the exact tier can put on a plane: a plane
/// meets a plane in a line and a cylinder or a sphere in a circle. What a cut
/// is *not* is a segment — every stage downstream needs each region to be
/// wholly one thing or the other, and a cut that stopped part way would leave a
/// region straddling it. See `.notes/KERNEL.md` §7.4.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Cut {
    /// A straight cut, the left of `along` kept.
    Straight {
        /// Somewhere on it.
        at: DVec2,
        /// Unit, the way it runs.
        along: DVec2,
        /// Which of the caller's runs this is, where the curve it came from is
        /// worth remembering.
        ///
        /// **A straight cut is not always a straight edge**, which is the whole
        /// reason this is here: a circle square to a cylinder's axis is the line
        /// `v = that` in the cylinder's parameters, and an edge along it that
        /// came back straight would be a chord across the bore rather than its
        /// rim. `None` only for a genuine line — a plane meeting a plane.
        run: Option<u32>,
    },
    /// A closed cut round an ellipse, the inside kept where its own `inward`
    /// says — see [`Oval`], which is the whole of what one is.
    Round(Oval),
    /// A cut along a cosine of the angle, kept above or below as its own
    /// `above` says — see [`Ripple`].
    Wave(Ripple),
}

impl Cut {
    /// The same cut with the other side kept.
    pub(super) fn turned(self) -> Self {
        match self {
            Self::Straight { at, along, run } => Self::Straight {
                at,
                along: -along,
                run,
            },
            Self::Round(oval) => Self::Round(Oval {
                inward: !oval.inward,
                ..oval
            }),
            Self::Wave(ripple) => Self::Wave(Ripple {
                above: !ripple.above,
                ..ripple
            }),
        }
    }

    /// What the corners this cut puts down are marked with.
    ///
    /// A straight imprint needs nothing remembered about it — a line between
    /// two places is the same line whoever drew it — so only a circle is
    /// numbered.
    pub(super) fn came(self) -> Came {
        match self {
            Self::Straight { run: Some(run), .. }
            | Self::Round(Oval { run, .. })
            | Self::Wave(Ripple { run, .. }) => Came::Arc(run),
            Self::Straight { run: None, .. } => Came::Edge,
        }
    }

    /// Whether it is a loop in its own right rather than a line across
    /// everything.
    pub(super) fn closed(self) -> bool {
        matches!(self, Self::Round(_))
    }

    /// How far off the cut `point` stands, positive on the side being kept.
    pub(super) fn side(self, point: DVec2) -> f64 {
        match self {
            Self::Straight { at, along, .. } => along.perp_dot(point - at),
            // **How far off along the ray from the middle**, which is what a
            // radius is to a circle and the nearest thing an ellipse has to
            // one. A true distance to an ellipse is a quartic; this agrees
            // with it exactly where the two halves are equal, and everywhere
            // else it is the same sign and the same nought, which is all
            // [`Side::of`] reads and all the walk asks.
            Self::Round(oval) => {
                let off = oval.reach(point);
                if oval.inward { off } else { -off }
            }
            // Straight up, which is a distance in `v` and an overstatement of
            // the distance to the wave itself by however steeply it runs. The
            // sign and the nought are exact, and those are what [`Side::of`]
            // and the walk read.
            Self::Wave(ripple) => {
                let off = point.y - ripple.crest(point.x);
                if ripple.above { off } else { -off }
            }
        }
    }

    /// How far along the cut `point` stands.
    ///
    /// A distance for a line and an angle for a circle, and what the two have
    /// in common is the only thing read off them: they increase the way the cut
    /// runs, so ordering by this is ordering along the cut — see
    /// `Splitting::close`, which reassembles by it and by nothing else.
    ///
    /// Read backwards by [`Oval::at`] and [`Ripple::at`], which is why both are
    /// written: `down(at(x)) == x` and `at(down(p))` is `p` back on the cut, so
    /// the walk can measure where it met the cut and the reassembly can put
    /// corners back along it without either spelling out which way round a
    /// circle runs. A straight cut has no corners of its own to give and never
    /// asks, which is why the pair is on the two curved shapes and not here.
    pub(super) fn down(self, point: DVec2) -> f64 {
        match self {
            Self::Straight { at, along, .. } => along.dot(point - at),
            Self::Round(oval) => {
                let off = oval.frame(point - oval.middle) / oval.half;
                let turned = off.y.atan2(off.x).rem_euclid(TAU);
                // Counterclockwise keeps the disc on the left, so keeping
                // everything *but* it runs the other way round.
                if oval.inward { turned } else { TAU - turned }
            }
            // The angle itself, the wave being a graph over it. Keeping what is
            // above puts that on the left of a walk running the way the angle
            // grows; keeping what is below runs the other way.
            Self::Wave(ripple) => {
                if ripple.above {
                    point.x
                } else {
                    -point.x
                }
            }
        }
    }

    /// Where the straight run from `from` to `to` crosses it.
    ///
    /// The two have to be on opposite sides, which every caller has just
    /// established — so for a line the denominator is away from nought by at
    /// least twice [`PLACED`], and for a circle exactly one root of the two
    /// lies on the run.
    pub(super) fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        match self {
            Self::Straight { .. } => {
                let (here, there) = (self.side(from), self.side(to));
                from.lerp(to, here / (here - there))
            }
            Self::Round(_) => {
                let [first, second] = self.roots(from, to).expect("the run crosses the ellipse");
                let along = if (0.0..=1.0).contains(&first) {
                    first
                } else {
                    second
                };
                from.lerp(to, along)
            }
            Self::Wave(ripple) => {
                let crested = ripple.crested(from, to);
                let along = crested
                    .into_iter()
                    .find(|&along| (0.0..=1.0).contains(&along))
                    .expect("the run crosses the wave");
                from.lerp(to, along)
            }
        }
    }

    /// Where the straight run from `from` to `to` crosses it *twice*, both ends
    /// standing on the same side.
    ///
    /// The case a bent cut has and a straight one cannot: a run whose ends are
    /// both outside an ellipse can still pass through it, and one whose ends
    /// both stand above a wave can still dip below it — so what this finds is a
    /// boundary crossing the cut and back between two of its corners, which the
    /// walk would otherwise step straight over. A line has no such case, and
    /// [`Cut::roots`] answers `None` for one.
    pub(super) fn grazes(self, from: DVec2, to: DVec2) -> Option<[DVec2; 2]> {
        let [first, second] = self.roots(from, to)?;
        let inside = |along: f64| (PLACED..=1.0 - PLACED).contains(&along);
        (inside(first) && inside(second)).then(|| [from.lerp(to, first), from.lerp(to, second)])
    }

    /// The corners of the cut between two places along it, in the direction it
    /// runs, exclusive of both.
    ///
    /// **Appends nothing for a straight cut**, and that is not an oversight: a
    /// stretch of a line between two points *is* the straight run between them,
    /// which whatever closes the loop has already got. A circle's is not, and a
    /// loop closed without this cuts the corner with a chord — a quarter disc
    /// coming back as the triangle under it.
    pub(super) fn between(self, from: f64, to: f64, into: &mut Vec<Corner>) {
        match self {
            Self::Straight { .. } => {}
            Self::Round(oval) => {
                let sweep = (to - from).rem_euclid(TAU);
                let count = arc::chords(oval.half.x, sweep, oval.half.x * ROUNDED);
                into.extend((1..count).map(|step| Corner {
                    at: oval.at(from + sweep * step as f64 / count as f64),
                    came: self.came(),
                }));
            }
            // Not wrapped, a wave being open: it runs from one edge of the
            // face to the other and `down` grows the whole way.
            Self::Wave(ripple) => {
                let sweep = to - from;
                let count = rippled(sweep);
                into.extend((1..count).map(|step| Corner {
                    at: ripple.at(from + sweep * step as f64 / count as f64),
                    came: self.came(),
                }));
            }
        }
    }

    /// The cut as a loop of corners, wound so the side kept is on its left.
    ///
    /// **Appends nothing for a cut that is not closed**, which is not a loop
    /// and cannot bound anything on its own.
    ///
    /// Flattened, and this is the one place in the boolean that flattens
    /// anything. What these corners are for is saying which region a place
    /// falls in and how much one covers; the *body* takes its curve from the
    /// meeting that produced the cut and never from here — see
    /// `.notes/KERNEL.md` §7.4.
    pub(super) fn walk(self, into: &mut Vec<Corner>) {
        let Self::Round(oval) = self else {
            return;
        };
        let count = arc::chords(oval.half.x, TAU, oval.half.x * ROUNDED);
        into.reserve_exact(count);
        into.extend((0..count).map(|step| Corner {
            at: oval.at(TAU * step as f64 / count as f64),
            came: self.came(),
        }));
    }

    /// Where along the run from `from` to `to` it meets the circle, in order.
    ///
    /// `None` where it misses or merely grazes, and where the cut is straight
    /// and has no two roots to speak of. The same rule the surfaces are met by
    /// one dimension up — see [`roots`](crate::math::quadratic::roots), which
    /// is also where a graze is argued to be a miss.
    fn roots(self, from: DVec2, to: DVec2) -> Option<[f64; 2]> {
        match self {
            Self::Straight { .. } => None,
            Self::Round(oval) => {
                // In the frame the ellipse is the unit circle in, where the run
                // is still a straight run and the meeting is still a quadratic.
                let run = oval.frame(to - from) / oval.half;
                let start = oval.frame(from - oval.middle) / oval.half;
                quadratic::roots(
                    run.length_squared(),
                    2.0 * run.dot(start),
                    start.length_squared() - 1.0,
                )
            }
            // Two, or the run went across rather than dipping — which is the
            // same rule the ellipse answers by, and the reason both are `None`
            // for one root as well as for none.
            Self::Wave(ripple) => ripple.crested(from, to).all().try_into().ok(),
        }
    }
}

/// How many chords a stretch of `sweep` of a wave is worth.
///
/// A cosine bends no harder than its own swing, so a chord over `h` of angle
/// strays by at most `swing·h²/8` — and the sagitta wanted is [`ROUNDED`] of
/// that same swing, which leaves `h = √(8·ROUNDED)` whatever the swing is. In
/// the face's own parameters, where the classification happens and the body
/// never looks.
fn rippled(sweep: f64) -> usize {
    ((sweep.abs() / (8.0 * ROUNDED).sqrt()).ceil() as usize).max(1)
}
