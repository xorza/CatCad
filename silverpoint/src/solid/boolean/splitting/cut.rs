//! A cut across a face, and what a region is asked about one.

use crate::loops::Loops;
use crate::math::arc;
use crate::math::bisect;
use crate::math::quadratic;
use crate::number::tolerance::PLACED;
use crate::solid::boolean::splitting::bow::{Bow, Bowed};
use crate::solid::boolean::splitting::corner::{Came, Corner};
use crate::solid::boolean::splitting::oval::Oval;
use crate::solid::boolean::splitting::reading::Reading;
use crate::solid::boolean::splitting::ripple::Ripple;
use crate::solid::boolean::splitting::traced::Traced;
use crate::solid::geometry::curve::Curve;
use glam::DVec2;
use std::f64::consts::{PI, TAU};

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
/// Five shapes, and what they have in common is that each divides the whole of
/// a face rather than a stretch of one. What a cut is *not* is a segment —
/// every stage downstream needs each region to be wholly one thing or the
/// other, and a cut that stopped part way would leave a region straddling it.
/// See `.notes/KERNEL.md` §7.4.
///
/// **Borrowed for the one call that splits by it**, which is what the fifth
/// shape asks and what nothing here loses by: a cut is built by `imprinted`
/// and read by [`Splitting::split`](super::Splitting::split), and no stage
/// keeps one. See [`Traced`], which is the arm that carries the borrow.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Cut<'a> {
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
    /// A cut traced from a curve's own places, which is what any curve with no
    /// closed form above gets — see [`Traced`]. A marched run and a general
    /// quartic both land here.
    Traced(Traced<'a>),
}

impl<'a> Cut<'a> {
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
            Self::Traced(traced) => Self::Traced(traced.turned()),
        }
    }

    /// What the corners this cut puts down at `at` are marked with.
    ///
    /// A straight imprint needs nothing remembered about it — a line between
    /// two places is the same line whoever drew it — so only a circle is
    /// numbered.
    ///
    /// **Asked of a place, because a marched cut is several curves.** A meeting
    /// walked rather than written down comes in pieces and the cut is the whole
    /// of it, so which curve a corner lies on is which piece it stands on — see
    /// [`Traced`]. Every other shape is one curve and reads nothing, so a run
    /// of corners along one asks this once and carries the answer.
    ///
    /// `at` has to be a place *on* the cut: a marched one finds the piece
    /// nearest it, and a place off the cut answers with whichever piece it
    /// happens to lie nearest.
    pub(super) fn came(self, at: DVec2) -> Came {
        match self {
            Self::Straight { run: Some(run), .. }
            | Self::Round(Oval { run, .. })
            | Self::Wave(Ripple { run, .. })
            | Self::Bow(Bow { run, .. }) => Came::Arc(run),
            Self::Traced(traced) => traced.came(at),
            Self::Straight { run: None, .. } => Came::Edge,
        }
    }

    /// Which piece of it the parameter `at` runs along.
    ///
    /// **One piece for every cut but a traced one**, which is the only shape
    /// that comes in disjoint curves — see [`Traced::piece`]. A meeting written
    /// down as one circle, one wave or one bow is one curve, however far round
    /// itself it goes.
    pub(super) fn piece(self, at: f64) -> usize {
        match self {
            Self::Traced(traced) => traced.piece(at),
            _ => 0,
        }
    }

    /// Whether it is a loop in its own right rather than a line across
    /// everything.
    pub(super) fn closed(self) -> bool {
        match self {
            Self::Round(_) => true,
            Self::Bow(bow) => bow.closed(),
            Self::Traced(traced) => traced.closed(),
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
            // The true distance to the *other surface*, which is the one shape
            // here that has one to give — see [`Traced::side`].
            Self::Traced(traced) => traced.side(point),
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
            // How far round the run it was walked as, measured from a place
            // the face does not hold — see [`Traced::down`].
            Self::Traced(traced) => traced.down(point),
        }
    }

    /// Where the stretch of boundary leaving `from` and reaching `to` crosses
    /// it.
    ///
    /// **On the run that stretch walks, and on the straight line between the
    /// two corners only where it walks none** — see [`Reading`], which argues
    /// what the difference between the two is worth.
    pub(super) fn crossing(self, from: Corner, to: Corner, reading: Reading<'_>) -> DVec2 {
        if let Came::Arc(run) = from.came
            && let Some(curve) = reading.curved(run)
        {
            return self.met_along(curve, from, to, reading);
        }
        self.met_across(from.at, to.at)
    }

    /// Where the stretch leaving `from` crosses it, walking `curve` between the
    /// two corners rather than the straight run between them — see [`Reading`],
    /// which argues why that is the difference between a body and a refusal.
    fn met_along(self, curve: Curve, from: Corner, to: Corner, reading: Reading<'_>) -> DVec2 {
        let start = reading.along(curve, from.at);
        let end = reading.along(curve, to.at);
        let middle = (from.at + to.at) / 2.0;
        // **Which way round the stretch runs, measured rather than assumed.** A
        // curve answers where a place stands on it in one turn, so the two
        // corners give the same pair of answers whichever way the boundary
        // walks between them. What tells the two apart is the stretch itself:
        // the way round whose middle stands nearer the middle of the straight
        // run between the corners is the way the boundary goes.
        //
        // Taking the near way round instead is right for a flattening, whose
        // corners are a chord apart, and wrong for the case that has no near
        // way — a face wrapping a whole cylinder is two corners half a turn
        // apart, and half a turn is the same distance both ways.
        let near = start + (end - start + PI).rem_euclid(TAU) - PI;
        let far = near - TAU.copysign(near - start);
        let strays = |ended: f64| {
            reading
                .at(curve, (start + ended) / 2.0, middle)
                .distance(middle)
        };
        let end = if strays(far) < strays(near) {
            far
        } else {
            near
        };
        // Read on the branch the loop itself runs on, which is what the
        // straight run between the corners is still good for: the face's
        // parameters are unwrapped along a loop — see
        // [`Face::flatten`](crate::solid::topology::face::Face) — so a curve's
        // own answer is as near the wrong end of a long stretch as the right
        // one.
        let place = |part: f64| {
            let along = start + (end - start) * part;
            reading.at(curve, along, from.at.lerp(to.at, part))
        };
        let part = bisect::crossed(0.0, 1.0, |part| self.side(place(part)))
            .expect("the stretch crosses the cut");
        place(part)
    }

    /// The same, across the straight run from `from` to `to`.
    ///
    /// The two have to be on opposite sides, which every caller has just
    /// established — so for a line the denominator is away from nought by at
    /// least twice [`PLACED`], and for a circle exactly one root of the two
    /// lies on the run.
    fn met_across(self, from: DVec2, to: DVec2) -> DVec2 {
        let [from, to] = ordered(from, to);
        if let Self::Straight { .. } = self {
            let (here, there) = (self.side(from), self.side(to));
            return from.lerp(to, here / (here - there));
        }
        if let Self::Traced(traced) = self {
            return traced.crossing(from, to);
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
        if let Self::Traced(traced) = self {
            return traced.grazes(from, to);
        }
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
    ///
    /// `false` where there is no such stretch, which only a marched cut has —
    /// see [`Traced::between`].
    pub(super) fn between(self, from: f64, to: f64, into: &mut Vec<Corner>) -> bool {
        match self {
            Self::Straight { .. } => {}
            Self::Round(oval) => {
                let sweep = (to - from).rem_euclid(TAU);
                let count = arc::chords(oval.half.x, sweep, oval.half.x * ROUNDED);
                let came = self.came(oval.at(from));
                into.extend((1..count).map(|step| Corner {
                    at: oval.at(from + sweep * step as f64 / count as f64),
                    came,
                }));
            }
            // Not wrapped, a wave being open: it runs from one edge of the
            // face to the other and `down` grows the whole way.
            Self::Wave(ripple) => {
                let sweep = to - from;
                let count = ripple.steps(sweep);
                let came = self.came(ripple.at(from));
                into.extend((1..count).map(|step| Corner {
                    at: ripple.at(from + sweep * step as f64 / count as f64),
                    came,
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
                let came = self.came(bow.at(from));
                into.extend((1..count).map(|step| Corner {
                    at: bow.at(from + sweep * step as f64 / count as f64),
                    came,
                }));
            }
            // The piece's own places rather than a shape read at a step, a
            // marched curve having no formula to read — see [`Traced::lay`].
            Self::Traced(traced) => return traced.between(from, to, into),
        }
        true
    }

    /// The cut as loops of corners, each wound so the side kept is on its left.
    ///
    /// **Appends nothing for a cut that is not closed**, which is not a loop
    /// and cannot bound anything on its own.
    ///
    /// Flattened, and this is the one place in the boolean that flattens
    /// anything. What these corners are for is saying which region a place
    /// falls in and how much one covers; the *body* takes its curve from the
    /// meeting that produced the cut and never from here — see
    /// `.notes/KERNEL.md` §7.4.
    pub(super) fn walk(self, into: &mut Loops<Corner>) {
        match self {
            Self::Round(oval) => into.add(|write| {
                let count = arc::chords(oval.half.x, TAU, oval.half.x * ROUNDED);
                let came = self.came(oval.at(0.0));
                write.reserve_exact(count);
                write.extend((0..count).map(|step| Corner {
                    at: oval.at(TAU * step as f64 / count as f64),
                    came,
                }));
            }),
            Self::Bow(bow) if bow.closed() => into.add(|write| {
                let count = bow.steps(TAU);
                let came = self.came(bow.at(0.0));
                write.reserve_exact(count);
                write.extend((0..count).map(|step| Corner {
                    at: bow.at(TAU * step as f64 / count as f64),
                    came,
                }));
            }),
            // **Several loops rather than one**, a marched meeting coming in
            // pieces — see [`Traced`].
            Self::Traced(traced) => traced.walk(into),
            Self::Straight { .. } | Self::Wave(_) | Self::Bow(_) => {}
        }
    }

    /// Where along the run from `from` to `to` the cut is met, in order.
    ///
    /// Nothing for a straight cut, which is not that it is never met: a line is
    /// met once and [`Cut::crossing`] has a better reading of where, so what
    /// this is for — a boundary that crosses the cut and comes back — a line
    /// does not have.
    ///
    /// And nothing for a marched one, which is met more times than an inline
    /// answer holds: both of its callers reach [`Traced`] before they reach
    /// here.
    fn met(self, from: DVec2, to: DVec2) -> Bowed {
        let mut met = Bowed::none();
        match self {
            Self::Straight { .. } | Self::Traced(_) => {}
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

/// The two ends of one stretch in one order, whichever way round the walk
/// handed them over.
///
/// **What makes a crossing the same place from either side of it.** A cut is
/// taken twice over the region it divides — once keeping each side — and a
/// later cut meets the two halves of the stretch it left walking opposite
/// ways. Interpolated from one end, a crossing rounds a little differently from
/// the same crossing interpolated from the other, so the two halves come back
/// carrying places an ulp apart and nothing downstream can tell that they are
/// one place. See `.notes/KERNEL.md` §9.1, where what that costs is argued.
///
/// Any total order does, so long as it is the same one on both sides. This is
/// [`f64::total_cmp`] over the two coordinates in turn, which orders every pair
/// of places a walk can hand over and reads nothing but the values themselves.
fn ordered(from: DVec2, to: DVec2) -> [DVec2; 2] {
    let by = from.x.total_cmp(&to.x).then(from.y.total_cmp(&to.y));
    match by.is_le() {
        true => [from, to],
        false => [to, from],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A crossing is the same place whichever end of the stretch it is
    /// measured from**, which is what lets two regions either side of one cut
    /// be told they share it.
    ///
    /// A cut is taken twice over the region it divides, once keeping each side,
    /// so the stretch it leaves is walked one way by one half and the other way
    /// by the other. A later cut then meets both, and the place it reads has to
    /// be one place: `from + t·(to − from)` and `to + (1 − t)·(from − to)` are
    /// the same point in arithmetic and two points in an `f64`, an ulp apart —
    /// which is a stretch nothing downstream can tell is shared.
    ///
    /// **And the naive form really does disagree**, which the count below is
    /// what holds: a test whose fixture happened to round alike either way
    /// would pass with the ordering taken out again.
    #[test]
    fn a_crossing_reads_the_same_from_either_end_of_its_stretch() {
        let cut = Cut::Straight {
            at: DVec2::new(0.3, 0.7),
            along: DVec2::new(1.0, 3.0).normalize(),
            run: None,
        };
        let naive = |from: DVec2, to: DVec2| {
            let (here, there) = (cut.side(from), cut.side(to));
            from.lerp(to, here / (here - there))
        };

        let mut asked = 0;
        let mut fooled = 0;
        for x in -9..10 {
            for y in -9..10 {
                let from = DVec2::new(f64::from(x) / 7.0, f64::from(y) / 11.0);
                let to = from + DVec2::new(1.0 / 3.0, -5.0 / 13.0);
                // Only a stretch that genuinely crosses has a crossing.
                if (cut.side(from) > 0.0) == (cut.side(to) > 0.0) {
                    continue;
                }
                asked += 1;
                assert_eq!(
                    cut.met_across(from, to),
                    cut.met_across(to, from),
                    "{from:?} to {to:?} crosses at two places",
                );
                if naive(from, to) != naive(to, from) {
                    fooled += 1;
                }
            }
        }
        assert!(asked > 20, "only {asked} of the grid crossed the cut");
        assert!(
            fooled * 3 > asked,
            "the naive form disagreed on only {fooled} of {asked}, which is no rounding to fix",
        );
    }
}
