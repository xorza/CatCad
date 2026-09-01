//! Walking a curve two surfaces meet in that neither of them can write down.
//!
//! What it lays down is a run of places, which
//! [`Marchings`](crate::solid::geometry::marchings::Marchings) holds and a
//! curve names — see `.notes/KERNEL.md` §7.3.

use crate::solid::geometry::surface::Surface;
use glam::DVec3;

/// How many rounds of correction one place is given.
///
/// Newton doubles the digits a place has each round, so one that is going to
/// arrive arrives inside a handful and the rest is the machine repeating
/// itself. A bound on the work and not on the answer: what says a place is good
/// is that it came back onto both surfaces, which its caller measures.
const ROUNDS: usize = 16;

/// How many steps one walk is given before it is called a runaway.
///
/// A whole turn of the finest curve a mesher asks for is a few thousand, so
/// this stands far past any honest walk. What it is for is a seed that was
/// never on the curve, and a curve that does not close.
const STEPS: usize = 1 << 17;

/// How much a step may grow from one to the next.
///
/// The sag of a chord goes as the square of its length, so the step that would
/// exactly meet the sagitta is the one just taken times the square root of how
/// much room was left — and that ratio runs away where a stretch is straight.
/// Held to a doubling, which reaches any honest step in a handful of them.
const QUICKEST: f64 = 2.0;

/// Walks the curve two surfaces meet in, keeping the room it works in.
///
/// **Newton onto both surfaces at once, and a step along the cross of their
/// normals.** A place off the curve is off two surfaces, which is two numbers,
/// and it has three to move in — so the correction is the smallest one that
/// clears both, and that is a two-by-two solve in the plane the two normals
/// span. The curve itself runs where both surfaces do, which is square to both
/// normals and so along their cross product.
///
/// **Stepped to a sagitta that is measured rather than predicted.** How far a
/// chord strays from the curve depends on how hard the curve bends, which is
/// nothing either surface can be asked. So each step is taken, the chord it
/// laid down is probed at three places along it, and a step that strayed too
/// far is halved and taken again — see [`Marching::sagging`]. What comes back
/// is the furthest any accepted chord strayed, which is the bound §4.1 says a
/// fitted result carries.
#[derive(Debug, Default)]
pub(crate) struct Marching {
    places: Vec<DVec3>,
}

impl Marching {
    /// Walk the curve `one` and `two` meet in, from `seed` round to it again,
    /// no chord of it straying further than `sagitta`.
    ///
    /// What comes back is how far the furthest chord strayed, and `None` where
    /// the walk did not come back: a seed that was not on the curve, a curve
    /// that does not close, or a place where the two surfaces lie tangent and
    /// there is no direction to go in. All three are answers this cannot give,
    /// and saying so beats laying down a run that is not the curve.
    pub(crate) fn walk(
        &mut self,
        one: &Surface,
        two: &Surface,
        seed: DVec3,
        sagitta: f64,
    ) -> Option<f64> {
        self.places.clear();
        let start = Self::onto(one, two, seed)?;
        self.places.push(start);
        let mut at = start;
        // The sagitta itself, which is short enough for any curve — and
        // [`QUICKEST`] reaches an honest step from it in a few doublings.
        let mut step = sagitta;
        let mut strayed = 0.0_f64;
        let mut last = Self::along(one, two, at)?;
        let mut left = false;
        for _ in 0..STEPS {
            // **The way it was going, not the way the cross came out.** Two
            // normals give a direction and not an orientation, and either
            // surface's own normal may turn over along the walk.
            let way = Self::along(one, two, at)?;
            let way = if way.dot(last) < 0.0 { -way } else { way };
            let next = Self::onto(one, two, at + way * step)?;
            let sag = Self::sagging(one, two, at, next)?;
            if sag > sagitta {
                step *= 0.5;
                continue;
            }
            self.places.push(next);
            strayed = strayed.max(sag);
            step *= (sagitta / sag).sqrt().clamp(1.0, QUICKEST);
            (last, at) = (way, next);
            // **Gone before it may come back.** A walk still taking its first
            // strides stands within one step of its seed the whole time, so a
            // closing test alone would close it before it had left. A step
            // grows by at most [`QUICKEST`] in one go, so standing further off
            // than that many of them is having gone somewhere.
            let away = at.distance(start);
            left |= away > QUICKEST * step;
            if left && away <= step {
                self.places.push(start);
                return Some(strayed);
            }
        }
        None
    }

    /// The places the last walk laid down, the one it began at repeated at the
    /// end where it closed.
    pub(crate) fn walked(&self) -> &[DVec3] {
        &self.places
    }

    /// `at` corrected onto both surfaces.
    ///
    /// **The smallest move that clears both**, which is what makes this a walk
    /// along the curve rather than a drift along it: a correction with anything
    /// of the curve's own direction in it would slide the place forwards or
    /// back, and the step is what decides that.
    ///
    /// How far a place stands off a surface is how far it stands from its own
    /// nearest place on it, read along the normal there — which is signed, and
    /// which every surface here answers in closed form. Asked by name rather
    /// than read back through the parameters, the two parting company on a cone
    /// — see [`Surface::nearest`]. `None` where the two
    /// stand tangent, having no plane between them to correct in.
    fn onto(one: &Surface, two: &Surface, at: DVec3) -> Option<DVec3> {
        let mut at = at;
        for _ in 0..ROUNDS {
            let (first, second) = (one.normal(one.uv(at)), two.normal(two.uv(at)));
            let (near, far) = (
                (at - one.nearest(at)).dot(first),
                (at - two.nearest(at)).dot(second),
            );
            let leaning = first.dot(second);
            let apart = 1.0 - leaning * leaning;
            if apart == 0.0 {
                return None;
            }
            let moved = first * ((leaning * far - near) / apart)
                + second * ((leaning * near - far) / apart);
            let next = at + moved;
            if !next.is_finite() {
                return None;
            }
            // The last bit the machine holds, which is where Newton stops
            // having anything left to say.
            if next == at {
                break;
            }
            at = next;
        }
        Some(at)
    }

    /// Which way the curve runs at `at`, or `None` where the two surfaces lie
    /// tangent and it runs nowhere.
    fn along(one: &Surface, two: &Surface, at: DVec3) -> Option<DVec3> {
        one.normal(one.uv(at))
            .cross(two.normal(two.uv(at)))
            .try_normalize()
    }

    /// How far the chord from `from` to `to` strays from the curve.
    ///
    /// **Probed rather than bounded**, which is what puts a marched curve in
    /// the fitted tier: three places along the chord are corrected onto both
    /// surfaces, and how far the furthest of them moved is the reading. A
    /// smooth curve leaves its chord furthest near the middle, so three catches
    /// what one would and a leaning chord besides — and it is a reading and not
    /// a proof, which is the whole difference §4.1 draws between the tiers.
    fn sagging(one: &Surface, two: &Surface, from: DVec3, to: DVec3) -> Option<f64> {
        let mut most = 0.0_f64;
        for share in [0.25, 0.5, 0.75] {
            let along = from.lerp(to, share);
            most = most.max(along.distance(Self::onto(one, two, along)?));
        }
        Some(most)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::geometry::axis::Axis;
    use crate::solid::geometry::curve::Curve;
    use crate::solid::geometry::cylinder::Cylinder;
    use crate::solid::geometry::fitted::Fitted;
    use crate::solid::geometry::natural::Natural;
    use crate::solid::geometry::torus::Torus;
    use crate::solid::meeting::Meeting;
    use std::f64::consts::TAU;

    /// The ring every walk below is taken on: three out to the tube's own
    /// centre, one thick, about the world's `+Y` through the origin.
    fn ring() -> Surface {
        Surface::Fitted(Fitted::Torus(Torus {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            major: 3.0,
            minor: 1.0,
        }))
    }

    /// The plane through `origin` facing `normal`, framed however.
    fn facing(origin: DVec3, normal: DVec3) -> Surface {
        Surface::Natural(Natural::Plane(
            Axis::about(origin, normal.normalize()).plane(),
        ))
    }

    /// How long the walk that was just taken is.
    fn length(marching: &Marching) -> f64 {
        marching
            .walked()
            .windows(2)
            .map(|pair| pair[0].distance(pair[1]))
            .sum()
    }

    /// Assert that every place of the walk stands on both surfaces.
    ///
    /// To the last bits the machine holds over places a few units wide, which
    /// is what a correction run until it stops moving the place comes to — a
    /// walk that merely came close would have stopped somewhere else.
    fn lies_on(marching: &Marching, one: &Surface, two: &Surface, what: &str) {
        for &at in marching.walked() {
            for (named, surface) in [("the first", one), ("the second", two)] {
                let off = surface.off(at);
                assert!(off < 1e-12, "{what}: {at:?} stands {off} off {named}");
            }
        }
    }

    /// **A march reproduces what the reducible table names**, which is two
    /// routes to one answer with no arithmetic between them.
    ///
    /// A plane six tenths up a ring of three by one crosses it in two circles
    /// about the axis, and a rod of three and a half sharing that axis crosses
    /// it in two of its own radius — `Meeting` says which, in closed form and
    /// without walking anything. Each of the four is then walked, and what it
    /// is held to is that closed form's own radius.
    ///
    /// **The seed is the one thing the two share**, and a place is not a curve:
    /// where a walk begins says nothing about where it goes, how long it turns
    /// out to be, or whether it comes back at all.
    #[test]
    fn a_march_walks_the_circles_the_reducible_table_names() {
        let ring = ring();
        let rod = Surface::Natural(Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            radius: 3.5,
        }));
        let level = facing(DVec3::Y * 0.6, DVec3::Y);
        let sagitta = 1e-4;
        let mut marching = Marching::default();

        for (named, other) in [("a level plane", &level), ("a coaxial rod", &rod)] {
            let Meeting::Along(along) = Meeting::of(&ring, other) else {
                panic!("{named}: the table names no curve to walk against");
            };
            for curve in along.all() {
                let Curve::Circle(circle) = curve else {
                    panic!("{named}: {curve:?} is not a circle");
                };
                let strayed = marching
                    .walk(&ring, other, circle.at(0.0), sagitta)
                    .unwrap_or_else(|| panic!("{named}: the walk did not close"));
                assert!(strayed <= sagitta, "{named}: strayed {strayed}");
                lies_on(&marching, &ring, other, named);
                let round = length(&marching);
                let want = TAU * circle.radius;
                assert!((round - want).abs() < 1e-3, "{named}: {round} not {want}");
            }
        }
    }

    /// **And it walks the curve no table has**, which is what it is for.
    ///
    /// A plane through the ring's middle leaning at forty-five degrees cuts it
    /// in a spiric quartic — one closed loop, the lean standing clear of the
    /// bitangent `√8/3` where the two would touch. There is no closed form for
    /// its length, so what is asserted is what there is: it closes, every place
    /// of it stands on both surfaces, it reaches the ring's own outer equator
    /// where the plane crosses it, and a finer sagitta reads a longer curve
    /// that closes on a limit from below.
    #[test]
    fn a_march_walks_a_spiric_section_no_closed_form_writes_down() {
        let ring = ring();
        let leaning = facing(DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0));
        // Where the plane crosses the ring's outer equator, which is on both.
        let seed = DVec3::new(0.0, 0.0, -4.0);

        let mut marching = Marching::default();
        let (mut last, mut gained) = (0.0, f64::INFINITY);
        for sagitta in [1e-3, 1e-4, 1e-5] {
            let strayed = marching
                .walk(&ring, &leaning, seed, sagitta)
                .expect("the leaning walk did not close");
            assert!(strayed <= sagitta, "strayed {strayed} past {sagitta}");
            lies_on(&marching, &ring, &leaning, "leaning");
            // The loop runs from the ring's outer equator to its inner one and
            // back, which is what a plane through the middle reaches.
            let out = |at: DVec3| (at.x * at.x + at.z * at.z).sqrt();
            let near = marching
                .walked()
                .iter()
                .map(|&at| out(at))
                .fold(f64::INFINITY, f64::min);
            let far = marching
                .walked()
                .iter()
                .map(|&at| out(at))
                .fold(0.0, f64::max);
            assert!((near - 2.0).abs() < 1e-2, "the loop reached in to {near}");
            assert!((far - 4.0).abs() < 1e-2, "the loop reached out to {far}");
            // **Longer each time, and by a tenth as much.** A chord's sag goes
            // as the square of its length, so a tenfold finer sagitta is a step
            // shorter by the square root of ten — and what a polyline falls
            // short of its curve by goes as the sag itself. So the reading
            // closes on its limit from below by a tenth each time, of which
            // five is asked.
            let round = length(&marching);
            let gain = round - last;
            assert!(gain > 0.0, "{sagitta} read no longer than the last");
            assert!(
                gain * 5.0 < gained,
                "{sagitta} gained {gain} against {gained}"
            );
            (last, gained) = (round, gain);
        }
    }
}
