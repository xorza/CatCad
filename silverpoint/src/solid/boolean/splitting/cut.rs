//! A cut across a face, and what a region is asked about one.

use crate::math::arc;
use crate::math::quadratic;
use crate::number::tolerance::PLACED;
use crate::solid::boolean::splitting::bow::{Bow, Bowed};
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
/// Four shapes, and what they have in common is that each divides the whole of
/// a face rather than a stretch of one. What a cut is *not* is a segment —
/// every stage downstream needs each region to be wholly one thing or the
/// other, and a cut that stopped part way would leave a region straddling it.
/// See `.notes/KERNEL.md` §7.4.
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
    /// A cut along a root of a sine of the angle, which is closed or open as
    /// its own [`Bow::closed`] says — see [`Bow`], which is the one shape here
    /// that is either.
    Bow(Bow),
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
            Self::Bow(bow) => Self::Bow(Bow {
                inward: !bow.inward,
                ..bow
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
            | Self::Wave(Ripple { run, .. })
            | Self::Bow(Bow { run, .. }) => Came::Arc(run),
            Self::Straight { run: None, .. } => Came::Edge,
        }
    }

    /// Whether it is a loop in its own right rather than a line across
    /// everything.
    pub(super) fn closed(self) -> bool {
        match self {
            Self::Round(_) => true,
            Self::Bow(bow) => bow.closed(),
            Self::Straight { .. } | Self::Wave(_) => false,
        }
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
            // Two measures in one arm, a bow being closed or open — see
            // [`Bow::side`], where each is argued.
            Self::Bow(bow) => bow.side(point),
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
            // An angle round the loop where it is closed and the cylinder's own
            // angle where it is not, which is the two above under one call.
            Self::Bow(bow) => bow.down(point),
        }
    }

    /// Where the straight run from `from` to `to` crosses it.
    ///
    /// The two have to be on opposite sides, which every caller has just
    /// established — so for a line the denominator is away from nought by at
    /// least twice [`PLACED`], and for a circle exactly one root of the two
    /// lies on the run.
    pub(super) fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        if let Self::Straight { .. } = self {
            let (here, there) = (self.side(from), self.side(to));
            return from.lerp(to, here / (here - there));
        }
        let along = self
            .met(from, to)
            .into_iter()
            .find(|&along| (0.0..=1.0).contains(&along))
            .expect("the run crosses the cut");
        from.lerp(to, along)
    }

    /// Where the straight run from `from` to `to` crosses it *twice*, both ends
    /// standing on the same side.
    ///
    /// The case a bent cut has and a straight one cannot: a run whose ends are
    /// both outside an ellipse can still pass through it, and one whose ends
    /// both stand above a wave can still dip below it — so what this finds is a
    /// boundary crossing the cut and back between two of its corners, which the
    /// walk would otherwise step straight over. A line has no such case, and
    /// [`Cut::met`] answers with nothing for one.
    pub(super) fn grazes(self, from: DVec2, to: DVec2) -> Option<[DVec2; 2]> {
        // **Two, or the run went across rather than dipping.** One crossing is
        // a boundary that ends on the far side and the walk has it already;
        // none at all, or the one a straight cut always answers, is nothing to
        // find here. A graze is a miss for the reason
        // [`roots`](crate::math::quadratic::roots) argues one dimension up.
        let [first, second]: [f64; 2] = self.met(from, to).all().try_into().ok()?;
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
                let count = ripple.steps(sweep);
                into.extend((1..count).map(|step| Corner {
                    at: ripple.at(from + sweep * step as f64 / count as f64),
                    came: self.came(),
                }));
            }
            // Wrapped where the loop closes and not where it does not, which
            // is the two above told apart by the one thing that tells them
            // apart anywhere.
            Self::Bow(bow) => {
                let sweep = if bow.closed() {
                    (to - from).rem_euclid(TAU)
                } else {
                    to - from
                };
                let count = bow.steps(sweep);
                into.extend((1..count).map(|step| Corner {
                    at: bow.at(from + sweep * step as f64 / count as f64),
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
        match self {
            Self::Round(oval) => {
                let count = arc::chords(oval.half.x, TAU, oval.half.x * ROUNDED);
                into.reserve_exact(count);
                into.extend((0..count).map(|step| Corner {
                    at: oval.at(TAU * step as f64 / count as f64),
                    came: self.came(),
                }));
            }
            Self::Bow(bow) if bow.closed() => {
                let count = bow.steps(TAU);
                into.reserve_exact(count);
                into.extend((0..count).map(|step| Corner {
                    at: bow.at(TAU * step as f64 / count as f64),
                    came: self.came(),
                }));
            }
            Self::Straight { .. } | Self::Wave(_) | Self::Bow(_) => {}
        }
    }

    /// Where along the run from `from` to `to` the cut is met, in order.
    ///
    /// Nothing for a straight cut, which is not that it is never met: a line is
    /// met once and [`Cut::crossing`] has a better reading of where, so what
    /// this is for — a boundary that crosses the cut and comes back — a line
    /// does not have.
    fn met(self, from: DVec2, to: DVec2) -> Bowed {
        let mut met = Bowed::none();
        match self {
            Self::Straight { .. } => {}
            Self::Round(oval) => {
                // In the frame the ellipse is the unit circle in, where the run
                // is still a straight run and the meeting is still a quadratic.
                let run = oval.frame(to - from) / oval.half;
                let start = oval.frame(from - oval.middle) / oval.half;
                let roots = quadratic::roots(
                    run.length_squared(),
                    2.0 * run.dot(start),
                    start.length_squared() - 1.0,
                );
                for along in roots.into_iter().flatten() {
                    met.push(along);
                }
            }
            Self::Wave(ripple) => {
                for along in ripple.crested(from, to) {
                    met.push(along);
                }
            }
            Self::Bow(bow) => met = bow.bowed(from, to),
        }
        met
    }
}
