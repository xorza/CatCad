//! The document the app opens with when it is given no file, and the scenery it
//! stands around every document.
//!
//! Placeholder content rather than a file: what the app starts with has to come
//! from somewhere, and a fixture whose answer is known is what makes a wrong
//! frame obvious. The visual suite raises this very thing, so it is as much the
//! test fixture as it is the startup content.

use aperture::{Mesh, Object, Styled};
use glam::{DVec2, Mat4, Vec3};
use silverpoint::{Constraint, Sketch};

use crate::build::Build;
use crate::document::Document;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature};

/// How far the demo's second plane is held off the ground.
///
/// Enough to read as a separate plane at the angle the view opens at, and not
/// so much that what is drawn on it leaves the frame.
pub(crate) const SHELF: f64 = 2.2;

/// The solids the drawing is modelled alongside.
///
/// No step made any of these, which is why they are not in the document and are
/// not written down by saving: they stand around whichever document is open.
/// They are a stand-in, and this goes when an extrude exists to *make* a solid —
/// until then the drawing needs ground to lie on, and scenery nobody authored is
/// the honest name for that.
///
/// The same world as the drawing — the slab's top face is the ground plane and
/// the boxes stand on it — so orbiting the view moves both together.
pub(crate) fn scenery() -> Vec<Object> {
    let plane = silverpoint::Plane::GROUND;
    let mut solids = Vec::new();
    // The ground the drawing lies on, and the reason the drawing carries a
    // depth bias at all: the slab's top face *is* the sketch plane, so the
    // two are exactly coplanar and something has to decide which reads.
    solids.push(Object {
        // A unit cube spans ±0.5, so dropping the slab by half its
        // thickness after scaling lands its top face on y = 0.
        transform: Mat4::from_translation(Vec3::new(4.0, -0.5, -2.5))
            * Mat4::from_scale(Vec3::new(12.0, 1.0, 9.0)),
        color: Vec3::new(0.30, 0.30, 0.34),
        ..Object::new(Mesh::cube(1.0))
    });
    for (size, at, color) in [
        (2.0, DVec2::new(2.0, 3.6), Vec3::new(0.55, 0.58, 0.62)),
        (0.8, DVec2::new(6.2, 1.1), Vec3::new(0.85, 0.35, 0.20)),
        (1.2, DVec2::new(6.6, 3.9), Vec3::new(0.25, 0.45, 0.75)),
    ] {
        // Half a cube up puts it on the plane rather than through it.
        let base = plane.point(at).as_vec3() + Vec3::Y * size * 0.5;
        solids.push(Object::new(Mesh::cube(size)).at(base).colored(color));
    }
    solids
}

/// Which region of the first sketch the demo's extrude is grown from.
///
/// The hub — the disc the circle in the middle of the frame shuts in — walked
/// third, after the frame around it and the eye on the arm.
///
/// The disc rather than the frame, and the reason is the whole point of naming a
/// region by what bounds it. The frame is bounded by the four edges of a
/// rectangle the arm swings right past, and the arm is *made* to be dragged
/// about: push it up and two of its bars and the eye cross the rectangle's base,
/// cutting the frame into six regions and leaving the name fitting none of them.
/// Which is the correct answer, and a poor thing to open the application on. The
/// disc is bounded by one circle nothing else reaches, and the drag the demo
/// offers on it is the rim — so pulling the rim about resizes the solid instead
/// of taking it away, which is what a parametric model is for.
///
/// A position, and pinned by
/// `the_demo_grows_its_solid_from_the_hub_and_keeps_it_through_a_drag` — because
/// a position among the faces is exactly the thing a [`Profile`] exists not to
/// trust, and this is the one moment it is allowed: the moment the name is
/// minted.
///
/// [`Profile`]: crate::profile::Profile
const HUB: usize = 2;

/// How far the demo's extrude stands off the ground.
///
/// Under [`SHELF`], so the solid does not reach the plane held above it and the
/// two read as separate things rather than as one stack.
const DEPTH: f64 = 1.4;

/// The demo as a document: two sketches on two planes, and a solid grown off
/// one of them.
pub(crate) fn document(build: &mut Build) -> Document {
    // Five steps: the ground, a drawing on it, a plane held clear of the
    // ground, a drawing on that, and a solid grown off the first. Neither
    // sketch carries a plane — each names one — so the second follows its datum
    // wherever that goes, and moving the datum is retyping one number.
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: sketch(),
    });
    let shelf = timeline.add(Feature::Plane(Datum::Offset {
        from: ground,
        by: SHELF,
    }));
    timeline.add(Feature::Sketch {
        on: shelf,
        sketch: aside(),
    });
    // Raised before the solid is grown off it, and that order is the whole of
    // why this is two steps rather than one. Naming a region means asking what
    // the drawing encloses, and what it encloses is what the *solve* decides —
    // where the coordinates in `sketch` are deliberately not the answer. So the
    // document is stood up and solved, and only then is there a region to name.
    let mut document = Document::new(build, timeline);
    let profile = document.models(build, drawn).open().profile(HUB);
    document.extrude(build, profile, DEPTH);
    document
}

/// A second drawing, on the shelf held clear of the ground the first lies on.
///
/// Small on purpose. What it is *for* is being another sketch: something to
/// click into, and something drawn as ground while you are working in its
/// neighbour. What it draws is a triangle, which is the least geometry that
/// still shuts a face in — so switching between the two shows the difference on
/// every kind of mark there is, strokes and markers and a filled face.
///
/// Determined but for its apex: the left corner is pinned, the base is
/// horizontal and 2.5 long, which leaves the apex free to be dragged about
/// while the base holds.
fn aside() -> Sketch {
    let mut sketch = Sketch::default();
    let left = sketch.add_point(DVec2::new(5.0, -2.2));
    let right = sketch.add_point(DVec2::new(7.4, -1.9));
    let apex = sketch.add_point(DVec2::new(6.2, -0.8));
    sketch.fix(left);
    sketch.add_segment(left, right);
    sketch.add_segment(right, apex);
    sketch.add_segment(apex, left);
    sketch.add_constraint(Constraint::Horizontal { a: left, b: right });
    sketch.add_constraint(Constraint::Distance {
        a: left,
        b: right,
        distance: 2.5,
    });
    sketch
}

/// A rigid frame, a hole through it that can be resized, and a jointed arm
/// below that can be taken hold of anywhere.
///
/// Every position below is a guess deliberately off the answer: what puts
/// the geometry where it belongs is the solve, not the coordinates. What
/// each part is *for* is the freedom it leaves behind, because that is what
/// a drag has to work with — between them they cover every way the drawing
/// can answer a cursor:
///
/// - The rectangle is fully determined, so none of it moves. Its anchored
///   corner refuses a drag for being pinned, and the other three refuse for
///   having nowhere legal to go.
/// - The circle is pinned to the rectangle's centre but carries no radius,
///   so its rim is the one dimension here the cursor can drive.
/// - The arm keeps both bar lengths and the right angle between them, so
///   however it is grabbed it travels as one rigid body — and the eye at
///   its end keeps a stated size while being carried around.
/// - The rail keeps the direction of the rectangle's base and nothing else,
///   so it stretches along it and rides up and down with the arm.
pub(crate) fn sketch() -> Sketch {
    const WIDTH: f64 = 8.0;
    const HEIGHT: f64 = 5.0;

    let mut sketch = Sketch::default();
    let corner = [
        sketch.add_point(DVec2::ZERO),
        sketch.add_point(DVec2::new(7.4, 0.6)),
        sketch.add_point(DVec2::new(8.6, 4.2)),
        sketch.add_point(DVec2::new(-0.5, 5.3)),
    ];
    // Without an anchor the rectangle is still free to slide and turn, and
    // the report would say so: three degrees of freedom left over.
    sketch.fix(corner[0]);
    let base = sketch.add_segment(corner[0], corner[1]);
    for pair in [[1, 2], [2, 3], [3, 0]] {
        sketch.add_segment(corner[pair[0]], corner[pair[1]]);
    }
    sketch.add_constraint(Constraint::Horizontal {
        a: corner[0],
        b: corner[1],
    });
    sketch.add_constraint(Constraint::Vertical {
        a: corner[1],
        b: corner[2],
    });
    sketch.add_constraint(Constraint::Horizontal {
        a: corner[2],
        b: corner[3],
    });
    sketch.add_constraint(Constraint::Vertical {
        a: corner[3],
        b: corner[0],
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[0],
        b: corner[1],
        distance: WIDTH,
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[1],
        b: corner[2],
        distance: HEIGHT,
    });

    let hub = sketch.add_point(DVec2::new(3.6, 2.1));
    // No `Radius`, deliberately: the centre is nailed down by the two
    // distances below and the radius is left to whatever it was made with,
    // so dragging the rim drives it and nothing pulls back.
    sketch.add_circle(hub, 1.5);
    // Both bottom corners sit half a diagonal from the centre. That leaves
    // a mirrored solution below the edge, which the guess above declines.
    let to_centre = (WIDTH * WIDTH + HEIGHT * HEIGHT).sqrt() * 0.5;
    sketch.add_constraint(Constraint::Distance {
        a: corner[0],
        b: hub,
        distance: to_centre,
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[1],
        b: hub,
        distance: to_centre,
    });

    // A rail and a two-bar arm, in the band of slab between the rectangle
    // and the near edge. They share the shoulder rather than being welded
    // there by a coincidence: one point is one marker, where a coincidence
    // would draw two on top of each other and leave the cursor to guess
    // between them.
    let rail_end = sketch.add_point(DVec2::new(0.6, -1.2));
    let shoulder = sketch.add_point(DVec2::new(3.4, -1.05));
    let elbow = sketch.add_point(DVec2::new(5.2, -0.3));
    let wrist = sketch.add_point(DVec2::new(5.85, -1.4));
    let rail = sketch.add_segment(rail_end, shoulder);
    let upper = sketch.add_segment(shoulder, elbow);
    let fore = sketch.add_segment(elbow, wrist);
    sketch.add_constraint(Constraint::Parallel {
        first: base,
        second: rail,
    });
    sketch.add_constraint(Constraint::Distance {
        a: shoulder,
        b: elbow,
        distance: 2.0,
    });
    sketch.add_constraint(Constraint::Distance {
        a: elbow,
        b: wrist,
        distance: 1.4,
    });
    sketch.add_constraint(Constraint::Perpendicular {
        first: upper,
        second: fore,
    });

    let eye = sketch.add_circle(wrist, 0.45);
    sketch.add_constraint(Constraint::Radius {
        circle: eye,
        radius: 0.45,
    });
    sketch
}

/// What the demo claims about itself, which is a claim about a *position* and so
/// has to be checked rather than trusted.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::Grip;
    use crate::intent::Change;
    use silverpoint::Plane;
    use std::f64::consts::PI;

    /// **The demo grows its solid from the hub, and keeps it through the drag
    /// the hub is there for.**
    ///
    /// Two claims, and [`HUB`] is why the first one is needed at all: a position
    /// among the faces is what a [`Profile`](crate::profile::Profile) exists not
    /// to trust, and minting the name is the one moment one is used. Anything
    /// that changed the order the drawing's curves are walked in would leave the
    /// demo silently extruding the frame, or the eye on the end of the arm.
    ///
    /// The second is the point of choosing the hub. Its rim is the one dimension
    /// in the demo a cursor can drive, and driving it has to *resize* the solid
    /// rather than take it away — the circle is still the same circle, and the
    /// region is still the inside of it.
    #[test]
    fn the_demo_grows_its_solid_from_the_hub_and_keeps_it_through_a_drag() {
        let mut build = Build::default();
        let mut document = document(&mut build);
        let drawn = document.opening();

        // The hub is drawn at radius 1.5, and its centre is pinned to the middle
        // of the frame by two half-diagonals — so the disc it shuts in covers
        // π·1.5², and neither the frame around it nor the eye on the arm is
        // anywhere near that.
        let covered = |document: &Document, build: &Build| {
            document.models(build, drawn).open().arrangement().faces()[HUB].area()
        };
        assert!(
            (covered(&document, &build) - PI * 1.5 * 1.5).abs() < 1e-9,
            "face {HUB} covers {} rather than the hub's disc",
            covered(&document, &build)
        );
        assert_eq!(document.models(&build, drawn).lost(), 0);

        // The rim pulled out to radius 2, which is still clear of the frame's
        // top and bottom edges two and a half away — so the drawing keeps its
        // shape and only the disc grows.
        let hub = document
            .models(&build, drawn)
            .open()
            .sketch()
            .circles()
            .next()
            .expect("the frame is drawn with a circle through it")
            .0;
        document.apply(
            &mut build,
            Change::Drag {
                sketch: drawn,
                grip: Grip::Rim(hub),
                to: Plane::GROUND.point(DVec2::new(6.0, 2.5)).as_vec3(),
            },
        );
        assert_eq!(
            document.models(&build, drawn).lost(),
            0,
            "growing the circle lost the region inside it"
        );
        // A solve's answer rather than arithmetic, so within a tolerance — and
        // the tolerance is a drag's, which reaches for the cursor through the
        // constraints rather than writing the radius.
        let grown = covered(&document, &build);
        assert!(
            (grown - PI * 2.0 * 2.0).abs() < 1e-6,
            "the disc covers {grown} rather than π·2²"
        );
    }
}
