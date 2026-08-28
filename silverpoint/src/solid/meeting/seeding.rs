//! Finding one place on every piece of a curve that has to be walked.
//!
//! **The half of M6 the spike found hard**, and the reason it is hard is not
//! that one place is difficult to reach: a Newton correction finds one from
//! almost anywhere. It is that a curve comes in *pieces*, and a search that
//! samples a grid finds a small piece by luck — see `.notes/KERNEL.md` §9.2,
//! where a loop `0.137` across wanted a quarter of a million samples once it was
//! moved half a cell off a node.
//!
//! **So it is done per pair and in closed form**, which is the same bargain the
//! reducible table strikes one shelf up. What is here is the first pair.
//!
//! **No production caller yet**, and the reason is the one
//! [`Marching`](super::marching::Marching) gives: what a walk lays down is a run
//! of places, and no `Curve` can carry one until there is an arena to put it in.
#![allow(dead_code)]

use crate::inline::Inline;
use crate::math::plane::Plane;
use crate::math::sinusoid;
use crate::solid::geometry::torus::Torus;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

/// One place on each piece of the curve `plane` and `torus` meet in.
///
/// **Two at most**, and that is a count rather than a hope — see [`Spiric`],
/// which is the reading the count falls out of.
///
/// Nothing where the two do not meet, and nothing for a plane square to the
/// axis: that one has no bearing to speak of and a reducible answer already.
pub(crate) fn spiric(plane: &Plane, torus: &Torus) -> Inline<DVec3, 2> {
    let mut found = Inline::none();
    let Some(spiric) = Spiric::of(plane, torus) else {
        return found;
    };
    let ends = spiric.ends();
    let ends = ends.all();
    // **No end anywhere is its own answer.** The curve then covers every angle
    // round the tube, and its two halves never join to become one piece.
    if ends.is_empty() {
        for far in [false, true] {
            if let Some(at) = spiric.at(0.0, far) {
                found.push(at);
            }
        }
        return found;
    }
    for step in 0..ends.len() {
        let (from, to) = (ends[step], ends[(step + 1) % ends.len()]);
        if let Some(at) = spiric.at(from + (to - from).rem_euclid(TAU) / 2.0, false) {
            found.push(at);
        }
    }
    found
}

/// What a plane comes to in a torus's own two angles.
///
/// **`A(v)·cos(u − phase) = B(v)`.** A place of the torus stands on the plane
/// exactly where `k(major + minor·cos v)·cos(u − phase) = c − minor·m·sin v`,
/// with `m` the lean of the plane's normal on the axis, `k` and `phase` the
/// size and bearing of what is left of that normal square to it, and `c` how
/// far the plane stands from the axis's own origin along the normal. That
/// leaves an angle to solve for at each `v`: two angles where `|B| < A`, one
/// where they are equal, and none beyond.
///
/// **So the curve is exactly the stretches of `v` where `|B| ≤ A`**, and their
/// ends are in closed form — four of them at most, which is two stretches.
/// Each stretch carries one closed piece: the two angles at a `v` inside it are
/// that piece's two halves, and they join where the stretch ends. Where there
/// is no end at all the two halves never join and are two pieces of their own,
/// which is the same pair of regimes a cross drilling has (§9.1).
#[derive(Debug, Clone, Copy)]
struct Spiric {
    torus: Torus,
    /// How far the plane's normal reaches square to the axis.
    wide: f64,
    /// Which way it reaches, in the torus's own first angle.
    phase: f64,
    /// How far the plane stands from the axis's origin along that normal.
    over: f64,
    /// How far the normal leans on the axis.
    lean: f64,
}

impl Spiric {
    /// How `plane` reads in `torus`'s own angles, or `None` for a plane square
    /// to the axis, which has no bearing to be read against.
    fn of(plane: &Plane, torus: &Torus) -> Option<Self> {
        let axis = torus.axis;
        let normal = plane.normal();
        let lean = normal.dot(axis.direction);
        let across = normal - axis.direction * lean;
        let wide = across.length();
        (wide != 0.0).then(|| Self {
            torus: *torus,
            wide,
            phase: axis.bearing(across),
            over: normal.dot(plane.origin - axis.origin),
            lean,
        })
    }

    /// `A(v)`: how far the tube reaches across the plane's own normal at `v`.
    fn reaching(&self, v: f64) -> f64 {
        self.wide * (self.torus.major + self.torus.minor * v.cos())
    }

    /// `B(v)`: how far it has to reach to arrive at the plane.
    fn standing(&self, v: f64) -> f64 {
        self.over - self.torus.minor * self.lean * v.sin()
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
    /// `B = ±A` is `α cos v + β sin v = γ`, which is one angle either side of
    /// `atan2(β, α)` and nothing at all where `γ` outreaches the pair.
    fn ends(&self) -> Inline<f64, 4> {
        let mut ends = Inline::none();
        for way in [1.0, -1.0] {
            let round = -way * self.wide * self.torus.minor;
            let up = -self.torus.minor * self.lean;
            let past = way * self.wide * self.torus.major - self.over;
            for turn in sinusoid::angles(round, up, past) {
                ends.push(turn.rem_euclid(TAU));
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
    use crate::solid::geometry::fitted::Fitted;
    use crate::solid::geometry::natural::Natural;
    use crate::solid::geometry::surface::Surface;
    use crate::solid::meeting::marching::Marching;

    /// The ring every plane below is held against: three out to the tube's own
    /// centre, one thick, about the world's `+Y` through the origin.
    fn ring() -> Torus {
        Torus {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            major: 3.0,
            minor: 1.0,
        }
    }

    /// The plane through `origin` facing `normal`, framed however.
    fn facing(origin: DVec3, normal: DVec3) -> Plane {
        Axis::about(origin, normal.normalize()).plane()
    }

    /// Assert that every seed stands on both the ring and `plane`.
    fn lies_on(found: &Inline<DVec3, 2>, plane: &Plane, what: &str) {
        let round = Surface::Fitted(Fitted::Torus(ring()));
        for &at in found.all() {
            let off = round.off(at);
            assert!(off < 1e-12, "{what}: {at:?} stands {off} off the ring");
            let along = (at - plane.origin).dot(plane.normal()).abs();
            assert!(along < 1e-12, "{what}: {at:?} stands {along} off the plane");
        }
    }

    /// Assert that a fine sweep of the curve finds nothing the seeds missed.
    ///
    /// **What says every piece was found.** The places of the curve at a fine
    /// sweep of `v` are had without asking where the *pieces* are, which is the
    /// one thing [`Spiric::ends`] decides and the one thing this does not use.
    /// So every place the sweep turns up has to stand on some loop that was
    /// walked, and a piece nobody was seeded on shows up as a place far from
    /// all of them.
    fn every_piece_is_reached(plane: &Plane, what: &str) {
        let torus = ring();
        let round = Surface::Fitted(Fitted::Torus(torus));
        let cut = Surface::Natural(Natural::Plane(*plane));
        let reading = Spiric::of(plane, &torus).expect("the plane leans on the axis");
        let mut marching = Marching::default();
        let mut walked = Vec::new();
        for &seed in spiric(plane, &torus).all() {
            marching
                .walk(&round, &cut, seed, 1e-4)
                .unwrap_or_else(|| panic!("{what}: a seeded walk did not close"));
            walked.extend_from_slice(marching.walked());
        }
        assert!(!walked.is_empty(), "{what}: nothing was walked at all");
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
                assert!(near < 0.05, "{what}: {at:?} stands {near} off every loop");
            }
        }
    }

    /// **Every piece of the curve is reached, and a plane that both leans and
    /// stands off the middle is where that stops being free.**
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

    /// **A plane through the middle of a ring cuts it in two pieces, and both
    /// are seeded** — by either of the two routes there are to two.
    ///
    /// Leaning at forty-five degrees, `|B|` never reaches `A` at all: the two
    /// angles at every `v` are two halves that never join, and each is a piece.
    /// Leaning gently, `|B|` reaches `A` four times, which is two stretches of
    /// `v` with a piece apiece — the ring's own two equators, deformed. Both
    /// stand clear of the bitangent `√8/3`, where the two would touch.
    ///
    /// **Walked from each seed, neither loop comes near the other's**, which is
    /// the whole of what a second seed is for and the one thing that says two
    /// pieces rather than one found twice.
    #[test]
    fn a_plane_through_a_ring_seeds_both_of_the_pieces_it_cuts() {
        let round = Surface::Fitted(Fitted::Torus(ring()));
        let mut marching = Marching::default();
        for lean in [DVec3::new(1.0, 1.0, 0.0), DVec3::new(0.2, 1.0, 0.0)] {
            let plane = facing(DVec3::ZERO, lean);
            let found = spiric(&plane, &ring());
            assert_eq!(found.all().len(), 2, "{lean:?}: {:?}", found.all());
            lies_on(&found, &plane, "through the middle");

            let cut = Surface::Natural(Natural::Plane(plane));
            let [here, there] = [found.all()[0], found.all()[1]];
            for (seed, other) in [(here, there), (there, here)] {
                marching
                    .walk(&round, &cut, seed, 1e-4)
                    .expect("a seeded walk did not close");
                let near = marching
                    .walked()
                    .iter()
                    .map(|at| at.distance(other))
                    .fold(f64::INFINITY, f64::min);
                assert!(near > 0.1, "{lean:?}: the two seeds are {near} apart");
            }
        }
    }

    /// **And the small loop the spike found by luck is found by arithmetic.**
    ///
    /// A plane parallel to the axis, a twentieth inside the ring's outer
    /// equator, cuts one small closed piece near it. `.notes/KERNEL.md` §9.2's
    /// spike needed a 512×512 subdivision to find one of these once it stood
    /// half a cell off a node; the ends of its stretch of `v` are two `acos`
    /// here, and where it is comes out with them.
    ///
    /// **Held to the ellipse it closes on.** For a plane `d` inside the outer
    /// equator the piece is an ellipse of semi-axes `√(2·minor·d)` along the
    /// axis and `√(2(major + minor)d)` across it, in the limit of small `d` —
    /// which for a twentieth of a ring of three by one is `0.316` by `0.632`.
    #[test]
    fn a_small_loop_just_inside_the_equator_is_seeded_by_arithmetic() {
        let inside = 0.05;
        let out = ring().major + ring().minor - inside;
        let grazing = facing(DVec3::X * out, DVec3::X);
        let found = spiric(&grazing, &ring());
        assert_eq!(found.all().len(), 1, "{:?}", found.all());
        lies_on(&found, &grazing, "grazing");

        let round = Surface::Fitted(Fitted::Torus(ring()));
        let cut = Surface::Natural(Natural::Plane(grazing));
        let mut marching = Marching::default();
        marching
            .walk(&round, &cut, found.all()[0], 1e-6)
            .expect("the small walk did not close");
        let reach = |of: fn(DVec3) -> f64| {
            let spread = marching.walked().iter().map(|&at| of(at));
            spread.fold(f64::NEG_INFINITY, f64::max)
        };
        let (up, across) = (reach(|at| at.y), reach(|at| at.z));
        let (tall, wide) = (
            (2.0 * ring().minor * inside).sqrt(),
            (2.0 * (ring().major + ring().minor) * inside).sqrt(),
        );
        assert!((up - tall).abs() < tall / 50.0, "{up} rather than {tall}");
        assert!(
            (across - wide).abs() < wide / 50.0,
            "{across} rather than {wide}"
        );
    }
}
