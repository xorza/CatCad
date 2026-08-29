//! The gizmo as the renderer is handed it: a scene, a camera, and the words
//! written on the faces that can be read.

use aperture::{
    Batch, Camera, Facing, Mesh, Object, Projection, Scene, Styled, Text, Turn, Vertex,
};
use glam::{Vec2, Vec3};

use crate::hud::cube::facet::{self, Facet, SIDES};
use crate::look;
use crate::look::Theme;

/// How far a face has to be turned toward the eye before its name is written
/// on it, as the cosine of the angle between them.
///
/// **A word is not worth the room until it can be read.** A name is painted on
/// its face, so it foreshortens with it — and a face near enough to edge-on
/// squeezes its word into a smear a stroke or two wide, which is noise across
/// the piece next to it. At this much the three faces of a corner view all
/// carry their names — a corner shows each at `0.577` — and a face swinging
/// away loses its name well before it loses its outline.
const READS: f32 = 0.3;

/// How far the cube's camera stands off it, in the units the solid is built in.
///
/// Says nothing about the size it is drawn at — the projection is parallel, so
/// the distance decides only how deep the slab it draws is. Well clear of a
/// solid every point of which lies within one unit of the middle.
const STANDOFF: f32 = 4.0;

/// The solid: one object per piece of it, and no words yet.
///
/// Built once and turned by the camera rather than rebuilt as it turns. What
/// the cube *is* does not change — only where it is looked at from — so a solid
/// written every frame would be a solid re-uploaded every frame for nothing.
/// The words are the half that does move; see [`name`].
pub(crate) fn scene(theme: &Theme) -> Scene {
    let chrome = &theme.chrome;
    let color = look::ink(chrome.cube_low);
    let mut scene = Scene::default();
    let mut ring = Vec::new();
    for facet in facet::EVERY {
        scene
            .solids
            .push(piece(facet, chrome.cube_chamfer, &mut ring, color));
    }
    scene
}

/// Write the name of every face turned far enough toward `eye` to read it.
///
/// **Painted on the cube rather than pinned over it**, which is the whole of
/// why the size moves with the bearing. A run laid into a plane advances along
/// that plane as the projection draws it but is *set* at the size it would have
/// had square to the viewer — right for a dimension on a drawing, which has to
/// stay readable however the sheet leans, and wrong for a word that has to stay
/// inside the face it names. Scaled by how square the face is, the word
/// foreshortens with what it is written on, exactly as paint would.
///
/// Refilled rather than rewritten, so the runs keep the room they have grown
/// to. Only the faces that read are written at all: a name is a run the shaper
/// lays out, and one nobody could read is one nobody should pay for.
pub(crate) fn name(theme: &Theme, eye: Vec3, texts: &mut Batch<Text>) {
    let chrome = &theme.chrome;
    let ink = look::ink(chrome.ink_lit);
    let reads = SIDES
        .into_iter()
        .filter(|side| side.out().dot(eye) >= READS);
    texts.refill(reads, |text, side| {
        // On the face's own middle, which is where the face reaches: the run
        // writes no depth and is biased forward, so it reads over the surface it
        // lies on rather than fighting it.
        *text = Text::new(
            side.out(),
            side.name,
            chrome.cube_name * side.out().dot(eye),
        )
        .anchored(Vec2::splat(0.5))
        .facing(Facing::Turned(Turn::new(side.u, side.out())))
        .colored(ink)
        .tagged(side.tag());
    });
}

/// Where the gizmo is seen from: the document's own bearing, framed to the box
/// it is drawn in.
///
/// **Parallel, whatever the drawing is being looked at through.** A cube drawn
/// in perspective reads as a box seen from somewhere rather than as which way
/// you are looking, and the near corner grows enough to make the two faces
/// beside it hard to aim at.
///
/// Everything else about the document's camera is dropped. How far it stands
/// off and what it is pointed at say nothing about the bearing, which is the
/// whole of what the gizmo shows.
pub(crate) fn camera(theme: &Theme, aim: Camera) -> Camera {
    Camera {
        projection: Projection::Orthographic,
        target: Vec3::ZERO,
        distance: STANDOFF,
        yaw: aim.yaw,
        pitch: aim.pitch,
        // A parallel camera frames `distance · tan(fov / 2)` either side of the
        // middle — see [`Camera::view_proj`] — so the field is whatever brings
        // that to the reach the box wants at the standoff above.
        fov_y: 2.0 * (theme.chrome.cube_extent() / STANDOFF).atan(),
        ..Camera::default()
    }
}

/// Where the eye stands, as a direction from what it is looking at.
///
/// **Taken off the camera's own answer rather than worked out again.** Which
/// way the offset runs from a yaw and a pitch is aperture's convention, and a
/// second copy of it here would be a gizmo that went on reading the old one the
/// day it changed. What is dropped is the distance, which says nothing about
/// which way round the cube reads.
pub(crate) fn eye(camera: &Camera) -> Vec3 {
    (camera.eye() - camera.target).normalize_or(Vec3::Y)
}

/// One piece of the solid, as the fan of triangles its ring is.
///
/// **Its own corners, and one normal across all of them.** A bevel is a flat
/// facet whose whole job is to catch the light differently from the two faces
/// it joins, and a corner shared with a neighbour would be a corner whose
/// normal averaged the two — which is a rounded edge, and the opposite of what
/// the cut is for.
fn piece(facet: Facet, chamfer: f32, ring: &mut Vec<Vec3>, color: Vec3) -> Object {
    facet.ring(chamfer, ring);
    let normal = facet.normal();
    let corners = ring
        .iter()
        .map(|&position| Vertex { position, normal })
        .collect();
    // A fan from the first corner, every piece being flat and convex — and
    // wound the way its ring is, which is what the solids pass culls on.
    let faces = (1..ring.len() as u32 - 1)
        .map(|at| [0, at, at + 1])
        .collect();
    Object::new(Mesh::new(corners, faces))
        .colored(color)
        .tagged(facet.tag())
}
