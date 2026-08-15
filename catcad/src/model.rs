//! The drawing as it currently stands: what is written down, and what the last
//! solve made of it.

use silverpoint::{Arrangement, Entity, Outcome, Plane, Sketch};

use crate::build::settled::Settled;
use crate::build::{Build, Revision};
use crate::drawing::Drawing;
use crate::part::Part;
use crate::timeline::FeatureId;

/// A sketch and what the last solve made of it, read together.
///
/// Nothing new — every field is something the application already owns. What it
/// is for is that they are never apart: what a drawing *says* and what the last
/// solve *made* of that are two readings of one moment, and a caller handed one
/// without the other could answer out of a mix of two frames. So everything
/// that reads the model reads it through here, and they travel as one argument
/// rather than as three.
///
/// Which is also why the build is taken whole and read here rather than picked
/// apart by the caller: a settling and a revision that came from two different
/// builds would be the very mix this exists to refuse.
///
/// The drawing rather than the whole document, deliberately. What paints a
/// drawing has no business with the camera looking at it or the solids standing
/// beside it — those belong to whoever is laying out a *scene*, and are asked of
/// the document directly by the two calls that want them.
///
/// One sketch rather than all of them, likewise. A document that holds several
/// hands out one of these apiece, and what draws them draws each in turn — so
/// nothing below has to say *which* sketch it means.
///
/// Borrowed and [`Copy`], so passing one down a stack costs what passing a
/// reference costs. A caller that wants to *write* takes the halves separately,
/// because writing them is exactly what has to happen in an order — see
/// [`Build`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Model<'a> {
    /// Which sketch of the timeline this is, which is half of what names
    /// anything picked out of it — see [`Part`].
    of: FeatureId,
    drawing: Drawing<'a>,
    settled: &'a Settled,
    /// The document's, not this sketch's: what compares it is a picture of the
    /// whole of it — see [`Build::revision`](crate::build::Build::revision).
    revision: Revision,
}

impl<'a> Model<'a> {
    /// The drawing at `at`, as `build` last left it.
    pub(crate) fn new(drawing: Drawing<'a>, build: &'a Build, at: FeatureId) -> Self {
        Self {
            of: at,
            drawing,
            settled: build.settled(at),
            revision: build.revision(),
        }
    }

    /// The sketch and the plane it lies on.
    pub(crate) fn drawing(self) -> Drawing<'a> {
        self.drawing
    }

    /// The geometry and the constraints over it.
    pub(crate) fn sketch(self) -> &'a Sketch {
        self.drawing.sketch()
    }

    /// Where the drawing lies in the world.
    pub(crate) fn plane(self) -> Plane {
        self.drawing.plane()
    }

    /// How the last run went, and what the constraints have decided.
    pub(crate) fn outcome(self) -> &'a Outcome {
        self.settled.outcome()
    }

    /// What the drawing's curves shut in.
    pub(crate) fn arrangement(self) -> &'a Arrangement {
        self.settled.arrangement()
    }

    /// Which version of the drawing this is.
    pub(crate) fn revision(self) -> Revision {
        self.revision
    }

    /// One of this sketch's entities, as something that can be picked out.
    ///
    /// Here rather than on [`Part`] itself, because the sketch half of the name
    /// is the one thing an entity handle cannot supply: a caller holding both
    /// is holding a model, and one that is not has no business minting a name.
    pub(crate) fn part(self, entity: impl Into<Entity>) -> Part {
        Part::Entity {
            sketch: self.of,
            entity: entity.into(),
        }
    }

    /// The region at `at` in what this sketch's curves enclose, likewise.
    pub(crate) fn face(self, at: usize) -> Part {
        Part::Face {
            sketch: self.of,
            at,
        }
    }

    /// Whether `part` is still there to be picked out.
    ///
    /// The two halves of what a part can be, answered by the two halves of the
    /// model: an entity by the drawing that holds it, and a face by there still
    /// being that many. Here rather than on either half, because neither can
    /// answer the whole question — which is the same reason they are borrowed
    /// together at all.
    ///
    /// **For this sketch only.** A part of another one is not this model's to
    /// answer for and comes back `false`, so a caller with several models asks
    /// each of them and takes any yes.
    pub(crate) fn holds(self, part: Part) -> bool {
        match part {
            Part::Entity { sketch, entity } => sketch == self.of && self.drawing.holds(entity),
            Part::Face { sketch, at } => sketch == self.of && at < self.arrangement().faces().len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Timeline;
    use crate::timeline::feature::{Datum, Feature};
    use glam::DVec2;
    use silverpoint::Sketch;

    /// Two sketches mint the same handles, and a part is what tells them apart.
    ///
    /// The arenas are per sketch, so the first point of one and the first point
    /// of the other are the *same* [`Entity`] — identical bits, different
    /// geometry, and nothing in the handle that could say which arena it came
    /// from. [`Id`](silverpoint::Id)'s own doc says as much. So a name carrying
    /// only the entity would make the two one thing, and a click on either
    /// would light both.
    #[test]
    fn two_sketches_mint_the_same_handles_and_are_told_apart_by_the_part() {
        let mut here = Sketch::default();
        let one = here.add_point(DVec2::ZERO);
        let mut there = Sketch::default();
        let other = there.add_point(DVec2::new(9.0, 9.0));
        assert_eq!(one, other, "two fresh arenas stopped agreeing on a handle");

        let mut timeline = Timeline::default();
        let ground = timeline.add(Feature::Plane(Datum::Ground));
        let first = timeline.add(Feature::Sketch {
            on: ground,
            sketch: here,
        });
        let second = timeline.add(Feature::Sketch {
            on: ground,
            sketch: there,
        });

        let mut build = Build::default();
        timeline.edit(first).opened(&mut build);
        timeline.edit(second).opened(&mut build);
        let a = Model::new(timeline.drawing(first), &build, first);
        let b = Model::new(timeline.drawing(second), &build, second);

        // The same entity, named twice, comes out as two different parts.
        assert_ne!(a.part(one), b.part(other), "one name for two points");

        // And each model answers for its own and refuses the other's, which is
        // what stops a prune over one sketch from dropping another's selection.
        assert!(a.holds(a.part(one)) && b.holds(b.part(other)));
        assert!(
            !a.holds(b.part(other)),
            "a model answered for another sketch"
        );
        assert!(!b.holds(a.part(one)));
    }
}
