//! Where three blends that do not agree about a corner stop, and what that
//! leaves to span.
//!
//! **The setback is the rounding's own choice**, and so is the opening it
//! leaves: the six places where the blends stop, the three cross sections and
//! the three springs between them. What spans the opening is a surface — see
//! [`Vertexed`], which takes the six sides and nothing else about how they
//! were arrived at.

use crate::number::predicate;
use crate::number::tolerance::PLACED;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::vertexed::{Along, Side, Vertexed};
use glam::DVec3;

/// The corner three blends of one reach leave, and the setback that opens it.
///
/// **Wound rather than paired**, which is what lets the six places come out in
/// one order: `blends[i]` and `blends[i + 1]` are the two that share
/// `faces[i]`, so a walk round the corner alternates a blend and a face without
/// asking which meets which.
///
/// **Every axis runs away from the corner.** A cylinder says where its blend
/// lies and not which way the edge under it runs, and the setback is measured
/// along that edge — so the caller hands the axes over pointing the way the
/// blend goes, and the sign is not worked out again here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Setback {
    /// Each blend's own axis, in the order the three run round the corner.
    ///
    /// **A line and not a cylinder**, the three sharing one reach and none of
    /// them wanting a frame: what a blend is here is where its axis runs and
    /// how far off it the surface stands.
    axes: [Line; 3],
    /// Which way the face each neighbouring pair shares points: `facing[i]` is
    /// the normal of the one both `blends[i]` and `blends[i + 1]` run out onto.
    ///
    /// **A normal and not a plane**, all three faces running through the corner
    /// — so the corner and the normal are the whole of one. Each points *out of
    /// the material*, the way its own face of the body does, which is what the
    /// patch's own plane adds up.
    facing: [DVec3; 3],
    /// The corner of the body the three swallow.
    at: DVec3,
    /// The one reach all three were raised at.
    reach: f64,
    /// How far along its own edge each blend stops short of that corner.
    setback: f64,
}

/// The six places the opening runs between.
///
/// **Two per blend and two per face**, which is the same six read either way: a
/// blend stops on a cross section whose two ends stand on the two faces it
/// divides, and a face carries the two ends of the two blends that reach it.
/// `made[i]` is where `blends[i]` stops, its end on `faces[i - 1]` first and
/// its end on `faces[i]` second — so a face's own pair is `made[i][1]` and
/// `made[i + 1][0]`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Opened {
    pub(super) made: [[DVec3; 2]; 3],
    /// How far every one of the six stands from the corner.
    ///
    /// One number and not six — see [`Setback::opened`], which refuses a
    /// corner whose blends do not agree about it.
    pub(super) reach: f64,
}

impl Setback {
    /// Three blends of one `reach` on the `axes`, dividing the faces that face
    /// `facing`, stopped `setback` short of the corner `at`.
    pub(super) fn new(
        axes: [Line; 3],
        facing: [DVec3; 3],
        at: DVec3,
        reach: f64,
        setback: f64,
    ) -> Self {
        Self {
            axes,
            facing,
            at,
            reach,
            setback,
        }
    }

    /// Where the blend at `which` stops on the face it shares with the blend
    /// after it, or before it where `ahead` is false.
    ///
    /// **Read off the rail rather than off the edge.** A blend is tangent to
    /// each face it divides along one line, and that line is its own axis
    /// dropped onto the face — the two standing a reach apart is what tangency
    /// is. The corner's foot on that line is where the blend would run to, and
    /// the setback carries it back along the way the axis points.
    fn stopped(&self, which: usize, ahead: bool) -> DVec3 {
        let axis = self.axes[which];
        let normal = self.facing[match ahead {
            true => which,
            false => (which + 2) % 3,
        }];
        let rail = axis.origin - normal * (axis.origin - self.at).dot(normal);
        rail + axis.direction * ((self.at - rail).dot(axis.direction) + self.setback)
    }

    /// The six places the opening runs between, or `None` where the corner is
    /// not one this spans.
    ///
    /// **All six stand at one distance or the corner is refused.** A rail
    /// stands `d` off the edge along its own face, so a stopped place stands
    /// `√(t² + d²)` from the corner — and where the three blends share a `d`
    /// that is one number for the six, which is what lets each spring be an arc
    /// of the one sphere about the corner. Three faces meeting square share it;
    /// three at arbitrary angles do not, and those want a rule that interpolates
    /// rather than one sphere. See `.notes/VERTEX-BLENDS.md` §5.
    ///
    /// **A setback under the reach is refused too.** At `t = d` the two places
    /// on a face fall together and the spring between them is nothing, which is
    /// the corner as it stands without a setback at all.
    pub(super) fn opened(&self) -> Option<Opened> {
        let mut made = [[DVec3::ZERO; 2]; 3];
        for (which, pair) in made.iter_mut().enumerate() {
            for (side, ahead) in [false, true].into_iter().enumerate() {
                pair[side] = self.stopped(which, ahead);
            }
        }
        let reach = self.at.distance(made[0][0]);
        for stop in made.into_iter().flatten() {
            if !predicate::touching((self.at.distance(stop) - reach).abs(), PLACED) {
                return None;
            }
        }
        // The two places on a face fall together at a setback of the rail's own
        // offset, and a spring between one place and itself spans nothing.
        if predicate::touching(made[0][1].distance(made[1][0]), PLACED) {
            return None;
        }
        Some(Opened { made, reach })
    }

    /// The cross section the blend at `which` stops on, from its end on the
    /// face before it round to its end on the face after.
    ///
    /// **Square to its own axis**, which is what makes it a circle of the
    /// blend's own reach rather than a section of some other plane: the setback
    /// is measured along the edge, so what it cuts is the section the edge's
    /// own direction is normal to.
    ///
    /// The way round is the one holding the place the blend faces the edge
    /// from, which is the quarter of the cylinder the blend actually raises.
    fn crossed(&self, opened: &Opened, which: usize) -> Side {
        let axis = self.axes[which];
        let ends = opened.made[which];
        let middle = axis.at((ends[0] - axis.origin).dot(axis.direction));
        let circle = Circle {
            axis: Axis::new(middle, axis.direction, (ends[0] - middle).normalize()),
            radius: self.reach,
        };
        let toward = self.at - axis.origin;
        Side::arced(
            circle,
            ends[0],
            ends[1],
            middle + toward - axis.direction * toward.dot(axis.direction),
            Along::Blend {
                filled: self.filled(which),
            },
        )
    }

    /// The spring the face at `which` carries, from the end of the blend before
    /// it round to the end of the blend after.
    ///
    /// **An arc of the sphere about the corner**, which every one of the six
    /// places stands on — see [`Setback::opened`], which refuses a corner
    /// where they do not.
    ///
    /// **The way round is which side the two blends stand on.** Both stand a
    /// reach off the face they share; a pair that disagrees stands them on
    /// opposite sides of it and leaves a corner the face turns a quarter over,
    /// and a pair that agrees stands them on one side and leaves one it turns
    /// three quarters over. So the second wants the long way round, and a
    /// straight run between the two ends would leave the face altogether.
    fn sprung(&self, opened: &Opened, which: usize) -> Side {
        let after = (which + 1) % 3;
        let (from, to) = (opened.made[which][1], opened.made[after][0]);
        let normal = self.facing[which];
        let circle = Circle {
            axis: Axis::new(self.at, normal, (from - self.at).normalize()),
            radius: opened.reach,
        };
        let sides = [which, after].map(|at| (self.axes[at].origin - self.at).dot(normal).signum());
        let ways = [self.axes[which].direction, self.axes[after].direction];
        let toward = match sides[0] == sides[1] {
            true => -(ways[0] + ways[1]),
            false => ways[0] + ways[1],
        };
        Side::arced(circle, from, to, self.at + toward, Along::Face)
    }

    /// Whether the material at the blend at `which` lies *outside* its own
    /// cylinder, which is what a blend filled into a concave edge does and one
    /// cut into a convex edge does not.
    ///
    /// **Read off which side of a face the axis stands.** A blend stands its
    /// axis a reach off each face it divides — on the material's own side where
    /// it was cut into the material, and on the far side where it was filled
    /// into the void.
    fn filled(&self, which: usize) -> bool {
        (self.axes[which].origin - self.at).dot(self.facing[which]) > 0.0
    }

    /// The patch spanning the opening, or `None` where the corner leaves none
    /// a body holds.
    ///
    /// Alternating a blend and the face after it, which is the order the six
    /// places come out in — see [`Opened::made`] — and the order
    /// [`Vertexed::side`] hands them back in.
    pub(super) fn spanned(&self) -> Option<Vertexed> {
        let opened = self.opened()?;
        let sides = std::array::from_fn(|at| match at % 2 {
            0 => self.crossed(&opened, at / 2),
            _ => self.sprung(&opened, at / 2),
        });
        Vertexed::new(self.at, sides)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::plane::Plane;
    use std::f64::consts::PI;

    /// The plane the patch is a height over, which is the surface's own to
    /// derive — see [`Vertexed`] — and asked here only by the rows below.
    fn flattening(corner: &Setback) -> Option<Plane> {
        let leaning: DVec3 = corner.facing.into_iter().sum();
        Some(Axis::about(corner.at, leaning.try_normalize()?).plane())
    }

    /// The notch's step corner, in the frame `.notes/VERTEX-BLENDS.md` reads it
    /// in: `u` across the floor, `v` along the reflex edge and `w` up the wall,
    /// with the corner at nought.
    ///
    /// The fill runs the reflex edge, its axis a reach into the void the notch
    /// leaves; the two cuts run the edges the cap makes with the floor and the
    /// wall, each a reach into the material.
    fn notch(reach: f64, setback: f64) -> Setback {
        let axes = [
            Line {
                origin: DVec3::new(-reach, 0.0, -reach),
                direction: DVec3::Y,
            },
            Line {
                origin: DVec3::new(0.0, reach, reach),
                direction: DVec3::NEG_X,
            },
            Line {
                origin: DVec3::new(reach, reach, 0.0),
                direction: DVec3::NEG_Z,
            },
        ];
        // The floor is shared by the fill and the first cut, the cap by the two
        // cuts, and the wall by the second cut and the fill.
        // Each pointing out of the material: the notch holds `v >= 0`, and
        // `u >= 0` or `w >= 0`.
        let facing = [DVec3::NEG_Z, DVec3::NEG_Y, DVec3::NEG_X];
        Setback::new(axes, facing, DVec3::ZERO, reach, setback)
    }

    /// **The six places are hand-computed off the rails.** A rail stands one
    /// reach off its edge along each face, and the setback carries it that far
    /// again along the edge — so the fill's place on the floor is `(−r, t, 0)`
    /// and its place on the wall is `(0, t, −r)`, and the two cuts read the
    /// same one turn round apiece.
    ///
    /// **And all six stand `√(t² + r²)` from the corner**, which is what the
    /// springs are arcs of one sphere about. Held at three reaches and two
    /// setbacks, the six agreeing to the last bit.
    #[test]
    fn the_six_places_of_the_opening_stand_at_one_distance() {
        for (reach, setback) in [(0.5, 1.0), (0.5, 0.75), (1.0, 1.5), (0.25, 0.5)] {
            let (r, t) = (reach, setback);
            let opened = notch(r, t).opened().expect("a square corner opens");
            // Each blend's end on the face before it first, and on the face
            // after it second — see [`Opened::made`].
            let want = [
                [DVec3::new(0.0, t, -r), DVec3::new(-r, t, 0.0)],
                [DVec3::new(-t, r, 0.0), DVec3::new(-t, 0.0, r)],
                [DVec3::new(r, 0.0, -t), DVec3::new(0.0, r, -t)],
            ];
            for (which, (got, pair)) in opened.made.iter().zip(&want).enumerate() {
                for (side, (got, want)) in got.iter().zip(pair).enumerate() {
                    assert!(
                        got.abs_diff_eq(*want, 1e-12),
                        "blend {which} side {side} stops at {got} where {want} is \
                         the rail",
                    );
                }
            }
            let want = (t * t + r * r).sqrt();
            assert!(
                (opened.reach - want).abs() < 1e-12,
                "the six stand {} from the corner where {want} is `√(t² + r²)`",
                opened.reach,
            );
        }
    }

    /// **Every side runs between the two places it is named by**, and lies on
    /// the shape it was cut from: a cross section a reach off its own blend's
    /// axis, a spring a reach off the corner and flat on its own face. And the
    /// patch is handed the six in that order, a blend and the face after it.
    #[test]
    fn the_six_sides_run_between_the_six_places() {
        let (r, t) = (0.5, 1.0);
        let corner = notch(r, t);
        let opened = corner.opened().expect("a square corner opens");
        let patch = corner.spanned().expect("a square corner spans a patch");
        for which in 0..3 {
            let side = corner.crossed(&opened, which);
            assert_eq!(
                side,
                patch.side(2 * which),
                "the patch holds another section"
            );
            assert_eq!(side.along, Along::Blend { filled: which == 0 });
            for (got, want) in side.ends().into_iter().zip(opened.made[which]) {
                assert!(
                    got.abs_diff_eq(want, 1e-12),
                    "{got} is not the place {want}"
                );
            }
            for step in 0..=8 {
                let along =
                    side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 8.0;
                let off = corner.axes[which].off(side.circle.at(along));
                assert!(
                    (off - r).abs() < 1e-12,
                    "the cross section stands {off} off"
                );
            }
        }
        for which in 0..3 {
            let side = corner.sprung(&opened, which);
            assert_eq!(
                side,
                patch.side(2 * which + 1),
                "the patch holds another spring"
            );
            assert_eq!(side.along, Along::Face);
            let want = [opened.made[which][1], opened.made[(which + 1) % 3][0]];
            for (got, want) in side.ends().into_iter().zip(want) {
                assert!(
                    got.abs_diff_eq(want, 1e-12),
                    "{got} is not the place {want}"
                );
            }
            let normal = corner.facing[which];
            assert_eq!(
                side.circle.axis.direction, normal,
                "the spring is not framed about its face"
            );
            for step in 0..=8 {
                let along =
                    side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 8.0;
                let at = side.circle.at(along);
                let flat = (at - corner.at).dot(normal).abs();
                assert!(flat < 1e-12, "the spring stands {flat} off its own face");
                assert!(
                    (corner.at.distance(at) - opened.reach).abs() < 1e-12,
                    "the spring stands {} off the corner",
                    corner.at.distance(at),
                );
            }
        }
    }

    /// **The spring on the face the two cuts share takes the long way round.**
    /// Both stand a reach off it on the one side, so the corner it keeps there
    /// turns three quarters and the near way would run out through the notch's
    /// own void. The other two faces have a blend either side and take the
    /// short way.
    #[test]
    fn the_spring_on_the_face_that_turns_past_a_half_runs_the_long_way() {
        let (r, t) = (0.5, 1.0);
        let corner = notch(r, t);
        let opened = corner.opened().expect("a square corner opens");
        let sweeps = [0, 1, 2].map(|which| {
            let side = corner.sprung(&opened, which);
            (side.bounds[1] - side.bounds[0]).abs()
        });
        assert!(sweeps[0] < PI, "the floor turns {} over", sweeps[0]);
        assert!(sweeps[2] < PI, "the wall turns {} over", sweeps[2]);
        assert!(sweeps[1] > PI, "the cap turns only {} over", sweeps[1]);

        // The cap is the notch's own L: everything but the quarter its void
        // takes, which is where a straight run between the two ends would go.
        let side = corner.sprung(&opened, 1);
        for step in 0..=32 {
            let along = side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 32.0;
            let at = side.circle.at(along);
            assert!(
                at.x >= -1e-12 || at.z >= -1e-12,
                "the spring stands at {at}, in the void the notch leaves",
            );
        }
    }

    /// **Every ray from the corner meets the opening once**: a spring stands the
    /// reach off the corner outright, and a cross section dips inside that and
    /// never to nothing. Which is what lets the patch fan its own walk from the
    /// corner — see [`Vertexed`].
    ///
    /// **And it is not enough to read the patch off the corner.** A graph about
    /// a place reads its own normal along the radial and never nought, and the
    /// radial at a spring lies *in* the face the spring is on — the corner
    /// being on that face too. So a graph about the corner is tangent to no
    /// face at all, and whatever the patch is read off stands away from it. See
    /// `.notes/VERTEX-BLENDS.md` §5. Held over the whole boundary at three
    /// reaches and two setbacks — and the notch's step corner reads `1.0212` at
    /// its nearest where the springs read `1.1180`.
    #[test]
    fn the_opening_stands_clear_of_the_corner_all_the_way_round() {
        for (r, t) in [(0.5, 1.0), (0.5, 0.75), (1.0, 1.5), (0.25, 0.5)] {
            let corner = notch(r, t);
            let opened = corner.opened().expect("a square corner opens");
            let mut least = f64::INFINITY;
            for which in 0..3 {
                for side in [
                    corner.crossed(&opened, which),
                    corner.sprung(&opened, which),
                ] {
                    for step in 0..=16 {
                        let along = side.bounds[0]
                            + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 16.0;
                        least = least.min(corner.at.distance(side.circle.at(along)));
                    }
                }
            }
            assert!(least > 0.0, "the opening runs through the corner itself");
            assert!(
                least <= opened.reach + 1e-12,
                "the opening stands {least} out where the springs stand {}",
                opened.reach,
            );
            if (r, t) == (0.5, 1.0) {
                assert!(
                    (least - 1.0212_f64).abs() < 1e-4,
                    "the notch's step corner reads {least} at its nearest",
                );
            }
        }
    }

    /// **The plane the patch stands over is the three faces' own, added up**,
    /// and every ray from the corner meets the opening once there too: the
    /// flattening is a parallel projection, which carries a line through the
    /// corner to a line through its image and keeps the order along it.
    #[test]
    fn the_opening_flattens_star_shaped_about_the_corner() {
        let corner = notch(0.5, 1.0);
        let opened = corner.opened().expect("a square corner opens");
        let over = flattening(&corner).expect("three faces that add up");
        assert!(
            over.normal().abs_diff_eq(-DVec3::ONE.normalize(), 1e-12),
            "the plane leans {}, not down the corner's own diagonal",
            over.normal(),
        );
        let mut bearings = Vec::new();
        for which in 0..3 {
            for side in [
                corner.crossed(&opened, which),
                corner.sprung(&opened, which),
            ] {
                for step in 0..64 {
                    let along =
                        side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 64.0;
                    let flat = over.flatten(side.circle.at(along));
                    bearings.push(flat.y.atan2(flat.x));
                }
            }
        }
        bearings.push(bearings[0]);
        let steps = bearings
            .windows(2)
            .map(|pair| (pair[1] - pair[0] + PI).rem_euclid(2.0 * PI) - PI)
            .collect::<Vec<_>>();
        let turned: f64 = steps.iter().sum();
        assert!(
            (turned.abs() - 2.0 * PI).abs() < 1e-9,
            "the rim winds {turned} about the corner, not once round"
        );
        assert!(
            steps.iter().all(|step| step.signum() == steps[0].signum()),
            "the rim turns back on itself"
        );
    }

    /// **A setback of the reach is the corner with no setback at all**, which
    /// is what the refusal is for: the two places on each face fall together
    /// there, the spring between them is nothing, and what is left is the three
    /// rail crossings the star already runs its legs from.
    #[test]
    fn a_setback_of_the_reach_leaves_no_spring_to_span() {
        let reach = 0.5;
        assert!(
            notch(reach, reach).spanned().is_none(),
            "a setback of the reach spans a patch, where its springs come to nothing",
        );
        assert!(
            notch(reach, reach + PLACED * 10.0).spanned().is_some(),
            "a setback past the reach leaves a spring and was refused",
        );
    }
}
