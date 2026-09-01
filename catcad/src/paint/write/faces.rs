//! Every region of the drawing, as the renderer holds one.

use aperture::{Batch, Object, Vertex};
use glam::Mat4;

use crate::look::Theme;
use crate::model::models::Models;
use crate::paint::layout::Sheets;
use crate::paint::names::Names;
use crate::paint::{FACE_SAGITTA, standing};

use crate::paint::write::remesh;

/// A sheet per region the drawing's curves shut in.
///
/// The one part of a drawing that is not drawn: a region is what the curves
/// *enclose*, so nothing here reads a segment or a circle — it reads what
/// [`Arrangement`](silverpoint::Arrangement) made of all of them together, and
/// a half-circle cut by an edge is as much a region as a rectangle traced by
/// four.
///
/// Meshes rather than overlays, because a region has area in the world where a
/// stroke has width on the screen. They go to the scene's own batch for them,
/// which is drawn two-sided and biased forward off the plane they lie in — see
/// [`Scene::faces`](aperture::Scene).
///
/// Named like everything else, so a region can be hovered and picked out. A
/// cursor over one still reaches the geometry bounding it first: a surface is
/// the least specific thing a pick can land on — see
/// [`HitAt`](aperture::HitAt) — and every stroke and marker that draws a region
/// lies within it.
///
/// Named *by position*, which is the one thing about a region that is not a
/// handle. See [`Part::Region`](crate::part::Part).
pub(crate) fn write(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    sheets: &mut Sheets,
    into: &mut Batch<Object>,
) {
    let Sheets { filler, fill, .. } = sheets;
    into.refill(
        models
            .iter()
            .flat_map(|model| (0..model.arrangement().faces().len()).map(move |at| (model, at))),
        |object, (model, at)| {
            let plane = model.plane();
            let normal = plane.normal().as_vec3();
            let arrangement = model.arrangement();
            let face = &arrangement.faces()[at];
            filler.fill(arrangement, face, FACE_SAGITTA, fill);
            remesh(
                &mut object.mesh,
                fill.corners.iter().map(|&corner| Vertex {
                    position: plane.point(corner).as_vec3(),
                    normal,
                }),
                &fill.triangles,
            );
            object.transform = Mat4::IDENTITY;
            object.color = if model.live() {
                theme.geometry.face
            } else {
                theme.geometry.dormant_face
            };
            object.precedence = standing(model);
            object.tag = Some(names.tag(model.region(at)));
        },
    );
}
