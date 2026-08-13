//! The drawing the app opens with.

use aperture::{Mesh, Object, Scene, Styled};
use glam::{DVec2, Mat4, Vec3};
use silverpoint::{Constraint, Sketch};

use crate::drawing::Drawing;
use crate::sketch_plane::SketchPlane;

/// A sketch, solved and drawn, with the solids it stands on.
///
/// Placeholder content rather than a document: until the app can open one,
/// what it starts with has to come from somewhere, and a fixture whose answer
/// is known is what makes a wrong frame obvious. The visual suite raises this
/// very thing, so it is as much the test fixture as it is the startup content.
#[derive(Debug)]
pub(crate) struct Demo {
    pub(crate) drawing: Drawing,
    pub(crate) scene: Scene,
}

impl Demo {
    /// Solve the sketch and lay it out in the world, framed by the camera.
    pub(crate) fn build() -> Self {
        // The sketch and the solids share one world: the drawing lies on the
        // ground plane and the boxes stand on it, so orbiting the view moves
        // both together.
        let mut drawing = Drawing::new(Self::sketch(), SketchPlane::GROUND);
        let plane = drawing.plane();
        let mut scene = Scene::default();
        drawing.write_into(scene.overlays_mut());
        // The ground the drawing lies on, and the reason the drawing carries a
        // depth bias at all: the slab's top face *is* the sketch plane, so the
        // two are exactly coplanar and something has to decide which reads.
        scene.objects.push(Object {
            mesh: Mesh::cube(1.0),
            // A unit cube spans ±0.5, so dropping the slab by half its
            // thickness after scaling lands its top face on y = 0.
            transform: Mat4::from_translation(Vec3::new(4.0, -0.5, -2.5))
                * Mat4::from_scale(Vec3::new(12.0, 1.0, 9.0)),
            color: Vec3::new(0.30, 0.30, 0.34),
            tag: None,
        });
        for (size, at, color) in [
            (2.0, DVec2::new(2.0, 3.6), Vec3::new(0.55, 0.58, 0.62)),
            (0.8, DVec2::new(6.2, 1.1), Vec3::new(0.85, 0.35, 0.20)),
            (1.2, DVec2::new(6.6, 3.9), Vec3::new(0.25, 0.45, 0.75)),
        ] {
            // Half a cube up puts it on the plane rather than through it.
            let base = plane.point(at) + Vec3::Y * size * 0.5;
            scene
                .objects
                .push(Object::new(Mesh::cube(size)).at(base).colored(color));
        }
        if let Some(bounds) = scene.bounds() {
            scene.camera.frame(bounds);
        }

        Self { drawing, scene }
    }

    /// A rectangle anchored at the origin with a circle at its centre. Every
    /// position below is a guess deliberately off the answer: what puts the
    /// geometry where it belongs is the solve, not the coordinates.
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
        for pair in [[0, 1], [1, 2], [2, 3], [3, 0]] {
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
        let hole = sketch.add_circle(hub, 0.9);
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
        sketch.add_constraint(Constraint::Radius {
            circle: hole,
            radius: 1.5,
        });

        // A pair joined a fixed span apart and tied to nothing else. The
        // rectangle above is fully determined, so nothing in it can be
        // dragged — its every point is already where its constraints put it.
        // This is where the drawing can actually be taken hold of: drag either
        // end and the other swings round to keep the span.
        let grip = sketch.add_point(DVec2::new(9.6, 1.2));
        let swing = sketch.add_point(DVec2::new(11.4, 2.6));
        sketch.add_segment(grip, swing);
        sketch.add_constraint(Constraint::Distance {
            a: grip,
            b: swing,
            distance: 2.0,
        });
        sketch
    }
}
