//! Cutting a face's triangles down until none of them leaves the surface.

use crate::math::triangulate::Fill;
use crate::number::predicate::{self, ApproxEq};
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
    scratch: Scratch,
}

/// Every list a pass works in, kept so that the next one need not ask for
/// them again.
///
/// Apart from the answer above rather than mixed in with it: what a refining
/// *is* is its corners and its triangles, and none of the below outlives the
/// call that filled it.
#[derive(Debug, Default)]
struct Scratch {
    /// The next pass's triangles, swapped in at the end of it.
    spare: Vec<[u32; 3]>,
    /// Every side of the current triangles, one entry apiece, sorted by its
    /// ends so either triangle carrying it finds the same one.
    sides: Vec<Side>,
    /// Where each triangle's own three sides sit in [`Scratch::sides`], in the
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
    /// Flat, with [`Scratch::starts`] beside it saying where each side's own
    /// run begins: a side carries none at all on almost every face, and a
    /// vector apiece would reach the heap once per side per frame.
    along: Vec<u32>,
    /// Where each side's run in [`Scratch::along`] begins, one longer than
    /// [`Scratch::sides`] so that the last run has an end.
    starts: Vec<u32>,
    /// One triangle's own corners and the corners put along its sides, in
    /// winding order.
    walk: Vec<u32>,
    /// [`Scratch::walk`] read as two chains from its lowest corner along the
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
                self.scratch.sides[self.scratch.slots[at * 3 + slot] as usize].wanted = true;
            }
        }

        self.scratch.along.clear();
        self.scratch.starts.clear();
        self.scratch
            .starts
            .reserve_exact(self.scratch.sides.len() + 1);
        for at in 0..self.scratch.sides.len() {
            self.scratch.starts.push(self.scratch.along.len() as u32);
            if !self.scratch.sides[at].wanted || !self.cuttable(at) {
                continue;
            }
            let [from, to] = self.between(self.scratch.sides[at].ends);
            self.scratch.crossed.clear();
            lattice.crossings(from, to, axis, &mut self.scratch.crossed);
            for crossing in 0..self.scratch.crossed.len() {
                let uv = self.scratch.crossed[crossing];
                let put = self.put(surface, uv);
                self.scratch.along.push(put);
            }
        }
        self.scratch.starts.push(self.scratch.along.len() as u32);
        if self.scratch.along.is_empty() {
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
        self.scratch.sides[at].carried > 1 || self.collapsed(at)
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
        let [from, to] = self.scratch.sides[at]
            .ends
            .map(|of| self.places[of as usize]);
        from.approx_eq(to, PLACED)
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
    /// lower first — which is also the key [`Scratch::sides`] is sorted by.
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
        self.scratch.sides.clear();
        self.scratch.sides.reserve(self.triangles.len() * 3);
        for at in 0..self.triangles.len() {
            for slot in 0..3 {
                let ends = self.ends(at, slot);
                self.scratch.sides.push(Side {
                    ends,
                    carried: 1,
                    wanted: false,
                });
            }
        }
        let sides = &mut self.scratch.sides;
        sides.sort_unstable_by_key(|side| side.ends);
        let mut kept = 0;
        for at in 0..sides.len() {
            if kept > 0 && sides[kept - 1].ends == sides[at].ends {
                sides[kept - 1].carried += 1;
            } else {
                sides[kept] = sides[at];
                kept += 1;
            }
        }
        sides.truncate(kept);

        self.scratch.slots.clear();
        self.scratch.slots.reserve_exact(self.triangles.len() * 3);
        for at in 0..self.triangles.len() {
            for slot in 0..3 {
                let ends = self.ends(at, slot);
                let found = self
                    .scratch
                    .sides
                    .binary_search_by_key(&ends, |side| side.ends)
                    .expect("every side of every triangle was gathered");
                self.scratch.slots.push(found as u32);
            }
        }
    }

    /// Lay the pieces down again around the corners put in along `axis`.
    fn rebuild(&mut self, axis: usize) {
        self.scratch.spare.clear();
        self.scratch
            .spare
            .reserve(self.triangles.len() + 2 * self.scratch.along.len());
        for at in 0..self.triangles.len() {
            self.scratch.walk.clear();
            for slot in 0..3 {
                let corners = self.triangles[at];
                self.scratch.walk.push(corners[slot]);
                let found = self.scratch.slots[at * 3 + slot] as usize;
                let run =
                    self.scratch.starts[found] as usize..self.scratch.starts[found + 1] as usize;
                // The corners of a side stand in the order they run from its
                // lower-numbered end, which is the order the triangle walks it
                // in only where it starts there.
                if corners[slot] == self.scratch.sides[found].ends[0] {
                    self.scratch
                        .walk
                        .extend_from_slice(&self.scratch.along[run]);
                } else {
                    self.scratch
                        .walk
                        .extend(self.scratch.along[run].iter().rev());
                }
            }
            self.strip(axis);
        }
        std::mem::swap(&mut self.triangles, &mut self.scratch.spare);
    }

    /// Lay triangles over the polygon in [`Scratch::walk`], which is one
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
        for at in 1..self.scratch.walk.len() {
            if self.sits(self.scratch.walk[at], axis) < self.sits(self.scratch.walk[low], axis) {
                low = at;
            }
            if self.sits(self.scratch.walk[at], axis) > self.sits(self.scratch.walk[high], axis) {
                high = at;
            }
        }
        // The second chain runs back the way the first came, which is a step of
        // one short of the whole once the walk is read round.
        for (chain, step) in [(0, 1), (1, self.scratch.walk.len() - 1)] {
            self.scratch.chains[chain].clear();
            let mut at = low;
            loop {
                self.scratch.chains[chain].push(self.scratch.walk[at]);
                if at == high {
                    break;
                }
                at = (at + step) % self.scratch.walk.len();
            }
        }

        let (mut one, mut two) = (0, 0);
        loop {
            let [ahead, behind] = [&self.scratch.chains[0], &self.scratch.chains[1]];
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
                self.scratch.spare.push(piece);
            }
        }
    }
}

#[cfg(test)]
mod tests;
