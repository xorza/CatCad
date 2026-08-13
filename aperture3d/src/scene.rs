//! What to draw, and where to look at it from.

use crate::bounds::Bounds;
use crate::camera::Camera;
use crate::curve::Curve;
use crate::hit::{Hit, HitAt};
use crate::object::Object;
use crate::point::Point;
use crate::viewport::Viewport;
use glam::{Mat4, Vec2, Vec3, Vec4};

/// Squared screen length below which a projected segment lands on a single
/// pixel and has no direction to project a cursor onto. A thousandth of a
/// pixel, squared — the floor `MIN_PX` holds in the shaders.
const MIN_RUN_PX2: f32 = 1e-6;

/// Floor under the sum of reciprocal depths that undoes the perspective
/// squeeze. Only a segment with both ends astronomically far off gets near it.
const MIN_RECIP_W: f32 = 1e-6;

/// The whole of the drawable world: shaded meshes, stroked curves, and the
/// camera viewing them. Flat for now — hierarchy, if it earns its place, goes
/// here.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub camera: Camera,
    pub objects: Vec<Object>,
    pub curves: Vec<Curve>,
    pub points: Vec<Point>,
}

impl Scene {
    /// What the scene occupies in world space, or `None` when there is
    /// nothing in it. Mesh vertices are measured after their object's
    /// transform, so this is where the geometry actually lands.
    ///
    /// Curve stroke width doesn't count: it is a screen-space quantity, and
    /// the distance that would satisfy it is the one being solved for.
    pub fn bounds(&self) -> Option<Bounds> {
        let mut bounds: Option<Bounds> = None;
        let mut include = |point| match &mut bounds {
            Some(bounds) => bounds.include(point),
            empty => *empty = Some(Bounds::point(point)),
        };
        for object in &self.objects {
            for vertex in &object.mesh.vertices {
                include(object.transform.transform_point3(vertex.position));
            }
        }
        for curve in &self.curves {
            for point in &curve.points {
                include(*point);
            }
        }
        // A marker's glyph is screen-sized, so like a stroke's width it says
        // nothing about where the world reaches — only its anchor counts.
        for point in &self.points {
            include(point.position);
        }
        bounds
    }

    /// Everything within `radius` of `cursor` on screen, nearest first.
    ///
    /// `cursor` and `radius` are in **logical** pixels, as is the [`Viewport`]
    /// they are measured against, and `cursor` counts down from the top-left
    /// corner.
    ///
    /// Not a free choice, unlike everywhere else a cursor and a viewport meet:
    /// what counts as a hit depends on how wide the thing is drawn, and a
    /// stroke's width and a marker's diameter are always logical — scaling
    /// them to the target is the renderer's job and happens after this. Aiming
    /// in physical pixels on a scaled display would ask for everything within
    /// a reach the glyph has already outgrown.
    ///
    /// Tested in screen space rather than against the world, because that is
    /// where the aim happened: a stroke is a pixel and a half wide however far
    /// off it is, and a marker is a fixed disc, so the distance that decides
    /// whether the cursor was on one is a distance in pixels. Anything drawn
    /// wider than `radius` is pickable anywhere it is visible — you can always
    /// grab what you can see.
    ///
    /// A *list* rather than the nearest one, because "what did I click" and
    /// "what did I mean" are different questions. Clicking again to cycle
    /// through what overlaps, ignoring kinds the current tool cannot use, and
    /// hovering differently from clicking are all answerable from one query
    /// this way, and none of them are if the choice is made here.
    ///
    /// Ordered by how specific the hit is — a marker beats a stroke running
    /// through it, because the smaller thing is the harder one to aim at and
    /// so the one the aim was meant for — then by distance from the cursor,
    /// then by distance from the eye. Untagged primitives are scenery and
    /// never appear.
    pub fn pick(&self, cursor: Vec2, viewport: Viewport, radius: f32) -> Vec<Hit> {
        let view_proj = self.camera.view_proj(viewport.aspect());
        let ray = self.camera.ray_through(cursor, viewport);
        let along = |world: Vec3| (world - ray.origin).dot(ray.direction);

        let mut hits = Vec::new();
        for point in &self.points {
            let Some(tag) = point.tag else { continue };
            let clip = view_proj * point.position.extend(1.0);
            // The marker's quad takes its depth from the anchor, so the anchor
            // clipping is the whole glyph clipping.
            if !Inside::of(clip).drawn() {
                continue;
            }
            let screen = cursor.distance(viewport.pixel_from_clip(clip));
            // A marker you can see is a marker you can hit, even where the
            // glyph outgrows the tolerance asked for.
            if screen <= radius.max(point.size * 0.5) {
                hits.push(Hit {
                    tag,
                    at: HitAt::Point,
                    world: point.position,
                    screen,
                    distance: along(point.position),
                });
            }
        }

        for curve in &self.curves {
            let Some(tag) = curve.tag else { continue };
            let reach = radius.max(curve.width * 0.5);
            let mut best: Option<Hit> = None;
            for (index, (a, b)) in curve.segments().enumerate() {
                let Some(near) = nearest_on_segment(a, b, view_proj, viewport, cursor) else {
                    continue;
                };
                if near.screen > reach {
                    continue;
                }
                if best.is_some_and(|best| best.screen <= near.screen) {
                    continue;
                }
                let world = a.lerp(b, near.t);
                best = Some(Hit {
                    tag,
                    at: HitAt::Segment { index, t: near.t },
                    world,
                    screen: near.screen,
                    distance: along(world),
                });
            }
            hits.extend(best);
        }

        hits.sort_by(|a, b| {
            a.at.rank()
                .cmp(&b.at.rank())
                .then(a.screen.total_cmp(&b.screen))
                .then(a.distance.total_cmp(&b.distance))
        });
        hits
    }
}

/// Where a cursor came closest to one segment.
#[derive(Debug, Clone, Copy)]
struct Nearest {
    /// How far along the segment, in world terms.
    t: f32,
    /// How far the cursor was from it on screen.
    screen: f32,
}

/// How far into the view volume a clip position sits, along each of the two
/// planes that can cut it: the near plane, and the far end of an orthographic
/// slab.
///
/// Reversed depth puts the near plane at `z == w` and the slab's far end at
/// `z == 0`, so both read as "non-negative is inside". These are the
/// half-spaces the hardware clips against, which is what makes what can be
/// picked the same as what was drawn. Perspective writes a constant positive
/// `clip.z` and has no far plane, so there the first is `w >= z_near` and the
/// second never fires.
#[derive(Debug, Clone, Copy)]
struct Inside {
    near: f32,
    far: f32,
}

impl Inside {
    fn of(clip: Vec4) -> Self {
        Self {
            near: clip.w - clip.z,
            far: clip.z,
        }
    }

    /// Whether the position survived both planes, and so is drawn.
    fn drawn(&self) -> bool {
        self.near >= 0.0 && self.far >= 0.0
    }
}

/// The stretch of a segment left after clipping, as fractions of the whole.
#[derive(Debug, Clone, Copy)]
struct Span {
    start: f32,
    end: f32,
}

impl Span {
    fn whole() -> Self {
        Self {
            start: 0.0,
            end: 1.0,
        }
    }

    /// Trim to where a quantity that is affine along the segment — given by
    /// its value at each end — is non-negative. `None` once nothing is left.
    ///
    /// Clip space is affine in the world parameter, so a crossing sits at the
    /// same fraction of the world segment as of the clip one and the surviving
    /// stretch can be picked on as itself.
    fn clip(self, at_start: f32, at_end: f32) -> Option<Self> {
        let Self { start, end } = match (at_start >= 0.0, at_end >= 0.0) {
            (true, true) => self,
            (false, false) => return None,
            (true, false) => Self {
                end: self.end.min(at_start / (at_start - at_end)),
                ..self
            },
            (false, true) => Self {
                start: self.start.max(at_start / (at_start - at_end)),
                ..self
            },
        };
        (start <= end).then_some(Self { start, end })
    }
}

/// The point of segment `a`–`b` nearest `cursor` on screen, or `None` if none
/// of it is drawn.
fn nearest_on_segment(
    a: Vec3,
    b: Vec3,
    view_proj: Mat4,
    viewport: Viewport,
    cursor: Vec2,
) -> Option<Nearest> {
    let (a_clip, b_clip) = (view_proj * a.extend(1.0), view_proj * b.extend(1.0));
    let (a_in, b_in) = (Inside::of(a_clip), Inside::of(b_clip));
    let span = Span::whole()
        .clip(a_in.near, b_in.near)?
        .clip(a_in.far, b_in.far)?;
    // Inside the near plane `w` is at least `z_near` under perspective and
    // exactly 1 under parallel rays, so what survived can be divided by it.
    let near = a_clip.lerp(b_clip, span.start);
    let far = a_clip.lerp(b_clip, span.end);

    let (from, to) = (
        viewport.pixel_from_clip(near),
        viewport.pixel_from_clip(far),
    );
    let run = to - from;
    let length = run.length_squared();
    // A segment that lands on one pixel has no direction to project onto, and
    // either end answers the same.
    let on_screen = if length > MIN_RUN_PX2 {
        ((cursor - from).dot(run) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let screen = cursor.distance(from + run * on_screen);

    // Screen distance runs evenly along the *projected* segment, and under
    // perspective that is not evenly along the world one — the far half of a
    // receding edge is squeezed into fewer pixels. Undoing that is what makes
    // the returned point land where the cursor actually is rather than short
    // of it, which is the difference between snapping to a midpoint and
    // snapping near one.
    let recip = (1.0 - on_screen) / near.w + on_screen / far.w;
    let in_span = if recip > MIN_RECIP_W {
        (on_screen / far.w) / recip
    } else {
        on_screen
    };
    Some(Nearest {
        t: span.start + in_span * (span.end - span.start),
        screen,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Projection;
    use crate::mesh::Mesh;
    use glam::UVec2;

    /// Looking straight down −Z from 5 away with a 90° fov, so a 100×100
    /// viewport puts the origin dead centre and the world spans ±5 across it
    /// at the target's depth: 10 pixels to the world unit.
    fn head_on() -> Scene {
        Scene {
            camera: Camera {
                target: Vec3::ZERO,
                distance: 5.0,
                yaw: 0.0,
                pitch: 0.0,
                fov_y: std::f32::consts::FRAC_PI_2,
                near_ratio: 1.0 / 5.0,
                projection: Projection::Perspective,
            },
            ..Default::default()
        }
    }

    const CENTRE: Vec2 = Vec2::new(50.0, 50.0);

    fn viewport() -> Viewport {
        Viewport::new(UVec2::new(100, 100))
    }

    #[test]
    fn a_marker_is_hit_within_its_own_glyph_or_the_asked_radius() {
        let mut scene = head_on();
        scene
            .points
            .push(Point::new(Vec3::ZERO).size(8.0).tagged(1));

        // Dead on.
        let hits = scene.pick(CENTRE, viewport(), 1.0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].tag, 1);
        assert_eq!(hits[0].at, HitAt::Point);
        assert_eq!(hits[0].world, Vec3::ZERO);
        assert!(hits[0].screen < 1e-4);
        // The eye is 5 off and the ray starts 1 along, so 4 remain.
        assert!((hits[0].distance - 4.0).abs() < 1e-4, "{hits:?}");

        // Three pixels off is inside the 8px glyph even at zero tolerance,
        // because what is drawn is grabbable.
        let near = scene.pick(CENTRE + Vec2::new(3.0, 0.0), viewport(), 0.0);
        assert_eq!(near.len(), 1);
        assert!((near[0].screen - 3.0).abs() < 1e-4);

        // Six is outside the glyph's four, and outside a one-pixel radius.
        assert!(
            scene
                .pick(CENTRE + Vec2::new(6.0, 0.0), viewport(), 1.0)
                .is_empty()
        );
        // But not outside a generous one.
        assert_eq!(
            scene
                .pick(CENTRE + Vec2::new(6.0, 0.0), viewport(), 8.0)
                .len(),
            1
        );
    }

    #[test]
    fn scenery_is_never_picked() {
        let mut scene = head_on();
        scene.points.push(Point::new(Vec3::ZERO).size(8.0));
        scene
            .curves
            .push(Curve::segment(-Vec3::X, Vec3::X).width(2.0));
        assert!(scene.pick(CENTRE, viewport(), 20.0).is_empty());
    }

    #[test]
    fn a_stroke_reports_where_along_it_the_cursor_fell() {
        let mut scene = head_on();
        // Spans x −2..2, which at ten pixels to the unit is 40 px either side
        // of centre.
        scene
            .curves
            .push(Curve::segment(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)).tagged(7));

        let hits = scene.pick(CENTRE + Vec2::new(10.0, 0.0), viewport(), 4.0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].tag, 7);
        // Ten pixels right of centre is world x = 1, which is three quarters
        // along a segment running from −2 to 2. The cursor sits on the line,
        // so nothing separates them.
        let HitAt::Segment { index: 0, t } = hits[0].at else {
            panic!("{hits:?}");
        };
        assert!((t - 0.75).abs() < 1e-5, "{t}");
        assert!(hits[0].world.abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-4));
        assert!(hits[0].screen < 1e-4, "{hits:?}");

        // The far end is at world x = 2, which is pixel 70. Past it the
        // nearest point on the segment is the end itself, until the cursor
        // walks out of the radius entirely.
        let beyond = scene.pick(Vec2::new(72.0, 50.0), viewport(), 4.0);
        assert_eq!(beyond.len(), 1, "{beyond:?}");
        assert_eq!(beyond[0].at, HitAt::Segment { index: 0, t: 1.0 });
        assert!((beyond[0].screen - 2.0).abs() < 1e-4, "{beyond:?}");
        assert!(
            scene
                .pick(Vec2::new(76.0, 50.0), viewport(), 4.0)
                .is_empty()
        );
    }

    #[test]
    fn a_receding_stroke_reports_where_the_cursor_is_in_the_world_not_on_screen() {
        let mut scene = head_on();
        // Runs away from the eye at 5: the near end is 1 off, the far end 21,
        // so the far half is squeezed into a fraction of the pixels the near
        // half gets. Halfway along on *screen* is nowhere near halfway along
        // the segment.
        scene
            .curves
            .push(Curve::segment(Vec3::new(0.0, -1.0, 4.0), Vec3::new(0.0, -1.0, -16.0)).tagged(3));

        // With a 90° fov the projected y is −1/w, so the ends land at pixel
        // 100 and 50 + 50/21 = 52.38, and their midpoint is 76.19.
        let hits = scene.pick(Vec2::new(50.0, 76.19), viewport(), 4.0);
        assert_eq!(hits.len(), 1, "{hits:?}");
        let HitAt::Segment { t, .. } = hits[0].at else {
            panic!("{hits:?}");
        };

        // Perspective-correct: t/w interpolates evenly, not t. Halfway across
        // the pixels is (0.5/21) / (0.5/1 + 0.5/21) along the segment, which
        // is a twenty-second of it — not the half a naive read would give.
        assert!((t - 0.04545).abs() < 1e-3, "{t} should be about 1/22");
        assert!(
            (hits[0].world.z - 3.09).abs() < 0.02,
            "{:?} should be just past the near end",
            hits[0].world
        );
    }

    #[test]
    fn a_marker_outranks_the_strokes_running_through_it() {
        let mut scene = head_on();
        // Two edges crossing at the origin, and a marker on the crossing —
        // the corner of any rectangle. Sorting on depth alone would bury the
        // marker under whichever edge rounded nearer.
        scene
            .curves
            .push(Curve::segment(-Vec3::X, Vec3::X).width(2.0).tagged(10));
        scene
            .curves
            .push(Curve::segment(-Vec3::Y, Vec3::Y).width(2.0).tagged(11));
        scene
            .points
            .push(Point::new(Vec3::ZERO).size(6.0).tagged(12));

        let hits = scene.pick(CENTRE, viewport(), 3.0);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].tag, 12, "the marker comes first: {hits:?}");
        assert_eq!(hits[0].at, HitAt::Point);
        // The strokes still come back — that is what lets a caller cycle.
        assert!(hits[1..].iter().all(|hit| hit.at.rank() == 1));
    }

    #[test]
    fn nearer_the_cursor_beats_nearer_the_eye() {
        let mut scene = head_on();
        // The closer stroke is a whole unit toward the eye but four pixels
        // off; the further one is dead under the cursor.
        scene.curves.push(
            Curve::segment(Vec3::new(-2.0, 0.4, 1.0), Vec3::new(2.0, 0.4, 1.0))
                .width(1.0)
                .tagged(20),
        );
        scene
            .curves
            .push(Curve::segment(-Vec3::X, Vec3::X).width(1.0).tagged(21));

        let hits = scene.pick(CENTRE, viewport(), 10.0);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].tag, 21, "aim beats depth: {hits:?}");
        assert!(hits[0].screen < hits[1].screen);
        assert!(hits[0].distance > hits[1].distance);
    }

    #[test]
    fn only_what_survived_the_near_plane_can_be_picked() {
        let mut scene = head_on();
        // Wholly behind: the eye is at z = 5 looking down −Z.
        scene
            .points
            .push(Point::new(Vec3::new(0.0, 0.0, 9.0)).tagged(1));
        assert!(scene.pick(CENTRE, viewport(), 50.0).is_empty());

        // And a marker the near plane cut is no more pickable than one behind
        // the eye — it is just as absent from the screen. The near plane is a
        // fifth of the 5-unit orbit distance in front of the eye, at z = 4.
        scene.points.clear();
        scene
            .points
            .push(Point::new(Vec3::new(0.0, 0.0, 4.5)).tagged(1));
        assert!(scene.pick(CENTRE, viewport(), 50.0).is_empty());

        // Straddling. The visible half still picks, and reports a parameter on
        // the *whole* segment rather than on the surviving piece. This one
        // recedes straight down the view axis, so all of it lands on one
        // pixel and the near end answers for the rest.
        scene.points.clear();
        scene
            .curves
            .push(Curve::segment(Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 0.0, 9.0)).tagged(2));
        let hits = scene.pick(CENTRE, viewport(), 20.0);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].tag, 2);
        assert_eq!(hits[0].at, HitAt::Segment { index: 0, t: 0.0 });
        assert_eq!(hits[0].world, Vec3::new(0.0, 0.0, -3.0));

        // Straddling *across* the view instead, so where the cut lands is
        // visible on screen. From (−1, 0, 6) to (1, 0, 0): z = 4 is a third of
        // the way along, at world x = −1/3, which at depth 1 under a 90° fov
        // is NDC −1/3 and so pixel 33.3. The far end is at depth 5 and world
        // x = 1, which is pixel 60.
        scene.curves.clear();
        scene.curves.push(
            Curve::segment(Vec3::new(-1.0, 0.0, 6.0), Vec3::new(1.0, 0.0, 0.0))
                .width(1.0)
                .tagged(3),
        );
        let hits = scene.pick(Vec2::new(40.0, 50.0), viewport(), 1.0);
        assert_eq!(hits.len(), 1, "inside the drawn stretch: {hits:?}");

        // Thirteen pixels short of where the near plane cut it. What lies that
        // way is the stretch between the near plane and the eye, which is
        // drawn nowhere, so a tolerance smaller than the gap finds nothing.
        assert!(
            scene
                .pick(Vec2::new(20.0, 50.0), viewport(), 4.0)
                .is_empty(),
            "picked a stretch the near plane cut"
        );

        // Widen the tolerance and the cut itself is what answers: a third
        // along, at the near plane, and 13.3 pixels from the cursor.
        let hits = scene.pick(Vec2::new(20.0, 50.0), viewport(), 20.0);
        assert_eq!(hits.len(), 1, "{hits:?}");
        let HitAt::Segment { t, .. } = hits[0].at else {
            panic!("{hits:?}");
        };
        assert!((t - 1.0 / 3.0).abs() < 1e-5, "{t}");
        assert!(
            hits[0]
                .world
                .abs_diff_eq(Vec3::new(-1.0 / 3.0, 0.0, 4.0), 1e-4),
            "{:?}",
            hits[0].world
        );
        assert!((hits[0].screen - 13.333).abs() < 1e-2, "{hits:?}");
    }

    #[test]
    fn bounds_cover_transformed_meshes_and_curves() {
        assert!(Scene::default().bounds().is_none());

        let mut scene = Scene::default();
        // A size-2 cube spans ±1 about its own origin, so shifting it 10 along
        // x puts its corners at 9 and 11.
        scene
            .objects
            .push(Object::new(Mesh::cube(2.0)).at(Vec3::new(10.0, 0.0, 0.0)));
        let cube = scene.bounds().unwrap();
        assert_eq!(cube.min, Vec3::new(9.0, -1.0, -1.0));
        assert_eq!(cube.max, Vec3::new(11.0, 1.0, 1.0));

        // A curve reaching past the cube drags the bounds out with it.
        scene
            .curves
            .push(Curve::segment(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO));
        let both = scene.bounds().unwrap();
        assert_eq!(both.min, Vec3::new(0.0, -1.0, -1.0));
        assert_eq!(both.max, Vec3::new(11.0, 4.0, 1.0));
        assert_eq!(both.centre(), Vec3::new(5.5, 1.5, 0.0));
    }
}
