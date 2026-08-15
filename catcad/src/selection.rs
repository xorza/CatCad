//! What the user has picked out, and the collection of it.

use crate::part::Part;

/// The parts of the drawing currently picked out.
///
/// Session state rather than the document's, like the tool in hand: what is
/// selected is what the *next* command will act on, and says nothing about what
/// the drawing is. So nothing here is written down by saving, and an undo puts
/// geometry back without disturbing what is picked out around it.
///
/// [`Part`]s rather than tags, deliberately. A tag is an index into one layout
/// of the drawing and is minted afresh whenever the drawing is laid out again —
/// which a drag does every frame — where a part names what it names across one:
/// a sketch handle for anything the sketch holds, and for a face, a position
/// among the faces that holds while the drawing's topology does.
///
/// In the order things were added, and nothing twice. Order is not used for
/// anything yet and is kept because it is free: the first thing picked is the
/// reference in every modeller's alignment commands, and a set would have
/// thrown that away before there was anything to ask it.
#[derive(Debug, Default)]
pub(crate) struct Selection {
    picked: Vec<Part>,
}

impl Selection {
    /// Make `what` the whole of what is selected — or nothing at all, when it
    /// is `None`.
    ///
    /// What a plain click asks for, whether or not it landed on anything: one
    /// rule, which is that a click selects exactly what is under it.
    pub(crate) fn select(&mut self, what: Option<Part>) {
        self.picked.clear();
        self.picked.extend(what);
    }

    /// Add `what` to what is selected, if it is not already there.
    ///
    /// Idempotent, which is what lets it be an intent at all: a replayed pass
    /// asks a second time and the answer does not move. See [`Intent`].
    ///
    /// [`Intent`]: crate::intent::Intent
    pub(crate) fn include(&mut self, what: Part) {
        if !self.contains(what) {
            self.picked.push(what);
        }
    }

    /// Whether `what` is picked out.
    pub(crate) fn contains(&self, what: Part) -> bool {
        self.picked.contains(&what)
    }

    /// Everything picked out, in the order it was picked.
    ///
    /// The order is the point of handing back a slice rather than a set: what a
    /// selection *admits* turns on it — see
    /// [`Drawing::offers`](crate::drawing::Drawing) — and a pair read the other
    /// way round is a different relation for every constraint that is not
    /// symmetric.
    pub(crate) fn picked(&self) -> &[Part] {
        &self.picked
    }

    /// Drop everything `keep` refuses.
    ///
    /// What holding the sketch's own handles costs: they outlive what they name
    /// whenever a step that created geometry is taken back, and a handle left
    /// over is not merely inert — the sketch is restored arenas and all, so the
    /// next entity created takes the very same one and would come up selected
    /// without anyone having picked it. So the selection is put back against
    /// the drawing whenever the drawing might have moved.
    pub(crate) fn retain(&mut self, keep: impl Fn(Part) -> bool) {
        self.picked.retain(|&part| keep(part));
    }
}

/// How many are picked out, which the application never asks — it draws what is
/// selected by walking the drawing and asking [`Selection::contains`] of each,
/// so a count would tell it nothing it goes on to use.
///
/// A test asks, because a count is what tells "added to" from "replaced": both
/// leave the newest entity picked out, and only the tally says whether the old
/// one survived.
#[cfg(test)]
impl Selection {
    pub(crate) fn count(&self) -> usize {
        self.picked.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::Build;
    use crate::model::Model;
    use crate::timeline::Timeline;
    use glam::DVec2;
    use silverpoint::Sketch;

    /// A click replaces what is selected and a shift-click adds to it, and
    /// neither ever holds the same entity twice.
    ///
    /// The repeat is the sharp half: shift-clicking something already picked
    /// out has to leave the selection where it was, because that is what makes
    /// the intent safe to apply again on a replayed pass.
    #[test]
    fn a_click_replaces_what_is_selected_and_a_shift_click_adds_to_it() {
        let mut sketch = Sketch::default();
        let start = sketch.add_point(DVec2::ZERO);
        let end = sketch.add_point(DVec2::new(1.0, 0.0));
        let segment = sketch.add_segment(start, end);
        let mut build = Build::default();
        let mut timeline = Timeline::of(sketch);
        let at = timeline.only_sketch();
        timeline.edit(at).opened(&mut build);
        let model = Model::new(timeline.drawing(at), &build, at);
        let (a, b) = (model.part(start), model.part(end));
        let c = model.part(segment);

        let mut selection = Selection::default();
        assert!(!selection.contains(a));

        selection.select(Some(a));
        assert!(selection.contains(a));
        assert!(!selection.contains(b));

        selection.include(b);
        selection.include(c);
        assert!(selection.contains(a) && selection.contains(b) && selection.contains(c));

        // Already there, so nothing moves — and the count says so, which
        // `contains` alone could not.
        selection.include(b);
        assert_eq!(selection.picked, [a, b, c]);

        // A plain click starts over, whatever had accumulated.
        selection.select(Some(c));
        assert_eq!(selection.picked, [c]);

        // And a click that landed on nothing selects nothing, by the same rule.
        selection.select(None);
        assert!(selection.picked.is_empty());
    }
}
