//! What bounds one region of a drawing, and out of what pieces.

use crate::sketch::arrangement::bound::Bound;
use crate::sketch::arrangement::edge::{Edge, Half};
use crate::sketch::arrangement::face::Face;
use crate::sketch::entity::Entity;

/// The room the reading works in, kept so that the next face need not ask for
/// it again.
///
/// One of the three things a rebuild works out once the drawing has been cut,
/// beside [`Components`](super::components::Components) and
/// [`Departures`](super::departures::Departures): each holds its own scratch,
/// is filled by one call, and is read back by another.
#[derive(Debug, Default)]
pub(super) struct Bounding {
    /// The boundary of the face being described, laid out flat.
    along: Vec<Walked>,
    /// The curves found bounding it.
    gathered: Vec<Gathered>,
}

impl Bounding {
    /// Work out what bounds `face`, and out of what pieces.
    ///
    /// Once per rebuild rather than once per ask, which is the whole of why it
    /// is a step at all and not a reading hung off [`Face`]. What bounds a
    /// region is a property of how the drawing was cut, so it moves only when
    /// this runs — where a solid being drawn walks its faces every frame, a
    /// selection asks whether a wall is still one of them, and a feature
    /// matches a name against every face there is.
    pub(super) fn fill(&mut self, face: &mut Face, edges: &[Edge]) {
        let Self { along, gathered } = self;
        let Face {
            outline,
            holes,
            named,
            ..
        } = face;

        // The boundary laid out flat, each piece with the curve it is of
        // and which loop walked it. Everything below reads this rather than
        // the loops: the curve a piece is of is worked out once here where
        // deriving it again per reading is what made asking dear in the
        // first place, and what is left is a slice to scan rather than an
        // iterator over loops to rebuild three times.
        along.clear();
        along.reserve_exact(outline.len() + holes.total());
        let mut lay = |run: &[Half], on_outline: bool| {
            along.extend(run.iter().map(|&half| {
                let bound = edges[half.edge].bound(half.forward);
                Walked {
                    bound,
                    key: key_of(bound),
                    on_outline,
                }
            }));
        };
        lay(outline, true);
        for hole in holes.iter() {
            lay(hole, false);
        }

        // Gathered by curve, so that the pieces of one fall together and
        // every reading below is a walk of the runs rather than a search
        // through them. Stable, so within a curve the pieces stay in the
        // order the region was walked along them.
        along.sort_by_key(|walked| walked.key);
        gathered.clear();
        let mut at = 0;
        while at < along.len() {
            let key = along[at].key;
            let mut end = at;
            let mut on_outline = false;
            while end < along.len() && along[end].key == key {
                on_outline |= along[end].on_outline;
                end += 1;
            }
            gathered.push(Gathered {
                key,
                bound: along[at].bound,
                on_outline,
            });
            at = end;
        }

        // A spur is walked out and back, so it appears both ways round and
        // bounds nothing at all; without this, drawing a stray line
        // touching a region would rename it. The far side of a curve is its
        // key with the low bit flipped, and the runs are in key order, so
        // finding it is a search of the curves rather than of the pieces.
        //
        // A name is made of the outline, so a spur dangling into a *hole* is
        // no part of what names the region and cannot take an outline curve
        // out of it.
        named.clear();
        for run in gathered.iter() {
            let turned = gathered
                .binary_search_by(|had| had.key.cmp(&(run.key ^ 1)))
                .ok()
                .map(|found| &gathered[found]);
            if run.on_outline && !turned.is_some_and(|had| had.on_outline) {
                named.push(run.bound);
            }
        }
    }
}

/// A number to gather a region's pieces by: which curve, and which side of it.
///
/// Ordered rather than merely compared, so that the pieces of one curve fall
/// together in a sort and a region bounded by a hundred curves is described by
/// walking runs instead of searching for them. The side is the low bit, which
/// puts the far side of a curve one flip away — and a spur is exactly a curve
/// whose far side is here too.
fn key_of(bound: Bound) -> u64 {
    // A slot is a `u32`, so one shifted up by the side bit still stops short of
    // the thirty-fourth: the segments fill the bottom of the range and the
    // circles start above everything they can reach.
    let (kind, slot) = match bound.of {
        Entity::Segment(id) => (0u64, id.slot()),
        Entity::Circle(id) => (1u64, id.slot()),
        // An edge is cut from a segment or a circle and from nothing else.
        of => unreachable!("{of:?} was never cut into an edge"),
    };
    (kind << 33) | ((slot as u64) << 1) | u64::from(bound.along)
}

/// One piece of curve a region is walked along, with what [`Bounding::fill`]
/// would otherwise work out about it more than once.
#[derive(Debug, Clone, Copy)]
struct Walked {
    /// The curve it is a piece of, and the side the region is on.
    bound: Bound,
    /// That same curve and side as something to sort by — see [`key_of`].
    key: u64,
    /// Whether it was walked by the region's outline as against by a hole.
    on_outline: bool,
}

/// One curve found bounding a region.
///
/// The whole of what a name is decided from: which curve and side, and whether
/// the region's *outline* runs along it as against only a hole.
#[derive(Debug, Clone, Copy)]
struct Gathered {
    key: u64,
    bound: Bound,
    on_outline: bool,
}
