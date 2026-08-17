//! What a press on what the drawing drew actually finds, and what a drag does
//! with it.
//!
//! Here rather than beside the app's own tests because a mark is pickable only
//! once a frame has laid it out: how far a run reaches is the shaper's answer,
//! filled in when the renderer draws it, and a harness with no device never
//! fills it. So the one place the whole chain can be asked — the drawing puts a
//! mark somewhere, the renderer lays it out, and a pick finds it — is a rendered
//! frame.

use aperture::{Aim, Camera, Facing, Scene, Tag, Viewport};
use catcad::CatCad;
use glam::{UVec2, Vec2, Vec3};
use palantir::internals::headless_test_gpu;
use palantir::{InputEvent, OffscreenHost, PointerButton, wgpu};

/// The target every frame below is drawn into, in *physical* pixels.
const PHYSICAL: UVec2 = UVec2::new(1200, 900);

/// Physical pixels to the logical one, which is what a display set to any scale
/// but 1 hands the application.
///
/// Not 1, and that is the whole reason it is stated: a run is laid out by the
/// shader in the target's pixels and picked in logical ones, so every factor
/// between the two is invisible at 1 and wrong everywhere else.
const RASTER: f32 = 1.5;

/// A target of [`PHYSICAL`] for a headless host to draw the app into.
fn target(gpu: &palantir::internals::HeadlessTestGpuLease) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("catcad.picking.target"),
        size: wgpu::Extent3d {
            width: PHYSICAL.x,
            height: PHYSICAL.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// The viewport a pick is answered in — [`PHYSICAL`] in logical pixels, which is
/// what the view lays out at.
fn viewport() -> Viewport {
    Viewport::new(UVec2::new(
        (PHYSICAL.x as f32 / RASTER) as u32,
        (PHYSICAL.y as f32 / RASTER) as u32,
    ))
}

/// One mark the app drew, read off the scene it last painted.
///
/// A named result rather than a tuple, and the fields are what the two sweeps
/// below ask between them: one wants the box on screen to hover, the other wants
/// it in the world to measure a drag against. Both start from the same walk, and
/// the walk is the only place the answer exists — where a mark stands is
/// `paint`'s, how far it reaches is the shaper's, and the two meet only in a run
/// that has been drawn.
#[derive(Debug, Clone)]
struct Drawn {
    tag: Tag,
    content: String,
    /// The middle of the box in the world: the point the run names, carried by
    /// the lift. See `paint::mark_centre`.
    middle: Vec3,
    /// The plane the run is laid in.
    normal: Vec3,
}

/// Every mark the app drew, in the order the scene holds them.
fn drawn(app: &CatCad) -> Vec<Drawn> {
    let renderer = app.renderer().borrow();
    let camera = *renderer.camera();
    let mut found = Vec::new();
    for text in renderer.scene().texts.iter() {
        let Some(tag) = text.tag else { continue };
        let Facing::Turned(turn) = text.facing else {
            panic!("a mark is laid in its sketch plane");
        };
        let step = camera.world_per_pixel(text.position, viewport());
        found.push(Drawn {
            tag,
            content: text.content.clone(),
            middle: text.position + turn.lift_world() * step,
            normal: turn.normal,
        });
    }
    found
}

/// Where `mark`'s box sits on screen, seen through the camera the app last
/// painted with.
fn on_screen(app: &CatCad, mark: &Drawn) -> Vec2 {
    app.renderer()
        .borrow()
        .camera()
        .screen_of(mark.middle, viewport())
        .expect("a drawn mark is somewhere the projection draws")
}

/// **A mark answers a hover over the whole of its box, not only over the middle
/// of it.**
///
/// The one test that goes through the application\'s own input path — a pointer
/// event, the response it lands in, the viewport the view reads off that, and
/// the highlight that comes back — rather than asking the scene a question the
/// view would have asked it. Everything above this stops one layer short of the
/// thing a user actually does.
///
/// What it caught: a run reported one depth for a box that is not all at one
/// depth. A drawing lies flat, so the face it encloses is coplanar with the
/// numbers on it and nearer than the middle of any of them over half their area
/// — and the lower half of every label read as being *behind* the sheet it is
/// drawn on. Hovering the digits did nothing while the empty space above them
/// lit the label.
///
/// Stated as a comparison rather than as a list of hot cursors, which is what
/// keeps it from being a second opinion about where marks go: wherever a mark
/// answers at the middle of its box it has to answer just off the middle too.
/// A mark the cylinder happens to stand over answers nowhere and is skipped by
/// the same clause, so the sweep needs no table of exceptions.
#[test]
fn a_mark_answers_a_hover_over_all_of_its_box() {
    let gpu = headless_test_gpu();
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    let target = target(&gpu);
    let mut app = CatCad::build();
    // Closer than the demo opens, which is where a drawing fills the view and
    // its own region lies under every number on it.
    app.camera_mut().distance *= 0.6;
    host.frame_offscreen(&target, RASTER, &mut app);
    host.frame_offscreen(&target, RASTER, &mut app);

    let marks: Vec<(Drawn, Vec2)> = drawn(&app)
        .into_iter()
        .map(|mark| {
            let at = on_screen(&app, &mark);
            (mark, at)
        })
        .collect();
    let every: Vec<Tag> = marks.iter().map(|(mark, _)| mark.tag).collect();
    let mut answered = 0;
    for (mark, middle) in &marks {
        // Any mark, off the middle, rather than this one. Marks stack and stand
        // beside each other, so a few pixels off one box is sometimes inside its
        // neighbour\'s and the neighbour rightly answers — which is a question
        // about two marks and not about either one\'s reach. What the sweep is
        // watching for is the press falling through the drawing altogether.
        let mut hover = |at: Vec2, of: &[Tag]| {
            host.ui().on_input(InputEvent::PointerMoved(at));
            host.frame_offscreen(&target, RASTER, &mut app);
            of.iter().any(|tag| app.hovering(*tag))
        };
        let (middle, content) = (*middle, &mark.content);
        if !hover(middle, &[mark.tag]) {
            continue;
        }
        answered += 1;
        // Four logical pixels off the middle, each way. The marks are set in a
        // thirteen-pixel face, so this is inside the box on both axes for the
        // narrowest of them — and it is the *down* one that was cold.
        for off in [
            Vec2::new(0.0, 4.0),
            Vec2::new(0.0, -4.0),
            Vec2::new(3.0, 0.0),
            Vec2::new(-3.0, 0.0),
        ] {
            assert!(
                hover(middle + off, &every),
                "the mark {content:?} answers a hover at the middle of its box, {middle:?}, \
                 and not {off:?} from it"
            );
        }
    }
    assert!(
        answered >= 8,
        "only {answered} marks answered a hover at all — the sweep asked nothing"
    );
}

/// **Every mark the drawing draws answers a press in the middle of its own
/// box.**
///
/// The claim that spans all three layers and that neither side's own tests can
/// make: `paint` decides where a mark goes, the renderer lays it out against the
/// screen, and a pick has to find it there. Each half can be right about its own
/// arithmetic while the two disagree — which is a number you can read and cannot
/// click.
///
/// Two things it has caught, both of them a run drawn in one place and reachable
/// in another. A lift in logical pixels multiplied by a step in physical ones,
/// so the clearance shrank with the display scale while the box stayed put. And
/// a hit answering from the point a run *names* rather than from where the run
/// stands, so at a grazing angle a label plainly drawn over a face read as being
/// behind it and the face took the click.
///
/// **Angles chosen so nothing the drawing draws is hidden**, which is what lets
/// this be a sweep rather than a list of exceptions: a mark behind the demo's
/// cylinder is correctly unpickable, and telling that case from a broken box
/// would mean this test knowing which marks those are. Both projections, because
/// the step a run is sized by is a constant under one and a depth under the
/// other.
#[test]
fn every_mark_is_picked_where_it_is_drawn() {
    let gpu = headless_test_gpu();
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("catcad.picking.target"),
        size: wgpu::Extent3d {
            width: PHYSICAL.x,
            height: PHYSICAL.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let viewport = Viewport::new(UVec2::new(
        (PHYSICAL.x as f32 / RASTER) as u32,
        (PHYSICAL.y as f32 / RASTER) as u32,
    ));

    let mut app = CatCad::build();
    let opened = *app.camera_mut();
    for projection in [
        aperture::Projection::Orthographic,
        aperture::Projection::Perspective,
    ] {
        // The last is the demo's own opening angle brought in close, which is
        // where the dormant sketch's region swings in front of the open one —
        // see the sweep's third claim.
        for (yaw, pitch, zoom, whole) in [
            (0.7f32, -0.5f32, 1.0f32, true),
            (-1.3, -1.2, 1.0, true),
            (opened.yaw, opened.pitch, 0.5, false),
        ] {
            {
                let camera = app.camera_mut();
                camera.yaw = yaw;
                camera.pitch = pitch;
                camera.distance = opened.distance * zoom;
                camera.projection = projection;
            }
            // Twice: the first frame lays the runs out and the second is the
            // steady one, which is the frame a user is ever looking at.
            host.frame_offscreen(&target, RASTER, &mut app);
            host.frame_offscreen(&target, RASTER, &mut app);

            let renderer = app.renderer().borrow();
            let camera = *renderer.camera();
            let scene: &Scene = renderer.scene();
            let mut marks = 0;
            for text in scene.texts.iter() {
                let Some(tag) = text.tag else { continue };
                let Facing::Turned(turn) = text.facing else {
                    panic!("a mark is laid in its sketch plane");
                };
                // The middle of the box, worked out the way the drawing works it
                // out: the lift carries the point the run names, and the run is
                // centred on what it carries. See `paint::mark_centre`.
                let step = camera.world_per_pixel(text.position, viewport);
                let middle = text.position + turn.lift_world() * step;
                let cursor = camera
                    .screen_of(middle, viewport)
                    .expect("a drawn mark is somewhere the projection draws");
                let found = scene.nearest(Aim::new(&camera, cursor, viewport, 6.0));
                let where_ = format!("{projection:?} at yaw {yaw} pitch {pitch} zoom {zoom}");
                let beaten = found.map(|hit| (hit.at, hit.precedence));
                // **Whatever takes the press is at least as near the eye as the
                // mark.** True at every angle, hidden or not, and the only thing
                // that can honestly be said where something *is* in front: a
                // mark loses to what covers it and to nothing else. Standing has
                // no say — it decides between what survives being in front, not
                // what is in front.
                let ray = camera.ray_through(cursor, viewport);
                let mine = (middle - ray.origin).dot(ray.direction);
                assert!(
                    found.is_some_and(|hit| hit.tag == tag || hit.distance <= mine * 1.001),
                    "{where_}: the mark {:?} is drawn with its box on {cursor:?} at {mine} \
                     along the ray, and a press there was taken by {beaten:?} further off",
                    text.content,
                );
                // And where nothing the drawing draws is hidden, the mark
                // answers outright.
                if whole {
                    assert_eq!(
                        found.map(|hit| hit.tag),
                        Some(tag),
                        "{where_}: the mark {:?} is drawn with its box on {cursor:?} and a \
                         press there found {beaten:?}",
                        text.content,
                    );
                }
                marks += 1;
            }
            // The sweep is worth nothing if the drawing stopped drawing marks,
            // which is exactly how it would fail silently.
            assert!(
                marks >= 10,
                "{projection:?} at yaw {yaw} pitch {pitch}: only {marks} marks were drawn"
            );
        }
    }
}

/// Where the cursor ray meets the plane through `on`, or `None` looking along it.
fn on_plane(camera: &Camera, cursor: Vec2, on: Vec3, normal: Vec3) -> Option<Vec3> {
    let ray = camera.ray_through(cursor, viewport());
    let along = normal.dot(ray.direction);
    (along.abs() > 1e-4)
        .then(|| ray.origin + ray.direction * ((on - ray.origin).dot(normal) / along))
}

/// **A number dragged by its box travels exactly as far as the cursor does.**
///
/// The whole of what a grab promises, and the one thing a placement can get
/// wrong in a way nothing else notices: the drawing is still correct after a
/// jump, the solve is untouched, and only the number has moved out from under
/// the pointer.
///
/// It caught two, and they are the same defect at two speeds. A dimension sharing
/// its place with its own relation rises a lane to clear it, and carrying it
/// away leaves it nothing to clear — so it dropped thirteen logical pixels, about
/// its own height, on the first frame of every drag; the demo has two of those.
/// And a **radius** stands off along its own leader, which runs out through
/// wherever the number was put — so dragging it turns the frame it stands off
/// in, continuously, and it never caught the cursor at all.
///
/// Compared against the **drawn box**, not the anchor the change writes. The
/// anchor tracked the cursor perfectly through both bugs; what moved was the
/// standoff between the two, which is exactly what a test of the anchor cannot
/// see. The radius is the sharp case: nothing taken once at the press can hold a
/// standoff that turns, so it is what says the correction is read afresh.
#[test]
fn a_number_dragged_by_its_box_travels_with_the_cursor() {
    let gpu = headless_test_gpu();
    let opened = *CatCad::build().camera_mut();
    for (yaw, pitch, zoom) in [
        (opened.yaw, opened.pitch, 1.0f32),
        (opened.yaw, opened.pitch, 0.6),
    ] {
        // One app and one host for the whole camera. A drag leaves the number it
        // moved where it put it, which is nothing to the next one — and building
        // a fresh app per mark is what made this the slowest test in the suite.
        let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
        let target = target(&gpu);
        let mut app = CatCad::build();
        {
            let camera = app.camera_mut();
            camera.yaw = yaw;
            camera.pitch = pitch;
            camera.distance = opened.distance * zoom;
        }
        host.frame_offscreen(&target, RASTER, &mut app);
        host.frame_offscreen(&target, RASTER, &mut app);

        let mut carried = 0;
        for nth in 0..drawn(&app).len() {
            let Drawn {
                tag,
                content,
                middle: was,
                normal,
            } = drawn(&app)[nth].clone();
            // Relations carry no placement and rightly do not move; a radius is
            // the gap named above.
            if !content.chars().any(|c| c.is_ascii_digit()) {
                continue;
            }
            let camera = *app.renderer().borrow().camera();
            let Some(cursor) = camera.screen_of(was, viewport()) else {
                continue;
            };

            host.ui().on_input(InputEvent::PointerMoved(cursor));
            host.frame_offscreen(&target, RASTER, &mut app);
            // Only where the press would take the number. Elsewhere it turns the
            // view, and a number that stayed put is right rather than stuck.
            if !app.hovering(tag) {
                continue;
            }
            host.ui()
                .on_input(InputEvent::PointerPressed(PointerButton::Left));
            host.frame_offscreen(&target, RASTER, &mut app);
            // Well past palantir's drag latch, so this is a drag and not a click
            // — and along both axes, since a placement is a pair.
            let moved = cursor + Vec2::new(21.0, -13.0);
            host.ui().on_input(InputEvent::PointerMoved(moved));
            host.frame_offscreen(&target, RASTER, &mut app);
            host.frame_offscreen(&target, RASTER, &mut app);
            host.ui()
                .on_input(InputEvent::PointerReleased(PointerButton::Left));
            host.frame_offscreen(&target, RASTER, &mut app);

            let now = drawn(&app)
                .into_iter()
                .find(|mark| mark.tag == tag)
                .expect("the mark survived its own drag")
                .middle;
            let (Some(from), Some(to)) = (
                on_plane(&camera, cursor, was, normal),
                on_plane(&camera, moved, was, normal),
            ) else {
                continue;
            };
            let off = ((now - was) - (to - from)).length();
            assert!(
                off < 0.02,
                "yaw {yaw} zoom {zoom}: the number {content:?} moved {:?} where the cursor \
                 carried it {:?} — out by {off}",
                now - was,
                to - from,
            );
            carried += 1;
        }
        assert!(
            carried >= 4,
            "yaw {yaw} zoom {zoom}: only {carried} numbers were dragged at all"
        );
    }
}

/// **A number the cursor has stopped moving stops moving.**
///
/// The property a drag has to have and the one nothing else asks for. A number
/// is grabbed by its box and placed by its *point*, so the gesture has to take
/// the clearance between them off — and taking off the last frame's is only
/// right where the clearance does not depend on the answer.
///
/// For every dimension but one it does not: the direction a mark is set along
/// comes from the geometry it measures, which a drag on the number leaves alone.
/// A radius is the one, because its leader runs out through wherever the number
/// was *put*. Subtracting there is a drag chasing an answer that keeps moving,
/// and held still it did not stop — at some bearings the number swapped between
/// two places a whole clearance apart, every frame, for ever.
///
/// Swept round the compass and at two reaches, because it is a question about
/// angle *and* about how long the placement is: subtraction converges only while
/// the placement is the longer of the two, so a number dragged in close is where
/// it gives out. `paint::mark_anchor` inverts the clearance instead, so what is
/// asked here is not "settles down to" but "is right on the frame it lands".
#[test]
fn a_number_held_still_stops_moving() {
    let gpu = headless_test_gpu();
    let target = target(&gpu);
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    let mut app = CatCad::build();
    host.frame_offscreen(&target, RASTER, &mut app);
    host.frame_offscreen(&target, RASTER, &mut app);

    let radius = |app: &CatCad| -> Drawn {
        drawn(app)
            .into_iter()
            .find(|mark| mark.content.starts_with('R'))
            .expect("the demo states a radius")
    };
    let start = on_screen(&app, &radius(&app));
    host.ui().on_input(InputEvent::PointerMoved(start));
    host.frame_offscreen(&target, RASTER, &mut app);
    host.ui()
        .on_input(InputEvent::PointerPressed(PointerButton::Left));
    host.frame_offscreen(&target, RASTER, &mut app);

    for step in 0..24 {
        let turn = step as f32 * std::f32::consts::TAU / 24.0;
        // A whole turn so no bearing is skipped, and the reach alternating
        // between well clear of the circle and in close — the second is where
        // subtracting the clearance stops converging at all.
        let reach = if step % 2 == 0 { 60.0 } else { 22.0 };
        let at = start + Vec2::new(turn.cos(), turn.sin()) * reach;
        host.ui().on_input(InputEvent::PointerMoved(at));
        // Three frames at one cursor. The first carries the move; the other two
        // have to agree with it *exactly*. Not "settle down to" — allowing a
        // frame of catching up is the difference between a drag that converges
        // and a drag that is simply right.
        host.frame_offscreen(&target, RASTER, &mut app);
        let settled = radius(&app).middle;
        host.frame_offscreen(&target, RASTER, &mut app);
        let then = radius(&app).middle;
        host.frame_offscreen(&target, RASTER, &mut app);
        let still = radius(&app).middle;
        let jitter = (then - settled).length().max((still - then).length());
        assert!(
            jitter < 1e-4,
            "at {:.0}° reaching {reach} px the number went on moving by {jitter} with the \
             cursor held: {settled:?} then {then:?} then {still:?}",
            turn.to_degrees()
        );
    }
    host.ui()
        .on_input(InputEvent::PointerReleased(PointerButton::Left));
}
