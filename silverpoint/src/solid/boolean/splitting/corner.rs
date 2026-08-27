//! One corner of a region's boundary, and the mark it carries.

use crate::math::winding;
use glam::DVec2;

/// One corner of a region, and where the stretch of boundary *leaving* it came
/// from.
///
/// **Carried rather than worked out again later**, which is the whole of what
/// makes a curved boolean exact where its regions are not. A closed cut is
/// flattened to be classified — see `ROUNDED` — so a circle imprinted on a
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
pub(crate) struct Corner {
    pub(crate) at: DVec2,
    pub(crate) came: Came,
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
pub(crate) fn passing(walk: &[Corner], step: usize) -> bool {
    let before = walk[(step + walk.len() - 1) % walk.len()].came;
    matches!(walk[step].came, Came::Arc(_)) && walk[step].came == before
}

/// Something one step of a loop carries a [`Came`] on.
///
/// Two of them — a corner of a region being cut, and a vertex of a loop being
/// sewn — and both are walked the other way round at some point, which is the
/// one thing worth writing once. See [`turned`].
pub(crate) trait Marked {
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
pub(crate) fn turned(walk: &mut [impl Marked]) {
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
pub(crate) enum Came {
    /// A straight run — of the face's own boundary, or of a straight imprint.
    Edge,
    /// A run along the curve at this index, which is the caller's to number —
    /// see `Imprints`, where one number per
    /// *stretch* and one per *curve* are held apart.
    Arc(u32),
}
