//! What a press on what the drawing drew actually finds.
//!
//! Here rather than beside the app's own tests because a mark is pickable only
//! once a frame has laid it out: how far a run reaches is the shaper's answer,
//! filled in when the renderer draws it, and a harness with no device never
//! fills it. So the one place the whole chain can be asked — the drawing puts a
//! mark somewhere, the renderer lays it out, and a pick finds it — is a rendered
//! frame.

use aperture::{Aim, Facing, Scene, Tag, Viewport};
use catcad::CatCad;
use glam::{UVec2, Vec2};
use palantir::internals::headless_test_gpu;
use palantir::{InputEvent, OffscreenHost, wgpu};

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

/// Where every mark the app drew has its box, as the tag that reports it and the
/// middle of the box on screen.
///
/// Read off the scene the app last painted, which is the only place the answer
/// exists: where a mark stands is `paint`\'s, how far it reaches is the shaper\'s,
/// and the two meet only in a run that has been drawn.
fn boxes(app: &CatCad) -> Vec<(String, Tag, Vec2)> {
    let renderer = app.renderer().borrow();
    let camera = *renderer.camera();
    let mut found = Vec::new();
    for text in renderer.scene().texts.iter() {
        let Some(tag) = text.tag else { continue };
        let Facing::Turned(turn) = text.facing else {
            panic!("a mark is laid in its sketch plane");
        };
        // The lift carries the point the run names, and the run is centred on
        // what it carries. See `paint::mark_centre`.
        let step = camera.world_per_pixel(text.position, viewport());
        let middle = text.position + turn.lift_world() * step;
        let cursor = camera
            .screen_of(middle, viewport())
            .expect("a drawn mark is somewhere the projection draws");
        found.push((text.content.clone(), tag, cursor));
    }
    found
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

    let drawn = boxes(&app);
    let every: Vec<Tag> = drawn.iter().map(|(_, tag, _)| *tag).collect();
    let mut answered = 0;
    for (content, tag, middle) in &drawn {
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
        let (middle, content) = (*middle, content);
        if !hover(middle, &[*tag]) {
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

/// The target every frame below is drawn into, in *physical* pixels.
const PHYSICAL: UVec2 = UVec2::new(1200, 900);

/// Physical pixels to the logical one, which is what a display set to any scale
/// but 1 hands the application.
///
/// Not 1, and that is the whole reason it is stated: a run is laid out by the
/// shader in the target's pixels and picked in logical ones, so every factor
/// between the two is invisible at 1 and wrong everywhere else.
const RASTER: f32 = 1.5;

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
