//! Which way each half-edge leaves the corner it starts from.

use crate::sketch::arrangement::edge::{Edge, Half};
use glam::DVec2;

/// Where each half-edge sits in the fan of them leaving its corner.
///
/// One run of half-edges gathered by corner rather than a vector per corner: a
/// drawing with a hundred corners would otherwise be a hundred heap blocks, and
/// emptying them to rebuild would hand every one of them straight back.
#[derive(Debug, Default)]
pub(super) struct Departures {
    /// Every half-edge, gathered by the corner it leaves and ordered within
    /// each corner by the direction it leaves in.
    leaving: Vec<Leaving>,
    /// Where each corner's fan begins in `leaving`, with the total on the end —
    /// so a corner's fan is `starts[corner]..starts[corner + 1]`, and a corner
    /// nothing leaves is the empty stretch between two equal entries.
    starts: Vec<usize>,
    /// Where each half-edge sits within its own fan.
    at: Vec<usize>,
}

impl Departures {
    /// Sort the half-edges leaving each corner by the direction they leave in —
    /// which is what the walk reads to decide where to turn.
    pub(super) fn fill(&mut self, corners: &[DVec2], edges: &[Edge]) {
        let Self {
            leaving,
            starts,
            at,
        } = self;
        // Both halves of what the sort reads, worked out once here rather than
        // per comparison. An arc's departure is a cosine and a sine and the
        // angle of it an `atan2`, and a sort asks its key of an item about
        // `log n` times — so measuring in the comparison measures the same
        // direction a dozen times over.
        leaving.clear();
        leaving.reserve_exact(edges.len() * 2);
        for (edge, piece) in edges.iter().enumerate() {
            for forward in [true, false] {
                let out = piece.departure(corners, forward);
                leaving.push(Leaving {
                    half: Half { edge, forward },
                    corner: piece.ends(forward)[0],
                    angle: out.y.atan2(out.x),
                });
            }
        }
        // Gathered by corner, and within a corner ordered by the direction the
        // edge leaves in — which is the fan the walk turns through. One sort
        // rather than one per corner, and no dearer for it: the angle is
        // compared only where the corners already match, which is exactly the
        // comparison a fan of its own would have made.
        leaving.sort_by(|a, b| {
            a.corner
                .cmp(&b.corner)
                .then_with(|| a.angle.total_cmp(&b.angle))
        });

        // Where each corner's fan begins, by counting what landed in it and
        // running the counts up.
        starts.clear();
        starts.resize(corners.len() + 1, 0);
        for leave in leaving.iter() {
            starts[leave.corner + 1] += 1;
        }
        for corner in 1..starts.len() {
            starts[corner] += starts[corner - 1];
        }

        // Where each half-edge sits within its own fan, which is what the walk
        // reads to decide where to turn.
        at.clear();
        at.resize(edges.len() * 2, 0);
        for corner in 0..corners.len() {
            let fan = &leaving[starts[corner]..starts[corner + 1]];
            for (position, leave) in fan.iter().enumerate() {
                at[leave.half.slot()] = position;
            }
        }
    }

    /// The half-edge leaving `corner` just clockwise of `half`.
    pub(super) fn after(&self, corner: usize, half: Half) -> Half {
        let fan = &self.leaving[self.starts[corner]..self.starts[corner + 1]];
        let position = self.at[half.slot()];
        fan[(position + fan.len() - 1) % fan.len()].half
    }
}

/// One half-edge in the fan at the corner it leaves, with what that fan is
/// ordered by carried alongside it.
#[derive(Debug, Clone, Copy)]
struct Leaving {
    half: Half,
    /// The corner it leaves, which is the fan it belongs to.
    corner: usize,
    /// Which way it heads as it goes, as an angle.
    angle: f64,
}
