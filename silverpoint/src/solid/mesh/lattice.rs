//! The grid a surface's own curvature rules over a face's parameters.

use crate::number::tolerance::ROUNDING;
use crate::solid::geometry::surface::Surface;
use glam::DVec2;

/// The grid of parameter lines a face on one surface may not be triangulated
/// across.
///
/// **One cell is one step in each direction, and a triangle no wider than one
/// is within the sagitta by construction.** That is the whole of what this is
/// for: [`Refining`](super::refining::Refining) cuts every side reaching over
/// more than a cell, so when none does, the three corners of every triangle
/// stand pairwise within one — and the step was chosen so that a triangle in a
/// box that size cannot stray further than was asked for. No tolerance is
/// compared against; the bound is the geometry of the grid.
///
/// **It is also the unit the face is measured in before it is cut.** A
/// triangulator joining each corner to its nearest is asking a question about
/// distance, and an angle in radians against a height in millimetres is two
/// units pretending to be one — a tenth of a radian reads as the smaller
/// number, so the clipper joins across a cylinder in preference to along it and
/// lays one triangle over the whole face. Divided through by [`Lattice::step`],
/// one unit means one step either way and the question has an answer.
///
/// Anchored on the face's own corner rather than on the origin, so that a
/// direction whose step is the whole face has no line inside it at all.
#[derive(Debug, Clone, Copy)]
pub(super) struct Lattice {
    /// The corner the lines are counted from — the low end of the face's own
    /// parameters.
    low: DVec2,
    /// How far apart the lines stand in each parameter.
    step: DVec2,
}

impl Lattice {
    /// The grid a face on `surface` is held to at `sagitta`, given the
    /// `outline` its boundary flattened to.
    ///
    /// The step is one of two things in each direction. Where the surface
    /// bends, it is what [`Surface::strides`] allows, which is the step
    /// [`chords`](crate::math::arc::chords) already cuts the boundary at — so a
    /// triangle across the face and a chord along its edge are held to one rule
    /// and cannot drift apart. Where it does not bend, no step is truer than
    /// any other and the step is *the whole face*: a wall is then one cell tall
    /// and as many wide as it has steps round, which is the strip it should be
    /// cut into, and there is no line across it to cut at. A plane, bending
    /// neither way, comes out the single cell it may as well be.
    ///
    /// Taking the face's own reach for a straight direction is also what makes
    /// the cut **invariant to the units the model is drawn in**, which the flat
    /// tolerances the cutter reads —
    /// [`TOUCHING`](crate::math::approx::TOUCHING) and its neighbours —
    /// otherwise are not: a face measured in metres and the same face in
    /// millimetres stood a thousandfold apart against a fixed figure on one
    /// axis, and against a radian that had not moved at all on the other.
    pub(super) fn of(surface: &Surface, outline: &[DVec2], sagitta: f64) -> Self {
        let mut low = DVec2::INFINITY;
        let mut high = DVec2::NEG_INFINITY;
        for &uv in outline {
            low = low.min(uv);
            high = high.max(uv);
        }
        let step = surface
            .strides(low.y.abs().max(high.y.abs()), sagitta)
            .min(high - low);
        // A face with no width at all in a parameter has no step to speak of,
        // so that parameter is left as it stands. It fills to nothing either
        // way — there is no triangle in a run of corners with no area between
        // them — and dividing by nought on the way there would make a mess of
        // saying so.
        Self {
            low,
            step: DVec2::select(step.cmpgt(DVec2::ZERO), step, DVec2::ONE),
        }
    }

    /// `uv` read in cells, which is the unit everything from the clipper
    /// inwards works in — see the note on [`Lattice`].
    pub(super) fn celled(self, uv: DVec2) -> DVec2 {
        uv / self.step
    }

    /// A place in cells read back into the surface's own parameters.
    ///
    /// Named alongside its opposite because the pair is the one place a sign
    /// slip would be silent: a face measured one way and evaluated the other
    /// comes out somewhere else entirely, and no assertion in the mesher would
    /// notice.
    pub(super) fn parameters(self, cell: DVec2) -> DVec2 {
        cell * self.step
    }

    /// Whether the run from `from` to `to` reaches over more than one cell of
    /// `axis`, **by more than a rounding**.
    ///
    /// **A run reaching over one cell or less is left alone**, wherever it
    /// lies, and that wants stating plainly: what the mesher owes is a triangle
    /// inside a cell-*sized* box, not a triangle inside a cell of the grid, and
    /// three corners pairwise within one cell of each other are inside such a
    /// box. Cutting a run that merely straddles a line would double the mesh of
    /// every curved face in the drawing to buy nothing.
    ///
    /// **The rounding is not caution.** A run of exactly one cell comes out a
    /// bit over as often as a bit under, and cutting one that is over by an ulp
    /// puts a corner where a corner already is. The piece left has no length,
    /// covers the cells its long side covers, and so asks to be cut again for
    /// ever — measured, a wall of sixteen triangles grew by twenty-four a round
    /// without end. This is the one place in the kernel a bare figure means
    /// something: the coordinates here are counts of cells rather than lengths.
    ///
    /// It keeps a coarser boundary out of trouble too. One chorded at one and a
    /// half cells lands corners a whisker off the lines, over and over — the
    /// chording and the grid are two quantizations of one sagitta and disagree
    /// in the last digits — and cutting at every straddle shaves a
    /// ten-thousandth of a cell off a run each time. Measured, that left a
    /// patch of a sphere gaining seventy-six triangles a round for ever, all of
    /// them with no area in them.
    pub(super) fn over(self, from: DVec2, to: DVec2, axis: usize) -> bool {
        (to[axis] - from[axis]).abs() / self.step[axis] > 1.0 + ROUNDING
    }

    /// Where the lines of `axis` cross the run from `from` to `to`, in the
    /// order they stand along it from `from`, appended to `into`.
    ///
    /// Named for what it answers rather than for how: these are the crossings
    /// worth cutting at, and a caller reading them as *every* crossing of the
    /// grid writes the wrong thing.
    ///
    /// **A run reaching over one cell or less is crossed by nothing**, however
    /// it lies — see [`Lattice::over`], which is that rule and the rounding it
    /// carries.
    ///
    /// **Every line it does cross, and not the one nearest its middle.** A
    /// triangle cut by a single line comes back as three, so a round that cuts
    /// one line costs three triangles for each halving of the face, and a face
    /// `w` cells across settles at `w` to the power of 1.585 of them. Cut by
    /// every line at once it comes back as `2w + 1`, which is what its own
    /// cells hold. Measured on a ball of radius three at a sagitta of a
    /// ten-thousandth: 3,359,590 triangles the one way against 296,000 the
    /// other.
    ///
    /// Each place comes back with its coordinate *on* the line exactly rather
    /// than interpolated onto it, so a piece cut here cannot come back and
    /// cross the same line again by a rounding.
    ///
    /// **One axis at a time, and the caller finishes one before it starts the
    /// other.** A corner put on a line of the second axis lands somewhere along
    /// a run that already reaches over no more than a cell of the first, so it
    /// cannot put back what the first pass took out. Both axes at once has no
    /// such order to it.
    pub(super) fn crossings(self, from: DVec2, to: DVec2, axis: usize, into: &mut Vec<DVec2>) {
        if !self.over(from, to, axis) {
            return;
        }
        let (start, end) = (from[axis], to[axis]);
        let counted = |at: f64| (at - self.low[axis]) / self.step[axis];
        let (below, above) = (counted(start.min(end)), counted(start.max(end)));
        // Strictly inside, which a run reaching over more than a cell always
        // has at least one of.
        let (first, last) = (below.floor() + 1.0, above.ceil() - 1.0);
        let count = (last - first) as usize + 1;
        into.reserve(count);
        for nth in 0..count {
            let step = nth as f64;
            let index = if end >= start {
                first + step
            } else {
                last - step
            };
            let line = self.low[axis] + index * self.step[axis];
            let mut at = from + (to - from) * ((line - start) / (end - start));
            at[axis] = line;
            into.push(at);
        }
    }
}
