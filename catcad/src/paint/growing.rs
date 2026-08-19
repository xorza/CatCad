//! A solid the user is still deciding the depth of.

use silverpoint::{Body, Builder, Extrusion};

use crate::lens::Lens;
use crate::model::Models;
use crate::paint::cut::Cut;
use crate::paint::gizmos::Carried;
use crate::timeline::FeatureId;

/// A solid being decided: a region, and how deep it currently reads.
///
/// What a form asking for a depth hands the drawing, so the solid is on screen
/// from the moment it is asked for. Everything a body is built from is here —
/// an arrangement, a region, a plane and a distance — so all of this is
/// drawable without a step existing, and cancelling the form leaves the
/// timeline never having heard of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Growing {
    pub(crate) sketch: FeatureId,
    pub(crate) region: usize,
    pub(crate) distance: f64,
}

impl Growing {
    /// Where the arrow that carries this stands, or `None` where the sketch no
    /// longer holds the region it names.
    ///
    /// At the middle of the far cap, so it sits on the face it is carrying and
    /// travels with it as the depth is typed — a handle that stayed at the base
    /// would stop being on the thing it moves the moment it moved anything.
    ///
    /// Where *on* that cap is [`Cut`]'s, and `cut` is handed in already cut for
    /// this region. It was worked out here, which meant putting the region
    /// through the filler and scanning its triangles on every frame the camera
    /// moved — a drawing's cost paid on the camera's clock, for an answer only
    /// the drawing can move. What is left is the part that does move with the
    /// camera: how far the depth carries the arrow, and which way it is laid to
    /// face the viewer.
    pub(super) fn carried(self, models: Models<'_>, cut: &Cut, lens: Lens) -> Option<Carried> {
        let model = models.at(self.sketch)?;
        let plane = model.plane();
        let normal = plane.normal().as_vec3();
        Some(Carried::new(
            cut.inside() + normal * self.distance as f32,
            normal,
            // Square to the view rather than to the sketch: the outline is
            // flat, and one laid out in a plane of the sketch's own collapses
            // to a line the moment the camera comes round to look along it —
            // which for a handle is the moment it stops being grabbable. The
            // fallback is the case where the camera looks straight down the
            // arrow, where there is no widest side to turn and the whole shape
            // is a dot whichever way it is laid out.
            lens.facing()
                .cross(normal)
                .try_normalize()
                .unwrap_or(plane.x.as_vec3()),
        ))
    }

    /// Build what it currently reads as into `into`, and say whether there was
    /// anything to build.
    ///
    /// **Raised by the drawing rather than kept by the document**, unlike every
    /// solid there is a step for: this one belongs to a form that is still
    /// open, and it changes whenever the depth does. What the document keeps is
    /// a [`Bodied`](crate::build::bodied::Bodied) per *step*, and there is no
    /// step yet.
    ///
    /// Into a body the layout holds, for the reason the layout holds every
    /// other buffer: a depth typed a digit at a time rebuilds this on every
    /// frame the form is open, and a body refilled in place reaches the heap
    /// once.
    pub(super) fn body(self, models: Models<'_>, builder: &mut Builder, into: &mut Body) -> bool {
        let standing = models
            .at(self.sketch)
            .filter(|model| self.region < model.arrangement().faces().len());
        let Some(model) = standing else {
            // The sketch has gone, or the region has: either way there is no
            // solid to show and the last one must not be left on screen.
            into.clear();
            return false;
        };
        let arrangement = model.arrangement();
        let extrusion = Extrusion::new(arrangement, self.region, model.plane(), self.distance);
        builder.extrude(&extrusion, into);
        true
    }
}
