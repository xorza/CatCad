//! Two panes in one frame: where each of them lands, and which reads over
//! which.

use crate::mesh::{Mesh, Vertex};
use crate::object::Object;
use crate::renderer::pane::{Pane, Placement};
use crate::renderer::tests::harness::{FRAME, Framed, square_on};
use crate::scene::Scene;
use crate::styled::Styled;
use glam::{UVec2, Vec3};
use palantir::Rect;
use palantir::internals::{HeadlessTestGpuLease, headless_test_gpu};

/// The rect the second pane takes: 80 logical pixels square, held 20 clear of
/// the frame's bottom-right corner.
///
/// At one physical pixel to the logical one — which is what these paint at —
/// they are the frame's pixels too, so the boundaries below are arithmetic
/// rather than measurement.
const CORNER: Rect = Rect::new(220.0, 140.0, 80.0, 80.0);

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

    let mut pinned = Pane::new(Scene::default(), Placement::At(CORNER));
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
/// one, and the second pane is [`CORNER`] — 220 up to 300 across, 140 up to
/// 220 down. Every pixel below is one step either side of one of those four
/// numbers.
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
