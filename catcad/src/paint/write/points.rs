//! Every marker of the drawing, as the renderer holds one.

use aperture::{Batch, Point, Styled};

use crate::look::Theme;
use crate::model::models::Models;
use crate::paint::names::Names;
use crate::paint::{shade, standing};

/// The sketch's points, one marker apiece — larger and pinned-coloured
/// where the solver may not move it.
///
/// The plane comes along for the same reason a stroke's does: a disc is
/// flat in depth and the surface under it is not, so without it the glyph
/// is sliced wherever the plane is seen at an angle.
pub(crate) fn write(models: Models<'_>, theme: &Theme, names: &mut Names, into: &mut Batch<Point>) {
    let geometry = &theme.geometry;
    into.refill(
        models
            .iter()
            .flat_map(|model| model.sketch().points().map(move |at| (model, at))),
        |marker, (model, (id, point))| {
            let plane = model.plane();
            // Pinned by hand outranks pinned by consequence: a fixed point is
            // determined too, but saying so in the same colour would lose the
            // one thing about it the user chose.
            let (color, size) = if point.fixed {
                (geometry.pinned, geometry.fixed_marker)
            } else {
                (
                    geometry.freedom(model.outcome().point(id)),
                    geometry.free_marker,
                )
            };
            // Assigned whole where a stroke is edited in place: a marker owns
            // nothing, so replacing one costs what overwriting it would.
            *marker = Point::new(plane.point(point.position).as_vec3())
                .colored(shade(theme, model, color))
                .size(size)
                .in_plane(plane.normal().as_vec3())
                .precedence(standing(model))
                .tagged(names.tag(model.part(id)));
        },
    );
}
