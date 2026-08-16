//! A solid the user is still deciding the depth of.

use aperture::Camera;
use glam::DVec2;
use silverpoint::Prism;

use crate::model::Models;
use crate::paint::FACE_SAGITTA;
use crate::paint::gizmos::Carried;
use crate::paint::layout::Sheets;
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
    /// **Inside the region rather than at the average of its corners**, which
    /// for a region with a hole in it is a point in the hole: the demo's frame
    /// is a rectangle with the hub cut out, and its corners average to the
    /// middle of the cut. The widest triangle the fill was cut into is inside
    /// the region by construction, and being the widest is what keeps the
    /// answer off a sliver at an edge.
    pub(super) fn carried(
        self,
        models: Models<'_>,
        sheets: &mut Sheets,
        camera: &Camera,
    ) -> Option<Carried> {
        let model = models.at(self.sketch)?;
        let plane = model.plane();
        let arrangement = model.arrangement();
        let face = arrangement.faces().get(self.region)?;
        let Sheets { filler, fill, .. } = sheets;
        filler.fill(arrangement, face, FACE_SAGITTA, fill);
        let widest = fill.triangles.iter().max_by(|&&a, &&b| {
            let area = |[x, y, z]: [u32; 3]| {
                let corner = |at: u32| fill.corners[at as usize];
                (corner(y) - corner(x))
                    .perp_dot(corner(z) - corner(x))
                    .abs()
            };
            area(a).total_cmp(&area(b))
        })?;
        let middle: DVec2 = widest
            .iter()
            .map(|&at| fill.corners[at as usize])
            .sum::<DVec2>()
            / 3.0;
        let normal = plane.normal().as_vec3();
        Some(Carried::new(
            plane.point(middle).as_vec3() + normal * self.distance as f32,
            normal,
            // Square to the view rather than to the sketch: the outline is
            // flat, and one laid out in a plane of the sketch's own collapses
            // to a line the moment the camera comes round to look along it —
            // which for a handle is the moment it stops being grabbable. The
            // fallback is the case where the camera looks straight down the
            // arrow, where there is no widest side to turn and the whole shape
            // is a dot whichever way it is laid out.
            camera
                .facing()
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
