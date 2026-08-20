//! Cutting a face's triangles down until none of them leaves the surface.

use crate::math::triangulate::Fill;
use crate::number::predicate;
use crate::solid::geometry::surface::Surface;
use crate::solid::mesh::lattice::Lattice;
use glam::{DVec2, DVec3};

/// No corner: a side nothing has been put along.
const NONE: u32 = u32::MAX;

/// How many rounds of cutting a face may take before something is wrong.
///
/// Every round cuts each over-long side at the line nearest its middle, so a
/// round halves how many cells such a side reaches over. Twenty-four of them
/// covers a face sixteen million cells across, which no sagitta any caller has
/// reason to ask for comes near.
const ROUNDS: usize = 24;

/// Cuts the triangles of one face until each stands within the sagitta of the
/// surface, keeping the room it works in.
///
/// **Flattening a boundary says nothing about a middle.** A face's edges are
/// chorded as finely as the caller asked and its triangles are cut from those
/// corners alone — so a triangle is free to run from one side of the face to
/// the other, and over anything curved that is a triangle which has left the
/// surface.
///
/// **Most faces never need a corner put in**, which is what the grid
/// [`Mesher`](crate::Mesher) measures a face in buys: a face read in the cells
/// its own surface rules over comes out long and thin, and the strip a
/// shortest-edge clipper lays over one already follows the surface. What is
/// left is the face whose chains are chorded apart from one another — a wall's
/// foot is a circle and its head may be an ellipse, cut into their own numbers
/// of steps at their own places — where a triangle bridging them reaches over
/// a step of each and so over more than one cell.
///
/// So: **cut every side that reaches over more than one cell**, at the line of
/// the grid nearest its middle. The corner goes on the line and on the surface;
/// an inside corner belongs to this face alone, so nothing another face walked
/// has to agree with it.
///
/// **What that buys is a proof rather than a tolerance.** When no side reaches
/// over more than a cell, the three corners of every triangle stand pairwise
/// within one cell, so the triangle lies in a box one cell across — and
/// [`Surface::strides`] chose the cell so that a triangle in such a box cannot
/// stray further than the sagitta. Nothing here compares a distance against a
/// tolerance; it counts cells, and [`Refining::held`] is where the counting is
/// tied back to the promise.
///
/// **The face's own boundary is never cut**, a corner put on a face's edge
/// being one the face across it does not have. Nothing is lost by that on a
/// plane, a cylinder or a cone: every curve covering any angle arrives chorded
/// to the same sagitta by [`chords`](crate::math::arc::chords), so no edge of
/// such a face reaches over a whole cell.
///
/// **One axis at a time, and one finished before the other starts.** A corner
/// put on a line of the second lands along a run that already reaches over no
/// more than a cell of the first, so it cannot put back what the first pass
/// took out — see [`Lattice::cutting`]. Both at once has no such order to it.
#[derive(Debug, Default)]
pub(super) struct Refining {
    /// Every corner in the surface's own parameters — the boundary's first, in
    /// the order the cutter left them, then one per corner put in since.
    params: Vec<DVec2>,
    /// The same corners in the world.
    places: Vec<DVec3>,
    triangles: Vec<[u32; 3]>,
    /// The next round's triangles, swapped in at the end of it.
    spare: Vec<[u32; 3]>,
    /// Every side of the current triangles, one entry apiece, sorted by its
    /// ends so either triangle carrying it finds the same one.
    sides: Vec<Side>,
}

/// One side of the mesh, and what is being done to it this round.
#[derive(Debug, Clone, Copy)]
struct Side {
    /// The two corners it runs between, lower first.
    ends: [u32; 2],
    /// How many triangles carry it. One is the face's own boundary; two is an
    /// inside side; more is a contour pinched against itself.
    carried: u32,
    /// The corner cutting it, or [`NONE`].
    cut: u32,
}

impl Refining {
    /// Cut the triangles of `fill` down until none of them strays further than
    /// `sagitta` from `surface`, cutting only where `lattice` says.
    ///
    /// `boundary` is where the corners of `fill` stand in the world, in the
    /// same order — the places the loops were walked at, kept rather than
    /// evaluated back, so that a corner shared with the face across an edge
    /// stays bit for bit the one that face has.
    pub(super) fn refine(
        &mut self,
        surface: &Surface,
        boundary: &[DVec3],
        fill: &Fill,
        lattice: Lattice,
        sagitta: f64,
    ) {
        debug_assert_eq!(boundary.len(), fill.corners.len(), "a fill lost corners");
        // The cutter worked in cells and the surface answers in its own
        // parameters, so the corners come back out of the one into the other
        // here rather than being written over where the cutter left them.
        self.params.clear();
        self.params
            .extend(fill.corners.iter().map(|&uv| lattice.parameters(uv)));
        self.places.clear();
        self.places.extend_from_slice(boundary);
        self.triangles.clear();
        self.triangles.extend_from_slice(&fill.triangles);

        for axis in 0..2 {
            self.rule(surface, lattice, axis);
        }
        debug_assert!(
            self.held(surface, lattice, sagitta),
            "a face was cut into its cells and still strays",
        );
    }

    /// Whether every triangle now stands within `sagitta` of the surface, or
    /// else carries a run of the face's own boundary reaching over more than
    /// one cell — the one thing cutting cannot mend, a corner put on an edge
    /// being one the face across it does not have.
    ///
    /// Within it to within what the arithmetic cannot promise away, a cell
    /// being allowed to come out a rounding wide — see [`Lattice::cutting`].
    ///
    /// **What the cutting is for, stated where it can be checked.** The rule
    /// itself never measures how far anything strays: it counts cells, and the
    /// step was chosen so that counting cells is enough. This is the tie
    /// between the two, and the reason [`Surface::straying`] is written down at
    /// all.
    fn held(&self, surface: &Surface, lattice: Lattice, sagitta: f64) -> bool {
        (0..self.triangles.len()).all(|at| {
            let corners = self.triangles[at].map(|of| self.params[of as usize]);
            // Nothing inside reaches over a cell by the time this is asked, so
            // a side that does is one of the face's own edges.
            predicate::touching(surface.straying(corners), predicate::slack(sagitta))
                || (0..3).any(|slot| {
                    let [from, to] = self.ends(at, slot).map(|of| self.params[of as usize]);
                    (0..2).any(|axis| lattice.cutting(from, to, axis).is_some())
                })
        })
    }

    /// Cut along one axis of the grid until nothing more can be.
    fn rule(&mut self, surface: &Surface, lattice: Lattice, axis: usize) {
        for _ in 0..ROUNDS {
            if !self.round(surface, lattice, axis) {
                return;
            }
        }
        debug_assert!(false, "a face would not cut down to its own cells");
    }

    /// Every corner in the surface's own parameters.
    pub(super) fn params(&self) -> &[DVec2] {
        &self.params
    }

    /// The same corners in the world.
    pub(super) fn places(&self) -> &[DVec3] {
        &self.places
    }

    /// Three corners apiece, wound counterclockwise in the parameters.
    pub(super) fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// One round of cutting along `axis`, and whether anything was cut.
    fn round(&mut self, surface: &Surface, lattice: Lattice, axis: usize) -> bool {
        // **Asked before anything is gathered**, because the answer is almost
        // always no. A face measured in its own surface's cells comes out of
        // the cutter with every side inside one, so sorting every side of every
        // triangle to find that out would be the largest thing the mesher does
        // on every face of every frame, spent on nothing.
        let over = |params: &[DVec2], ends: [u32; 2]| {
            let [from, to] = ends.map(|of| params[of as usize]);
            lattice.cutting(from, to, axis)
        };
        let reaching = (0..self.triangles.len())
            .any(|at| (0..3).any(|slot| over(&self.params, self.ends(at, slot)).is_some()));
        if !reaching {
            return false;
        }
        self.gather();

        let mut split = false;
        for at in 0..self.sides.len() {
            // The face's own boundary is never cut — see the note on
            // [`Refining`]. A run of it reaching over more than a cell is the
            // one thing that leaves a triangle standing wider than the sagitta.
            if self.sides[at].carried < 2 {
                continue;
            }
            let Some(uv) = over(&self.params, self.sides[at].ends) else {
                continue;
            };
            self.sides[at].cut = self.put(surface, uv);
            split = true;
        }
        if !split {
            return false;
        }
        self.rebuild();
        true
    }

    /// The two corners the side `slot` of the triangle at `at` runs between.
    fn ends(&self, at: usize, slot: usize) -> [u32; 2] {
        let corners = self.triangles[at];
        [corners[slot], corners[(slot + 1) % 3]]
    }

    /// Take on a corner at the parameters `uv`, and say which one it is.
    fn put(&mut self, surface: &Surface, uv: DVec2) -> u32 {
        self.params.push(uv);
        self.places.push(surface.at(uv));
        self.params.len() as u32 - 1
    }

    /// Take every side of every triangle, one entry apiece and sorted.
    fn gather(&mut self) {
        self.sides.clear();
        self.sides.reserve(self.triangles.len() * 3);
        for corners in &self.triangles {
            for slot in 0..3 {
                let (from, to) = (corners[slot], corners[(slot + 1) % 3]);
                self.sides.push(Side {
                    ends: [from.min(to), from.max(to)],
                    carried: 1,
                    cut: NONE,
                });
            }
        }
        self.sides.sort_unstable_by_key(|side| side.ends);
        let mut kept = 0;
        for at in 0..self.sides.len() {
            if kept > 0 && self.sides[kept - 1].ends == self.sides[at].ends {
                self.sides[kept - 1].carried += 1;
            } else {
                self.sides[kept] = self.sides[at];
                kept += 1;
            }
        }
        self.sides.truncate(kept);
    }

    /// Lay the triangles down again around the corners put in this round.
    fn rebuild(&mut self) {
        let Self {
            params,
            triangles,
            spare,
            sides,
            ..
        } = self;
        spare.clear();
        spare.reserve(triangles.len() * 2);
        for &[a, b, c] in triangles.iter() {
            let found = |from: u32, to: u32| {
                let key = [from.min(to), from.max(to)];
                sides[sides
                    .binary_search_by_key(&key, |side| side.ends)
                    .expect("every side of every triangle was gathered")]
                .cut
            };
            let (p, q, r) = (found(a, b), found(b, c), found(c, a));
            // Each shape below is the corner between the cut sides taken off,
            // and whatever is left divided by whichever of its two diagonals is
            // shorter — the longer one leans towards a sliver.
            let across =
                |one: u32, two: u32| params[one as usize].distance_squared(params[two as usize]);
            match (p != NONE, q != NONE, r != NONE) {
                (false, false, false) => spare.push([a, b, c]),
                (true, false, false) => spare.extend([[a, p, c], [p, b, c]]),
                (false, true, false) => spare.extend([[a, b, q], [a, q, c]]),
                (false, false, true) => spare.extend([[a, b, r], [b, c, r]]),
                (true, true, false) => {
                    spare.push([p, b, q]);
                    if across(a, q) <= across(p, c) {
                        spare.extend([[a, p, q], [a, q, c]]);
                    } else {
                        spare.extend([[a, p, c], [p, q, c]]);
                    }
                }
                (false, true, true) => {
                    spare.push([q, c, r]);
                    if across(a, q) <= across(b, r) {
                        spare.extend([[a, b, q], [a, q, r]]);
                    } else {
                        spare.extend([[a, b, r], [b, q, r]]);
                    }
                }
                (true, false, true) => {
                    spare.push([r, a, p]);
                    if across(p, c) <= across(b, r) {
                        spare.extend([[p, b, c], [p, c, r]]);
                    } else {
                        spare.extend([[p, b, r], [b, c, r]]);
                    }
                }
                (true, true, true) => {
                    spare.extend([[a, p, r], [p, b, q], [r, q, c], [p, q, r]]);
                }
            }
        }
        std::mem::swap(&mut self.triangles, &mut self.spare);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loops::Loops;
    use crate::math::plane::Plane;
    use crate::math::triangulate::Cutter;
    use crate::solid::geometry::axis::Axis;
    use crate::solid::geometry::cylinder::Cylinder;
    use crate::solid::geometry::sphere::Sphere;

    /// The frame every surface below is built on.
    fn upright() -> Axis {
        Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X)
    }

    /// One face's parameters, ready to hand to [`Refining::refine`].
    ///
    /// Held together because the three go together: the cutter works in cells,
    /// the surface answers in its own parameters, and the boundary places are
    /// what the walk left — see [`Mesher::patch`](crate::Mesher).
    struct Patched {
        fill: Fill,
        boundary: Vec<DVec3>,
        lattice: Lattice,
    }

    impl Patched {
        /// Flatten `around`, cut it in the cells `surface` rules over.
        fn of(surface: &Surface, around: &[DVec2], sagitta: f64) -> Self {
            let lattice = Lattice::of(surface, around, sagitta);
            let celled: Vec<DVec2> = around.iter().map(|&uv| lattice.celled(uv)).collect();
            let mut fill = Fill::default();
            Cutter::default().polygon(&celled, &Loops::default(), &mut fill);
            let boundary = fill
                .corners
                .iter()
                .map(|&uv| surface.at(lattice.parameters(uv)))
                .collect();
            Self {
                fill,
                boundary,
                lattice,
            }
        }

        /// The corners of the fill in the surface's own parameters.
        fn params(&self) -> Vec<DVec2> {
            self.fill
                .corners
                .iter()
                .map(|&uv| self.lattice.parameters(uv))
                .collect()
        }

        /// Throw the triangles away and fan the whole boundary off its first
        /// corner, which is the worst triangulation of it there is and the one
        /// ear clipping fell into before a face was measured in cells.
        ///
        /// Corners of the fan that fall in a line leave triangles with no area
        /// in them, which is not a reason to leave them out: what is being
        /// asked here is whether the cutting holds up against *whatever* it is
        /// handed, and a triangulation with slivers in it is one of the things
        /// it may be handed.
        fn fanned(mut self) -> Self {
            let len = self.fill.corners.len() as u32;
            self.fill.triangles.clear();
            self.fill
                .triangles
                .extend((1..len - 1).map(|at| [0, at, at + 1]));
            self
        }

        /// Cut it down, and hand back the refining that did it.
        fn refined(&self, surface: &Surface, sagitta: f64) -> Refining {
            let mut refining = Refining::default();
            refining.refine(surface, &self.boundary, &self.fill, self.lattice, sagitta);
            refining
        }
    }

    /// Six cells of a unit cylinder two tall, its foot chorded every nine
    /// tenths of a cell and its head every seven tenths.
    ///
    /// Chorded in cells rather than in radians, so that asking for a finer
    /// sagitta re-chords the boundary the way
    /// [`chords`](crate::math::arc::chords) would rather than leaving it
    /// coarser than the grid — which is a face no walk hands over, every curve
    /// covering any angle arriving cut to the same sagitta.
    fn wall(sagitta: f64) -> Vec<DVec2> {
        let cell = crate::math::arc::widest(1.0, sagitta);
        let (wide, tall) = (6.0 * cell, 2.0);
        let mut around = Vec::new();
        let mut at = 0.0;
        while at < wide {
            around.push(DVec2::new(at, 0.0));
            at += 0.9 * cell;
        }
        around.push(DVec2::new(wide, 0.0));
        let mut at = wide;
        while at > 0.0 {
            around.push(DVec2::new(at, tall));
            at -= 0.7 * cell;
        }
        around.push(DVec2::new(0.0, tall));
        around
    }

    /// How far the worst triangle of a mesh leaves the surface.
    fn worst(surface: &Surface, params: &[DVec2], triangles: &[[u32; 3]]) -> f64 {
        triangles.iter().fold(0.0_f64, |far, &[a, b, c]| {
            far.max(surface.straying([params[a as usize], params[b as usize], params[c as usize]]))
        })
    }

    /// Twice the area a mesh covers in the parameters, which no conforming cut
    /// may change.
    fn covered(params: &[DVec2], triangles: &[[u32; 3]]) -> f64 {
        triangles.iter().fold(0.0, |total, &[a, b, c]| {
            let (a, b, c) = (params[a as usize], params[b as usize], params[c as usize]);
            total + (b - a).perp_dot(c - a)
        })
    }

    /// **However badly a wall is triangulated, it is cut back to the
    /// sagitta** — which is what makes the sagitta a promise rather than
    /// something the clipper is trusted to arrive at.
    ///
    /// Six cells of a unit cylinder, two tall, its foot chorded every nine
    /// tenths of a cell and its head every seven tenths — each inside a cell,
    /// as everything [`chords`](crate::math::arc::chords) hands over is, and
    /// out of step with each other, as a circle and an ellipse over one turn
    /// are. Measured in cells the clipper already lays a strip over this and
    /// nothing needs cutting, which is the ordinary case and is why the mesher
    /// pays almost nothing for any of it. So the strip is thrown away and the
    /// whole boundary fanned off one corner instead, which is what ear clipping
    /// did before the cells were there: a triangle reaching the whole six cells
    /// across, standing `1 − cos(0.268)` off the wall, or thirty-five times
    /// what was asked for.
    ///
    /// Afterwards: nothing strays, the boundary is untouched corner for corner,
    /// every corner put in is on the cylinder, and the mesh covers exactly what
    /// it covered — the last two together saying the cut was conforming rather
    /// than merely finer.
    #[test]
    fn however_badly_a_wall_is_tiled_it_is_cut_back_to_the_sagitta() {
        let sagitta = 1e-3;
        let surface = Surface::Cylinder(Cylinder {
            axis: upright(),
            radius: 1.0,
        });
        let given = Patched::of(&surface, &wall(sagitta), sagitta).fanned();
        let params = given.params();
        let coarse = worst(&surface, &params, &given.fill.triangles);
        assert!(
            coarse > 30.0 * sagitta,
            "the fan this is given already follows the wall: {coarse}",
        );

        let refining = given.refined(&surface, sagitta);
        // To within a rounding, a cell being allowed to come out that much
        // wide — see [`Lattice::cutting`].
        let fine = worst(&surface, refining.params(), refining.triangles());
        assert!(
            predicate::touching(fine, predicate::slack(sagitta)),
            "a triangle strays {fine} of {sagitta}",
        );
        assert!(
            refining.triangles().len() > given.fill.triangles.len(),
            "nothing was cut",
        );

        assert_eq!(&refining.params()[..params.len()], &params[..]);
        assert_eq!(
            &refining.places()[..given.boundary.len()],
            &given.boundary[..]
        );
        for &at in &refining.places()[given.boundary.len()..] {
            assert!(surface.off(at) < 1e-12, "{at:?} is not on the wall");
        }
        let (was, now) = (
            covered(&params, &given.fill.triangles),
            covered(refining.params(), refining.triangles()),
        );
        assert!(
            (now - was).abs() < 1e-12,
            "the mesh covered {was} and now covers {now}",
        );

        // **A tenth of the sagitta is a grid three times as fine**, so the same
        // wall comes back with more of everything — which is what says the
        // sagitta is read at all rather than one mesh being handed back.
        let finer = Patched::of(&surface, &wall(sagitta / 10.0), sagitta / 10.0).fanned();
        let refining = finer.refined(&surface, sagitta / 10.0);
        assert!(refining.triangles().len() > given.fill.triangles.len());
        let fine = worst(&surface, refining.params(), refining.triangles());
        assert!(
            predicate::touching(fine, predicate::slack(sagitta / 10.0)),
            "a triangle strays {fine}",
        );
    }

    /// **A face wide both ways is given corners in the middle of it**, which
    /// is the case no triangulation of a boundary alone can reach.
    ///
    /// A square patch of a unit sphere, six tenths of a radian either way,
    /// nineteen cells across each way at a sagitta of a thousandth. Its middle
    /// stands a tenth of a radius proud of any triangle laid across it and
    /// there is no corner out there to hang one on, so the corners have to be
    /// put there — on the lines of the grid, one axis after the other.
    ///
    /// Nothing the kernel builds is shaped like this: a cylinder and a cone
    /// stand one cell tall however far they run, and a plane has no line at
    /// all. It is written down because the *rule* does not know that, and a
    /// rule that only ever runs on strips is a rule nobody has watched work.
    #[test]
    fn a_face_wide_in_both_directions_is_given_corners_in_the_middle() {
        let sagitta = 1e-3;
        let surface = Surface::Sphere(Sphere {
            axis: upright(),
            radius: 1.0,
        });
        let mut around = Vec::new();
        for side in 0..4 {
            for step in 0..20 {
                let along = -0.6 + 1.2 * step as f64 / 20.0;
                around.push(match side {
                    0 => DVec2::new(along, -0.6),
                    1 => DVec2::new(0.6, along),
                    2 => DVec2::new(-along, 0.6),
                    _ => DVec2::new(-0.6, -along),
                });
            }
        }
        let given = Patched::of(&surface, &around, sagitta);
        let params = given.params();
        let coarse = worst(&surface, &params, &given.fill.triangles);
        assert!(
            coarse > 0.09,
            "the patch this is given already follows the ball: {coarse}",
        );

        let refining = given.refined(&surface, sagitta);
        let fine = worst(&surface, refining.params(), refining.triangles());
        assert!(
            predicate::touching(fine, predicate::slack(sagitta)),
            "a triangle strays {fine} of {sagitta}",
        );

        assert_eq!(&refining.params()[..params.len()], &params[..]);
        assert_eq!(
            &refining.places()[..given.boundary.len()],
            &given.boundary[..]
        );
        for &at in &refining.places()[given.boundary.len()..] {
            assert!(surface.off(at) < 1e-12, "{at:?} is not on the ball");
        }
        let (was, now) = (
            covered(&params, &given.fill.triangles),
            covered(refining.params(), refining.triangles()),
        );
        assert!(
            (now - was).abs() < 1e-12,
            "the mesh covered {was} and now covers {now}",
        );
    }

    /// A face on a plane is handed back exactly as it came, which is what keeps
    /// every flat face in a drawing costing the mesher nothing.
    #[test]
    fn a_flat_face_is_handed_back_as_it_came() {
        let surface = Surface::Plane(Plane::GROUND);
        let around = [
            DVec2::new(-3.0, -3.0),
            DVec2::new(3.0, -3.0),
            DVec2::new(3.0, 3.0),
            DVec2::new(0.0, 1.0),
            DVec2::new(-3.0, 3.0),
        ];
        let given = Patched::of(&surface, &around, 1e-6);
        let refining = given.refined(&surface, 1e-6);
        assert_eq!(refining.params(), &given.params()[..]);
        assert_eq!(refining.triangles(), &given.fill.triangles[..]);
    }
}
