//! Every rim of the drawing, as the renderer holds one.

use aperture::{Batch, Ring, Styled};
use silverpoint::{Circle, CircleId};

use crate::look::Theme;
use crate::model::Model;
use crate::model::models::Models;
use crate::paint::names::Names;
use crate::paint::{shade, standing};

use crate::paint::write::Band;

/// The sketch's circles, one ring apiece.
///
/// Not tessellated into strokes: the count that looks round depends on how
/// large the circle lands on screen, and the renderer resolves a ring in
/// the fragment stage instead, which is round at every zoom and needs no
/// rebuilding when the camera moves.
///
/// No plane named, unlike the strokes — a ring's band is widened in its
/// own plane, so the depth it carries is already the surface's.
pub(crate) fn write(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    band: Option<Band>,
    into: &mut Batch<Ring>,
) {
    let geometry = &theme.geometry;
    into.refill(
        models
            .iter()
            .flat_map(|model| {
                model
                    .sketch()
                    .circles()
                    .map(move |(id, circle)| Rim::Circle(model, id, circle))
            })
            .chain(band.map(Rim::Band)),
        |ring, rim| {
            // Assigned whole, like a marker and unlike a stroke: a rim owns
            // nothing either.
            *ring = match rim {
                Rim::Circle(model, id, circle) => {
                    let (sketch, plane) = (model.sketch(), model.plane());
                    Ring::new(
                        plane.point(sketch.point(circle.center).position).as_vec3(),
                        circle.radius.abs() as f32,
                        plane.normal().as_vec3(),
                    )
                    .colored(shade(
                        theme,
                        model,
                        geometry.freedom(model.outcome().circle(id)),
                    ))
                    .precedence(standing(model))
                    .tagged(names.tag(model.part(id)))
                }
                // Through the cursor rather than out to it: the second click
                // says how big by naming somewhere on the rim. Untagged, like
                // the band among the strokes.
                Rim::Band(band) => Ring::new(
                    band.ends.from,
                    band.ends.from.distance(band.ends.to),
                    band.normal,
                )
                .colored(geometry.ghost),
            }
            .width(geometry.edge);
        },
    );
}

/// One rim to write: a circle the sketch holds, or the band a tool is in the
/// middle of drawing.
#[derive(Debug)]
enum Rim<'a> {
    Circle(Model<'a>, CircleId, Circle),
    Band(Band),
}
