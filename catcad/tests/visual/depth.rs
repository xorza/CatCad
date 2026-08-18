//! What hides what, and what is allowed to show through.

use crate::harness::{DEMO_FRAME, Frame, edge_on, painted, shown, staged};
use crate::ink::{Stroke, differing, strokes};
use aperture::{Camera, Mesh, Object, Scene, Vertex};
use glam::{Mat4, UVec2, Vec3};

/// The bias's other bound, and the reason it can't simply be made generous.
///
/// Enough of it settles a coplanar tie; too much and the drawing floats out of
/// the model and shows through solids genuinely standing in front of it. This
/// column runs down through the cylinder the demo grows off the hub, which
/// hides the rectangle's far edge, so the cylinder's silhouette has to come back
/// clean. Pinning both ends is what keeps the constant honest: the grazing test
/// stops it being lowered, this one stops it being raised.
///
/// The solid is the document's own now rather than scenery standing beside it,
/// which makes the claim stronger than it was: what has to be hidden is a stroke
/// behind a solid grown from the very drawing that stroke belongs to.
///
/// Judged by where the strokes are rather than by how many, because how many is
/// the demo's business: everything the drawing puts in front of the cylinder
/// crosses this column too, and none of it says anything about the bias.
#[test]
fn solids_still_hide_the_strokes_behind_them() {
    /// Down the middle of the cylinder, whose silhouette spans roughly 310 to
    /// 490 at this view.
    const COLUMN: u32 = 400;
    /// The far edge of the rectangle lands at row 255 with nothing in front of
    /// it — see the columns either side of the cylinder, which draw it. Anything
    /// at or above this is that edge showing through.
    const BEHIND: u32 = 330;

    let frame = shown(DEMO_FRAME, edge_on(0.45));
    let found = strokes(&frame, COLUMN);
    let through: Vec<&Stroke> = found.iter().filter(|drawn| drawn.row <= BEHIND).collect();
    assert!(
        through.is_empty(),
        "the far edge is behind the cylinder, so nothing may be drawn over it: {through:?}"
    );
    // Otherwise a column that crossed nothing at all would pass.
    assert!(
        !found.is_empty(),
        "column {COLUMN} found no stroke below the cube either, so it proves nothing"
    );
}

/// Geometry that reaches past the camera still has to draw the part in front
/// of it.
///
/// Zoomed in close the ground slab spans the whole view, which puts some of
/// its corners behind the eye. Those belong to the hardware's near-plane
/// clip — anything the vertex shader does to their `z` first changes where the
/// clip lands, and a whole face can disappear. Reversed depth makes that easy
/// to get wrong: the projection writes a *constant* `clip.z`, so a guard
/// phrased against `clip.w` fires on every vertex nearer than the near plane
/// rather than the handful it was meant for.
#[test]
fn a_surface_reaching_behind_the_camera_still_draws() {
    let frame = shown(DEMO_FRAME, |camera| {
        camera.yaw = 0.0;
        camera.pitch = 0.25;
        // Inside the slab's footprint, so it runs off every edge of the view
        // and its far corners sit behind the eye.
        camera.distance = 1.5;
        camera.target = glam::Vec3::new(4.0, 0.0, -2.5);
    });

    // Sample across the lower half, which the slab should cover completely.
    let mut lit = 0;
    let mut total = 0;
    for y in (frame.size.y / 2..frame.size.y).step_by(8) {
        for x in (0..frame.size.x).step_by(8) {
            total += 1;
            if frame.lit(UVec2::new(x, y)) {
                lit += 1;
            }
        }
    }
    let covered = lit as f32 / total as f32;
    assert!(
        covered > 0.95,
        "the slab should fill the lower half, but only {:.0}% of it is lit — \
         the near-plane clip ate the face",
        covered * 100.0
    );
}

/// A sketch face lying in the very plane of the slab under it wins that plane
/// outright, rather than fighting it.
///
/// The demo puts the two exactly coplanar on purpose — the slab's top face *is*
/// the sketch plane. They are meshed quite differently, though: the slab's top
/// is one quad and the face is an arrangement triangulated to a sagitta, so
/// their rasterised depths disagree by rounding, and worst along the long thin
/// triangles. What that looked like was slivers of slab lying *along* the face's
/// own triangle edges.
///
/// Counted as the pixels the face *changes*, rather than as pixels of its own
/// colour, because a translucent face has no colour of its own — what it comes
/// out as depends on what it was drawn over. Weighed against the same frame with
/// the slab dropped out of the plane, which is the one reference that needs no
/// threshold: nothing else moves, the cubes standing on the face occlude exactly
/// what they did, so the face has to reach exactly as many pixels either way.
/// Any it is short of is the slab taking pixels of a surface it is level with.
#[test]
fn a_face_coplanar_with_the_slab_under_it_is_not_fought_for() {
    /// The demo at `pitch`, with the slab either in the sketch plane or
    /// `dropped` below it, and the faces either drawn or taken away.
    fn frame_of(pitch: f32, dropped: f32, faces: bool) -> Frame {
        painted(UVec2::new(700, 520), |renderer| {
            edge_on(pitch)(renderer.camera_mut());
            renderer.camera_mut().distance = 6.0;
            let scene = renderer.scene_mut();
            // Only the two surfaces in question. The drawing standing on the
            // face is neither, and would only be noise here.
            scene.curves.clear();
            scene.rings.clear();
            scene.points.clear();
            scene.texts.clear();
            scene.gizmos.clear();
            if !faces {
                scene.faces.clear();
            }
            // The slab is the first solid the demo pushes; the cubes after it
            // stay where they are, so what they hide does not move either.
            let slab = scene
                .solids
                .iter_mut()
                .next()
                .expect("the demo stands its drawing on a slab");
            slab.transform = Mat4::from_translation(Vec3::new(0.0, -dropped, 0.0)) * slab.transform;
        })
    }

    // How many pixels the faces reach with the slab `dropped` this far.
    let reach = |pitch: f32, dropped: f32| {
        differing(
            &frame_of(pitch, dropped, true),
            &frame_of(pitch, dropped, false),
        )
    };

    // Overhead, and then down to where the plane is nearly edge-on. The shallow
    // end is where too small a bias shows worst — the depth of a plane changes
    // fastest across a pixel there, so two copies of it disagree by the most.
    for pitch in [0.9f32, 0.4, 0.15, 0.05] {
        let clear = reach(pitch, 5.0);
        let level = reach(pitch, 0.0);
        assert!(
            // Well under the seventeen thousand the shallowest of these reaches
            // and the quarter-million the steepest does. A floor at all is here
            // so that a build drawing no faces cannot pass by drawing none twice.
            clear > 10_000,
            "at pitch {pitch} the faces reach only {clear} px with the slab out of the way, \
             so this measures nothing"
        );
        // A few hundred, for the multisampled edge where the two now meet in one
        // plane rather than one standing well below the other.
        assert!(
            clear.abs_diff(level) < 400,
            "at pitch {pitch} the faces reach {level} px level with the slab against {clear} px \
             clear of it, so the two are fighting over {} px",
            clear.abs_diff(level)
        );
    }
}

/// A translucent face shows what is behind it, whichever order the faces were
/// handed over in.
///
/// Blending mixes a surface with what is *already* in the target, so a face has
/// to be drawn after whatever stands behind it. Nothing about the order a caller
/// pushes faces in says anything about depth — it is the order the sketches were
/// created in — so the pass sorts. Drawn the other way round the near face
/// writes depth first and the far one is rejected outright: not faint, gone.
///
/// Two sheets facing the camera, one red in front of one blue, pushed both ways
/// round. What comes out has to be the same picture, and it has to carry blue.
#[test]
fn a_translucent_face_blends_with_the_one_behind_it_either_way_round() {
    /// A flat quad facing the camera at `z`.
    fn sheet(z: f32, color: Vec3) -> Object {
        let at = |x: f32, y: f32| Vertex {
            position: Vec3::new(x, y, z),
            normal: Vec3::Z,
        };
        Object {
            color,
            ..Object::new(Mesh::new(
                vec![at(-2.0, -2.0), at(2.0, -2.0), at(2.0, 2.0), at(-2.0, 2.0)],
                vec![0, 1, 2, 0, 2, 3],
            ))
        }
    }

    /// The pixel where the two overlap, with the near sheet pushed first or last.
    fn overlap(near_first: bool) -> [u8; 4] {
        let mut scene = Scene::default();
        let (near, far) = (
            sheet(1.0, Vec3::new(1.0, 0.0, 0.0)),
            sheet(-1.0, Vec3::new(0.0, 0.0, 1.0)),
        );
        for object in if near_first { [near, far] } else { [far, near] } {
            scene.faces.push(object);
        }
        let square_on = Camera {
            yaw: 0.0,
            pitch: 0.0,
            distance: 10.0,
            target: Vec3::ZERO,
            ..Camera::default()
        };
        staged(UVec2::new(400, 400), square_on, scene)
            .frame
            .pixel(UVec2::new(200, 200))
    }

    let [.., blue_of_first, _] = overlap(true);
    let [.., blue_of_last, _] = overlap(false);
    // The blue sheet is behind and has to reach the frame either way. Before the
    // pass sorted, pushing the near one first left this at the background's 28
    // against the 111 the other order gave.
    assert!(
        blue_of_first > 80,
        "the far face contributed {blue_of_first} of blue, so it was rejected rather than blended"
    );
    assert_eq!(
        blue_of_first, blue_of_last,
        "the same two faces came out differently for having been pushed the other way round"
    );
}

/// The drawing reads through a face crossing in front of it.
///
/// A face is drawn see-through so the model underneath can be read, and a
/// surface you can see the model through but not the *drawing* is a strange
/// kind of transparent. Faces are drawn before every overlay, so one that wrote
/// depth culled outright every stroke and rim behind it — a sketch crossed by a
/// face on some other plane lost its edges where they passed under it, rather
/// than losing a little contrast.
///
/// Measured as the ink the strokes put down, against the same frame with the
/// faces taken away: what crosses in front may shade them, and may not take
/// them. Steep enough that the demo's shelf carries its face over the drawing
/// on the ground — at a shallower angle the two do not overlap and this measures
/// nothing, which is why the pitches are what they are.
#[test]
fn strokes_behind_a_face_still_reach_the_frame() {
    /// The demo at `pitch` with the solids and markers gone, so what is left is
    /// the strokes and — at the caller's word — the faces they cross.
    fn frame_of(pitch: f32, faces: bool, strokes: bool) -> Frame {
        painted(UVec2::new(820, 560), |renderer| {
            edge_on(pitch)(renderer.camera_mut());
            renderer.camera_mut().distance = 11.0;
            let scene = renderer.scene_mut();
            scene.solids.clear();
            scene.points.clear();
            scene.texts.clear();
            // The controls and a dimension's lines are strokes of their own, and
            // this counts what the *drawing's* reach, so they would be weighed
            // as though they were part of it.
            scene.gizmos.clear();
            if !faces {
                scene.faces.clear();
            }
            if !strokes {
                scene.curves.clear();
                scene.rings.clear();
            }
        })
    }

    for pitch in [0.6f32, 0.9] {
        let alone = differing(
            &frame_of(pitch, false, true),
            &frame_of(pitch, false, false),
        );
        let over = differing(&frame_of(pitch, true, true), &frame_of(pitch, true, false));
        assert!(
            alone > 5_000,
            "at pitch {pitch} the strokes reach only {alone} px with nothing over them, so \
             this measures nothing"
        );
        // Exactly equal, not nearly: a face over a stroke changes what colour
        // that pixel comes out, which this counts either way — what it must not
        // do is stop the stroke reaching the pixel at all. Writing depth cost
        // 172 px here and 322 at the steeper angle.
        assert_eq!(
            alone, over,
            "at pitch {pitch} the strokes reach {over} px through the faces against {alone} \
             with none, so a face is culling what is behind it"
        );
    }
}
