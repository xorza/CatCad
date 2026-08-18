//! One region of the drawing, cut once and read on the camera's schedule.

use glam::{DVec2, Vec3};
use silverpoint::Fill;

use crate::build::Revision;
use crate::model::Models;
use crate::paint::FACE_SAGITTA;
use crate::paint::layout::Sheets;
use crate::timeline::FeatureId;

/// Where one region lies: the corners its fill was cut from, and a point inside
/// it.
///
/// **Both are the filler's answer over a whole boundary, and both are read by
/// the half of a frame that runs on the camera's clock.** A form standing beside
/// a region is placed against the projection on every frame it is shown, and the
/// arrow carrying a solid's depth is rebuilt whenever the camera moves — so cut
/// where they are read, a region is triangulated afresh on every frame of an
/// orbit for an answer only the *drawing* can move. Kept here, it is cut when
/// the drawing is.
///
/// One region, because one is all anything ever asks about: a form is open over
/// a single region, and the arrow carrying its depth stands on that same one.
/// Which one it is rides along, so a cut describing another is found out rather
/// than answered with.
///
/// Cut through the same [`Filler`](silverpoint::Filler) the sheets are, which is
/// what makes a form stand clear of exactly what is drawn — see
/// [`write::faces`](crate::paint::write::faces).
///
/// **In the world throughout.** Both readings are handed to callers holding a
/// [`Lens`](crate::lens::Lens) or a normal rather than a plane, so a reading kept
/// in the sketch's own coordinates would be one every reader had to put on the
/// plane itself — which is the plane this cut already holds and they would have
/// to fetch again.
#[derive(Debug, Default)]
pub(crate) struct Cut {
    /// What this was cut for, or `None` where the last cut found no such region
    /// — which is also what a layout that has cut nothing says.
    of: Option<Asked>,
    /// Where its fill puts its corners.
    ///
    /// The fill rather than the boundary curves, because that is the shape the
    /// region actually covers: a crescent's bounding box taken off its two arcs'
    /// endpoints would sit half outside it.
    ///
    /// Kept for its room as well as its contents — a region that comes back the
    /// same shape asks the heap for nothing.
    corners: Vec<Vec3>,
    /// A point inside it.
    ///
    /// **Inside the region rather than at the average of its corners**, which
    /// for a region with a hole in it is a point in the hole: the demo's frame
    /// is a rectangle with the hub cut out, and its corners average to the
    /// middle of the cut. See [`widest`].
    inside: Vec3,
}

impl Cut {
    /// Where the region at `at` of `sketch` lies, cutting it unless it is the
    /// one already here — or `None` where the drawing holds no such region.
    ///
    /// Answers with itself rather than with the two readings, because a caller
    /// wants one or the other and neither wants both: the corners are what a
    /// form is placed against and the inside is where a handle stands.
    pub(super) fn region(
        &mut self,
        models: Models<'_>,
        sheets: &mut Sheets,
        sketch: FeatureId,
        at: usize,
    ) -> Option<&Self> {
        let asked = Asked {
            revision: models.revision(),
            sketch,
            at,
        };
        if self.of != Some(asked) {
            self.of = self.fill(models, sheets, asked).then_some(asked);
        }
        self.of.is_some().then_some(&*self)
    }

    /// The corners its fill was cut from.
    pub(crate) fn corners(&self) -> &[Vec3] {
        &self.corners
    }

    /// A point inside it.
    pub(super) fn inside(&self) -> Vec3 {
        self.inside
    }

    /// Cut what `asked` names afresh, answering whether there was anything to
    /// cut.
    ///
    /// A `bool` rather than a `Result` or an `Option` of the answer: what it
    /// found goes into the fields, and the one thing left to say is whether the
    /// drawing still holds what was asked for. Three ways it may not — the
    /// sketch has gone, the region has gone, or what is left of it is too
    /// degenerate to cut into a single triangle — and none of them is a failure
    /// to report. A profile that has lost its footing is a state the document is
    /// allowed to be in, and every caller here treats it as a frame with nothing
    /// to place.
    fn fill(&mut self, models: Models<'_>, sheets: &mut Sheets, asked: Asked) -> bool {
        self.corners.clear();
        let Some(model) = models.at(asked.sketch) else {
            return false;
        };
        let arrangement = model.arrangement();
        let Some(face) = arrangement.faces().get(asked.at) else {
            return false;
        };
        let Sheets { filler, fill, .. } = sheets;
        filler.fill(arrangement, face, FACE_SAGITTA, fill);
        let Some(inside) = widest(fill) else {
            return false;
        };
        let plane = model.plane();
        self.corners
            .extend(fill.corners.iter().map(|&at| plane.point(at).as_vec3()));
        self.inside = plane.point(inside).as_vec3();
        true
    }
}

/// The middle of the widest triangle `fill` was cut into, or `None` where it was
/// cut into none.
///
/// What a handle standing on a region wants, and the whole of why it is a
/// triangle's middle rather than the region's own: a triangle of the fill is
/// inside the region by construction, where an average of the corners is only
/// inside a region that is convex and has nothing punched out of it.
///
/// The *widest* rather than any, and that is what keeps the answer off a sliver
/// at an edge — where a handle would sit under the stroke bounding the region
/// instead of on the region.
fn widest(fill: &Fill) -> Option<DVec2> {
    let corner = |at: u32| fill.corners[at as usize];
    let area = |[x, y, z]: [u32; 3]| {
        (corner(y) - corner(x))
            .perp_dot(corner(z) - corner(x))
            .abs()
    };
    let widest = fill
        .triangles
        .iter()
        .max_by(|&&a, &&b| area(a).total_cmp(&area(b)))?;
    Some(widest.iter().map(|&at| corner(at)).sum::<DVec2>() / 3.0)
}

/// What a [`Cut`] was asked for: which region of which sketch, and which version
/// of the document it was to be cut from.
///
/// Named for the asking rather than for the region, because it is not one: a
/// region is a face of an arrangement — the thing [`Part::Region`] and
/// [`Profile`] each name in their own way — and this is the question a cut
/// answers, kept so that a cut answering another is found out rather than
/// handed over.
///
/// The revision rides along because it is what moves a region's shape: every
/// edit to the document bumps it and nothing else does — see [`Revision`] — so a
/// cut that is still good is one whose revision has not moved. Conservative in
/// the same direction the revision is, which is the right one: a spare cut costs
/// one triangulation, and a stale one puts a form beside a shape that is no
/// longer there.
///
/// [`Part::Region`]: crate::part::Part
/// [`Profile`]: crate::profile::Profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Asked {
    revision: Revision,
    sketch: FeatureId,
    at: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::Build;
    use crate::demo;
    use crate::document::Document;
    use crate::drawing::Grip;
    use crate::intent::change::Change;
    use crate::timeline::Timeline;
    use crate::timeline::feature::{Datum, Feature, World};
    use silverpoint::{Plane, Sketch};

    /// Where the region at 0 of `sketch` lies, as its fill's corners.
    ///
    /// A function rather than a closure, so it borrows the document for the
    /// length of the call rather than for the length of the test — which is
    /// what lets the drawing be edited between two of them.
    fn corners(
        cut: &mut Cut,
        sheets: &mut Sheets,
        document: &Document,
        build: &Build,
        sketch: FeatureId,
    ) -> Vec<Vec3> {
        cut.region(document.models(build, sketch), sheets, sketch, 0)
            .map(|cut| cut.corners().to_vec())
            .expect("the sketch encloses a region")
    }

    /// A cut answers for the region it was asked about, and follows the drawing.
    ///
    /// **What the key is for**, and the whole of what could break in silence.
    /// Keeping a cut is only sound while a stale one is found out: forget the
    /// revision and a form stands beside where its region *was*, forget the
    /// sketch and it stands beside whatever sits at that slot in another
    /// drawing. Neither looks wrong — the form is on screen, in a plausible
    /// place, describing something else.
    ///
    /// The corners are compared rather than measured against arithmetic, because
    /// what they are is the filler's answer and this is not a test of the
    /// filler. What is asserted is which answer came back.
    #[test]
    fn a_cut_answers_for_the_region_asked_and_follows_the_drawing() {
        // Two sketches on two planes, so a cut of the same region *number* in
        // each is a different shape in a different place — which is what says
        // the sketch is part of the key.
        let mut timeline = Timeline::default();
        let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
        let shelf = timeline.add(Feature::Plane(Datum::Offset {
            from: ground,
            by: 3.0,
        }));
        let mut boxed = |on, side: f64| {
            let mut sketch = Sketch::default();
            let corner = [
                sketch.add_point(DVec2::ZERO),
                sketch.add_point(DVec2::new(side, 0.0)),
                sketch.add_point(DVec2::new(side, side)),
                sketch.add_point(DVec2::new(0.0, side)),
            ];
            for pair in [[0, 1], [1, 2], [2, 3], [3, 0]] {
                sketch.add_segment(corner[pair[0]], corner[pair[1]]);
            }
            timeline.add(Feature::Sketch { on, sketch })
        };
        let here = boxed(ground, 2.0);
        let there = boxed(shelf, 4.0);

        let mut build = Build::default();
        let mut document = Document::new(&mut build, timeline);
        let mut sheets = Sheets::default();
        let mut cut = Cut::default();

        let square = corners(&mut cut, &mut sheets, &document, &build, here);
        // A two-by-two square, cut from its four corners. Where *inside* it a
        // handle then stands is asked below, on the fixture that can catch a
        // wrong answer.
        assert_eq!(square.len(), 4, "a square was cut from {square:?}");
        // Asked again, the same answer — which is the whole point of keeping it.
        assert_eq!(
            corners(&mut cut, &mut sheets, &document, &build, here),
            square
        );

        // The other sketch's region 0 is the same *number* and another shape on
        // another plane, so a key that held only the position would answer with
        // the square above.
        let other = corners(&mut cut, &mut sheets, &document, &build, there);
        assert_ne!(other, square, "a cut answered for the wrong sketch");
        assert!(
            other.iter().all(|at| at.y > 2.9),
            "the other sketch's region was not cut on its own plane: {other:?}"
        );
        // And back again, which says the second cut replaced the first rather
        // than the two being confused for one another.
        assert_eq!(
            corners(&mut cut, &mut sheets, &document, &build, here),
            square
        );

        // Now move the drawing under it. The square's far corner is dragged out,
        // so the region it encloses is a different shape at the same position in
        // the same sketch — everything a cut is keyed on but the revision.
        let far = document
            .drawing_at(here)
            .sketch()
            .points()
            .nth(2)
            .expect("the square has four corners")
            .0;
        document.apply(
            &mut build,
            Change::Drag {
                sketch: here,
                grip: Grip::Point(far),
                to: Plane::GROUND.point(DVec2::new(5.0, 5.0)).as_vec3(),
            },
        );
        let stretched = corners(&mut cut, &mut sheets, &document, &build, here);
        assert_ne!(
            stretched, square,
            "a cut went on describing a region the drawing had moved"
        );
    }

    /// A region with a hole in it puts its inside point in the region and not in
    /// the hole.
    ///
    /// The failure the widest triangle is chosen against, and the demo is the
    /// fixture that has it: its frame is a rectangle with the hub cut out of the
    /// middle, so the average of its corners lands squarely in the hole — which
    /// for the arrow carrying a depth means a handle standing on nothing.
    #[test]
    fn a_punched_region_puts_its_inside_point_in_the_region() {
        let mut build = Build::default();
        let document = demo::document(&mut build);
        let drawn = document.opening();
        let models = document.models(&build, drawn);
        // The frame around the hub is the widest thing the demo encloses, so it
        // is the region with the largest area — and the one with the hole.
        let framed = (0..models.open().arrangement().faces().len())
            .max_by(|&a, &b| {
                let area = |at: usize| models.open().arrangement().faces()[at].area();
                area(a).total_cmp(&area(b))
            })
            .expect("the demo encloses something");

        let mut sheets = Sheets::default();
        let mut cut = Cut::default();
        let inside = cut
            .region(models, &mut sheets, drawn, framed)
            .expect("the demo holds the region it draws")
            .inside();

        // The hub is a circle about the middle of the frame, and the average of
        // the frame's own corners is that same middle — so a point taken that
        // way is a radius deep inside the hole. Measured against the circle the
        // demo draws rather than a number written here, and on the plane the cut
        // answers in, where a rigid map leaves the distance what it was.
        let hub = models.open().sketch().circles().next().expect("the hub").1;
        let plane = models.open().plane();
        let middle = plane
            .point(models.open().sketch().point(hub.center).position)
            .as_vec3();
        assert!(
            f64::from(inside.distance(middle)) > hub.radius,
            "the frame's inside point sits {} from the hub's centre, inside a \
             hole of radius {}",
            inside.distance(middle),
            hub.radius
        );
    }
}
