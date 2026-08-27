//! Cutting a region of a plane along a straight line.
//!
//! The half of a boolean that decides *shape*: every face of one body is cut by
//! every plane of the other that reaches it, and what falls out is a set of
//! regions each of which lies wholly inside the other body or wholly outside
//! it. That is the property the whole pipeline rests on — a region that
//! straddled the other body's boundary could not be classified at all — and it
//! is why the cut is taken along the whole line rather than along the stretch
//! where the two faces actually meet.
//!
//! **Cutting further than necessary is deliberate.** A plane of the other body
//! that only clips a corner still cuts this face from edge to edge, leaving two
//! regions where the boundary between them is no boundary at all. That costs
//! faces and, while every surface was a plane, nothing else: the two lie on one
//! surface, so the edge between them is flagged as no crease, they carry the
//! same [`Grown`](crate::solid::grown::Grown) name, and a caller asking what
//! faces the body has is answered in names rather than in pieces. See
//! `.notes/KERNEL.md` §4.4 and §5.
//!
//! It costs one thing more now that a surface may be round. What divides a face
//! has to be writable in that face's own parameters, and some crossings are not
//! — so a surface the face never actually meets can refuse the whole boolean
//! for a cut that would have divided nothing. Asking whether the *faces* meet
//! before asking whether the surfaces do is the answer, and it is not written.

use crate::loops::Loops;
use crate::math::arc;
use crate::math::quadratic;
use crate::math::winding::{self, holds};
use crate::number::predicate;
use crate::number::tolerance::{ENCLOSED, PLACED};
use glam::DVec2;
use std::f64::consts::{PI, TAU};
use std::ops::Range;

/// How finely a closed cut is flattened, as a fraction of its longer half.
///
/// **A classification tolerance and not a geometry one**, which is what lets it
/// be this coarse. What the corners are for is saying which region a place
/// falls in and how much one covers, and the body's own curve comes from the
/// meeting rather than from them — so what this has to be fine enough for is a
/// sample point landing on the right side of a hole, not for a face. Taken off
/// the tolerance ladder instead it would be seventy thousand chords to a
/// circle, for an answer no better.
const ROUNDED: f64 = 1e-3;

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
pub(super) enum Cut {
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
    /// A closed cut round an ellipse, the inside kept where `inward`.
    ///
    /// **Closed, which is the whole of what makes it a different case.** A
    /// straight cut always meets a region's boundary if it meets the region at
    /// all; a closed one can lie wholly within a region and take a disc out of
    /// its middle without touching an edge of it, and it can be crossed twice
    /// by one straight run of boundary. Both are states the straight arm never
    /// reaches — see [`Cut::closed`] and [`Cut::grazes`].
    ///
    /// **An ellipse and not a circle, which costs one divide and buys a
    /// milestone.** A circle is the pair of halves being equal, and everything
    /// below reads them as a pair either way — where a second variant for the
    /// round case would be eleven more arms saying nearly the same thing. A
    /// plane meeting a cylinder squarely gives the circle, and obliquely the
    /// ellipse; two equal cylinders on crossing axes give two of the second.
    Round {
        middle: DVec2,
        /// Unit, the way the longer half runs.
        along: DVec2,
        /// The two halves: `x` along [`Cut::Round::along`] and `y` across it.
        /// Equal for a circle.
        half: DVec2,
        /// Whether the inside is kept rather than everything but it.
        inward: bool,
        /// Which of the caller's runs this is — see [`Came::Arc`].
        ///
        /// **Bookkeeping on a piece of geometry, and it earns its place.** What
        /// a round cut puts down has to be marked with the curve it came from,
        /// and carried beside the cut instead it would be a second value
        /// threaded through six calls in step with the first — which is six
        /// chances to split by one cut and stamp another's number.
        run: u32,
    },
    /// A cut along `v = level + swing·cos(θ − phase)`, everything above it kept
    /// where `above`.
    ///
    /// **What an ellipse is in a cylinder's own parameters.** A plane meeting a
    /// cylinder obliquely crosses it in one, and so does a second cylinder of
    /// the same radius on a crossing axis — which is the mitred pipe and the
    /// Steinmetz solid, and between them the whole of what M5 had left. On a
    /// plane that curve is an ellipse and [`Cut::Round`] carries it; on the
    /// cylinder it is a *graph over the angle*, which is why it is its own
    /// shape rather than a case of one.
    ///
    /// Open, not closed: the parameter it is a graph over wraps, but a face may
    /// not — `.notes/KERNEL.md` §4.4 — so within any one face it runs right
    /// across like a line. It can still be met twice by one straight run, which
    /// a line cannot, and that is the one place the two part company.
    Wave {
        /// The height it swings about, and how far either way.
        level: f64,
        swing: f64,
        /// The angle its high side stands at.
        phase: f64,
        /// Whether what is kept is above it rather than below.
        above: bool,
        /// Which of the caller's runs this is — see [`Came::Arc`].
        run: u32,
    },
}

/// How much of a wave a straight run crosses, and where.
///
/// Three, because that is the most there can be: a run less than a whole turn
/// wide meets `v = swing·cos(θ − phase)` where a line meets a cosine, and the
/// difference of the two turns at most twice over that span — see
/// [`Cut::crested`].
#[derive(Debug, Clone, Copy, Default)]
struct Crested {
    along: [f64; 3],
    count: u8,
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
            Self::Round {
                middle,
                along,
                half,
                inward,
                run,
            } => Self::Round {
                middle,
                along,
                half,
                inward: !inward,
                run,
            },
            Self::Wave {
                level,
                swing,
                phase,
                above,
                run,
            } => Self::Wave {
                level,
                swing,
                phase,
                above: !above,
                run,
            },
        }
    }

    /// What the corners this cut puts down are marked with.
    ///
    /// A straight imprint needs nothing remembered about it — a line between
    /// two places is the same line whoever drew it — so only a circle is
    /// numbered.
    fn came(self) -> Came {
        match self {
            Self::Straight { run: Some(run), .. }
            | Self::Round { run, .. }
            | Self::Wave { run, .. } => Came::Arc(run),
            Self::Straight { run: None, .. } => Came::Edge,
        }
    }

    /// Whether it is a loop in its own right rather than a line across
    /// everything.
    fn closed(self) -> bool {
        matches!(self, Self::Round { .. })
    }

    /// How far off the cut `point` stands, positive on the side being kept.
    fn side(self, point: DVec2) -> f64 {
        match self {
            Self::Straight { at, along, .. } => along.perp_dot(point - at),
            // **How far off along the ray from the middle**, which is what a
            // radius is to a circle and the nearest thing an ellipse has to
            // one. A true distance to an ellipse is a quartic; this agrees
            // with it exactly where the two halves are equal, and everywhere
            // else it is the same sign and the same nought, which is all
            // [`Side::of`] reads and all the walk asks.
            Self::Round { inward, .. } => {
                let off = self.reach(point);
                if inward { off } else { -off }
            }
            // Straight up, which is a distance in `v` and an overstatement of
            // the distance to the wave itself by however steeply it runs. The
            // sign and the nought are exact, and those are what [`Side::of`]
            // and the walk read.
            Self::Wave { above, .. } => {
                let off = point.y - self.crest(point.x);
                if above { off } else { -off }
            }
        }
    }

    /// How high the wave stands at the angle `across`.
    fn crest(self, across: f64) -> f64 {
        let Self::Wave {
            level,
            swing,
            phase,
            ..
        } = self
        else {
            return 0.0;
        };
        level + swing * (across - phase).cos()
    }

    /// How far along the cut `point` stands.
    ///
    /// A distance for a line and an angle for a circle, and what the two have
    /// in common is the only thing read off them: they increase the way the cut
    /// runs, so ordering by this is ordering along the cut — see
    /// [`Splitting::close`], which reassembles by it and by nothing else.
    fn down(self, point: DVec2) -> f64 {
        match self {
            Self::Straight { at, along, .. } => along.dot(point - at),
            Self::Round {
                middle,
                half,
                inward,
                ..
            } => {
                let off = self.frame(point - middle) / half;
                let turned = off.y.atan2(off.x).rem_euclid(std::f64::consts::TAU);
                // Counterclockwise keeps the disc on the left, so keeping
                // everything *but* it runs the other way round.
                if inward {
                    turned
                } else {
                    std::f64::consts::TAU - turned
                }
            }
            // The angle itself, the wave being a graph over it. Keeping what is
            // above puts that on the left of a walk running the way the angle
            // grows; keeping what is below runs the other way.
            Self::Wave { above, .. } => {
                if above {
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
    fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        match self {
            Self::Straight { .. } => {
                let (here, there) = (self.side(from), self.side(to));
                from.lerp(to, here / (here - there))
            }
            Self::Round { .. } => {
                let [first, second] = self.roots(from, to).expect("the run crosses the ellipse");
                let along = if (0.0..=1.0).contains(&first) {
                    first
                } else {
                    second
                };
                from.lerp(to, along)
            }
            Self::Wave { .. } => {
                let crested = self.crested(from, to);
                let along = crested.along[..usize::from(crested.count)]
                    .iter()
                    .copied()
                    .find(|&along| (0.0..=1.0).contains(&along))
                    .expect("the run crosses the wave");
                from.lerp(to, along)
            }
        }
    }

    /// Where along the run from `from` to `to` the wave is met, in order.
    ///
    /// **Bisected, there being nothing to solve.** A straight run against
    /// `v = level + swing·cos(θ − phase)` is a line against a cosine, and that
    /// has no closed form — the one crossing in this kernel that has not. What
    /// there *is* in closed form is where the difference of the two turns:
    /// `swing·sin(θ − phase)·dθ = −dv`, which has at most two answers over a
    /// run less than a whole turn wide. Split there, the difference is monotone
    /// on each piece, so a sign change brackets exactly one root and bisection
    /// walks it down to the last bit the two ends can be told apart by.
    ///
    /// Converged rather than tolerated, which is the distinction that matters:
    /// what comes back is the root to the precision the numbers hold, not a
    /// place within some bound of it.
    fn crested(self, from: DVec2, to: DVec2) -> Crested {
        let Self::Wave { swing, phase, .. } = self else {
            return Crested::default();
        };
        let run = to - from;
        let at = |along: f64| {
            let place = from.lerp(to, along);
            place.y - self.crest(place.x)
        };
        // The run split where the difference turns, ends included. Two turns at
        // most, and each of them once: the run is a stretch of one face's own
        // parameters and no face wraps, so it reaches over less than a whole
        // turn — and which way round it runs says nothing about that, which is
        // why the span is taken as a range rather than walked from one end.
        let mut turns = [0.0, 1.0, 0.0, 0.0];
        let mut pieces = 2;
        let leaning = -run.y / (swing * run.x);
        if run.x != 0.0 && leaning.abs() <= 1.0 {
            let (lo, hi) = (from.x.min(to.x), from.x.max(to.x));
            let first = leaning.asin();
            for turn in [first, PI - first] {
                let over = ((lo - phase - turn) / TAU).ceil();
                let across = phase + turn + TAU * over;
                let along = (across - from.x) / run.x;
                if across < hi && (0.0..1.0).contains(&along) && pieces < turns.len() {
                    turns[pieces] = along;
                    pieces += 1;
                }
            }
        }
        let turns = &mut turns[..pieces];
        turns.sort_by(|one, two| one.partial_cmp(two).expect("a run is finite"));
        let mut crested = Crested::default();
        for step in 1..turns.len() {
            let (lo, hi) = (turns[step - 1], turns[step]);
            let (here, there) = (at(lo), at(hi));
            if here == 0.0 {
                continue;
            }
            if (here > 0.0) == (there > 0.0) {
                continue;
            }
            crested.along[usize::from(crested.count)] = halved(lo, hi, at);
            crested.count += 1;
        }
        crested
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
    fn grazes(self, from: DVec2, to: DVec2) -> Option<[DVec2; 2]> {
        let [first, second] = self.roots(from, to)?;
        let inside = |along: f64| (PLACED..=1.0 - PLACED).contains(&along);
        (inside(first) && inside(second)).then(|| [from.lerp(to, first), from.lerp(to, second)])
    }

    /// The place `down` along the cut, which is [`Cut::down`] read backwards.
    ///
    /// The pair is why both are written: `down(at(x)) == x` and `at(down(p))`
    /// is `p` back on the cut, so the walk can measure where it met the cut and
    /// the reassembly can put corners back along it without either of them
    /// spelling out which way round a circle runs. Nought for a straight cut,
    /// which has no corners of its own to give and never asks.
    fn at(self, down: f64) -> DVec2 {
        match self {
            Self::Straight { .. } => DVec2::ZERO,
            // `down` runs the way the cut does; counterclockwise keeps the
            // inside on the left, so the cut runs the other way round where it
            // is not the inside being kept.
            Self::Round {
                middle,
                along,
                half,
                inward,
                ..
            } => {
                let (across, ahead) = if inward { down } else { -down }.sin_cos();
                middle + along * (half.x * ahead) + along.perp() * (half.y * across)
            }
            // The same the other way about: keeping what is below runs the cut
            // against the angle.
            Self::Wave { above, .. } => {
                let across = if above { down } else { -down };
                DVec2::new(across, self.crest(across))
            }
        }
    }

    /// The corners of the cut between two places along it, in the direction it
    /// runs, exclusive of both.
    ///
    /// **Appends nothing for a straight cut**, and that is not an oversight: a
    /// stretch of a line between two points *is* the straight run between them,
    /// which whatever closes the loop has already got. A circle's is not, and a
    /// loop closed without this cuts the corner with a chord — a quarter disc
    /// coming back as the triangle under it.
    fn between(self, from: f64, to: f64, into: &mut Vec<Corner>) {
        match self {
            Self::Straight { .. } => {}
            Self::Round { half, .. } => {
                let sweep = (to - from).rem_euclid(TAU);
                let count = arc::chords(half.x, sweep, half.x * ROUNDED);
                into.extend((1..count).map(|step| Corner {
                    at: self.at(from + sweep * step as f64 / count as f64),
                    came: self.came(),
                }));
            }
            // Not wrapped, a wave being open: it runs from one edge of the
            // face to the other and `down` grows the whole way.
            Self::Wave { .. } => {
                let sweep = to - from;
                let count = rippled(sweep);
                into.extend((1..count).map(|step| Corner {
                    at: self.at(from + sweep * step as f64 / count as f64),
                    came: self.came(),
                }));
            }
        }
    }

    /// The cut as a loop of corners, wound so the side kept is on its left.
    ///
    /// **Appends nothing for a straight cut**, which is not a loop and cannot
    /// bound anything on its own.
    ///
    /// Flattened, and this is the one place in the boolean that flattens
    /// anything. What these corners are for is saying which region a place
    /// falls in and how much one covers; the *body* takes its curve from the
    /// meeting that produced the cut and never from here — see
    /// `.notes/KERNEL.md` §7.4.
    fn walk(self, into: &mut Vec<Corner>) {
        let Self::Round { half, .. } = self else {
            return;
        };
        let count = arc::chords(half.x, TAU, half.x * ROUNDED);
        into.reserve_exact(count);
        into.extend((0..count).map(|step| Corner {
            at: self.at(TAU * step as f64 / count as f64),
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
            Self::Round { middle, half, .. } => {
                // In the frame the ellipse is the unit circle in, where the run
                // is still a straight run and the meeting is still a quadratic.
                let run = self.frame(to - from) / half;
                let start = self.frame(from - middle) / half;
                quadratic::roots(
                    run.length_squared(),
                    2.0 * run.dot(start),
                    start.length_squared() - 1.0,
                )
            }
            // Two, or the run went across rather than dipping — which is the
            // same rule the ellipse answers by, and the reason both are `None`
            // for one root as well as for none.
            Self::Wave { .. } => {
                let crested = self.crested(from, to);
                (crested.count == 2).then(|| [crested.along[0], crested.along[1]])
            }
        }
    }

    /// `off` turned into the cut's own frame — along its longer half, and
    /// across it.
    ///
    /// A rotation and nothing else, so a length here is a length there. Takes a
    /// direction rather than a place, the middle being what tells the two
    /// apart and only some callers wanting it taken off.
    fn frame(self, off: DVec2) -> DVec2 {
        let Self::Round { along, .. } = self else {
            return DVec2::ZERO;
        };
        DVec2::new(off.dot(along), along.perp_dot(off))
    }

    /// How far `point` stands inside the ellipse, along the ray from its
    /// middle.
    ///
    /// Positive within. Reduces to `radius − |p − middle|` where the two halves
    /// are equal, which is what makes an ellipse the one shape here rather than
    /// a second one beside the circle.
    fn reach(self, point: DVec2) -> f64 {
        let Self::Round { middle, half, .. } = self else {
            return 0.0;
        };
        let off = self.frame(point - middle);
        let out = (off / half).length();
        if out < f64::MIN_POSITIVE {
            // The middle itself, where every ray is as good and the nearest
            // place on the ellipse is a shorter half away.
            return half.min_element();
        }
        off.length() * (1.0 / out - 1.0)
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

/// The place between `lo` and `hi` where `at` changes sign, found by halving.
///
/// Not [`Cut::between`], which walks a cut's own corners — this is the one
/// piece of arithmetic in the kernel with no closed form to reach for.
///
/// The two have to bracket one, which every caller has just shown. Halved until
/// the middle is one of the two ends, which is the last bit an `f64` holds
/// between them — about fifty rounds, and no tolerance anywhere in it.
fn halved(lo: f64, hi: f64, at: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    let rising = at(hi) > 0.0;
    loop {
        let middle = 0.5 * (lo + hi);
        if middle <= lo || middle >= hi {
            return middle;
        }
        if (at(middle) > 0.0) == rising {
            hi = middle;
        } else {
            lo = middle;
        }
    }
}

/// One corner of a region, and where the stretch of boundary *leaving* it came
/// from.
///
/// **Carried rather than worked out again later**, which is the whole of what
/// makes a curved boolean exact where its regions are not. A closed cut is
/// flattened to be classified — see [`ROUNDED`] — so a circle imprinted on a
/// face arrives here as a hundred corners; without this the sewing would lift
/// every one of them into a vertex and hang a hundred straight edges off them,
/// and a body whose faces are exact would be bounded by a polygon. With it the
/// sewing collapses the run back into the arc it came from and asks the meeting
/// for the curve.
///
/// Recovering it instead — asking, of each corner, whether it happens to lie on
/// one of the cuts — reads a *chord* of the imprint circle as an arc of it
/// wherever the face's own boundary already had two corners on that circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Corner {
    pub(super) at: DVec2,
    pub(super) came: Came,
}

impl winding::Place for Corner {
    fn place(self) -> DVec2 {
        self.at
    }
}

impl Marked for Corner {
    fn mark(&mut self) -> &mut Came {
        &mut self.came
    }
}

/// Whether the corner at `step` is one the boundary merely passes through.
///
/// **The rule that turns a flattened arc back into an arc.** True where the
/// stretch entering a corner and the stretch leaving it run along the same
/// imprint: the corner is then a place the flattening put there rather than a
/// place anything meets, and a body that kept it would have a vertex in the
/// middle of a circular edge and two straight edges either side of it.
///
/// Only an arc, never [`Came::Edge`]: two straight stretches meeting at a
/// corner are two edges however straight they both are, because what a face's
/// own boundary calls a corner is a corner.
pub(super) fn passing(walk: &[Corner], step: usize) -> bool {
    let before = walk[(step + walk.len() - 1) % walk.len()].came;
    matches!(walk[step].came, Came::Arc(_)) && walk[step].came == before
}

/// Something one step of a loop carries a [`Came`] on.
///
/// Two of them — a corner of a region being cut, and a vertex of a loop being
/// sewn — and both are walked the other way round at some point, which is the
/// one thing worth writing once. See [`turned`].
pub(super) trait Marked {
    fn mark(&mut self) -> &mut Came;
}

/// Walk one loop the other way round, marks and all.
///
/// **Not simply reversed**, which is the whole reason this is written down. A
/// mark says what the stretch *leaving* its step runs along; walked the other
/// way, the stretch leaving a step is the one that used to *enter* it — so the
/// marks step round by one as well as turning over, where the steps themselves
/// only turn over.
///
/// Over three steps `A B C` marked `a b c`, the loop reversed is `C B A` and
/// its stretches are `b a c`: turning the marks over gives `c b a`, and
/// stepping them round by one gives `b a c`.
/// In place and with one mark's worth of room, because a boolean turns a loop
/// round for every face it lays out and a document is rebuilt on every frame of
/// a drag: taking the marks out to rotate them would be a heap block per loop
/// per face per frame.
pub(super) fn turned(walk: &mut [impl Marked]) {
    walk.reverse();
    let Some(first) = walk.first_mut().map(|it| *it.mark()) else {
        return;
    };
    for step in 1..walk.len() {
        let mark = *walk[step].mark();
        *walk[step - 1].mark() = mark;
    }
    *walk
        .last_mut()
        .expect("a walk with a first has a last")
        .mark() = first;
}

/// Where one stretch of a region's boundary came from.
///
/// Two, and a straight cut is the first rather than a third: a line between two
/// places is the same line whoever drew it, so an imprint that is straight
/// needs nothing remembered about it. Only an arc does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Came {
    /// A straight run — of the face's own boundary, or of a straight imprint.
    Edge,
    /// A run along the curve at this index, which is the caller's to number —
    /// see [`Imprints`](super::imprints::Imprints), where one number per
    /// *stretch* and one per *curve* are held apart.
    Arc(u32),
}

/// Which side of a cut a corner fell.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
    /// On the side being kept, and not on the cut itself.
    Kept,
    /// On the cut, to within [`PLACED`].
    On,
    /// On the side being dropped.
    Dropped,
}

impl Side {
    fn of(cut: Cut, point: DVec2) -> Self {
        let off = cut.side(point);
        if predicate::touching(off.abs(), PLACED) {
            Self::On
        } else if off > 0.0 {
            Self::Kept
        } else {
            Self::Dropped
        }
    }
}

/// Regions of one plane, each an outline and the loops punched out of it.
///
/// Flat, like everything else the kernel holds several of: one buffer of loops
/// and a range per region. A cut reads one of these and writes another, and the
/// two are swapped rather than replaced, so cutting a face by a dozen planes
/// reaches the heap for none of them.
#[derive(Debug, Default)]
pub(super) struct Cells {
    loops: Loops<Corner>,
    /// Which runs each region owns: the outline first, then its holes.
    owned: Vec<Range<usize>>,
}

impl Cells {
    pub(super) fn clear(&mut self) {
        self.loops.clear();
        self.owned.clear();
    }

    /// How many regions there are.
    pub(super) fn len(&self) -> usize {
        self.owned.len()
    }

    /// Every loop of the region at `at`, the outline first.
    pub(super) fn cell(&self, at: usize) -> impl Iterator<Item = &[Corner]> + Clone {
        self.owned[at].clone().map(|run| self.loops.get(run))
    }

    /// Add a region, its loops written by `write` — the outline first.
    ///
    /// A region that writes no loop is no region, and is dropped rather than
    /// recorded: a cut that keeps nothing has to leave nothing behind, or every
    /// reader afterwards has to know about regions that are not there.
    pub(super) fn add(&mut self, write: impl FnOnce(&mut Loops<Corner>)) {
        let from = self.loops.len();
        write(&mut self.loops);
        if self.loops.len() > from {
            self.owned.push(from..self.loops.len());
        }
    }
}

/// Whether a region the cut runs along the boundary of is on the side kept.
///
/// **Which side it is on, where its boundary cannot say.** A corner on the cut
/// is on neither side, so the answer comes off the first corner that is not —
/// an outline standing clear of a hole the cut lies along, most often.
///
/// Where every corner is on it, the region *is* what the cut bounds, and then
/// the middle of the cut is the one place inside it there is to ask. A straight
/// cut has no middle and needs none: a region every corner of which lies on one
/// line has no width, and bounds nothing on either side of it.
fn kept<'a>(region: impl Iterator<Item = &'a [Corner]>, cut: Cut) -> bool {
    for walk in region {
        for corner in walk {
            match Side::of(cut, corner.at) {
                Side::On => continue,
                side => return side == Side::Kept,
            }
        }
    }
    match cut {
        Cut::Round { middle, .. } => cut.side(middle) > 0.0,
        // Neither of these is closed, so a region every corner of which lies on
        // one has no width and bounds nothing on either side of it.
        Cut::Straight { .. } | Cut::Wave { .. } => false,
    }
}

/// Cuts regions along a line or a circle, keeping the room it works in.
#[derive(Debug, Default)]
pub(super) struct Splitting {
    /// Whether the last split met a shape it does not handle.
    ///
    /// One flag rather than a `Result` through five calls: what a caller does
    /// about it is the same whatever met it — refuse the whole boolean — and
    /// the walk that finds one is three frames down from the call that reports.
    beyond: bool,
    /// Whether a loop of the region being cut lay wholly *on* the cut.
    ///
    /// A flag for the reason the one above is: what finds it is the walk over
    /// one loop, and what has to act on it is the walk over the region those
    /// loops belong to. See [`Splitting::region`].
    alongside: bool,
    /// Which side of the cut each corner of the loop being walked fell.
    sides: Vec<Side>,
    /// A closed cut as a loop of its own, flattened — see [`Cut::walk`].
    round: Vec<Corner>,
    /// One loop with a place put in each dip the cut takes out of it — see
    /// [`Splitting::dip`].
    dipped: Vec<Corner>,
    /// The one stretch of boundary the walk is inside, before it reaches the
    /// cut again — see [`Splitting::chain`].
    stretch: Vec<Corner>,
    /// The stretches of boundary that survived, laid end to end.
    ///
    /// Open, unlike everything else here: each runs from where the boundary
    /// entered the kept side to where it left again, and closing them is what
    /// the reassembly below is for.
    chains: Loops<Corner>,
    /// Where each chain of `chains` begins and ends along the cut.
    ends: Vec<Ends>,
    /// The loops the cut never reached, which come through whole.
    whole: Loops<Corner>,
    /// The chains in the order the cut meets them, and whether each has been
    /// taken into a loop yet.
    order: Vec<usize>,
    taken: Vec<bool>,
    /// One reassembled loop, before it is known to be an outline or a hole.
    closed: Loops<Corner>,
    /// What each of those loops shuts in and where it lies — see [`Shut`].
    shut: Vec<Shut>,
    /// Which outline each hole was found inside, in outline order — see
    /// [`Splitting::gather`], which is the only thing that reads it.
    held: Vec<Held>,
}

/// What one reassembled loop shuts in, and the box it lies in.
///
/// The box is what keeps a hole from being cast against every outline in full:
/// a place outside the box is outside the loop, and that is two comparisons
/// where the ray cast is a walk of the whole boundary.
#[derive(Debug, Clone, Copy)]
struct Shut {
    /// The true area rather than the shoelace's own doubling of it, so what it
    /// is held against is a bound on an area and reads as one.
    area: f64,
    low: DVec2,
    high: DVec2,
}

impl Shut {
    fn of(walk: &[Corner]) -> Self {
        let mut low = DVec2::splat(f64::INFINITY);
        let mut high = DVec2::splat(f64::NEG_INFINITY);
        for corner in walk {
            low = low.min(corner.at);
            high = high.max(corner.at);
        }
        Self {
            area: winding::swept(walk) / 2.0,
            low,
            high,
        }
    }

    /// Whether the box holds `at`, which anything inside the loop does.
    ///
    /// A loop with nothing in it holds nowhere, its box having come out
    /// inverted — which is what [`Bounds`](crate::math::bounds::Bounds) means
    /// by the same pair one dimension up.
    fn holds(self, at: DVec2) -> bool {
        at.cmpge(self.low).all() && at.cmple(self.high).all()
    }
}

/// One hole, and the outline it was found inside.
#[derive(Debug, Clone, Copy)]
struct Held {
    by: u32,
    hole: u32,
}

/// Where one open chain of surviving boundary begins and ends, measured along
/// the cut.
#[derive(Debug, Clone, Copy)]
struct Ends {
    entered: f64,
    left: f64,
}

impl Splitting {
    /// Cut every region of `from` along `cut`, keeping both sides, and write
    /// what falls out into `into`.
    ///
    /// `into` is emptied first. What comes back is every region the cut leaves,
    /// each wholly to one side of it — which is the property a boolean needs
    /// before it can ask of any of them whether to keep it.
    pub(super) fn split(&mut self, from: &Cells, cut: Cut, into: &mut Cells) -> bool {
        into.clear();
        self.beyond = false;
        self.append(from, cut, into);
        self.append(from, cut.turned(), into);
        !self.beyond
    }

    /// The same, onto whatever `into` already holds.
    fn append(&mut self, from: &Cells, cut: Cut, into: &mut Cells) {
        for at in 0..from.len() {
            self.region(from.cell(at), cut, into);
        }
    }

    /// One region, cut.
    fn region<'a>(
        &mut self,
        region: impl Iterator<Item = &'a [Corner]> + Clone,
        cut: Cut,
        into: &mut Cells,
    ) {
        self.chains.clear();
        self.ends.clear();
        self.whole.clear();
        self.alongside = false;
        let held = region.clone();
        for walk in region {
            self.chain(walk, cut);
        }
        // **A cut running along a loop of the boundary divides nothing.** The
        // region is whole on one side of it and absent from the other, which is
        // the round answer to what a coplanar pair of faces is in the flat one:
        // the surface is described twice and at most one description survives.
        // Reached whenever a body is cut against one grown off the same circle
        // — a boss on a plate, a second feature off the drawing that made the
        // first — and read any other way the region comes back with the cut
        // added as a second copy of a hole it already had.
        //
        // Nothing crossed, which a closed cut running along a loop guarantees —
        // a hole cannot be crossed by the outline that holds it — and a
        // straight one does not: a zero-width loop lying on a line says nothing
        // about a cut that divides the region elsewhere.
        if self.alongside && self.chains.len() == 0 {
            if kept(held.clone(), cut) {
                into.add(|write| {
                    for walk in held {
                        write.push(walk);
                    }
                });
            }
            return;
        }
        // **A closed cut the boundary never met.** A circle can lie wholly
        // within a region and take a disc out of its middle without touching an
        // edge of it, which is the one way a cut divides something while
        // crossing nothing — and it is what a plane meeting a cylinder does to
        // the end of a block it bores through. A straight cut has no such case:
        // one that meets a region at all meets its boundary.
        if cut.closed() && self.chains.len() == 0 {
            self.punch(held, cut, into);
            return;
        }
        self.close(cut);
        self.gather(into);
    }

    /// What a closed cut lying clear of the boundary leaves.
    ///
    /// **Four answers off two questions**, which is why this is not the walk
    /// with a special case bolted on. The cut met no boundary, so every corner
    /// of the region is on one side of it — and the region either holds the
    /// circle or does not:
    ///
    /// | | circle within the region | circle clear of it |
    /// | --- | --- | --- |
    /// | corners kept | the region, the circle punched out of it | the region whole |
    /// | corners dropped | the disc, and the region's holes inside it | nothing |
    ///
    /// The two on the left are the cut dividing something it never touched,
    /// which is what a plane meeting a cylinder does to the end of a block it
    /// is bored through. The two on the right are a cut that missed, and they
    /// are why "the corners are on the dropped side" is not on its own an
    /// answer: a region swallowed whole by the disc has every corner *kept* and
    /// nothing punched out of it.
    fn punch<'a>(
        &mut self,
        held: impl Iterator<Item = &'a [Corner]> + Clone,
        cut: Cut,
        into: &mut Cells,
    ) {
        self.round.clear();
        cut.walk(&mut self.round);
        let Some(somewhere) = self.round.first().map(|corner| corner.at) else {
            return;
        };
        let mut loops = held.clone();
        let Some(outline) = loops.next() else {
            return;
        };
        // Within the region, which is within its outline and within none of its
        // holes. Asked of one point of the circle because the circle meets no
        // boundary: every point of it stands where that one does.
        let within = holds(outline, somewhere) && !loops.any(|walk| holds(walk, somewhere));
        // Which side the region is on, off any corner of it — the cut met no
        // boundary, so they are all on the one side.
        let kept = cut.side(outline[0].at) > 0.0;
        let round = &self.round;
        match (kept, within) {
            // The region, with one more hole in it.
            (true, true) => into.add(|write| {
                for walk in held {
                    write.push(walk);
                }
                write.push(round);
            }),
            // The region, untouched.
            (true, false) => into.add(|write| {
                for walk in held {
                    write.push(walk);
                }
            }),
            // The disc, and the region's own holes that fell in it.
            (false, true) => into.add(|write| {
                write.push(round);
                for walk in held.skip(1).filter(|walk| holds(round, walk[0].at)) {
                    write.push(walk);
                }
            }),
            // A cut that missed.
            (false, false) => {}
        }
    }

    /// Walk `walk` again with a place put in the middle of every dip the cut
    /// takes out of it, and say whether there was one to put.
    ///
    /// **What gives the walk a corner that fell away.** A closed cut can bite
    /// into a straight run of boundary and come out again without either end of
    /// that run leaving the kept side — see [`Cut::grazes`] — and then no corner
    /// of the loop is on the dropped side for [`Splitting::chain`] to begin at.
    /// The bite's two crossings are the chord of a circle, so the place halfway
    /// between them lies inside it, which is the dropped side exactly when the
    /// corners are all on the kept one: a run between two places *inside* a
    /// circle never leaves it, so a dip at all means the outside is what is
    /// being kept.
    ///
    /// `false` where the place put there is on the cut rather than across it,
    /// which is a bite shallower than [`PLACED`] and no bite at all. Walking
    /// again would find the same thing and put the same place, so it is refused
    /// instead — the one thing here that must not be a loop that never ends.
    fn dip(&mut self, walk: &[Corner], cut: Cut) -> bool {
        let count = walk.len();
        // Lent out and handed back, because [`Splitting::chain`] writes the
        // buffers beside it and cannot be called while this one is borrowed.
        let mut dipped = std::mem::take(&mut self.dipped);
        dipped.clear();
        for at in 0..count {
            dipped.push(walk[at]);
            if let Some([out, again]) = cut.grazes(walk[at].at, walk[(at + 1) % count].at) {
                dipped.push(Corner {
                    at: (out + again) / 2.0,
                    came: walk[at].came,
                });
            }
        }
        let fell = dipped
            .iter()
            .any(|corner| Side::of(cut, corner.at) == Side::Dropped);
        if fell {
            self.chain(&dipped, cut);
        }
        self.dipped = dipped;
        fell
    }

    /// Break one loop into the stretches of it that lie on the kept side.
    ///
    /// A loop the cut never crosses comes through whole or not at all. One it
    /// does cross comes through as open chains, each recorded with where along
    /// the cut it began and ended, because that is what says which chain
    /// carries on from which.
    fn chain(&mut self, walk: &[Corner], cut: Cut) {
        let count = walk.len();
        self.sides.clear();
        self.sides
            .extend(walk.iter().map(|corner| Side::of(cut, corner.at)));
        if !self.sides.contains(&Side::Dropped) {
            // **Every corner on the kept side, and the boundary still dipping
            // across a closed cut between two of them.** A circle clipping the
            // side of a region between two of its corners, which is what a
            // shaft with a flat milled down it does to the face the flat is
            // cut by. The walk below needs a corner that fell away to start
            // from, so that nothing is closed before it was opened — so one is
            // put where the dip is and the loop walked again.
            if (0..count).any(|at| cut.grazes(walk[at].at, walk[(at + 1) % count].at).is_some()) {
                if !self.dip(walk, cut) {
                    self.beyond = true;
                }
                return;
            }
            // Nothing of it fell away. A loop lying wholly *on* the cut is the
            // cut running along the boundary rather than dividing it, which is
            // the region's business and not this loop's.
            if self.sides.contains(&Side::Kept) {
                self.whole.push(walk);
            } else {
                self.alongside = true;
            }
            return;
        }
        let Some(start) = self.sides.iter().position(|&side| side == Side::Dropped) else {
            return;
        };
        // Walked from a corner that fell away, so the first chain opened is a
        // chain the boundary genuinely entered on rather than one it was
        // already inside when the walk began. `entered` is where along the cut
        // that one began, and `None` while none is open.
        let mut entered: Option<f64> = None;
        for step in 0..count {
            let here = (start + step) % count;
            let next = (start + step + 1) % count;
            let (from, to) = (walk[here], walk[next]);
            let (leaving, arriving) = (self.sides[here], self.sides[next]);
            // A place the cut put there, and so a place the boundary leaves
            // along the cut — where a corner of the region's own boundary
            // carries whatever it already carried.
            let onto = |at: DVec2| Corner {
                at,
                came: cut.came(),
            };
            let back = |at: DVec2| Corner {
                at,
                came: from.came,
            };
            if leaving == Side::Kept && entered.is_some() {
                self.stretch.push(from);
            }
            match (leaving, arriving) {
                // Onto the kept side, across the line or off a corner standing
                // on it. Either way the chain begins where the cut is met.
                (Side::Dropped, Side::Kept) => {
                    let at = cut.crossing(from.at, to.at);
                    entered = Some(cut.down(at));
                    self.open(back(at));
                }
                (Side::On, Side::Kept) => {
                    entered = Some(cut.down(from.at));
                    self.open(from);
                }
                // And off it again.
                (Side::Kept, Side::Dropped) => {
                    let at = cut.crossing(from.at, to.at);
                    self.shut(&mut entered, onto(at), cut);
                }
                (Side::Kept, Side::On) => self.shut(&mut entered, onto(to.at), cut),
                // Both ends on one side, with the run between them dipping
                // across the cut and back. Only a closed cut can be met twice
                // by one straight run — see [`Cut::grazes`] — and stepping over
                // it would lose a whole stretch of boundary.
                (Side::Kept, Side::Kept) => {
                    if let Some([out, again]) = cut.grazes(from.at, to.at) {
                        self.shut(&mut entered, onto(out), cut);
                        entered = Some(cut.down(again));
                        self.open(back(again));
                    }
                }
                (Side::Dropped, Side::Dropped) => {
                    if let Some([into, out]) = cut.grazes(from.at, to.at) {
                        entered = Some(cut.down(into));
                        self.open(back(into));
                        self.shut(&mut entered, onto(out), cut);
                    }
                }
                // Both ends away from it, or an edge lying along it — neither
                // of which opens or closes anything.
                _ => {}
            }
        }
        debug_assert!(entered.is_none(), "a chain was left open by a closed loop");
    }

    /// Begin a chain at `at`, over the room the last one took.
    ///
    /// One chain is open at a time, so one buffer serves every chain of every
    /// loop rather than each taking one of its own.
    fn open(&mut self, at: Corner) {
        self.stretch.clear();
        self.stretch.push(at);
    }

    /// Record a chain that has just reached the cut again at `at`.
    ///
    /// There is always one open: the walk starts from a corner that fell away,
    /// so every stretch on the kept side was entered before it could be left.
    /// Reaching here with nothing open would mean the walk had lost count of
    /// which side it was on.
    fn shut(&mut self, entered: &mut Option<f64>, at: Corner, cut: Cut) {
        let entered = entered.take().expect("a chain is left only once entered");
        self.stretch.push(at);
        self.chains.push(&self.stretch);
        self.ends.push(Ends {
            entered,
            left: cut.down(at.at),
        });
    }

    /// Join the open chains back into closed loops.
    ///
    /// **Along the cut, in the direction that keeps the region on the left.**
    /// A chain ends where the boundary left the kept side; the region carries
    /// on along the cut from there until the boundary comes back, which is the
    /// next chain to begin at or after that point. Sorted, that is the next one
    /// along — which is the whole of the reassembly, and the reason the ends
    /// were measured rather than merely remembered.
    fn close(&mut self, cut: Cut) {
        self.closed.clear();
        self.order.clear();
        self.order.extend(0..self.chains.len());
        let ends = &self.ends;
        self.order.sort_by(|&a, &b| {
            ends[a]
                .entered
                .partial_cmp(&ends[b].entered)
                .expect("finite")
        });
        self.taken.clear();
        self.taken.resize(self.chains.len(), false);

        for start in 0..self.order.len() {
            if self.taken[self.order[start]] {
                continue;
            }
            let Self {
                chains,
                ends,
                order,
                taken,
                closed,
                ..
            } = self;
            closed.add(|into| {
                let mut at = start;
                loop {
                    let chain = order[at];
                    taken[chain] = true;
                    into.extend_from_slice(chains.get(chain));
                    // Along the cut to where the boundary comes back: the first
                    // chain that begins at or after this one left off. Sorted
                    // by where each begins, so that is the nearest one along —
                    // and none that far along means round the far end of the
                    // cut to the first of all.
                    let left = ends[chain].left;
                    // Halved rather than walked from the front: `order` was
                    // just sorted by the very number this asks about.
                    let found = order.partition_point(|&chain| ends[chain].entered < left - PLACED);
                    let next = if found == order.len() { 0 } else { found };
                    // Along the cut itself, where the cut has a length worth
                    // walking. A straight one has not — see [`Cut::between`].
                    cut.between(left, ends[order[next]].entered, into);
                    // **Back where the loop began, which is what closes it.**
                    // Asked of the position rather than of whether the chain
                    // has been walked: a chain reached again is the loop coming
                    // round, where one merely *taken* is a different loop that
                    // happens to have been closed already — and stopping on
                    // that would join two regions that never touch.
                    if next == start {
                        break;
                    }
                    at = next;
                }
            });
        }
        // The loops the cut never reached are closed already, and belong beside
        // the ones that had to be closed again.
        for walk in self.whole.iter() {
            self.closed.push(walk);
        }
    }

    /// Sort the closed loops and the untouched ones into regions.
    ///
    /// By signed area, exactly as the two-dimensional arrangement does: what
    /// comes out positive is a region, what comes out negative is a hole in
    /// one. Which region each hole belongs to is the outline it stands inside,
    /// of which there is exactly one — the regions a cut leaves are disjoint,
    /// so there is no tighter container to prefer.
    fn gather(&mut self, into: &mut Cells) {
        let Self {
            closed, shut, held, ..
        } = self;
        shut.clear();
        shut.extend(closed.iter().map(Shut::of));

        // **Each hole to its outline once, before any region is written.**
        // Deciding it inside the walk over the outlines asked every loop about
        // every other, and each of those questions is a ray cast over a whole
        // boundary. Here each hole asks until it is answered, and the answer is
        // sorted into outline order for the walk below to read a run of.
        held.clear();
        for (hole, punched) in shut.iter().enumerate() {
            if punched.area >= -ENCLOSED {
                continue;
            }
            let at = closed.get(hole)[0].at;
            // The first outline that holds it, which is the only one: the
            // regions one cut leaves are disjoint, so there is no tighter
            // container to prefer and nothing to go on looking for.
            let mut inside = shut.iter().enumerate().filter(|&(by, outline)| {
                outline.area > ENCLOSED && outline.holds(at) && holds(closed.get(by), at)
            });
            if let Some((by, _)) = inside.next() {
                debug_assert!(inside.next().is_none(), "two outlines hold one hole");
                held.push(Held {
                    by: by as u32,
                    hole: hole as u32,
                });
            }
        }
        // By the outline, and by the hole within it, so a region's holes come
        // out in the order the loops were reassembled in however the outlines
        // fell.
        held.sort_unstable_by_key(|it| (it.by, it.hole));

        let mut from = 0;
        for (at, outline) in shut.iter().enumerate() {
            if outline.area <= ENCLOSED {
                continue;
            }
            // Sorted by the outline, and the outlines taken in the same order,
            // so what is left of the list begins with this outline's holes.
            let to = from + held[from..].partition_point(|it| it.by == at as u32);
            into.add(|loops| {
                loops.push(closed.get(at));
                for held in &held[from..to] {
                    loops.push(closed.get(held.hole as usize));
                }
            });
            from = to;
        }
    }
}

#[cfg(test)]
mod internals {
    use super::*;

    impl Cells {
        /// The outline of the region at `at`.
        pub(super) fn outline(&self, at: usize) -> &[Corner] {
            self.loops.get(self.owned[at].start)
        }
    }

    impl Splitting {
        /// Cut every region of `from` along `cut`, keeping the left of it, and
        /// write what survives into `into`.
        ///
        /// `into` is emptied first. Cutting the other way is the same call with
        /// [`Cut::turned`]. What production wants is both sides at once, which
        /// is [`Splitting::split`]; one side is what a test asks for, to say
        /// which of the two a given piece ended up in.
        pub(super) fn halve(&mut self, from: &Cells, cut: Cut, into: &mut Cells) -> bool {
            into.clear();
            self.beyond = false;
            self.append(from, cut, into);
            !self.beyond
        }
    }
}

#[cfg(test)]
mod tests;
