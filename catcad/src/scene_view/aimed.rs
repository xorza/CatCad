//! The pointer, resolved: where it is over the view, and what that names in the
//! world.

use aperture::{Aim, Camera, Motion, Viewport};
use glam::{UVec2, Vec2, Vec3};
use palantir::ResponseState;

use crate::document::Document;
use crate::scene_view::HOVER_REACH;

/// Where the pointer is over the view, and the viewport that measures it.
///
/// `pointer_local` is already what [`Scene::nearest`](aperture::Scene::nearest)
/// asks for — logical pixels from the widget's own top-left — so nothing is
/// converted. It is measured against `layout_rect` rather than the visible
/// `rect`, and the viewport is built from that same rect, or the two would
/// disagree the moment anything clipped the view.
///
/// Its own file rather than a satellite of [`SceneView`](crate::scene_view),
/// because nothing about it is the view's: it turns a pointer event into a place
/// on screen and a place in the world, which is a question anything drawing into
/// a viewport would ask the same way.
#[derive(Debug, Clone, Copy)]
pub(super) struct Aimed {
    cursor: Vec2,
    viewport: Viewport,
}

impl Aimed {
    /// What the pointer is aiming at this frame, or `None` if it is off the
    /// surface or the view has not arranged yet.
    ///
    /// Says nothing about whether the pointer is over *this* view: it is the
    /// offset from this widget's corner wherever the pointer is, including
    /// well off the widget. A caller that cares asks `response.hovered`, and
    /// one mid-drag deliberately does not.
    pub(super) fn of(response: &ResponseState) -> Option<Self> {
        Some(Self {
            cursor: response.pointer_local?,
            viewport: Self::viewport(response)?,
        })
    }

    /// The viewport the widget lays out at, whether or not the pointer is over
    /// it.
    ///
    /// **The one place a response becomes a viewport.** Everything sized against
    /// the screen reads one of these — a pick measures the cursor in it, and the
    /// controls are cut in the world against what one of its pixels is worth —
    /// so two spellings of it would be a gizmo built at one size and clicked at
    /// another. It was spelt twice, four lines apart, and stayed harmless only
    /// because both truncated the same way.
    ///
    /// Apart from [`Aimed::of`] because a viewport is not the pointer's: the
    /// drawing is cut against one on every frame, including the frames where the
    /// pointer is somewhere else entirely.
    pub(super) fn viewport(response: &ResponseState) -> Option<Viewport> {
        let rect = response.layout_rect?;
        Some(Viewport::new(UVec2::new(
            rect.size.w as u32,
            rect.size.h as u32,
        )))
    }

    /// The pick this cursor makes, seen through `camera`.
    ///
    /// Everything that asks the scene a question about the cursor goes through
    /// here — the hover, the press and the click alike — so all three reach the
    /// same distance and none can be given a camera the others were not.
    pub(super) fn aim(self, camera: &Camera) -> Aim {
        Aim::new(camera, self.cursor, self.viewport, HOVER_REACH)
    }

    /// The world step that slides the picture by `screen` logical pixels, seen
    /// through `camera`.
    ///
    /// Here rather than on the camera's own call because the viewport is here:
    /// a caller reaching for the viewport to hand it back would be one more
    /// place the widget's rect is turned into one, and the two would disagree
    /// the first time either was measured differently.
    ///
    /// The cursor is not read at all — a scroll carries no position of its own.
    /// It arrives through an `Aimed` all the same, because that is where a
    /// `Response` becomes a viewport.
    pub(super) fn pan_step(self, camera: &Camera, screen: Vec2) -> Vec3 {
        camera.pan_step(screen, self.viewport)
    }
}

/// Where the cursor lands on `motion`, or `None` if it cannot say.
///
/// A motion the cursor cannot resolve against — a plane gone edge-on — answers
/// with nothing rather than jumping, which is what makes turning the view
/// mid-drag survivable and what keeps a click across an edge-on sketch from
/// putting a point somewhere nobody asked for.
///
/// No `hovered` filter, unlike hovering and grabbing. A drag that outruns the
/// view keeps hold of what it grabbed, and a click is already the view's by the
/// time palantir calls it one.
///
/// A free fn rather than a method on [`Aimed`], because what a caller has is a
/// `Response`: every one of them would otherwise open by making an `Aimed` and
/// threading the `Option` on by hand.
pub(super) fn landing(
    response: &ResponseState,
    document: &Document,
    motion: Motion,
) -> Option<Vec3> {
    let aimed = Aimed::of(response)?;
    motion.resolve(&aimed.aim(&document.camera()))
}
