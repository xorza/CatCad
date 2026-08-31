//! Finding one place on every piece of a curve that has to be walked.
//!
//! **The hard half of the marched route**, and the reason it is hard is not
//! that one place is difficult to reach: a Newton correction finds one from
//! almost anywhere. It is that a curve comes in *pieces*, and a search that
//! samples a grid finds a small piece by luck — see `.notes/KERNEL.md` §7.3,
//! where a loop `0.137` across wants a quarter of a million samples once it
//! stands half a cell off a node.
//!
//! **So it is done per pair and solved rather than searched**, which is the same
//! bargain the reducible table strikes one shelf up. What the pairs share is
//! the *shape* of the answer rather than its arithmetic — see [`Reading`].
//!
//! **The one scan left is over the tube's own angle**, and a leaning drill is
//! what wants it — see [`Leaning::ends`], where a degree bounds how often the
//! answer can move. That is root isolation in one variable and not the hunt
//! above, which is a small loop adrift over a whole surface with nothing saying
//! how small it can be.
//!
use crate::inline::Inline;
use crate::math::sinusoid;
use crate::number::predicate;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use crate::solid::meeting::seeding::leaning::{ENDS, Leaning};
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

mod leaning;

/// One place on each piece of the curve `surface` and `torus` meet in, into
/// `found`.
///
/// **As many as the ends allow rather than as many as the geometry gives.** The
/// stretches that hold and the ones that do not alternate round the tube, so
/// eight ends are four pieces — but an end where `|B|` merely *touches* `A`
/// leaves both stretches either side of it holding, and eight is what that
/// allows. See [`Reading::ends`], where those ends are laid.
///
/// **A leaning drill is offered more than one place per piece**, and the walk
/// is what tells them apart: its four turns at one `v` are paired by nothing
/// this can read, and a piece of its curve spans several stretches of the tube
/// — so each stretch offers every turn it holds. What drops the repeats is
/// the boolean's own `Combining::march`, where a seed standing on a run already
/// walked is passed over.
///
/// **`false` for a pair no reading is written for, and nothing in `found`
/// where the two genuinely do not meet.** Those are two answers and not one:
/// what asks is a boolean that has already been told the pair meets somewhere
/// unwritable, so a pair nobody can seed has to refuse it where a pair that
/// misses divides nothing and is no trouble at all.
///
/// A coaxial pair is not here: it reduces to circles outright and never wants
/// walking — see [`Meeting::coaxial`](crate::solid::meeting::Meeting).
pub(crate) fn seeded(surface: &Surface, torus: &Torus, found: &mut Vec<DVec3>) -> bool {
    found.clear();
    let Some(reading) = Reading::of(surface, torus) else {
        return false;
    };
    let ends = reading.ends();
    let ends = ends.all();
    // **The two turns at one `v` are the two halves of one stretch**, and they
    // fall together where it ends — so the first of them stands for the piece
    // and the other is the same loop walked the other way round. With no end
    // anywhere they never join, and a leaning drill's turns are not a pair at
    // all; both of those want every one of them.
    let each = ends.is_empty() || matches!(reading.against, Against::Leaning(_));
    let mut lay = |v: f64| {
        let turns = reading.turns(v);
        let want = match each {
            true => turns.all().len(),
            false => 1,
        };
        for &turn in turns.all().iter().take(want) {
            found.push(torus.at(DVec2::new(turn, v)));
        }
    };
    // **No end anywhere is its own answer.** The curve then covers every angle
    // round the tube, and its two halves never join to become one piece.
    if ends.is_empty() {
        lay(0.0);
        return true;
    }
    for step in 0..ends.len() {
        let (from, to) = (ends[step], ends[(step + 1) % ends.len()]);
        lay(from + (to - from).rem_euclid(TAU) / 2.0);
    }
    true
}

/// What a surface comes to in a torus's own two angles.
///
/// **`A(v)·cos(u − phase) = B(v)` for the two pairs written down first.**
/// Standing on the other one is a single equation in the torus's two angles,
/// and for a plane and for a drill that runs parallel it rearranges into that:
/// an angle to solve for at each `v`, which is two angles where `|B| < A`, one
/// where they are equal, and none beyond.
///
/// **So the curve is exactly the stretches of `v` where `|B| ≤ A`**, and each
/// stretch carries one closed piece — the two angles at a `v` inside it are that
/// piece's two halves, and they join where the stretch ends. Where there is no
/// end at all the two halves never join and are two pieces of their own, which
/// is the same pair of regimes the exact tier's own curve has — see
/// [`Closing`](crate::solid::geometry::quartic::Closing).
///
/// **A drill that leans carries a second harmonic and none of that holds** —
/// see [`Leaning`], which answers up to four angles at a `v` and pairs none of
/// them.
///
/// What is per pair is where the curve stands and where the stretches end, and
/// that is all [`Against`] holds. Everything above it is one walk.
#[derive(Debug, Clone, Copy)]
struct Reading {
    torus: Torus,
    against: Against,
}

/// The half of a [`Reading`] that is the other surface's own.
///
/// The two arms a single wave answers both carry the bearing and the size of
/// whatever that surface offers square to the axis: a plane's own normal turned
/// that way, or how far a parallel drill's axis stands off. A surface that
/// offers nothing there stands square on the axis and reduces to circles.
#[derive(Debug, Clone, Copy)]
enum Against {
    /// A plane that is not square to the axis: how far its normal leans on the
    /// axis, and how far the plane stands from the axis's origin along it.
    Flat {
        phase: f64,
        wide: f64,
        lean: f64,
        over: f64,
    },
    /// A cylinder of radius `across` whose axis runs parallel to the torus's
    /// own, standing off it by `wide`.
    Beside { phase: f64, wide: f64, across: f64 },
    /// A cylinder whose axis leans on the torus's, at any lean and from
    /// anywhere.
    Leaning(Leaning),
}

impl Reading {
    /// How `surface` reads in `torus`'s own angles, or `None` for a pair no
    /// reading is written for.
    ///
    /// A surface standing square on the axis has no bearing to be read against
    /// — a plane square across it and a coaxial cylinder both — and each of
    /// those reduces to circles outright anyway.
    fn of(surface: &Surface, torus: &Torus) -> Option<Self> {
        let axis = torus.axis;
        let square = |of: DVec3| of - axis.direction * of.dot(axis.direction);
        let held = |against: Against| Self {
            torus: *torus,
            against,
        };
        match surface {
            Surface::Natural(Natural::Plane(plane)) => {
                let normal = plane.normal();
                let across = square(normal);
                let wide = across.length();
                (wide != 0.0).then(|| {
                    held(Against::Flat {
                        phase: axis.bearing(across),
                        wide,
                        lean: normal.dot(axis.direction),
                        over: normal.dot(plane.origin - axis.origin),
                    })
                })
            }
            Surface::Natural(Natural::Cylinder(tube)) => {
                if !predicate::parallel(tube.axis.direction, axis.direction) {
                    return Some(held(Against::Leaning(Leaning::of(tube, torus))));
                }
                let across = square(tube.axis.origin - axis.origin);
                let wide = across.length();
                (wide != 0.0).then(|| {
                    held(Against::Beside {
                        phase: axis.bearing(across),
                        wide,
                        across: tube.radius,
                    })
                })
            }
            _ => None,
        }
    }

    /// The angles round the axis the curve stands at at `v`, in no order.
    ///
    /// **`A(v)` is how far the other surface can be reached at `v` and `B(v)`
    /// how far it has to be reached to arrive there**, so the two answers are
    /// the bearing and one `acos` either side of it, and there are none where
    /// `|B|` runs past `A`. A leaning drill answers this its own way and up to
    /// four times.
    fn turns(&self, v: f64) -> Inline<f64, 4> {
        let mut found = Inline::none();
        let out = self.torus.major + self.torus.minor * v.cos();
        let (phase, reaching, standing) = match self.against {
            Against::Flat {
                phase,
                wide,
                lean,
                over,
            } => (phase, wide * out, over - self.torus.minor * lean * v.sin()),
            Against::Beside {
                phase,
                wide,
                across,
            } => (
                phase,
                2.0 * wide * out,
                out * out + wide * wide - across * across,
            ),
            Against::Leaning(leaning) => return leaning.turns(&self.torus, v),
        };
        let share = standing / reaching;
        if share.abs() <= 1.0 {
            found.push(phase + share.acos());
            found.push(phase - share.acos());
        }
        found
    }

    /// The angles a piece of the curve begins or ends at, which are where two
    /// of the turns fall together, in order.
    ///
    /// **`B = ±A`, and each pair solves it its own way.** A plane's is
    /// `α cos v + β sin v = γ`, one angle either side of a bearing. A parallel
    /// cylinder's turns on how far out the tube reaches and on nothing else:
    /// `(out ∓ off)² = across²` is `out = ±off ± across`, four distances the
    /// tube either reaches or does not, and one angle either way round where it
    /// does. A leaning drill's are the zeros of its own discriminant — see
    /// [`Leaning::ends`].
    fn ends(&self) -> Inline<f64, ENDS> {
        let mut ends = Inline::none();
        let torus = self.torus;
        match self.against {
            Against::Flat {
                wide, lean, over, ..
            } => {
                for way in [1.0, -1.0] {
                    let round = -way * wide * torus.minor;
                    let up = -torus.minor * lean;
                    let past = way * wide * torus.major - over;
                    for turn in sinusoid::angles(round, up, past) {
                        ends.push(turn.rem_euclid(TAU));
                    }
                }
            }
            Against::Beside { wide, across, .. } => {
                for out in [wide + across, wide - across, across - wide, -wide - across] {
                    let share = (out - torus.major) / torus.minor;
                    if share.abs() > 1.0 {
                        continue;
                    }
                    ends.push(share.acos());
                    ends.push(TAU - share.acos());
                }
            }
            Against::Leaning(leaning) => return leaning.ends(&torus),
        }
        let sorted = ends.all_mut();
        sorted.sort_by(f64::total_cmp);
        ends
    }
}

#[cfg(test)]
mod tests;
