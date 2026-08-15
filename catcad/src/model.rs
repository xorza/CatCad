//! The drawing as it currently stands: what is written down, and what the last
//! solve made of it.

use silverpoint::{Arrangement, Outcome, Plane, Sketch};

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

    /// Whether `part` is still there to be picked out.
    ///
    /// The two halves of what a part can be, answered by the two halves of the
    /// model: an entity by the drawing that holds it, and a face by there still
    /// being that many. Here rather than on either half, because neither can
    /// answer the whole question — which is the same reason the two are
    /// borrowed together at all.
    pub(crate) fn holds(self, part: Part) -> bool {
        match part {
            Part::Entity(entity) => self.drawing.holds(entity),
            Part::Face(at) => at < self.arrangement().faces().len(),
        }
    }
}
