//! The drawing as it currently stands: what is written down, and what the last
//! solve made of it.

use silverpoint::{Arrangement, Outcome, Plane, Sketch};

use crate::drawing::Drawing;
use crate::part::Part;
use crate::workshop::{Revision, Workshop};

/// The two halves of the model, borrowed together.
///
/// Nothing new — both fields are borrows of something the application already
/// owns. What it is for is that the two are never apart: what a drawing *says*
/// and what the last solve *made* of that are two readings of one moment, and a
/// caller handed one without the other could answer out of a mix of two frames.
/// So everything that reads the model reads it through here, and the pair
/// travel as one argument rather than as two.
///
/// The drawing rather than the whole document, deliberately. What paints a
/// drawing has no business with the camera looking at it or the solids standing
/// beside it — those belong to whoever is laying out a *scene*, and are asked of
/// the document directly by the two calls that want them.
///
/// Borrowed and [`Copy`], so passing one down a stack costs what passing a
/// reference costs. A caller that wants to *write* takes the halves separately,
/// because writing them is exactly what has to happen in an order — see
/// [`Workshop`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Model<'a> {
    drawing: Drawing<'a>,
    workshop: &'a Workshop,
}

impl<'a> Model<'a> {
    /// The drawing as `workshop` last left it.
    pub(crate) fn new(drawing: Drawing<'a>, workshop: &'a Workshop) -> Self {
        Self { drawing, workshop }
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
        self.workshop.outcome()
    }

    /// What the drawing's curves shut in.
    pub(crate) fn arrangement(self) -> &'a Arrangement {
        self.workshop.arrangement()
    }

    /// Which version of the drawing this is.
    pub(crate) fn revision(self) -> Revision {
        self.workshop.revision()
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
