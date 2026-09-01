//! Every stroke of the drawing, as the renderer holds one.

use aperture::{Batch, Curve, Precedence};
use silverpoint::{Segment, SegmentId};

use crate::look::Theme;
use crate::model::Model;
use crate::model::models::Models;
use crate::paint::names::Names;
use crate::paint::{shade, standing};

use crate::paint::write::Band;

/// The sketch's straight strokes, one edge per segment, biased clear of
/// the solids in depth so the drawing reads over them. Circles are not
/// strokes — see [`rings`].
pub(crate) fn write(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    band: Option<Band>,
    into: &mut Batch<Curve>,
) {
    let geometry = &theme.geometry;
    // Written over the strokes already there rather than into fresh ones, which
    // for a `Curve` is the difference between a frame that reaches the heap and
    // one that does not — see `Batch::refill`. That is also why all three kinds
    // are chained into one refill rather than written in three passes: a stroke
    // appended outside it would be dropped by the next rewrite of the drawing
    // and allocated afresh by the one after, once a frame for as long as a line
    // is being drawn.
    //
    // The drawing, then what is being drawn now. What it is drawn *on* is no
    // part of this batch: a plane's square holds its size on screen, so it is
    // cut against the camera with the other handles — see
    // [`gizmos::write`](crate::paint::gizmos::write).
    into.refill(
        models
            .iter()
            .flat_map(|model| {
                model
                    .sketch()
                    .segments()
                    .map(move |(id, edge)| Stroke::Edge(model, id, edge))
            })
            .chain(band.map(Stroke::Band)),
        |curve, stroke| {
            curve.width = geometry.edge;
            match stroke {
                Stroke::Edge(model, id, edge) => {
                    let (sketch, plane) = (model.sketch(), model.plane());
                    let a = plane.point(sketch.point(edge.a).position).as_vec3();
                    let b = plane.point(sketch.point(edge.b).position).as_vec3();
                    curve.set_segment(a, b);
                    curve.color =
                        shade(theme, model, geometry.freedom(model.outcome().segment(id)));
                    curve.precedence = standing(model);
                    curve.plane_normal = Some(plane.normal().as_vec3());
                    curve.tag = Some(names.tag(model.part(id)));
                }
                // Untagged, which is what keeps the band out of the way: a pick
                // skips a primitive with no tag, so it cannot be hovered,
                // grabbed or picked out, and the click that finishes the line
                // resolves against the geometry behind it.
                Stroke::Band(band) => {
                    curve.set_segment(band.ends.from, band.ends.to);
                    curve.color = geometry.ghost;
                    curve.precedence = Precedence::Shaped;
                    curve.plane_normal = Some(band.normal);
                    curve.tag = None;
                }
            }
        },
    );
}

/// One stroke to write: an edge the sketch holds, or the band a tool is in the
/// middle of drawing.
#[derive(Debug)]
enum Stroke<'a> {
    Edge(Model<'a>, SegmentId, Segment),
    Band(Band),
}
