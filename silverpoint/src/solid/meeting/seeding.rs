//! Finding one place on every piece of a curve that has to be walked.
//!
//! **The hard half of the marched route**, and the reason it is hard is not
//! that one place is difficult to reach: a Newton correction finds one from
//! almost anywhere. It is that a curve comes in *pieces*, and a search that
//! samples a grid finds a small piece by luck — see `.notes/KERNEL.md` §7.3,
//! where a loop `0.137` across wants a quarter of a million samples once it
//! stands half a cell off a node.
//!
//! **So it is done per pair and in closed form**, which is the same bargain the
//! reducible table strikes one shelf up. What the pairs share is the *shape* of
//! the answer rather than its arithmetic — see [`Reading`].
//!
use crate::inline::Inline;
use crate::math::sinusoid;
use crate::number::predicate;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

/// One place on each piece of the curve `surface` and `torus` meet in.
///
/// **As many as the ends allow rather than as many as the geometry gives.** The
/// stretches that hold and the ones that do not alternate round the tube, so
/// eight ends are four pieces — but an end where `|B|` merely *touches* `A`
/// leaves both stretches either side of it holding, and eight is what that
/// allows. See [`Reading::ends`], where those ends are laid.
///
/// **`None` for a pair no reading is written for, and none of them where the
/// two genuinely do not meet.** Those are two answers and not one: what asks is
/// a boolean that has already been told the pair meets somewhere unwritable, so
/// a pair nobody can seed has to refuse it where a pair that misses divides
/// nothing and is no trouble at all.
///
/// A coaxial pair is not here: it reduces to circles outright and never wants
/// walking — see [`Meeting::coaxial`](crate::solid::meeting::Meeting).
pub(crate) fn seeded(surface: &Surface, torus: &Torus) -> Option<Inline<DVec3, 8>> {
    let mut found = Inline::none();
    let reading = Reading::of(surface, torus)?;
    let ends = reading.ends();
    let ends = ends.all();
    // **No end anywhere is its own answer.** The curve then covers every angle
    // round the tube, and its two halves never join to become one piece.
    if ends.is_empty() {
        for far in [false, true] {
            if let Some(at) = reading.at(0.0, far) {
                found.push(at);
            }
        }
        return Some(found);
    }
    for step in 0..ends.len() {
        let (from, to) = (ends[step], ends[(step + 1) % ends.len()]);
        if let Some(at) = reading.at(from + (to - from).rem_euclid(TAU) / 2.0, false) {
            found.push(at);
        }
    }
    Some(found)
}

/// What a surface comes to in a torus's own two angles.
///
/// **`A(v)·cos(u − phase) = B(v)`, whatever the surface.** Standing on the
/// other one is a single equation in the torus's two angles, and for every pair
/// here it rearranges into that: an angle to solve for at each `v`, which is two
/// angles where `|B| < A`, one where they are equal, and none beyond.
///
/// **So the curve is exactly the stretches of `v` where `|B| ≤ A`**, and each
/// stretch carries one closed piece — the two angles at a `v` inside it are that
/// piece's two halves, and they join where the stretch ends. Where there is no
/// end at all the two halves never join and are two pieces of their own, which
/// is the same pair of regimes the exact tier's own curve has — see
/// [`Closing`](crate::solid::geometry::quartic::Closing).
///
/// What is per pair is `A`, `B` and where the stretches end, and that is all
/// [`Against`] holds. Everything above it is one walk.
#[derive(Debug, Clone, Copy)]
struct Reading {
    torus: Torus,
    /// The bearing and the size of whatever the other surface offers square to
    /// the axis: a plane's own normal turned that way, or how far a parallel
    /// cylinder's axis stands off. Both pairs read it, and a surface that
    /// offers nothing there stands square on the axis and reduces to circles.
    phase: f64,
    wide: f64,
    against: Against,
}

/// The half of a [`Reading`] that is the other surface's own.
#[derive(Debug, Clone, Copy)]
enum Against {
    /// A plane that is not square to the axis: how far its normal leans on the
    /// axis, and how far the plane stands from the axis's origin along it.
    Flat { lean: f64, over: f64 },
    /// A cylinder of radius `across` whose axis runs parallel to the torus's
    /// own, standing off it by [`Reading::wide`].
    Beside { across: f64 },
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
        let held = |across: DVec3, against: Against| {
            let wide = across.length();
            (wide != 0.0).then(|| Self {
                torus: *torus,
                phase: axis.bearing(across),
                wide,
                against,
            })
        };
        match surface {
            Surface::Natural(Natural::Plane(plane)) => {
                let normal = plane.normal();
                let across = square(normal);
                held(
                    across,
                    Against::Flat {
                        lean: normal.dot(axis.direction),
                        over: normal.dot(plane.origin - axis.origin),
                    },
                )
            }
            // Parallel and standing off. One that leans wants a reading nobody
            // has written yet.
            Surface::Natural(Natural::Cylinder(tube)) => {
                predicate::parallel(tube.axis.direction, axis.direction).then_some(())?;
                held(
                    square(tube.axis.origin - axis.origin),
                    Against::Beside {
                        across: tube.radius,
                    },
                )
            }
            _ => None,
        }
    }

    /// How far out from the axis the torus reaches at `v`.
    fn out(&self, v: f64) -> f64 {
        self.torus.major + self.torus.minor * v.cos()
    }

    /// `A(v)`: how far the other surface can be reached at `v`.
    fn reaching(&self, v: f64) -> f64 {
        let reach = self.wide * self.out(v);
        match self.against {
            Against::Flat { .. } => reach,
            Against::Beside { .. } => 2.0 * reach,
        }
    }

    /// `B(v)`: how far it has to be reached to arrive there.
    fn standing(&self, v: f64) -> f64 {
        match self.against {
            Against::Flat { lean, over } => over - self.torus.minor * lean * v.sin(),
            Against::Beside { across } => {
                let out = self.out(v);
                out * out + self.wide * self.wide - across * across
            }
        }
    }

    /// The place at `v` on the `far` half of the curve, or `None` where the
    /// curve does not reach that far round the tube.
    fn at(&self, v: f64, far: bool) -> Option<DVec3> {
        let share = self.standing(v) / self.reaching(v);
        (share.abs() <= 1.0).then(|| {
            let turn = if far { -share.acos() } else { share.acos() };
            self.torus.at(DVec2::new(self.phase + turn, v))
        })
    }

    /// The angles a piece of the curve begins or ends at, which are where the
    /// two halves fall together.
    ///
    /// **`B = ±A`, and each pair solves it its own way.** A plane's is
    /// `α cos v + β sin v = γ`, one angle either side of a bearing. A parallel
    /// cylinder's turns on how far out the tube reaches and on nothing else:
    /// `(out ∓ off)² = across²` is `out = ±off ± across`, four distances the
    /// tube either reaches or does not, and one angle either way round where it
    /// does.
    fn ends(&self) -> Inline<f64, 8> {
        let mut ends = Inline::none();
        let (torus, wide) = (self.torus, self.wide);
        match self.against {
            Against::Flat { lean, over } => {
                for way in [1.0, -1.0] {
                    let round = -way * wide * torus.minor;
                    let up = -torus.minor * lean;
                    let past = way * wide * torus.major - over;
                    for turn in sinusoid::angles(round, up, past) {
                        ends.push(turn.rem_euclid(TAU));
                    }
                }
            }
            Against::Beside { across } => {
                for out in [wide + across, wide - across, across - wide, -wide - across] {
                    let share = (out - torus.major) / torus.minor;
                    if share.abs() > 1.0 {
                        continue;
                    }
                    ends.push(share.acos());
                    ends.push(TAU - share.acos());
                }
            }
        }
        let sorted = ends.all_mut();
        sorted.sort_by(f64::total_cmp);
        ends
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::geometry::axis::Axis;
    use crate::solid::geometry::cylinder::Cylinder;
    use crate::solid::geometry::fitted::Fitted;
    use crate::solid::meeting::marching::Marching;

    /// The ring every surface below is held against: three out to the tube's
    /// own centre, one thick, about the world's `+Y` through the origin.
    fn ring() -> Torus {
        Torus {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            major: 3.0,
            minor: 1.0,
        }
    }

    /// The plane through `origin` facing `normal`, framed however.
    fn facing(origin: DVec3, normal: DVec3) -> Surface {
        Surface::Natural(Natural::Plane(
            Axis::about(origin, normal.normalize()).plane(),
        ))
    }

    /// A rod of `radius` running the ring's own way, `off` from its axis.
    fn beside(off: f64, radius: f64) -> Surface {
        Surface::Natural(Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::X * off, DVec3::Y, DVec3::X),
            radius,
        }))
    }

    /// Assert that a fine sweep of the curve finds nothing the seeds missed.
    ///
    /// **What says every piece was found.** The places of the curve at a fine
    /// sweep of `v` are had without asking where the *pieces* are, which is the
    /// one thing [`Reading::ends`] decides and the one thing this does not use.
    /// So every place the sweep turns up has to stand on some loop that was
    /// walked, and a piece nobody was seeded on shows up as a place far from
    /// all of them.
    ///
    /// Every seed is held against both surfaces on the way, to the last bits
    /// the machine keeps: a seed here is a place of the torus worked out
    /// outright rather than one walked onto.
    fn every_piece_is_reached(surface: &Surface, what: &str) {
        let torus = ring();
        let round = Surface::Fitted(Fitted::Torus(torus));
        let reading = Reading::of(surface, &torus).expect("the pair has a reading");
        let found = seeded(surface, &torus).expect("the pair has a reading");
        assert!(!found.all().is_empty(), "{what}: nothing was seeded");
        let mut marching = Marching::default();
        let mut walked = Vec::new();
        for &seed in found.all() {
            for (named, on) in [("the ring", &round), ("the other", surface)] {
                let off = on.off(seed);
                assert!(off < 1e-12, "{what}: {seed:?} stands {off} off {named}");
            }
            marching
                .walk(&round, surface, seed, 1e-4)
                .unwrap_or_else(|| panic!("{what}: a seeded walk did not close"));
            walked.extend_from_slice(marching.walked());
        }
        for step in 0..512 {
            let v = TAU * step as f64 / 512.0;
            for far in [false, true] {
                let Some(at) = reading.at(v, far) else {
                    continue;
                };
                let near = walked
                    .iter()
                    .map(|on| on.distance(at))
                    .fold(f64::INFINITY, f64::min);
                // A chord of the walk above is `√(8·ρ·sagitta)` long, which
                // for a curve of this ring's own size is a few hundredths — so
                // half of one is well under this, and a piece nobody walked is
                // a whole loop away rather than a chord.
                assert!(near < 0.05, "{what}: {at:?} stands {near} off every loop");
            }
        }
    }

    /// **Every piece of the curve a leaning plane cuts is seeded**, and a plane
    /// that both leans and stands off the middle is where that stops being
    /// free.
    ///
    /// Four planes, and the last is the one that needs the ends solved as
    /// written: it leans, it stands off the middle, *and* it cuts two stretches
    /// rather than one. Through the middle the equation is symmetric enough
    /// that a sign the wrong way round answers with the same set, and with one
    /// stretch a wrong end still leaves a midpoint on the curve to seed from.
    /// Only two stretches off the middle tells the two apart.
    #[test]
    fn every_piece_of_a_leaning_plane_is_seeded() {
        for (what, origin, lean) in [
            ("steeply", DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0)),
            ("gently", DVec3::ZERO, DVec3::new(0.2, 1.0, 0.0)),
            (
                "gently and raised",
                DVec3::Y * 0.5,
                DVec3::new(0.2, 1.0, 0.0),
            ),
            (
                "nearly square and raised",
                DVec3::Y * 0.2,
                DVec3::new(0.15, 1.0, 0.0),
            ),
        ] {
            every_piece_is_reached(&facing(origin, lean), what);
        }
    }

    /// **And every piece a rod bored the ring's own way cuts**, which is the
    /// bolt hole through a flange.
    ///
    /// A rod parallel to the axis meets the ring where
    /// `2·out·off·cos(u − phase) = out² + off² − across²`, and the ends of that
    /// turn on how far out the tube reaches and on nothing else. Three rods:
    /// one straight through the tube's own middle, one biting the ring's outer
    /// edge, and a wide one that swallows the axis and cuts the ring from
    /// inside.
    #[test]
    fn every_piece_of_a_rod_bored_the_rings_own_way_is_seeded() {
        for (what, off, radius) in [
            ("through the tube", 3.0, 0.3),
            ("biting the outer edge", 3.8, 0.5),
            ("swallowing the axis", 0.5, 2.6),
        ] {
            every_piece_is_reached(&beside(off, radius), what);
        }
    }

    /// **And the small loop a search finds by luck is found by arithmetic.**
    ///
    /// A plane parallel to the axis, a twentieth inside the ring's outer
    /// equator, cuts one small closed piece near it. A 512×512 subdivision
    /// misses one of these once it stands half a cell off a node — see
    /// `.notes/KERNEL.md` §7.3. The ends of its stretch are two `acos` here,
    /// and where it is comes out with them.
    ///
    /// **Held to the ellipse it closes on.** For a plane `d` inside the outer
    /// equator the piece is an ellipse of semi-axes `√(2·minor·d)` along the
    /// axis and `√(2(major + minor)d)` across it, in the limit of small `d` —
    /// which for a twentieth of a ring of three by one is `0.316` by `0.632`.
    #[test]
    fn a_small_loop_just_inside_the_equator_is_seeded_by_arithmetic() {
        let torus = ring();
        let inside = 0.05;
        let out = torus.major + torus.minor - inside;
        let grazing = facing(DVec3::X * out, DVec3::X);
        let found = seeded(&grazing, &torus).expect("a plane inside the equator has a reading");
        assert_eq!(found.all().len(), 1, "{:?}", found.all());

        let round = Surface::Fitted(Fitted::Torus(torus));
        let mut marching = Marching::default();
        marching
            .walk(&round, &grazing, found.all()[0], 1e-6)
            .expect("the small walk did not close");
        let reach = |of: fn(DVec3) -> f64| {
            marching
                .walked()
                .iter()
                .map(|&at| of(at))
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let (up, across) = (reach(|at| at.y), reach(|at| at.z));
        let (tall, wide) = (
            (2.0 * torus.minor * inside).sqrt(),
            (2.0 * (torus.major + torus.minor) * inside).sqrt(),
        );
        assert!((up - tall).abs() < tall / 50.0, "{up} rather than {tall}");
        assert!(
            (across - wide).abs() < wide / 50.0,
            "{across} rather than {wide}"
        );
    }

    /// **A rod through the tube's middle cuts two pieces**, and the angles they
    /// end at are read off the arithmetic rather than off the walk.
    ///
    /// With the rod's axis three out — the tube's own centre circle — the tube
    /// reaches it between `out = 3 ± 0.3`, which is `cos v = ±0.3`. Two
    /// stretches, each a closed piece: the hole the rod makes going in and the
    /// one it makes coming out.
    #[test]
    fn a_rod_through_the_tube_ends_where_the_arithmetic_says() {
        let torus = ring();
        let rod = beside(3.0, 0.3);
        let reading = Reading::of(&rod, &torus).expect("a parallel rod reads");
        let mut want = [
            0.3f64.acos(),
            TAU - 0.3f64.acos(),
            (-0.3f64).acos(),
            TAU - (-0.3f64).acos(),
        ];
        want.sort_by(f64::total_cmp);
        let ends = reading.ends();
        assert_eq!(ends.all().len(), 4, "{:?}", ends.all());
        for (got, want) in ends.all().iter().zip(want) {
            assert!((got - want).abs() < 1e-12, "{:?} misses {want}", ends.all());
        }
        let found = seeded(&rod, &torus).expect("a parallel rod has a reading");
        assert_eq!(found.all().len(), 2, "one piece each way");
    }
}
