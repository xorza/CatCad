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
use crate::text::turn::{Facing, Turn};
use crate::viewport::Viewport;
use glam::{IVec2, Mat4, UVec2, Vec2, Vec3};
use palantir::OffscreenHost;
use palantir::internals::{HeadlessTestGpuLease, headless_test_gpu};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn flatten_bakes_transforms_into_world_space() {
    let mut scene = Scene::default();
    scene.solids.push(Object::new(Mesh::cube(2.0)));
    scene.solids.push(
        Object::new(Mesh::cube(2.0))
            .at(Vec3::new(10.0, 0.0, 0.0))
            .colored(Vec3::new(1.0, 0.0, 0.0)),
    );
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    let triangles = &renderer.cpu.solids;

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
    scene.solids.push(Object {
        transform: Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0)),
        color: Vec3::ZERO,
        ..Object::new(mesh)
    });
    let mut renderer = Renderer::new(scene);
    renderer.refresh(1.0);
    let triangles = &renderer.cpu.solids;

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

/// The middle pixel of the frame, RGB as the target holds it — which is
/// sRGB-encoded, the pass having written linear colour into an sRGB target.
///
/// The middle because that is where a test puts the thing it is asking about,
/// and one pixel because what these ask is what colour came out rather than how
/// much of it there was. Fully covered, so the resolve has nothing to average.
fn middle_pixel(gpu: &HeadlessTestGpuLease, target: &wgpu::Texture) -> [i32; 3] {
    let pixels = frame_pixels(gpu, target);
    let at = ((FRAME.y / 2 * FRAME.x + FRAME.x / 2) * 4) as usize;
    [
        i32::from(pixels[at]),
        i32::from(pixels[at + 1]),
        i32::from(pixels[at + 2]),
    ]
}

/// A quad facing the camera, big enough to cover the middle of the frame
/// and small enough to stay inside it.
fn facing_quad() -> Mesh {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    Mesh {
        vertices: corners
            .map(|(x, y)| Vertex {
                position: Vec3::new(x, y, 0.0),
                normal: Vec3::Z,
            })
            .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// A lift brightens what an object is drawn in, and an ink replaces it.
///
/// The distinction a control depends on. A datum's axes say *which axis they
/// are* by their colour, so a hover cannot spend it — where for a sketch entity
/// spending it is the entire point, since what a selection means is that
/// everything in it reads alike.
///
/// Asked of the flattened vertices rather than of [`Tint::over`], which is what
/// makes it a test of anything: the question is whether the mesh path reaches
/// for the tint at all, and a flatten that wrote the object's colour straight
/// through would pass every test of the tint by itself.
#[test]
fn a_lifted_highlight_brightens_what_an_object_is_drawn_in_and_an_inked_one_replaces_it() {
    const OWN: Vec3 = Vec3::new(0.4, 0.1, 0.2);
    /// Ink sharing no channel with [`OWN`] and with none at zero, so a flatten
    /// that multiplied the two where it should replace cannot come out right by
    /// accident.
    const INK: Vec3 = Vec3::new(1.0, 0.8, 0.2);

    let mut scene = Scene::default();
    scene
        .solids
        .push(Object::new(facing_quad()).colored(OWN).tagged(Tag::new(1)));
    let mut renderer = Renderer::new(scene);

    // Unlit first, so what the lift is measured against is what the flatten
    // actually wrote rather than what the object was built with.
    renderer.refresh(1.0);
    let flat = |renderer: &Renderer| renderer.cpu.solids.vertices[0].color;
    assert_eq!(flat(&renderer), OWN.to_array());

    // Twice as bright, channel for channel, so the hue is untouched and only
    // the brightness moved. Doubling is exact in binary, so this is an equality
    // rather than a tolerance.
    renderer.highlight_only(Lit {
        tag: Tag::new(1),
        look: Highlight::lifted(2.0),
    });
    renderer.refresh(1.0);
    assert_eq!(
        flat(&renderer),
        [0.8, 0.2, 0.4],
        "a lift recoloured the object instead of brightening it"
    );

    renderer.highlight_only(Lit {
        tag: Tag::new(1),
        look: Highlight::new(INK),
    });
    renderer.refresh(1.0);
    assert_eq!(
        flat(&renderer),
        INK.to_array(),
        "an ink was blended with what it should have replaced"
    );
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
    scene.solids.push(Object::new(Mesh::cube(2.0)));
    scene.curves.push(Curve::segment(Vec3::ZERO, Vec3::X));
    scene.rings.push(Ring::new(Vec3::ZERO, 1.0, Vec3::Y));
    scene.points.push(Point::new(Vec3::X));
    let mut renderer = Renderer::new(scene);

    // Everything was written to build it, so everything is owed once. Asking
    // takes the mark, which is what the second refresh below then relies on.
    renderer.refresh(1.0);
    let cpu = &mut renderer.cpu;
    assert!(
        cpu.solids.vertices_to_upload().is_some() && cpu.solids.indices_to_upload().is_some(),
        "the first flatten owes both"
    );
    assert_eq!(cpu.curves.ordinary_to_upload().map(<[_]>::len), Some(1));
    assert_eq!(cpu.rings.ordinary_to_upload().map(<[_]>::len), Some(1));
    assert_eq!(cpu.points.ordinary_to_upload().map(<[_]>::len), Some(1));
    // Empty, and owed anyway: a pass left holding what was lit last time would
    // go on drawing it.
    assert_eq!(cpu.curves.lit_to_upload().map(<[_]>::len), Some(0));

    // And nothing twice. A still frame is the common case, not the odd one.
    renderer.refresh(1.0);
    let cpu = &mut renderer.cpu;
    assert!(cpu.solids.vertices_to_upload().is_none() && cpu.solids.indices_to_upload().is_none());
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
        cpu.solids.vertices_to_upload().is_none(),
        "adding a stroke asked for every mesh to be flattened again"
    );
    assert!(cpu.rings.ordinary_to_upload().is_none());
    assert!(cpu.points.ordinary_to_upload().is_none());

    // A relight owes an untagged mesh nothing at all. Nothing can light a solid
    // the caller never named, so a pointer crossing the drawing in front of the
    // model must not rewrite one triangle of it — which taking `relight` at its
    // word did, on every frame the pointer moved.
    renderer.highlight_only(Lit {
        tag: Tag::new(1),
        look: Highlight::new(Vec3::Y),
    });
    renderer.refresh(1.0);
    assert!(
        renderer.cpu.solids.vertices_to_upload().is_none(),
        "a relight rewrote a mesh that nothing can light"
    );

    // Name one, and the same relight owes its vertices — a mesh carries its
    // colour in them — and still owes no index, because an index says which
    // vertex and nothing about how it looks.
    renderer
        .scene_mut()
        .solids
        .push(Object::new(Mesh::cube(1.0)).tagged(Tag::new(1)));
    renderer.refresh(1.0);
    let solids = &mut renderer.cpu.solids;
    assert!(
        solids.vertices_to_upload().is_some() && solids.indices_to_upload().is_some(),
        "the push moved geometry"
    );
    renderer.highlight_only(Lit {
        tag: Tag::new(1),
        look: Highlight::new(Vec3::X),
    });
    renderer.refresh(1.0);
    let solids = &mut renderer.cpu.solids;
    assert!(
        solids.vertices_to_upload().is_some(),
        "a relit mesh keeps its old colour"
    );
    assert!(
        solids.indices_to_upload().is_none(),
        "a colour change re-uploaded indices that cannot carry one"
    );

    // And dropping the highlight owes the vertices once more, to take the
    // colour back off. This is the half a batch cannot learn from the new set
    // alone — it no longer names the object at all.
    renderer.highlight_all(&[]);
    renderer.refresh(1.0);
    assert!(
        renderer.cpu.solids.vertices_to_upload().is_some(),
        "an unlit mesh kept the colour it had just lost"
    );

    // Settled again, and now nothing is lit on either side, so the next relight
    // is refused as the first one was.
    renderer.highlight_only(Lit {
        tag: Tag::new(404),
        look: Highlight::new(Vec3::Z),
    });
    renderer.refresh(1.0);
    assert!(
        renderer.cpu.solids.vertices_to_upload().is_none(),
        "a relight naming nothing in the batch still rewrote it"
    );
}

/// A resort owes the indices and leaves the corners alone.
///
/// The faces are drawn back to front — see [`Order::BackToFront`] — so a camera
/// turning over a drawing reorders them on most frames of an orbit. Nothing
/// about the corners has changed on such a frame: they are where they were, in
/// the colour they were. What has changed is the order to read them in, and an
/// index list is the whole of what says that.
///
/// Two sheets a plane apart, seen from one side and then the other, which is the
/// smallest scene whose order can flip. Both halves are compared against
/// themselves across the turn, because the assertions about what is *owed* would
/// all pass on a refresh that had quietly stopped sorting.
#[test]
fn a_resort_owes_the_indices_and_leaves_the_corners_alone() {
    let mut scene = Scene::default();
    scene
        .faces
        .push(Object::new(Mesh::cube(1.0)).at(Vec3::Z * -4.0));
    scene
        .faces
        .push(Object::new(Mesh::cube(1.0)).at(Vec3::Z * 4.0));
    let mut renderer = Renderer::new(scene);
    // Down −Z from well back, so both sheets are in front of the eye and the
    // nearer one is the one at +4.
    *renderer.camera_mut() = Camera {
        target: Vec3::ZERO,
        distance: 20.0,
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    };

    renderer.refresh(1.0);
    let faces = &mut renderer.cpu.faces;
    assert!(
        faces.vertices_to_upload().is_some() && faces.indices_to_upload().is_some(),
        "the first flatten owes both"
    );
    let before = renderer.cpu.faces.indices.clone();
    let corners: Vec<[f32; 3]> = renderer
        .cpu
        .faces
        .vertices
        .iter()
        .map(|at| at.position)
        .collect();

    // Round to the other side. Which sheet is furthest has swapped, and
    // nothing else about the scene has moved at all.
    renderer.camera_mut().yaw = std::f32::consts::PI;
    renderer.refresh(1.0);
    let faces = &mut renderer.cpu.faces;
    assert!(
        faces.indices_to_upload().is_some(),
        "the order flipped and nothing said so"
    );
    assert!(
        faces.vertices_to_upload().is_none(),
        "a resort rewrote corners that had not moved"
    );
    assert_ne!(
        renderer.cpu.faces.indices, before,
        "the order did not actually flip, so this test proves nothing"
    );
    // The first cube's corners are still the first twenty-four, which is what
    // lets an index rebased on `bases` mean anything.
    let after: Vec<[f32; 3]> = renderer
        .cpu
        .faces
        .vertices
        .iter()
        .map(|at| at.position)
        .collect();
    assert_eq!(after, corners, "a resort moved the corners themselves");

    // And turning back owes the indices again rather than settling into
    // whichever order was reached last.
    renderer.camera_mut().yaw = 0.0;
    renderer.refresh(1.0);
    let faces = &mut renderer.cpu.faces;
    assert!(faces.indices_to_upload().is_some() && faces.vertices_to_upload().is_none());
    assert_eq!(renderer.cpu.faces.indices, before);
}

#[test]
fn flatten_of_an_empty_scene_uploads_nothing() {
    let mut renderer = Renderer::new(Scene::default());
    renderer.refresh(1.0);

    let cpu = &renderer.cpu;
    assert!(cpu.solids.vertices.is_empty());
    assert!(cpu.solids.indices.is_empty());
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
            .width(3.0),
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
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::X).in_plane(Vec3::new(0.0, 5.0, 0.0)));
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
    renderer.highlight_all(&[]);
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

    let look = Highlight::new(Vec3::new(1.0, 0.0, 0.0)).scale(3.0);
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
    assert!(
        cpu.curves
            .lit
            .iter()
            .all(|i| i.look.color == [1.0, 0.0, 0.0])
    );
    assert!(cpu.curves.lit.iter().all(|i| i.look.half_extent == 3.0)); // 2.0/2 × 3
    assert_eq!(cpu.rings.lit[0].look.half_extent, 4.5); // 3.0/2 × 3

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
        look: Highlight::new(Vec3::Y).scale(1.0),
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
    renderer.highlight_all(&[]);
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
    renderer.highlight_all(&[]);
    assert!(renderer.relight);

    // And clearing what is already clear is the same nothing: a pointer over
    // empty space says so every frame it does not move.
    renderer.relight = false;
    renderer.highlight_all(&[]);
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
    assert_eq!(built.solids.index_count, 0);
    assert!(built.solids.indices.buffer().is_none());

    // Nothing is drawn until something is uploaded, whichever kind.
    for pass in [
        &built.solids,
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
    drawn_ink(gpu, target).count
}

/// Where the drawn pixels are, and how many — the same readback, asked the
/// fuller question.
#[derive(Debug)]
struct Ink {
    count: usize,
    min: UVec2,
    max: UVec2,
}

/// The whole frame, RGBA a byte a channel, as the target holds it — which is
/// sRGB-encoded, the pass having written linear colour into an sRGB target.
///
/// Its own function because two questions are asked of it: how much was drawn,
/// and what colour a given pixel came out. Both are one readback, and neither
/// wants the other's answer.
fn frame_pixels(gpu: &HeadlessTestGpuLease, target: &wgpu::Texture) -> Vec<u8> {
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

    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .expect("the readback was mapped");
    let pixels = mapped.to_vec();
    drop(mapped);
    readback.unmap();
    pixels
}

fn drawn_ink(gpu: &HeadlessTestGpuLease, target: &wgpu::Texture) -> Ink {
    /// Above the background and far below anything drawn.
    const LIT: u8 = 80;

    let mut ink = Ink {
        count: 0,
        min: UVec2::splat(u32::MAX),
        max: UVec2::ZERO,
    };
    for (at, pixel) in frame_pixels(gpu, target).chunks_exact(4).enumerate() {
        if pixel[0].max(pixel[1]).max(pixel[2]) <= LIT {
            continue;
        }
        let at = UVec2::new(at as u32 % FRAME.x, at as u32 / FRAME.x);
        ink.count += 1;
        ink.min = ink.min.min(at);
        ink.max = ink.max.max(at);
    }
    ink
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
/// six draws something whichever five of them are broken.
#[test]
fn every_kind_reaches_the_frame() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(&gpu, Camera::default());

    // The baseline the rest is measured against: with nothing in the scene the
    // frame is background, so a kind that draws has to move this off zero.
    view.paint(1.0);
    let empty = view.drawn();
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
            stage: |scene| scene.solids.push(Object::new(Mesh::cube(2.0))),
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
            batch: "gizmos",
            stage: |scene| {
                scene
                    .gizmos
                    .push(Curve::segment(Vec3::NEG_Y, Vec3::Y).width(8.0))
            },
        },
        Staged {
            batch: "texts",
            stage: |scene| scene.texts.push(Text::new(Vec3::ZERO, "125.4", 48.0)),
        },
    ];
    for Staged { batch, stage } in kinds {
        view.edit(|scene| {
            scene.clear();
            stage(scene);
        });
        view.paint(1.0);
        let drawn = view.drawn();
        assert!(
            drawn > 0,
            "nothing of the {batch} batch reached the frame — it flattened, it \
             uploaded, and no pass drew it"
        );
    }
}

/// A headless view a frame is painted into, and what that frame inked.
///
/// Every test here that asks the *picture* rather than the buffers wants the
/// same four things — a device, a host, a target and a pane — and they were the
/// same fourteen lines in each. What varies is the camera it is set up with and
/// what is put in front of it.
///
/// One pane for the life of it, which is not a saving but a requirement: the
/// host initialises the view it is first given, and a second [`Renderer`] handed
/// to the same host has never been through that. So a drawing is changed by
/// rewriting the scene rather than by standing up another view over it.
#[derive(Debug)]
struct Framed<'a> {
    gpu: &'a HeadlessTestGpuLease,
    host: OffscreenHost,
    target: wgpu::Texture,
    pane: ScenePane,
}

impl<'a> Framed<'a> {
    fn new(gpu: &'a HeadlessTestGpuLease, camera: Camera) -> Self {
        let pane = ScenePane {
            view: Rc::new(RefCell::new(Renderer::new(Scene::default()))),
        };
        *pane.view.borrow_mut().camera_mut() = camera;
        Self {
            host: OffscreenHost::builder(gpu.device.clone(), gpu.queue.clone()).build(),
            target: frame_target(&gpu.device),
            pane,
            gpu,
        }
    }

    /// Rewrite what stands in front of the camera.
    fn edit(&mut self, staging: impl FnOnce(&mut Scene)) {
        let mut view = self.pane.view.borrow_mut();
        staging(view.scene_mut());
    }

    /// Paint one frame, on a display of `scale` physical pixels to the logical
    /// one — which leaves the target and the framing alone and changes only what
    /// a logical pixel is worth.
    fn paint(&mut self, scale: f32) {
        self.host
            .frame_offscreen(&self.target, scale, &mut self.pane);
    }

    /// How much of the frame it last painted is something other than the
    /// background.
    fn drawn(&self) -> usize {
        drawn_pixels(self.gpu, &self.target)
    }

    /// What colour the middle of that frame came out.
    fn middle(&self) -> [i32; 3] {
        middle_pixel(self.gpu, &self.target)
    }

    /// What `text` inks, as the only thing in the scene, at one physical pixel
    /// to the logical one.
    ///
    /// A whole run rather than a facing, because the lettering tests vary the
    /// facing, the turn and the anchor between them, and a [`Text`] is the one
    /// thing that carries all three.
    fn ink(&mut self, text: Text) -> Ink {
        self.ink_at(text, 1.0)
    }

    /// The same, seen at `scale` physical pixels to the logical one.
    fn ink_at(&mut self, text: Text, scale: f32) -> Ink {
        self.edit(|scene| {
            scene.texts.clear();
            scene.texts.push(text);
        });
        self.paint(scale);
        drawn_ink(self.gpu, &self.target)
    }
}

/// Squared to the view, which every laid-run test below looks through.
fn square_on() -> Camera {
    Camera {
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    }
}

/// The run those tests lay in a plane, before it is told which.
fn run() -> Text {
    Text::new(Vec3::ZERO, "1234", 24.0)
}

/// A run laid in the plane the camera faces is the screen run, turned.
///
/// **What says the shader and [`Turn::axes`] read one rule.** The Rust side is
/// pinned against hand-computed boxes in `text::tests`, and that pins picking;
/// a vertex shader that built the axes differently would flatten, upload and
/// draw exactly as it should, and only the picture would disagree — so the
/// picture is what this asks.
///
/// Face-on throughout, which is what makes every box below arithmetic: with
/// nothing foreshortened, laying a run in a plane can only turn it, and the
/// three claims are that it turns by exactly as much as the plane does. What a
/// *raked* plane does to it is the test after this one.
///
/// Squared to the view, so world +x runs across the screen and +y up it. Small
/// enough type that a quarter turn about the anchor still lands inside the
/// frame, since ink clipped at an edge is a box that agrees with nothing.
#[test]
fn a_run_laid_in_the_plane_it_faces_is_the_screen_run_turned() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(&gpu, square_on());
    let mut ink_of = |facing| view.ink(run().facing(facing));

    let flat = ink_of(Facing::Screen { on: None });
    assert!(flat.count > 0, "nothing was drawn to compare against");

    // **The constant-size claim outright.** A laid run is built in the world out
    // of a size in pixels, and the depth is the whole of what the shader
    // multiplies by — it takes `at.w * world_per_logical_px` — so one that dropped
    // the `w` would size the run by the orbit distance and land nowhere near.
    // Set along the plane's +x that round trip is the identity; along −x the
    // readable-side rule brings it back to the same place rather than hanging it
    // off the other side of the anchor, which is where the projection alone
    // would have put it.
    for right in [Vec3::X, Vec3::NEG_X] {
        let same = ink_of(Facing::Turned(Turn::new(right, Vec3::Z)));
        let (min, max) = (same.min.as_ivec2(), same.max.as_ivec2());
        let (was_min, was_max) = (flat.min.as_ivec2(), flat.max.as_ivec2());
        assert!(
            (min - was_min).abs().max_element() <= 1 && (max - was_max).abs().max_element() <= 1,
            "{right:?} drew the run at {min:?}..{max:?}, and unturned it is at \
             {was_min:?}..{was_max:?}"
        );
    }

    // Along +y the run advances *up* the screen. The world origin is the orbit
    // target, so its anchor is the middle pixel exactly, and a quarter turn
    // about it sends every corner (x, y) to (y, −x) — which is the box's top
    // edge becoming its left one, and its width becoming its height. Four
    // relations rather than the two the sizes come to, because a box of the
    // right shape in the wrong place would pass those.
    let up = ink_of(Facing::Turned(Turn::new(Vec3::Y, Vec3::Z)));
    let anchor = IVec2::new(FRAME.x as i32, FRAME.y as i32) / 2;
    let (was_min, was_max) = (flat.min.as_ivec2() - anchor, flat.max.as_ivec2() - anchor);
    let (min, max) = (up.min.as_ivec2() - anchor, up.max.as_ivec2() - anchor);
    for (got, want, edge) in [
        (min.x, was_min.y, "left"),
        (max.x, was_max.y, "right"),
        (min.y, -was_max.x, "top"),
        (max.y, -was_min.x, "bottom"),
    ] {
        assert!(
            (got - want).abs() <= 2,
            "the turned run's {edge} edge is at {got} where a quarter turn of \
             {was_min:?}..{was_max:?} puts it at {want}"
        );
    }
}

/// A run laid in a raked plane foreshortens with it.
///
/// The half only a plane turned away from the viewer can show, where the test
/// above holds everything face-on. Sixty degrees about the run's own advance
/// leaves the advance in the screen and tilts the box's down out of it, so the
/// box keeps its width and halves its height about the anchor.
///
/// **Parallel rays rather than perspective**, for the exactness: with no
/// foreshortening of its own to add, the cosine is the whole of the answer and
/// the numbers below are hand-computed rather than fitted.
#[test]
fn a_run_laid_in_a_raked_plane_foreshortens_with_it() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(
        &gpu,
        Camera {
            projection: Projection::Orthographic,
            ..square_on()
        },
    );
    let mut ink_of = |turn| view.ink(run().facing(Facing::Turned(turn)));

    // The plane raked about +x, which is the run's own advance.
    let (sin, cos) = 60f32.to_radians().sin_cos();
    let square = ink_of(Turn::new(Vec3::X, Vec3::Z));
    assert!(square.count > 0, "nothing was drawn to compare against");
    let raked = ink_of(Turn::new(Vec3::X, Vec3::new(0.0, sin, cos)));

    // Measured from the anchor, which the projection puts on the middle pixel.
    let anchor = IVec2::new(FRAME.x as i32, FRAME.y as i32) / 2;
    let (was_min, was_max) = (
        square.min.as_ivec2() - anchor,
        square.max.as_ivec2() - anchor,
    );
    let (min, max) = (raked.min.as_ivec2() - anchor, raked.max.as_ivec2() - anchor);
    assert_eq!(
        (min.x, max.x),
        (was_min.x, was_max.x),
        "the advance lies in the screen, so raking the plane about it cannot \
         touch how wide the run comes out"
    );
    for (got, was, edge) in [(min.y, was_min.y, "top"), (max.y, was_max.y, "bottom")] {
        let want = was as f32 * cos;
        assert!(
            (got as f32 - want).abs() <= 1.0,
            "the raked run's {edge} edge is {got} from the anchor where the \
             cosine puts it at {want:.2}, square being {was}"
        );
    }
}

/// A lift carries the run off the point it names, and a run that comes round to
/// stay readable only changes direction.
///
/// **The second half is what the lift is for, and it is the one a picture can
/// settle.** Two turns half a degree either side of straight up: the one whose
/// advance leans right is set as it stands, and the one leaning left is turned
/// over so it does not read upside down. Their planes are the same and their
/// authored axes all but identical, so a lift stated against *those* leaves the
/// box where it was — while one stated against the run's own frame would swing
/// it clean across to the other side of the point it names.
///
/// Squared to the view, so world +x runs across the screen and +y up it, and the
/// world origin lands on the middle pixel. Centred on its anchor, because that
/// is the arrangement catcad's marks use and the one the invariance is stated
/// for.
#[test]
fn a_lifted_run_only_changes_direction_when_it_comes_round() {
    let gpu = headless_test_gpu();
    let mut view = Framed::new(&gpu, square_on());
    let mut ink_of = |turn| {
        view.ink(
            run()
                .anchored(Vec2::splat(0.5))
                .facing(Facing::Turned(turn)),
        )
    };

    // Set near enough straight up the screen that the two lean the same way,
    // and either side of the boundary the half turn fires on.
    let upward = |lean: f32| Turn::new(Vec3::new(lean, 1.0, 0.0), Vec3::Z);
    let lift = Vec2::new(0.0, -12.0);
    let sitting = ink_of(upward(0.01));
    assert!(sitting.count > 0, "nothing was drawn to compare against");
    let (one, other) = (
        ink_of(upward(0.01).lifted(lift)),
        ink_of(upward(-0.01).lifted(lift)),
    );

    // The lift carried it, and by the twelve pixels it asked for: the plane's
    // authored down is `z × right`, which for a run set near +y is near −x, so
    // twelve pixels along it is twelve pixels of screen to the right.
    let (was, now) = (sitting.min.as_ivec2(), one.min.as_ivec2());
    assert!(
        (now.x - was.x - 12).abs() <= 1 && (now.y - was.y).abs() <= 1,
        "a lift of {lift:?} moved the run from {was:?} to {now:?}"
    );

    // And coming round left it there. A run whose lift rode in its own frame
    // would be twenty-four pixels away, on the other side of the point.
    let (min, max) = (other.min.as_ivec2(), other.max.as_ivec2());
    let (was_min, was_max) = (one.min.as_ivec2(), one.max.as_ivec2());
    assert!(
        (min - was_min).abs().max_element() <= 2 && (max - was_max).abs().max_element() <= 2,
        "coming round moved the run from {was_min:?}..{was_max:?} to {min:?}..{max:?}"
    );

    // **And the twelve are logical pixels, like the shaping they stand off
    // from.** On a display of two physical pixels to the logical one the box
    // doubles about its anchor, and so must the lift — a lift left in physical
    // pixels would hold at twelve on the target and so at six logical, while
    // `Text::pick` measures the box a full twelve out.
    //
    // Read off the box's *middle*, which is the anchor the run is centred on
    // carried by the lift and nothing else: its edges move with the type size as
    // well, and only one of those two is under test.
    const SCALE: f32 = 2.0;
    let middle = |ink: Ink| (ink.min.as_ivec2() + ink.max.as_ivec2()) / 2;
    let (sat, went) = (
        middle(
            view.ink_at(
                run()
                    .anchored(Vec2::splat(0.5))
                    .facing(Facing::Turned(upward(0.01))),
                SCALE,
            ),
        ),
        middle(
            view.ink_at(
                run()
                    .anchored(Vec2::splat(0.5))
                    .facing(Facing::Turned(upward(0.01).lifted(lift))),
                SCALE,
            ),
        ),
    );
    let want = (-lift.y * SCALE) as i32;
    assert!(
        (went.x - sat.x - want).abs() <= 2 && (went.y - sat.y).abs() <= 2,
        "at {SCALE} physical pixels to the logical one a lift of {lift:?} moved \
         the run from {sat:?} to {went:?}, where it owes {want} across"
    );
}

/// **The box a pick measures is the box the run was drawn in.**
///
/// The one test that asks the two sides about *each other*. Every other run test
/// pins one of them against arithmetic — `text::tests` hand-computes the box a
/// pick opens, and the three above hand-compute where the ink lands — and both
/// were right about their own half while a lift drew at two-thirds of the
/// standoff it was picked at. Nothing compared a box against the pixels.
///
/// Read the ink out of the frame, find the pick box by sweeping cursors, and
/// compare. What is compared is the **middle** of each: a run's box is its line
/// box and its ink is the inked part of that, so the two differ by the ascent
/// and descent whatever else is right, and only where the box *sits* is a claim
/// about agreement. Edges are asserted as containment, which is the weaker half
/// and catches a box the wrong size.
///
/// Swept over the raster scale above all, because every factor between logical
/// and physical pixels is invisible at 1 — and over a raked plane, both
/// projections and both ways of anchoring, since each is a rule the shader and
/// `Turn::axes` implement separately.
#[test]
fn a_run_is_picked_over_the_pixels_it_was_drawn_on() {
    let gpu = headless_test_gpu();
    for (plane, camera) in [
        ("square", square_on()),
        (
            "raked",
            Camera {
                yaw: 0.6,
                pitch: -0.7,
                ..square_on()
            },
        ),
        (
            "raked flat",
            Camera {
                yaw: 0.6,
                pitch: -0.7,
                projection: Projection::Orthographic,
                ..square_on()
            },
        ),
    ] {
        let mut view = Framed::new(&gpu, camera);
        // Centred and lifted is what a drawing's marks are; hung off a corner
        // with no lift is the other end of what `Text::origin` decides, and the
        // two fold in opposite directions.
        for (anchor, lift) in [
            (Vec2::splat(0.5), Vec2::new(0.0, -20.0)),
            (Vec2::ZERO, Vec2::ZERO),
        ] {
            let mut apart: Vec<Vec2> = Vec::new();
            for scale in [1.0f32, 1.5, 2.0] {
                let turn = Turn::new(Vec3::X, Vec3::Z).lifted(lift);
                let mut text = run().anchored(anchor).facing(Facing::Turned(turn));
                text.tag = Some(Tag::new(1));
                let ink = view.ink_at(text, scale);
                let case = format!("{plane} at scale {scale}, anchor {anchor:?}, lift {lift:?}");
                assert!(ink.count > 0, "{case}: nothing was drawn");

                // The run as the frame left it: the extent is the shaper's
                // answer, filled by the very pass that drew the glyphs.
                let drawn = view.pane.view.borrow().scene().texts[0].clone();
                let logical = UVec2::new(
                    (FRAME.x as f32 / scale) as u32,
                    (FRAME.y as f32 / scale) as u32,
                );
                let viewport = Viewport::new(logical);
                let camera = *view.pane.view.borrow().camera();
                let (ink_min, ink_max) = (ink.min.as_vec2() / scale, ink.max.as_vec2() / scale);

                // Swept around the ink rather than over the whole view, which is
                // both cheaper and sharper: a box displaced further than this
                // window finds nothing at all and says so.
                let middle = (ink_min + ink_max) * 0.5;
                let mut lo = Vec2::splat(f32::MAX);
                let mut hi = Vec2::splat(f32::MIN);
                for down in -80..=80 {
                    for across in -80..=80 {
                        let cursor = middle + Vec2::new(across as f32, down as f32);
                        let aim = crate::aim::Aim::new(&camera, cursor, viewport, 0.0);
                        if drawn.pick(&aim).is_some_and(|hit| hit.screen == 0.0) {
                            lo = lo.min(cursor);
                            hi = hi.max(cursor);
                        }
                    }
                }
                assert!(lo.x <= hi.x, "{case}: the pick box is nowhere near the ink");

                // **Where the box sits, not how big it is.** Its size is shared
                // by construction — one `Text::extent` feeds both the origin the
                // glyphs hang off and the rectangle a pick opens — so a size
                // that drifted would have to drift in the shaper, and it does:
                // a run comes out a few percent wider than its own box on a
                // display above 1, because `TextGlyphs::measure` scales one
                // shaping linearly where `TextGlyphs::line` shapes again at the
                // raster size and quantises there. Bounded, palantir's to
                // settle, and not what this is asking about. What can drift here
                // is placement, and that is what is measured.
                assert!(
                    lo.abs_diff_eq(ink_min, 8.0),
                    "{case}: the ink starts at {ink_min:?} and the pick box at {lo:?}"
                );
                apart.push(lo - ink_min);
            }

            // **And how far the box sits off the ink does not depend on the
            // display.** The sharp half. A run's box is its line box and its ink
            // is the inked part of one, so the two start a little apart — by the
            // bearing and the ascent, in logical pixels, the same at every scale.
            // Anything that reaches only one of the two through the raster scale
            // moves this and nothing else, which is exactly what a lift spent in
            // the target's pixels and picked in logical ones did.
            //
            // Measured at the corner the box *starts* from rather than at its
            // middle, and that is not a detail: the far end carries the width
            // drift noted above, so a middle would move with the shaper and this
            // does not.
            let first = apart[0];
            for (scale, off) in [1.0f32, 1.5, 2.0].into_iter().zip(&apart) {
                assert!(
                    off.abs_diff_eq(first, 1.5),
                    "{plane}, anchor {anchor:?}, lift {lift:?}: the box starts {off:?} off the \
                     ink at scale {scale} and {first:?} at scale 1"
                );
            }
        }
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
    let mut view = Framed::new(&gpu, Camera::default());

    // One tag over four of the five, so a single highlight has to reach every
    // overlay pass and leave the solids alone.
    let lit = Tag::new(1);
    view.edit(|scene| {
        scene.solids.push(Object::new(Mesh::cube(1.0)));
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
    });
    view.paint(1.0);

    {
        let renderer = view.pane.view.borrow();
        let built = renderer.gpu.as_ref().expect("init runs before paint");

        // A cube is 24 corners and 36 indices, drawn as one instance of one
        // triangle list.
        assert_eq!(built.solids.instances, 1);
        assert_eq!(built.solids.index_count, 36);
        // One record apiece: a segment, a rim, a marker.
        assert_eq!(built.curves.ordinary.instances, 1);
        assert_eq!(built.rings.ordinary.instances, 1);
        assert_eq!(built.points.ordinary.instances, 1);
        // Five characters of "125.4", every one of them with ink.
        assert_eq!(built.texts.ordinary.instances, 5);

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
    }

    // Lit between the frames, which is the only edit — so the second frame
    // rebuilds the highlights and re-uploads nothing else.
    view.pane.view.borrow_mut().highlight_only(Lit {
        tag: lit,
        look: Highlight::new(Vec3::Y),
    });
    view.paint(1.0);

    let renderer = view.pane.view.borrow();
    let built = renderer.gpu.as_ref().expect("init runs before paint");
    assert_eq!(built.curves.lit.instances, 1);
    assert_eq!(built.rings.lit.instances, 1);
    assert_eq!(built.points.lit.instances, 1);
    assert_eq!(
        built.texts.lit.instances, 5,
        "a lit run is the same run shaped again"
    );
    // The ordinary passes are untouched by a highlight: it doubles what is
    // drawn rather than replacing it.
    assert_eq!(built.curves.ordinary.instances, 1);
    assert_eq!(built.solids.instances, 1);
}

/// A mark left behind is one that fires again on the next frame, which is what
/// an early return owes the batch it returned over.
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

/// A highlighted mesh is written in the colour it was given, where it stands.
///
/// The one kind that cannot be *doubled*: an overlay grows and rides forward of
/// what it repeats, and a mesh has neither a width nor a bias to do it with. So
/// a highlight reaches it by changing the colour flattened into its vertices,
/// which means it has to be re-flattened when the highlights move and not only
/// when the geometry does.
#[test]
fn a_highlighted_mesh_is_recoloured_where_it_stands() {
    let plain = Vec3::new(0.2, 0.3, 0.4);
    let mut scene = Scene::default();
    scene.faces.push(
        Object::new(Mesh::cube(1.0))
            .colored(plain)
            .tagged(Tag::new(1)),
    );
    // Untagged, so it is scenery and never lit however the highlights move.
    scene
        .faces
        .push(Object::new(Mesh::cube(1.0)).colored(plain));
    let mut renderer = Renderer::new(scene);

    renderer.refresh(1.0);
    let corners = renderer.cpu.faces.vertices.len();
    assert!(corners > 0, "the faces were never flattened");
    assert!(
        renderer
            .cpu
            .faces
            .vertices
            .iter()
            .all(|v| v.color == plain.to_array()),
        "a mesh was lit before anything named it"
    );

    // Named, and re-flattened though not one vertex moved — which is the whole
    // of what this pins. A `refresh` that only watched the batch's own mark
    // would leave the colour where it was.
    let look = Highlight::new(Vec3::new(1.0, 0.0, 0.0)).scale(3.0);
    renderer.highlight_only(Lit {
        tag: Tag::new(1),
        look,
    });
    renderer.refresh(1.0);

    let lit: Vec<[f32; 3]> = renderer
        .cpu
        .faces
        .vertices
        .iter()
        .map(|v| v.color)
        .collect();
    assert_eq!(lit.len(), corners, "the flatten dropped geometry");
    // Half the corners are the named cube and half the untagged one, and only
    // the first half moved: `scale` has nothing to act on here, so
    // the colour is the whole of what a highlight does to a mesh.
    let (named, scenery) = lit.split_at(corners / 2);
    assert!(
        named.iter().all(|&color| color == [1.0, 0.0, 0.0]),
        "the named mesh kept its own colour"
    );
    assert!(
        scenery.iter().all(|&color| color == plain.to_array()),
        "an untagged mesh was lit"
    );

    // And it goes back when the highlight does, rather than staying lit for
    // the rest of the run.
    renderer.highlight_all(&[]);
    renderer.refresh(1.0);
    assert!(
        renderer
            .cpu
            .faces
            .vertices
            .iter()
            .all(|v| v.color == plain.to_array()),
        "a mesh stayed lit after nothing named it"
    );
}

/// A gizmo behind a face is drawn behind it, and one in front is drawn in
/// front.
///
/// The question a control has to answer like any other geometry, and the one
/// the pass got wrong twice. A gizmo lies among the faces on a datum, so
/// *which* of the two is nearer is a fact about the scene rather than about
/// which pass ran first — and it went wrong in both directions. Writing no
/// depth, a control was blended over by every face there was, because two
/// passes that both decline to write cannot sort against each other at all and
/// draw order decided. Drawn after the faces instead, it painted over them the
/// same way, including the ones genuinely in front of it.
///
/// So both orders are asked, of one scene, moving only where the two sheets
/// sit. Anything that answered by pass order gives the same pixel twice.
#[test]
fn a_gizmo_sorts_against_a_face_by_which_is_nearer_rather_than_by_pass_order() {
    /// Far apart in hue, so the blend cannot be mistaken for either.
    const AXIS: Vec3 = Vec3::new(0.80, 0.10, 0.10);
    const REGION: Vec3 = Vec3::new(0.10, 0.10, 0.80);
    /// The camera looks down −Z from +Z, so a greater z is nearer the eye.
    const NEAR: f32 = 1.0;
    const FAR: f32 = -1.0;

    let gpu = headless_test_gpu();
    // Straight down the axis, so a point on it lands dead centre whatever its
    // depth — which is what lets one pixel answer for both sheets. The stock
    // camera is pitched, and there a nearer point projects clear of a further
    // one rather than onto it, so the stroke misses the pixel the face is read
    // at and the test reads a face that won nothing.
    let mut view = Framed::new(&gpu, square_on());

    let mut sheets_at = |gizmo_at: f32, face_at: f32| {
        view.edit(|scene| {
            scene.clear();
            scene.gizmos.push(
                Curve::segment(
                    Vec3::new(-1.0, 0.0, gizmo_at),
                    Vec3::new(1.0, 0.0, gizmo_at),
                )
                .width(40.0)
                .colored(AXIS),
            );
            scene.faces.push(
                Object::new(facing_quad())
                    .colored(REGION)
                    .at(Vec3::Z * face_at),
            );
        });
        view.paint(1.0);
        view.middle()
    };

    // In front: the face loses the depth test where the control covers it, so
    // nothing of the region reaches the frame there.
    let over = sheets_at(NEAR, FAR);
    // Behind: the face is nearer, passes, and blends its own colour over the
    // control — which still shows through, a face being see-through by design.
    let under = sheets_at(FAR, NEAR);

    assert_ne!(
        over, under,
        "the same two sheets gave one pixel whichever was in front, so what \
         decided it was the order the passes ran in"
    );
    // The two frames against each other rather than either against a number,
    // which is what keeps the claim exact: a region is 45% opaque *and* shaded,
    // so a control behind one still comes back mostly its own colour. What says
    // the region got there is that more of its blue did.
    assert!(
        under[2] > over[2],
        "the region's colour reached the frame no more when the control was \
         behind it ({under:?}) than when it was in front ({over:?})"
    );
    // And the other way: a control in front is the only one drawn undiluted.
    assert!(
        over[0] > under[0],
        "a control in front of a region ({over:?}) came back no more its own \
         colour than one behind it ({under:?})"
    );
}
