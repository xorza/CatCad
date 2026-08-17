//! A solid the user is still deciding the depth of.

use silverpoint::Prism;

use crate::lens::Lens;
use crate::model::Models;
use crate::paint::cut::Cut;
use crate::paint::gizmos::Carried;
use crate::timeline::FeatureId;

/// A solid being decided: a region, and how deep it currently reads.
///
/// What a form asking for a depth hands the drawing, so the solid is on screen
/// from the moment it is asked for. A [`Prism`] is a reading rather than a
/// thing the document holds — an arrangement, a region, a plane and a distance
/// — so all of this is drawable without a step existing, and cancelling the
/// form leaves the timeline never having heard of it.
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

    /// What it currently reads as, or `None` where the sketch no longer holds
    /// the region it names.
    pub(super) fn prism(self, models: Models<'_>) -> Option<Prism<'_>> {
        let model = models.at(self.sketch)?;
        let arrangement = model.arrangement();
        (self.region < arrangement.faces().len())
            .then(|| Prism::new(arrangement, self.region, model.plane(), self.distance))
    }
}
