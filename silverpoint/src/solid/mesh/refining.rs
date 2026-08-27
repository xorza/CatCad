//! Cutting a face's triangles down until none of them leaves the surface.

use crate::math::triangulate::Fill;
use crate::number::predicate;
use crate::number::tolerance::PLACED;
use crate::solid::geometry::surface::Surface;
use crate::solid::mesh::lattice::Lattice;
use glam::{DVec2, DVec3};

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
/// So: **cut every side that reaches over more than one cell**, at every line
/// of the grid it crosses. The corners go on the lines and on the surface; an
/// inside corner belongs to this face alone, so nothing another face walked has
/// to agree with it.
///
/// **Every line at once, and that is what keeps the mesh the size of the
/// face** — see [`Lattice::crossings`], which is where that is argued. One pass
/// an axis then does the whole of the cutting.
///
/// **What that buys is a proof rather than a measurement.** When no side reaches
/// over more than a cell, the three corners of every triangle stand pairwise
/// within one cell, so the triangle lies in a box one cell across — and
/// [`Surface::strides`] chose the cell so that a triangle in such a box cannot
/// stray further than the sagitta. Counting cells is cheap, and it answers for
/// every triangle of almost every face.
///
/// **The face's own boundary is never cut**, a corner put on a face's edge
/// being one the face across it does not have. Nothing is lost by that on a
/// plane, a cylinder or a cone: every curve covering any angle arrives chorded
/// to the same sagitta by [`chords`](crate::math::arc::chords), so no edge of
/// such a face reaches over a whole cell.
///
/// **A sphere loses by it.** Its cell is that same widest chord over the square
/// root of two, a triangle inside one having to fit the chord corner to corner
/// rather than side to side, while its meridians still arrive chorded at the
/// whole width. A run of its own boundary then reaches over more than a cell,
/// and the triangle carrying that run has no corner the grid can offer to bring
/// it inside one: the window such a corner would fall in is narrower than a
/// cell. What the counting asks for there cannot be had.
///
/// So **a triangle the counting condemns is asked outright how far it strays**,
/// and one already within the sagitta is left alone — see [`Refining::strays`].
/// The counting is sufficient and it is not necessary, and this is the promise
/// itself rather than a stand-in for it, so nothing is lost by stopping there
/// and a fifth of the mesh is saved by it. It is asked of the handful of
/// triangles the counting has already picked out rather than of every triangle
/// of every face.
///
/// **One axis at a time, and one finished before the other starts.** A corner
/// put on a line of the second lands along a run that already reaches over no
/// more than a cell of the first, so it cannot put back what the first pass
/// took out — see [`Lattice::crossings`]. Both at once has no such order to it.
#[derive(Debug, Default)]
pub(super) struct Refining {
    /// Every corner in the surface's own parameters — the boundary's first, in
    /// the order the cutter left them, then one per corner put in since.
    params: Vec<DVec2>,
    /// The same corners in the world.
    places: Vec<DVec3>,
    triangles: Vec<[u32; 3]>,
    /// The next pass's triangles, swapped in at the end of it.
    spare: Vec<[u32; 3]>,
    /// Every side of the current triangles, one entry apiece, sorted by its
    /// ends so either triangle carrying it finds the same one.
    sides: Vec<Side>,
    /// Where each triangle's own three sides sit in [`Refining::sides`], in the
    /// order [`Refining::ends`] numbers them.
    ///
    /// Written once a pass rather than searched for wherever a side is wanted:
    /// the table is sorted by the corners a side runs between, and a pass reads
    /// each triangle's three twice over — once to mark what wants cutting, once
    /// to lay the pieces down.
    slots: Vec<u32>,
    /// The corners put along every side, each side's in the order they stand
    /// along it from its lower-numbered end.
    ///
    /// Flat, with [`Refining::starts`] beside it saying where each side's own
    /// run begins: a side carries none at all on almost every face, and a
    /// vector apiece would reach the heap once per side per frame.
    along: Vec<u32>,
    /// Where each side's run in [`Refining::along`] begins, one longer than
    /// [`Refining::sides`] so that the last run has an end.
    starts: Vec<u32>,
    /// One triangle's own corners and the corners put along its sides, in
    /// winding order.
    walk: Vec<u32>,
    /// [`Refining::walk`] read as two chains from its lowest corner along the
    /// axis to its highest, the first forward and the second back.
    chains: [Vec<u32>; 2],
    /// Where the lines cross one side, as [`Lattice::crossings`] answers.
    crossed: Vec<DVec2>,
}

/// One side of the mesh, and what is being done to it this pass.
#[derive(Debug, Clone, Copy)]
struct Side {
    /// The two corners it runs between, lower first.
    ends: [u32; 2],
    /// How many triangles carry it. One is the face's own boundary; two is an
    /// inside side; more is a contour pinched against itself.
    carried: u32,
    /// Whether a triangle carrying it strays further than was asked for, and so
    /// stands to gain by the cut.
    wanted: bool,
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
            self.rule(surface, lattice, axis, sagitta);
        }
        debug_assert!(
            self.held(surface, lattice, sagitta),
            "a face was cut into its cells and still strays",
        );
    }

    /// Whether every triangle now stands within `sagitta` of the surface, or
    /// else stands as wide as a run of the face's own boundary holds it — the
    /// one thing cutting cannot mend, a corner put on an edge being one the
    /// face across it does not have.
    ///
    /// **What the cutting is for, asked of what it produced.** A pass cuts what
    /// it may, and this says that what it left was nothing it *needed* to cut —
    /// the two being different answers, and only the second one the promise.
    fn held(&self, surface: &Surface, lattice: Lattice, sagitta: f64) -> bool {
        (0..self.triangles.len()).all(|at| {
            // A triangle still straying stands on a run of the face's own
            // boundary, that being the one thing no pass could have taken.
            !self.strays(surface, sagitta, at) || (0..2).any(|axis| self.wide(lattice, axis, at))
        })
    }

    /// Cut every triangle by every line of `axis` that crosses a side of it.
    ///
    /// One pass and no more: a side cut at every line it crosses comes back in
    /// pieces reaching over no more than a cell apiece, and the sides the
    /// pieces are laid down along run between corners a line apart, so they
    /// reach over no more than a cell either.
    fn rule(&mut self, surface: &Surface, lattice: Lattice, axis: usize, sagitta: f64) {
        // **Asked before anything is gathered**, because the answer is almost
        // always no. A face measured in its own surface's cells comes out of
        // the cutter with every side inside one, so sorting every side of every
        // triangle to find that out would be the largest thing the mesher does
        // on every face of every frame, spent on nothing.
        if !(0..self.triangles.len()).any(|at| self.wide(lattice, axis, at)) {
            return;
        }
        self.gather();

        // **Asked of the triangles the counting has picked out, and of no
        // others.** A triangle every side of which stands inside a cell is
        // within the sagitta by construction and has nothing to gain; the rest
        // are asked outright, and one already within it is left alone whatever
        // the cells say — see the note on [`Refining`].
        for at in 0..self.triangles.len() {
            if !self.wide(lattice, axis, at) || !self.strays(surface, sagitta, at) {
                continue;
            }
            for slot in 0..3 {
                self.sides[self.slots[at * 3 + slot] as usize].wanted = true;
            }
        }

        self.along.clear();
        self.starts.clear();
        self.starts.reserve_exact(self.sides.len() + 1);
        for at in 0..self.sides.len() {
            self.starts.push(self.along.len() as u32);
            if !self.sides[at].wanted || !self.cuttable(at) {
                continue;
            }
            let [from, to] = self.between(self.sides[at].ends);
            self.crossed.clear();
            lattice.crossings(from, to, axis, &mut self.crossed);
            for crossing in 0..self.crossed.len() {
                let uv = self.crossed[crossing];
                let put = self.put(surface, uv);
                self.along.push(put);
            }
        }
        self.starts.push(self.along.len() as u32);
        if self.along.is_empty() {
            return;
        }
        self.rebuild(axis);
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

    /// Whether any side of the triangle at `at` reaches over more than one cell
    /// of `axis` — the cheap half of asking whether it wants cutting, and the
    /// only half almost every triangle of almost every face needs.
    fn wide(&self, lattice: Lattice, axis: usize, at: usize) -> bool {
        (0..3).any(|slot| {
            let [from, to] = self.between(self.ends(at, slot));
            lattice.over(from, to, axis)
        })
    }

    /// Whether the triangle at `at` stands further from the surface than
    /// `sagitta`.
    ///
    /// Further by more than the arithmetic reading it can promise away — see
    /// [`predicate::slack`].
    fn strays(&self, surface: &Surface, sagitta: f64, at: usize) -> bool {
        let corners = self.triangles[at].map(|of| self.params[of as usize]);
        !predicate::touching(surface.straying(corners), predicate::slack(sagitta))
    }

    /// Whether the side at `at` in the table may be cut at all.
    ///
    /// The face's own boundary may not be — see the note on [`Refining`] —
    /// except where it stands in one place, which no face across it can
    /// disagree about. See [`Refining::collapsed`].
    fn cuttable(&self, at: usize) -> bool {
        self.sides[at].carried > 1 || self.collapsed(at)
    }

    /// Whether the side at `at` in the table stands in one place — the side of
    /// a region that collapsed to a point.
    ///
    /// **Cut like an inside one, though it lies on the boundary.** A cone's
    /// apex and a sphere's poles are one place however far the angle runs, so
    /// the side of the region that meets them has no length in the world: there
    /// is no face across a point to disagree about a corner put on it, and the
    /// corner comes back at the same place whatever angle it is given.
    ///
    /// Left uncut it holds the piece carrying it as wide as the whole angle it
    /// covers, which on a cone is every triangle that meets the apex.
    fn collapsed(&self, at: usize) -> bool {
        let [from, to] = self.sides[at].ends.map(|of| self.places[of as usize]);
        predicate::coincident(from, to, PLACED)
    }

    /// Where the corner numbered `of` stands along `axis`.
    fn sits(&self, of: u32, axis: usize) -> f64 {
        self.params[of as usize][axis]
    }

    /// Where the two corners a side runs between stand in the surface's own
    /// parameters.
    fn between(&self, ends: [u32; 2]) -> [DVec2; 2] {
        ends.map(|of| self.params[of as usize])
    }

    /// The two corners the side `slot` of the triangle at `at` runs between,
    /// lower first — which is also the key [`Refining::sides`] is sorted by.
    fn ends(&self, at: usize, slot: usize) -> [u32; 2] {
        let corners = self.triangles[at];
        let (from, to) = (corners[slot], corners[(slot + 1) % 3]);
        [from.min(to), from.max(to)]
    }

    /// Take on a corner at the parameters `uv`, and say which one it is.
    fn put(&mut self, surface: &Surface, uv: DVec2) -> u32 {
        self.params.push(uv);
        self.places.push(surface.at(uv));
        self.params.len() as u32 - 1
    }

    /// Take every side of every triangle, one entry apiece and sorted, and say
    /// where each triangle's own three ended up.
    fn gather(&mut self) {
        self.sides.clear();
        self.sides.reserve(self.triangles.len() * 3);
        for at in 0..self.triangles.len() {
            for slot in 0..3 {
                let ends = self.ends(at, slot);
                self.sides.push(Side {
                    ends,
                    carried: 1,
                    wanted: false,
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

        self.slots.clear();
        self.slots.reserve_exact(self.triangles.len() * 3);
        for at in 0..self.triangles.len() {
            for slot in 0..3 {
                let ends = self.ends(at, slot);
                let found = self
                    .sides
                    .binary_search_by_key(&ends, |side| side.ends)
                    .expect("every side of every triangle was gathered");
                self.slots.push(found as u32);
            }
        }
    }

    /// Lay the pieces down again around the corners put in along `axis`.
    fn rebuild(&mut self, axis: usize) {
        self.spare.clear();
        self.spare
            .reserve(self.triangles.len() + 2 * self.along.len());
        for at in 0..self.triangles.len() {
            self.walk.clear();
            for slot in 0..3 {
                let corners = self.triangles[at];
                self.walk.push(corners[slot]);
                let found = self.slots[at * 3 + slot] as usize;
                let run = self.starts[found] as usize..self.starts[found + 1] as usize;
                // The corners of a side stand in the order they run from its
                // lower-numbered end, which is the order the triangle walks it
                // in only where it starts there.
                if corners[slot] == self.sides[found].ends[0] {
                    self.walk.extend_from_slice(&self.along[run]);
                } else {
                    self.walk.extend(self.along[run].iter().rev());
                }
            }
            self.strip(axis);
        }
        std::mem::swap(&mut self.triangles, &mut self.spare);
    }

    /// Lay triangles over the polygon in [`Refining::walk`], which is one
    /// triangle with corners along its sides.
    ///
    /// **Paired across the polygon rather than fanned from one corner of it.**
    /// A fan over a triangle cut into twenty strips is twenty slivers reaching
    /// its whole width; walking the two chains together takes the strips one at
    /// a time and leaves each of them the shape of its own cell. The count is
    /// the same either way — a polygon of `n` corners is `n - 2` triangles —
    /// and the shapes are not.
    ///
    /// The polygon is a triangle with corners on its sides, so it is convex and
    /// both chains run one way along `axis`. That is the whole of what the walk
    /// below needs, and it is why the two chains can be taken in step.
    ///
    /// A side the boundary holds carries no corners — see the note on
    /// [`Refining`] — so its chain is the bare side and the pieces beside it
    /// come out as wide as it is. That is the coarseness the chording already
    /// forces and no cut of this face could mend.
    fn strip(&mut self, axis: usize) {
        let (mut low, mut high) = (0, 0);
        for at in 1..self.walk.len() {
            if self.sits(self.walk[at], axis) < self.sits(self.walk[low], axis) {
                low = at;
            }
            if self.sits(self.walk[at], axis) > self.sits(self.walk[high], axis) {
                high = at;
            }
        }
        // The second chain runs back the way the first came, which is a step of
        // one short of the whole once the walk is read round.
        for (chain, step) in [(0, 1), (1, self.walk.len() - 1)] {
            self.chains[chain].clear();
            let mut at = low;
            loop {
                self.chains[chain].push(self.walk[at]);
                if at == high {
                    break;
                }
                at = (at + step) % self.walk.len();
            }
        }

        let (mut one, mut two) = (0, 0);
        loop {
            let [ahead, behind] = [&self.chains[0], &self.chains[1]];
            let (over, under) = (one + 1 < ahead.len(), two + 1 < behind.len());
            if !over && !under {
                break;
            }
            // Whichever chain reaches the next line first, so that a piece is
            // the strip between two lines rather than a wedge across several.
            let taken = over
                && (!under || self.sits(ahead[one + 1], axis) <= self.sits(behind[two + 1], axis));
            let piece = if taken {
                [ahead[one], ahead[one + 1], behind[two]]
            } else {
                [ahead[one], behind[two + 1], behind[two]]
            };
            if taken {
                one += 1
            } else {
                two += 1
            }
            // The two chains meet at both ends, so the first step off one end
            // and the last step onto the other name a corner twice over. Two
            // steps of the walk carry no piece, which is what leaves a polygon
            // of `n` corners the `n - 2` triangles it holds.
            if piece[0] != piece[1] && piece[1] != piece[2] && piece[2] != piece[0] {
                self.spare.push(piece);
            }
        }
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
    /// coarser than the grid. A wall's cell *is* that chord, so a wall is never
    /// handed over chorded coarser than one — a sphere is, and
    /// [`a_face_chorded_coarser_than_its_cells_still_settles`] is that face.
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

    /// A square patch of a sphere, six tenths of a radian either way, each side
    /// of it cut into `steps` even chords.
    fn dome(steps: usize) -> Vec<DVec2> {
        let mut around = Vec::with_capacity(4 * steps);
        for side in 0..4 {
            for step in 0..steps {
                let along = -0.6 + 1.2 * step as f64 / steps as f64;
                around.push(match side {
                    0 => DVec2::new(along, -0.6),
                    1 => DVec2::new(0.6, along),
                    2 => DVec2::new(-along, 0.6),
                    _ => DVec2::new(-0.6, -along),
                });
            }
        }
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
        // wide — see [`Lattice::over`].
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
        let around = dome(20);
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

    /// **A face whose boundary arrives chorded more coarsely than its own cells
    /// still settles**, which every face on a sphere does.
    ///
    /// The patch above, chorded at the widest chord the sagitta allows rather
    /// than inside a cell. That is what [`chords`](crate::math::arc::chords)
    /// hands over, and a sphere's cell is that chord over the square root of
    /// two, so a run of the boundary reaches over more than one cell. It may
    /// not be cut, and the triangle carrying it has no corner the grid can
    /// offer to bring it inside one: the window such a corner would have to
    /// fall in is narrower than a cell. Read as a thing to cut down, the rounds
    /// step that corner from the line below the window to the line above it and
    /// back, for ever.
    ///
    /// So what is asserted is the promise rather than the counting that usually
    /// stands in for it. No triangle strays further than a chord of the
    /// boundary does, the boundary is untouched corner for corner, and the mesh
    /// covers what it covered.
    #[test]
    fn a_face_chorded_coarser_than_its_cells_still_settles() {
        let sagitta = 1e-3;
        let surface = Surface::Sphere(Sphere {
            axis: upright(),
            radius: 1.0,
        });
        // Divided evenly into chords no wider than the sagitta allows, which is
        // what a walk hands over. Each comes out a hair under the widest, and
        // the cell is that widest over the square root of two.
        let widest = crate::math::arc::widest(1.0, sagitta);
        let steps = (1.2 / widest).ceil() as usize;
        let around = dome(steps);
        let given = Patched::of(&surface, &around, sagitta);
        let params = given.params();
        let reach = given.lattice.celled(DVec2::new(1.2 / steps as f64, 0.0)).x;
        assert!(
            reach > 1.0 && reach < 1.5,
            "the boundary is not chorded coarser than a cell: {reach}",
        );

        let refining = given.refined(&surface, sagitta);
        // A triangle carrying a chord of the boundary stands at least as far
        // off as that chord does, whatever is done to its other two sides, and
        // a chord this wide stands off by the sagitta itself. Twice it is what
        // a corner one cell away from both ends of such a chord costs.
        let fine = worst(&surface, refining.params(), refining.triangles());
        assert!(
            fine < 2.0 * sagitta,
            "a triangle strays {fine} of {sagitta}",
        );

        assert_eq!(&refining.params()[..params.len()], &params[..]);
        assert_eq!(
            &refining.places()[..given.boundary.len()],
            &given.boundary[..]
        );
        let (was, now) = (
            covered(&params, &given.fill.triangles),
            covered(refining.params(), refining.triangles()),
        );
        assert!(
            (now - was).abs() < 1e-12,
            "the mesh covered {was} and now covers {now}",
        );
    }

    /// **A face comes back the size its own cells are**, and stays that size as
    /// the sagitta falls.
    ///
    /// The one thing a cut that put in a single corner did not buy. A triangle
    /// cut by one line comes back as three, so every halving of the face cost
    /// three triangles where its cells ask for two, and the mesh settled at `w`
    /// to the power of 1.585 for a face `w` cells across. Cut by every line it
    /// crosses at once it comes back as `2w + 1` — see [`Lattice::crossings`].
    ///
    /// **Asserted as a ratio that does not climb**, rather than as a count: the
    /// count is a fact about one patch and the ratio is the property. Here the
    /// ratio falls, the patch being bounded by eighty corners however fine the
    /// sagitta — a face six cells across is mostly its own boundary and one
    /// sixty cells across is not. Cut a line at a time it climbed instead, and
    /// climbed without bound.
    #[test]
    fn a_face_comes_back_the_size_its_own_cells_are() {
        let surface = Surface::Sphere(Sphere {
            axis: upright(),
            radius: 1.0,
        });
        let around = dome(20);
        let mut coarsest = 0.0;
        let mut last = 0.0;
        for sagitta in [1e-2, 1e-3, 1e-4] {
            let given = Patched::of(&surface, &around, sagitta);
            let refining = given.refined(&surface, sagitta);
            let wide = given.lattice.celled(DVec2::new(1.2, 1.2));
            let apiece = refining.triangles().len() as f64 / (wide.x * wide.y);
            if coarsest == 0.0 {
                coarsest = apiece;
            }
            assert!(
                apiece <= coarsest,
                "{sagitta} came back {apiece} triangles a cell against {coarsest}",
            );
            last = apiece;
        }
        // Two is what a grid of cells holds outright. Twice that is the room a
        // triangulation cut across the grid rather than laid along it takes,
        // and it is measured rather than reasoned: 4.64 at the finest above.
        assert!(last < 5.0, "a face came back {last} triangles a cell");
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
