//! Two panes in one frame: where each of them lands, and which reads over
//! which.

use crate::mesh::{Mesh, Vertex};
use crate::object::Object;
use crate::renderer::pane::{Corner, Pane, Placement};
use crate::renderer::tests::harness::{FRAME, Framed, square_on};
use crate::scene::Scene;
use crate::styled::Styled;
use glam::{UVec2, Vec2, Vec3};
use palantir::internals::{HeadlessTestGpuLease, headless_test_gpu};

/// How far the pinned pane reaches, in logical pixels, and how far it is held
/// off the corner it hangs from.
///
/// At one physical pixel to the logical one — which is what these paint at —
/// they are the frame's pixels too, so the boundaries below are arithmetic
/// rather than measurement.
const SIDE: f32 = 80.0;
const INSET: f32 = 20.0;

/// The colour of the pane that fills the view, and of the one pinned over it.
///
/// Told apart by which channel wins rather than by an exact value, because both
/// are shaded: a mesh is lit, so what comes out is the colour times a diffuse
/// term. Both walls face the camera the same way, so the term is the same for
/// both and the ordering survives it.
const BEHIND: Vec3 = Vec3::new(0.9, 0.0, 0.0);
const OVER: Vec3 = Vec3::new(0.0, 0.9, 0.0);

/// A wall of one colour at `z`, large enough to fill any pane it is put in.
///
/// Two hundred units across, against a field of view that covers about three at
/// the distance a default camera stands off — so what a test reads is the wall
/// wherever on the pane it reads, and never the ground behind it.
fn wall(color: Vec3, z: f32) -> Object {
    let far = 100.0;
    let corners = [(-far, -far), (far, -far), (far, far), (-far, far)];
    Object::new(Mesh::new(
        corners
            .map(|(x, y)| Vertex {
                position: Vec3::new(x, y, z),
                normal: Vec3::Z,
            })
            .to_vec(),
        vec![[0, 1, 2], [0, 2, 3]],
    ))
    .colored(color)
}

/// A view of two walls: one filling it, one pinned into its bottom-right
/// corner, each at the depth it is given.
fn walls(gpu: &HeadlessTestGpuLease, behind: f32, over: f32) -> Framed<'_> {
    let mut view = Framed::new(gpu, square_on());
    view.edit(|scene| scene.solids.push(wall(BEHIND, behind)));

    let mut pinned = Pane::new(
        Scene::default(),
        Placement::Pinned {
            at: Corner::BottomRight,
            size: Vec2::splat(SIDE),
            inset: Vec2::splat(INSET),
        },
    );
    pinned.camera = square_on();
    pinned.scene.solids.push(wall(OVER, over));
    let nth = view.app.view.borrow_mut().push_pane(pinned);
    assert_eq!(nth, 1, "the pinned pane was not pushed over the first");
    view.paint(1.0);
    view
}

/// Whether the pinned pane's colour is what came out at `at`.
fn over(view: &Framed<'_>, at: UVec2) -> bool {
    let pixel = view.pixel(at);
    pixel[1] > pixel[0]
}

/// **A pane draws inside its own rect and nowhere else**, read on both sides of
/// all four of its edges.
///
/// The frame is 320 by 240 and painted at one physical pixel to the logical
/// one, so a pane 80 across held 20 off the bottom-right corner runs from
/// `320 − 80 − 20` to `320 − 20` across — 220 up to 300 — and from
/// `240 − 80 − 20` to `240 − 20` down, which is 140 up to 220. Every pixel
/// below is one step either side of one of those four numbers.
///
/// The boundaries are exact rather than blurred: what confines a pane is a
/// scissor, which drops whole samples, so the pixel outside an edge takes none
/// of the pane's and the pixel inside takes all of them. There is nothing for
/// the multisample resolve to average.
#[test]
fn a_pane_draws_inside_its_own_rect_and_nowhere_else() {
    let gpu = headless_test_gpu();
    let view = walls(&gpu, 0.0, 0.0);
    for (at, inside) in [
        // Across the left edge, then the right.
        (UVec2::new(220, 180), true),
        (UVec2::new(219, 180), false),
        (UVec2::new(299, 180), true),
        (UVec2::new(300, 180), false),
        // And down through the top edge, then the bottom.
        (UVec2::new(260, 140), true),
        (UVec2::new(260, 139), false),
        (UVec2::new(260, 219), true),
        (UVec2::new(260, 220), false),
    ] {
        assert_eq!(
            over(&view, at),
            inside,
            "{at:?} came out {:?}, which is the wrong pane",
            view.pixel(at),
        );
    }
    // And the pane behind it is still drawn everywhere else, rather than the
    // pinned one having taken the frame or the frame having taken it.
    assert!(!over(&view, FRAME / 2), "the pane behind lost the middle");
}

/// **A pane over another reads over it, whatever the world says.**
///
/// The wall in the pinned pane is put four units *behind* the wall in the pane
/// under it, so on the geometry alone it loses the depth test everywhere and
/// nothing of it is drawn at all. It is drawn, because a pane takes a slice of
/// the depth range rather than sharing one — which is the whole of what lets an
/// orientation gizmo sit over a model that surrounds it.
///
/// The two walls face the same way at the same size, so nothing but the depth
/// separates them and the answer cannot come from anywhere else.
#[test]
fn a_pane_in_front_reads_over_one_that_is_nearer_in_the_world() {
    let gpu = headless_test_gpu();
    let view = walls(&gpu, 2.0, -2.0);
    let middle = UVec2::new(260, 180);
    assert!(
        over(&view, middle),
        "the nearer wall behind took the pane in front: {:?}",
        view.pixel(middle),
    );
    // And the near wall still wins where it is the only pane, so the slice
    // moved the panes apart rather than moving the geometry.
    assert!(!over(&view, FRAME / 2), "the pane behind lost the middle");
}

/// Which pane a point of the view falls in, frontmost first.
///
/// The rect is the one worked out above — 220 to 300 across, 140 to 220 down —
/// and a point inside it is in the pinned pane however much of the pane behind
/// is under it. `local` is measured from the pane's own corner, so the pinned
/// pane's middle at 260, 180 is 40, 40 into it.
#[test]
fn a_point_falls_in_the_frontmost_pane_that_holds_it() {
    let gpu = headless_test_gpu();
    let view = walls(&gpu, 0.0, 0.0);
    let renderer = view.app.view.borrow();
    let across = FRAME.as_vec2();

    let inside = renderer
        .pane_at(Vec2::new(260.0, 180.0), across)
        .expect("the middle of the pinned pane is in a pane");
    assert_eq!(inside.nth, 1, "a point over the gizmo answered the drawing");
    assert_eq!(inside.local, Vec2::splat(40.0));

    // One pixel outside it, and the pane behind answers — with the point
    // measured from the view's own corner, that pane filling it.
    let outside = renderer
        .pane_at(Vec2::new(219.0, 180.0), across)
        .expect("the drawing fills the view");
    assert_eq!(outside.nth, 0);
    assert_eq!(outside.local, Vec2::new(219.0, 180.0));

    // And off the view entirely is no pane at all, rather than the last one
    // that happened to be looked at.
    assert_eq!(renderer.pane_at(across + Vec2::ONE, across), None);
}
