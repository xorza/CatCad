use super::*;
use crate::camera::Projection;
use crate::curve::Curve;
use crate::highlight::Highlight;
use crate::mesh::{Mesh, Vertex};
use crate::object::Object;
use crate::point::Point;
use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::band::{QUAD_INDICES, RING_INDICES};
use crate::renderer::internals::ScenePane;
use crate::renderer::uniforms::Uniforms;
use crate::ring::Ring;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::text::Text;
use glam::{Mat4, Vec3};
use palantir::OffscreenHost;
use palantir::internals::{HeadlessTestGpuLease, headless_test_gpu};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn flatten_bakes_transforms_into_world_space() {
    let mut scene = Scene::default();
    scene.objects.push(Object::new(Mesh::cube(2.0)));
    scene.objects.push(
        Object::new(Mesh::cube(2.0))
            .at(Vec3::new(10.0, 0.0, 0.0))
            .colored(Vec3::new(1.0, 0.0, 0.0)),
    );
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    let triangles = &renderer.cpu.meshes;

    // Two cubes: 24 corners and 36 indices each.
    assert_eq!(triangles.vertices.len(), 48);
    assert_eq!(triangles.indices.len(), 72);

    // The second object's indices are rebased past the first's vertices,
    // so the halves address disjoint ranges.
    assert!(triangles.indices[..36].iter().all(|&i| i < 24));
    assert!(
        triangles.indices[36..]
            .iter()
            .all(|&i| (24..48).contains(&i))
    );
    assert_eq!(triangles.indices[36], triangles.indices[0] + 24);

    // Corners of a size-2 cube are (±1, ±1, ±1), shifted 10 along x for
    // the second, and the colour rides along per vertex.
    for vertex in &triangles.vertices[..24] {
        assert_eq!(vertex.position.map(f32::abs), [1.0, 1.0, 1.0]);
        assert_eq!(vertex.color, [0.7, 0.7, 0.7]);
    }
    for vertex in &triangles.vertices[24..] {
        assert!((vertex.position[0] - 10.0).abs() == 1.0, "{vertex:?}");
        assert_eq!(vertex.color, [1.0, 0.0, 0.0]);
    }

    // Translation leaves normals alone.
    assert_eq!(triangles.vertices[0].normal, triangles.vertices[24].normal);
}

#[test]
fn flatten_uses_the_inverse_transpose_for_normals() {
    // One triangle whose normal points diagonally, so a non-uniform scale
    // tells the two candidate transforms apart.
    let diagonal = Vec3::new(1.0, 1.0, 0.0).normalize();
    let mesh = Mesh {
        vertices: vec![
            Vertex {
                position: Vec3::ZERO,
                normal: diagonal,
            };
            3
        ],
        indices: vec![0, 1, 2],
    };
    let mut scene = Scene::default();
    scene.objects.push(Object {
        mesh,
        transform: Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0)),
        color: Vec3::ZERO,
        tag: None,
    });
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    let triangles = &renderer.cpu.meshes;

    // Scaling x by 2 flattens the surface toward the x axis, so its normal
    // tips *away* from x: inverse transpose diag(0.5, 1, 1) sends
    // (1, 1, 0)/√2 to (0.5, 1, 0)/√2, i.e. (1, 2, 0) normalized.
    let expected = Vec3::new(1.0, 2.0, 0.0).normalize();
    let actual = Vec3::from_array(triangles.vertices[0].normal);
    assert!(actual.abs_diff_eq(expected, 1e-6), "{actual:?}");

    // Transforming the normal directly would have tipped it the other way.
    let naive = Vec3::new(2.0, 1.0, 0.0).normalize();
    assert!(!actual.abs_diff_eq(naive, 1e-3));
}

/// A refresh takes each batch's mark, so a frame that changed nothing owes the
/// GPU nothing — and a frame that changed one kind owes only that kind.
///
/// The claim the whole design rests on, and the one that would break silently:
/// every mark left behind re-flattens and re-uploads a list nobody touched, on
/// every frame, for the rest of the run. Nothing would look wrong.
#[test]
fn a_refresh_owes_the_gpu_only_what_was_written_to() {
    let mut scene = Scene::default();
    scene.objects.push(Object::new(Mesh::cube(2.0)));
    scene.curves.push(Curve::segment(Vec3::ZERO, Vec3::X));
    scene.rings.push(Ring::new(Vec3::ZERO, 1.0, Vec3::Y));
    scene.points.push(Point::new(Vec3::X));
    let mut renderer = Renderer::new(scene);

    // Everything was written to build it, so everything is owed once. Asking
    // takes the mark, which is what the second refresh below then relies on.
    renderer.refresh(1.0);
    let cpu = &mut renderer.cpu;
    assert!(cpu.meshes.take_dirty());
    assert_eq!(cpu.curves.ordinary_to_upload().map(<[_]>::len), Some(1));
    assert_eq!(cpu.rings.ordinary_to_upload().map(<[_]>::len), Some(1));
    assert_eq!(cpu.points.ordinary_to_upload().map(<[_]>::len), Some(1));
    // Empty, and owed anyway: a pass left holding what was lit last time would
    // go on drawing it.
    assert_eq!(cpu.curves.lit_to_upload().map(<[_]>::len), Some(0));

    // And nothing twice. A still frame is the common case, not the odd one.
    renderer.refresh(1.0);
    let cpu = &mut renderer.cpu;
    assert!(!cpu.meshes.take_dirty());
    assert!(cpu.curves.ordinary_to_upload().is_none());
    assert!(cpu.rings.ordinary_to_upload().is_none());
    assert!(cpu.points.ordinary_to_upload().is_none());
    assert!(cpu.curves.lit_to_upload().is_none(), "nothing was relit");

    // One kind written, one kind owed. This is what `scene_mut` costs: reaching
    // for the whole scene and adding a stroke is a stroke's worth of work, and
    // the solids beside it are not re-flattened.
    renderer
        .scene_mut()
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::Y));
    renderer.refresh(1.0);
    let cpu = &mut renderer.cpu;
    assert_eq!(cpu.curves.ordinary_to_upload().map(<[_]>::len), Some(2));
    assert!(
        !cpu.meshes.take_dirty(),
        "adding a stroke asked for every mesh to be flattened again"
    );
    assert!(cpu.rings.ordinary_to_upload().is_none());
    assert!(cpu.points.ordinary_to_upload().is_none());
}

#[test]
fn flatten_of_an_empty_scene_uploads_nothing() {
    let mut renderer = Renderer::new(Scene::default());
    renderer.refresh(1.0);

    let cpu = &renderer.cpu;
    assert!(cpu.meshes.vertices.is_empty());
    assert!(cpu.meshes.indices.is_empty());
    assert!(cpu.curves.ordinary.is_empty());
    assert!(cpu.points.ordinary.is_empty());
}

#[test]
fn flatten_curves_ships_one_instance_per_segment() {
    let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0));
    let mut scene = Scene::default();
    scene.curves.push(
        Curve::new(vec![a, b, c])
            .colored(Vec3::new(0.25, 0.5, 0.75))
            .width(3.0)
            .z_offset(64),
    );
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    let records = &renderer.cpu.curves.ordinary;

    // Three points, two segments, one record each — the four corners are the
    // shader's business now.
    assert_eq!(records.len(), 2);

    // Both ends travel so the shader can take the ribbon's direction from
    // their difference, and half the authored width rides along.
    assert_eq!(records[0].start, a.to_array());
    assert_eq!(records[0].end, b.to_array());
    assert_eq!(records[1].start, b.to_array());
    assert_eq!(records[1].end, c.to_array());
    assert!(records.iter().all(|i| i.look.half_extent == 1.5));
    assert!(records.iter().all(|i| i.look.color == [0.25, 0.5, 0.75]));
    // The bias is the segment's, not a corner's: a ribbon tilted in depth
    // against itself would z-fight along its own length.
    assert!(records.iter().all(|i| i.look.z_offset == 64.0));
    // No plane named, so the shader gets all-zero and falls back to reading
    // depth off the centreline.
    assert!(records.iter().all(|i| i.plane == [0.0; 3]));
}

/// The corner layout the shaders reconstruct, kept honest from the Rust side:
/// `QUAD_INDICES` is what `@builtin(vertex_index)` delivers, and each shader
/// derives its corner from that number alone.
#[test]
fn the_shared_quad_covers_itself_without_overlapping() {
    assert_eq!(QUAD_INDICES, [0, 1, 2, 2, 1, 3]);
    // Two triangles, each corner used, and the shared edge running 1–2.
    let mut used = QUAD_INDICES.to_vec();
    used.sort_unstable();
    used.dedup();
    assert_eq!(used, [0, 1, 2, 3]);

    // `point_vs` reads x off bit 0 and y off bit 1, which has to reproduce
    // the ±1 square the markers used to carry per corner.
    let corners: Vec<[f32; 2]> = (0..4u32)
        .map(|index| {
            [
                if index & 1 != 0 { 1.0 } else { -1.0 },
                if index & 2 != 0 { 1.0 } else { -1.0 },
            ]
        })
        .collect();
    assert_eq!(
        corners,
        [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]]
    );

    // `curve_vs` puts corners 0 and 1 at `start` and 2 and 3 at `end`, with
    // the sides inverting across the middle so each pair holds one edge.
    let sides: Vec<(bool, f32)> = (0..4u32)
        .map(|index| {
            (
                index >= 2,
                if index == 1 || index == 2 { -1.0 } else { 1.0 },
            )
        })
        .collect();
    assert_eq!(
        sides,
        [(false, 1.0), (false, -1.0), (true, -1.0), (true, 1.0)]
    );
}

#[test]
fn flatten_curves_normalizes_and_spreads_a_named_plane() {
    let mut scene = Scene::default();
    // Deliberately not unit length: the shader tests `dot(n, n) > 0.5` to
    // decide a plane was named at all, so a stray magnitude would both skew
    // the gradient and risk reading as "no plane".
    scene.curves.push(
        Curve::segment(Vec3::ZERO, Vec3::X)
            .in_plane(Vec3::new(0.0, 5.0, 0.0))
            .z_offset(32),
    );
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    let records = &renderer.cpu.curves.ordinary;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].plane, [0.0, 1.0, 0.0], "{records:?}");
}

#[test]
fn flatten_curves_strokes_the_closing_segment_too() {
    let corners = vec![
        Vec3::ZERO,
        Vec3::X,
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let mut scene = Scene::default();
    scene.curves.push(Curve::new(corners.clone()).closed());
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    let closed = &renderer.cpu.curves.ordinary;
    // Four corners closed is four segments; open would be three.
    assert_eq!(closed.len(), 4);
    // The closing segment runs from the last point back to the first.
    assert_eq!(closed[3].start, corners[3].to_array());
    assert_eq!(closed[3].end, corners[0].to_array());

    let mut scene = Scene::default();
    scene.curves.push(Curve::new(corners));
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    assert_eq!(renderer.cpu.curves.ordinary.len(), 3);
}

/// The records are held between frames now, so refilling them has to leave no
/// trace of what they held before.
///
/// Shrinking is the case that would show it: a buffer that only ever grew would
/// pass on a stale tail nobody cleared. Both directions are checked, and both on
/// records that are refilled rather than rebuilt.
#[test]
fn refilled_records_hold_only_what_the_scene_holds_now() {
    let mut scene = Scene::default();
    for i in 0..4u64 {
        scene
            .curves
            .push(Curve::segment(Vec3::X * i as f32, Vec3::Y).tagged(Tag::new(i)));
    }
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    assert_eq!(renderer.cpu.curves.ordinary.len(), 4);
    let grown = renderer.cpu.curves.ordinary.capacity();

    // Down to one: the other three must be gone, not merely overwritten.
    renderer.scene_mut().curves.truncate(1);
    renderer.refresh(1.0);
    assert_eq!(renderer.cpu.curves.ordinary.len(), 1);
    assert_eq!(
        renderer.cpu.curves.ordinary[0].start,
        Vec3::ZERO.to_array(),
        "the surviving instance is the surviving curve's"
    );
    assert_eq!(
        renderer.cpu.curves.ordinary.capacity(),
        grown,
        "the room it grew to is the point of holding it"
    );

    // And the `lit` records, which are what a hover refills every frame.
    renderer.highlight_only(Lit {
        tag: Tag::new(0),
        look: Highlight::new(Vec3::Y),
    });
    renderer.refresh(1.0);
    assert_eq!(renderer.cpu.curves.lit.len(), 1);
    renderer.clear_highlights();
    renderer.refresh(1.0);
    assert!(
        renderer.cpu.curves.lit.is_empty(),
        "unlighting has to empty what lighting filled"
    );
}

/// The plane probes step a share of the viewport, and where that share comes
/// from is the one thing the two projections disagree on.
#[test]
fn the_probe_reach_takes_its_scale_from_whatever_the_projection_left_out() {
    let mut camera = Camera {
        distance: 5.0,
        ..Camera::default()
    };

    // Perspective clip `w` is the view depth, so the share rides on it
    // already and the reach is the bare fraction.
    camera.projection = Projection::Perspective;
    assert_eq!(Uniforms::probe_reach(&camera), 0.25);

    // Orthographic `w` is a constant 1 that says nothing about scale, so the
    // orbit distance has to stand in for it — which makes this the one that
    // follows a dolly.
    camera.projection = Projection::Orthographic;
    assert_eq!(Uniforms::probe_reach(&camera), 0.25 * 5.0);
    camera.distance = 20.0;
    assert_eq!(Uniforms::probe_reach(&camera), 0.25 * 20.0);
}

/// A highlight doubles the primitive it names — same geometry, different look
/// — and touches nothing else.
#[test]
fn a_highlight_repeats_only_what_its_tag_names() {
    let mut scene = Scene::default();
    scene.curves.push(
        Curve::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y])
            .width(2.0)
            .z_offset(10)
            .tagged(Tag::new(1)),
    );
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::Z).tagged(Tag::new(2)));
    scene.rings.push(
        Ring::new(Vec3::ZERO, 1.0, Vec3::Y)
            .width(3.0)
            .tagged(Tag::new(1)),
    );
    scene.points.push(Point::new(Vec3::X).tagged(Tag::new(2)));
    let mut renderer = Renderer::new(scene);

    // Nothing named, nothing doubled.
    renderer.refresh(1.0);
    let cpu = &renderer.cpu;
    assert!(cpu.curves.lit.is_empty() && cpu.rings.lit.is_empty() && cpu.points.lit.is_empty());

    let look = Highlight::new(Vec3::new(1.0, 0.0, 0.0)).scale(3.0).lift(64);
    renderer.highlight_only(Lit {
        tag: Tag::new(1),
        look,
    });
    renderer.refresh(1.0);
    let cpu = &renderer.cpu;

    // Tag 1 is the three-point curve and the ring: two segments and one rim.
    // The curve tagged 2 and the marker tagged 2 are left alone.
    assert_eq!(cpu.curves.lit.len(), 2);
    assert_eq!(cpu.rings.lit.len(), 1);
    assert!(cpu.points.lit.is_empty());

    // The look replaces the colour, multiplies the width, and adds to the
    // bias rather than replacing it — a highlight has to clear the lift the
    // primitive already carried.
    assert!(
        cpu.curves
            .lit
            .iter()
            .all(|i| i.look.color == [1.0, 0.0, 0.0])
    );
    assert!(cpu.curves.lit.iter().all(|i| i.look.half_extent == 3.0)); // 2.0/2 × 3
    assert!(cpu.curves.lit.iter().all(|i| i.look.z_offset == 74.0)); // 10 + 64
    assert_eq!(cpu.rings.lit[0].look.half_extent, 4.5); // 3.0/2 × 3
    assert_eq!(cpu.rings.lit[0].look.z_offset, 64.0);

    // The geometry is the primitive's own, untouched. Copied out first: the
    // records are held on the renderer now, so flattening another one needs
    // it back.
    let doubled = cpu.curves.lit[0];
    renderer.refresh(1.0);
    let plain = &renderer.cpu.curves.ordinary;
    assert_eq!(doubled.start, plain[0].start);
    assert_eq!(doubled.end, plain[0].end);

    // Naming a tag again replaces its look rather than stacking a second one,
    // so a hover reads over a selection and both still draw once.
    renderer.highlight_only(Lit {
        tag: Tag::new(1),
        look: Highlight::new(Vec3::Y).scale(1.0).lift(0),
    });
    renderer.refresh(1.0);
    let cpu = &renderer.cpu;
    assert_eq!(cpu.curves.lit.len(), 2, "still doubled once, not twice");
    assert_eq!(cpu.rings.lit[0].look.half_extent, 1.5);
    assert_eq!(cpu.rings.lit[0].look.color, [0.0, 1.0, 0.0]);

    // Lighting one thing alone drops the rest, and clearing drops everything.
    renderer.highlight_only(Lit {
        tag: Tag::new(2),
        look,
    });
    renderer.refresh(1.0);
    let cpu = &renderer.cpu;
    assert!(cpu.curves.lit.len() == 1 && cpu.points.lit.len() == 1 && cpu.rings.lit.is_empty());
    renderer.clear_highlights();
    renderer.refresh(1.0);
    let cpu = &renderer.cpu;
    assert!(cpu.curves.lit.is_empty() && cpu.rings.lit.is_empty() && cpu.points.lit.is_empty());
}

/// Re-asking for a look already in force leaves the records alone, which is
/// what lets a caller drive highlighting straight off a pointer that is not
/// moving.
#[test]
fn re_lighting_what_is_already_lit_dirties_nothing() {
    let mut scene = Scene::default();
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::X).tagged(Tag::new(1)));
    let mut renderer = Renderer::new(scene);
    let lit = Lit {
        tag: Tag::new(1),
        look: Highlight::new(Vec3::Y),
    };

    // `new` starts everything outstanding, so the flag says nothing until it
    // has been cleared once.
    renderer.relight = false;
    renderer.highlight_only(lit);
    assert!(renderer.relight, "the first look is a change");

    renderer.relight = false;
    renderer.highlight_only(lit);
    renderer.highlight_only(lit);
    assert!(!renderer.relight, "neither call changed anything");

    // A different look for the same tag is a change, and so is dropping it.
    renderer.highlight_only(Lit {
        look: Highlight::new(Vec3::X),
        ..lit
    });
    assert!(renderer.relight);
    renderer.relight = false;
    renderer.clear_highlights();
    assert!(renderer.relight);

    // And clearing what is already clear is the same nothing: a pointer over
    // empty space says so every frame it does not move.
    renderer.relight = false;
    renderer.clear_highlights();
    assert!(!renderer.relight, "nothing was lit to drop");
}

/// Emptying the scene's text owes the GPU an empty buffer, not silence.
///
/// The one way a retained renderer draws what nobody asked for: the records
/// outlive the batch they were flattened from, and the buffers behind them go
/// on being drawn for the rest of the run. Nothing looks wrong at the point the
/// mistake is made.
///
/// Text reaches it by a route the other overlays do not. Laying a run out needs
/// a shaper, so the flatten is guarded — and a guard that asked only whether
/// there was anything *to* lay out skipped the clearing along with the work.
#[test]
fn emptying_the_text_owes_the_gpu_an_empty_buffer() {
    let mut scene = Scene::default();
    scene
        .texts
        .push(Text::new(Vec3::ZERO, "125.4", 16.0).tagged(Tag::new(1)));
    let mut renderer = Renderer::new(scene);
    renderer.shape_with(palantir::TextShaper::new());

    renderer.refresh(1.0);
    let drawn = renderer
        .cpu
        .texts
        .records
        .ordinary_to_upload()
        .expect("the first flatten owes the glyphs")
        .len();
    assert!(drawn > 0, "five characters flattened to {drawn} glyphs");

    // Taken away after it was drawn, which is the only way to reach the bug:
    // a scene that never had text has nothing left over to clear.
    renderer.scene_mut().texts.clear();
    renderer.refresh(1.0);
    assert_eq!(
        renderer
            .cpu
            .texts
            .records
            .ordinary_to_upload()
            .map(<[_]>::len),
        Some(0),
        "an emptied batch owes the GPU an empty buffer, not nothing at all",
    );

    // And having said so once, it is quiet again.
    renderer.refresh(1.0);
    assert!(renderer.cpu.texts.records.ordinary_to_upload().is_none());
}

/// What palantir composites into, and so what the pipelines are built against.
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Every pipeline builds — the only thing in this crate that checks the
/// shaders at all.
///
/// `Gpu::new` compiles one WGSL module out of six files and builds five
/// pipelines from it, and the Rust compiler checks none of it: an entry point
/// is found by joining `spec.name` onto `_vs` at run time, the ring band's
/// step count arrives as a pipeline override, and each vertex layout is matched
/// against what the shader declares. All of that fails at device init, which
/// until this test happened only in the application — or in catcad's visual
/// suite, one crate downstream of where the mistake was made.
///
/// Building it *is* the assertion: a bad entry point, a shader that will not
/// compile, or a layout wgpu rejects raises a validation error, which panics.
/// What is checked below is the one thing that is a choice rather than a
/// requirement — which passes were built holding their triangle list and which
/// were left to grow one.
#[test]
fn every_pipeline_builds() {
    let gpu = headless_test_gpu();
    let built = Gpu::new(&gpu.device, TARGET_FORMAT, &GlyphAtlas::default());

    // The overlays are built holding the list they draw every instance
    // through, so it is there before a frame is and never rewritten.
    for (pass, indices) in [
        (&built.curves.ordinary, QUAD_INDICES.len()),
        (&built.points.ordinary, QUAD_INDICES.len()),
        (&built.texts.ordinary, QUAD_INDICES.len()),
        (&built.rings.ordinary, RING_INDICES.len()),
    ] {
        assert_eq!(pass.index_count, indices as u32);
        assert!(pass.indices.buffer().is_some(), "the list was not filled");
        assert!(
            pass.records.buffer().is_none(),
            "a pass allocated records before it had any"
        );
    }

    // Meshes are the one pass whose list changes, so it grows like the records
    // do and there is nothing in it yet.
    assert_eq!(built.meshes.index_count, 0);
    assert!(built.meshes.indices.buffer().is_none());

    // Nothing is drawn until something is uploaded, whichever kind.
    for pass in [
        &built.meshes,
        &built.curves.ordinary,
        &built.curves.lit,
        &built.rings.ordinary,
        &built.rings.lit,
        &built.points.ordinary,
        &built.points.lit,
        &built.texts.ordinary,
        &built.texts.lit,
    ] {
        assert_eq!(pass.instances, 0, "a fresh pass has something in it");
    }
}

/// Where a test frame is drawn. 320 px of RGBA is 1280 bytes, already a
/// multiple of the 256 a texture-to-buffer copy has to align its rows to — so
/// a readback has no padding to drop.
const FRAME: UVec2 = UVec2::new(320, 240);

/// What the offscreen host composites a frame into.
fn frame_target(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("aperture.test.target"),
        size: wgpu::Extent3d {
            width: FRAME.x,
            height: FRAME.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        // `COPY_DST` because the offscreen host always composes into its
        // backbuffer and copies from there, whatever the frame drew.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// How many pixels of the frame are something other than the background.
///
/// The clear is near black — 0.02 linear, which the sRGB target encodes to
/// about 40 — and everything this crate draws is lit well clear of it, so the
/// threshold is a wide gap rather than a tuned one.
fn drawn_pixels(gpu: &HeadlessTestGpuLease, target: &wgpu::Texture) -> usize {
    /// Above the background and far below anything drawn.
    const LIT: u8 = 80;

    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("aperture.test.readback"),
        size: u64::from(FRAME.x * FRAME.y * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("aperture.test.readback"),
        });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(FRAME.x * 4),
                rows_per_image: Some(FRAME.y),
            },
        },
        wgpu::Extent3d {
            width: FRAME.x,
            height: FRAME.y,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.expect("map readback"));
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("device poll");

    let pixels = readback
        .slice(..)
        .get_mapped_range()
        .expect("the readback was mapped");
    let drawn = pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0].max(pixel[1]).max(pixel[2]) > LIT)
        .count();
    drop(pixels);
    readback.unmap();
    drawn
}

/// One kind put into an otherwise empty scene, and what to call it when the
/// frame comes back with nothing in it.
#[derive(Debug)]
struct Staged {
    batch: &'static str,
    stage: fn(&mut Scene),
}

/// Every kind reaches the picture, one kind at a time.
///
/// The failure this is for is a kind that flattens and uploads and is never
/// drawn. `paint` reaches its passes through a hand-written list of nine, and a
/// kind left out of that list uploads exactly as it should and appears nowhere
/// — instance counts and dirty marks all agree, and only the frame disagrees.
/// So this asks the frame.
///
/// One kind at a time, against a baseline of none, because a scene holding all
/// five draws something whichever four of them are broken.
#[test]
fn every_kind_reaches_the_frame() {
    let gpu = headless_test_gpu();
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    let target = frame_target(&gpu.device);
    let mut pane = ScenePane {
        view: Rc::new(RefCell::new(Renderer::new(Scene::default()))),
    };

    // The baseline the rest is measured against: with nothing in the scene the
    // frame is background, so a kind that draws has to move this off zero.
    host.frame_offscreen(&target, 1.0, &mut pane);
    let empty = drawn_pixels(&gpu, &target);
    assert_eq!(
        empty, 0,
        "an empty scene lit {empty} pixels, so nothing below proves anything"
    );

    // Drawn large: what is being asked is whether the pass runs at all, and a
    // kind that reaches the frame as three pixels answers that no better than
    // one that reaches it as three hundred.
    let kinds = [
        Staged {
            batch: "objects",
            stage: |scene| scene.objects.push(Object::new(Mesh::cube(2.0))),
        },
        Staged {
            batch: "curves",
            stage: |scene| {
                scene
                    .curves
                    .push(Curve::segment(Vec3::NEG_X, Vec3::X).width(8.0))
            },
        },
        Staged {
            batch: "rings",
            stage: |scene| {
                scene
                    .rings
                    .push(Ring::new(Vec3::ZERO, 1.0, Vec3::Z).width(8.0))
            },
        },
        Staged {
            batch: "points",
            stage: |scene| scene.points.push(Point::new(Vec3::ZERO).size(32.0)),
        },
        Staged {
            batch: "texts",
            stage: |scene| scene.texts.push(Text::new(Vec3::ZERO, "125.4", 48.0)),
        },
    ];
    for Staged { batch, stage } in kinds {
        {
            let mut view = pane.view.borrow_mut();
            let scene = view.scene_mut();
            scene.objects.clear();
            scene.curves.clear();
            scene.rings.clear();
            scene.points.clear();
            scene.texts.clear();
            stage(scene);
        }
        host.frame_offscreen(&target, 1.0, &mut pane);
        let drawn = drawn_pixels(&gpu, &target);
        assert!(
            drawn > 0,
            "nothing of the {batch} batch reached the frame — it flattened, it \
             uploaded, and no pass drew it"
        );
    }
}

/// One of every kind, through a real device, twice.
///
/// Where [`every_kind_reaches_the_frame`] asks the picture, this asks the
/// buffers: that each kind's records arrive at the pass that draws them, and
/// that a highlight fills the second pass without disturbing the first.
///
/// Painted twice because the second frame is the one that reaches the re-upload
/// path, and because a highlight arriving between them is what fills the `lit`
/// passes that start empty.
#[test]
fn a_frame_uploads_every_kind() {
    let gpu = headless_test_gpu();
    let mut host = OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build();
    let target = frame_target(&gpu.device);

    // One tag over four of the five, so a single highlight has to reach every
    // overlay pass and leave the solids alone.
    let lit = Tag::new(1);
    let mut scene = Scene::default();
    scene.objects.push(Object::new(Mesh::cube(1.0)));
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::X).tagged(lit));
    scene
        .rings
        .push(Ring::new(Vec3::ZERO, 1.0, Vec3::Z).tagged(lit));
    scene.points.push(Point::new(Vec3::ZERO).tagged(lit));
    scene
        .texts
        .push(Text::new(Vec3::ZERO, "125.4", 16.0).tagged(lit));

    let mut pane = ScenePane {
        view: Rc::new(RefCell::new(Renderer::new(scene))),
    };
    host.frame_offscreen(&target, 1.0, &mut pane);

    let glyphs = {
        let view = pane.view.borrow();
        let built = view.gpu.as_ref().expect("init runs before paint");

        // A cube is 24 corners and 36 indices, drawn as one instance of one
        // triangle list.
        assert_eq!(built.meshes.instances, 1);
        assert_eq!(built.meshes.index_count, 36);
        // One record apiece: a segment, a rim, a marker.
        assert_eq!(built.curves.ordinary.instances, 1);
        assert_eq!(built.rings.ordinary.instances, 1);
        assert_eq!(built.points.ordinary.instances, 1);
        // Five characters, and every one of them has ink.
        let glyphs = built.texts.ordinary.instances;
        assert_eq!(glyphs, 5, "one instance per glyph of \"125.4\"");

        // Nothing was lit, so every highlight pass is still empty — and an
        // empty pass draws nothing rather than drawing what it last held.
        for pass in [
            &built.curves.lit,
            &built.rings.lit,
            &built.points.lit,
            &built.texts.lit,
        ] {
            assert_eq!(pass.instances, 0, "something was lit that nothing named");
        }
        glyphs
    };

    // Lit between the frames, which is the only edit — so the second frame
    // rebuilds the highlights and re-uploads nothing else.
    pane.view.borrow_mut().highlight_only(Lit {
        tag: lit,
        look: Highlight::new(Vec3::Y),
    });
    host.frame_offscreen(&target, 1.0, &mut pane);

    let view = pane.view.borrow();
    let built = view.gpu.as_ref().expect("init runs before paint");
    assert_eq!(built.curves.lit.instances, 1);
    assert_eq!(built.rings.lit.instances, 1);
    assert_eq!(built.points.lit.instances, 1);
    assert_eq!(
        built.texts.lit.instances, glyphs,
        "a lit run is the same run, shaped again"
    );
    // The ordinary passes are untouched by a highlight: it doubles what is
    // drawn rather than replacing it.
    assert_eq!(built.curves.ordinary.instances, 1);
    assert_eq!(built.meshes.instances, 1);
}

#[test]
fn a_refresh_takes_the_text_mark_even_when_there_is_nothing_to_lay_out() {
    let mut renderer = Renderer::new(Scene::default());
    renderer.shape_with(palantir::TextShaper::new());

    // Written to and left empty, which is what a caller refilling a batch from
    // an arena that turned out to hold nothing does.
    renderer.scene_mut().texts.mark();
    renderer.refresh(1.0);
    assert!(
        !renderer.scene_mut().texts.take_dirty(),
        "the mark outlived the refresh that had nothing to do with it"
    );
}
