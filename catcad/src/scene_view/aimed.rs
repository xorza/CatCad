//! The pointer, resolved: where it is over the view, and what that names in the
//! world.

use aperture::{Aim, Motion, Viewport};
use glam::{UVec2, Vec2, Vec3};
use palantir::ResponseState;

use crate::lens::Lens;

/// How far from the cursor a thing may be and still count as under it, in
/// logical pixels.
///
/// Wider than anything drawn, because aiming is not precise: a stroke of
/// `EDGE_WIDTH` is under two pixels across and is not a target. It has to stay
/// above half the widest marker too — `Aim::reach` takes whichever of the two
/// is larger, so a marker grown past twice this would quietly become the pick
/// radius instead.
pub(super) const HOVER_REACH: f32 = 6.0;

/// Where the pointer is over the view.
///
/// `pointer_local` is already what [`Scene::nearest`](aperture::Scene::nearest)
/// asks for — logical pixels from the widget's own top-left — so nothing is
/// converted. It is measured against `layout_rect` rather than the visible
/// `rect`, which is the rect a [`Lens`] is built from too, or the two would
/// disagree the moment anything clipped the view.
///
/// The cursor alone, and everything it is asked *through* arrives as a lens. A
/// pointer position outlives a viewpoint — this one is kept from the recorded
/// half of a frame to the settled half, and the camera turns in between — so a
/// viewport carried along here would be a second copy of one, free to answer a
/// pick through a camera the frame had already moved.
///
/// Its own file rather than a satellite of [`SceneView`](crate::scene_view),
/// because nothing about it is the view's: it turns a pointer event into a place
/// on screen and a place in the world, which is a question anything drawing into
/// a viewport would ask the same way.
#[derive(Debug, Clone, Copy)]
pub(super) struct Aimed {
    cursor: Vec2,
}

impl Aimed {
    /// What the pointer is aiming at this frame, or `None` if it is off the
    /// surface.
    ///
    /// Says nothing about whether the pointer is over *this* view: it is the
    /// offset from this widget's corner wherever the pointer is, including
    /// well off the widget. A caller that cares asks `response.hovered`, and
    /// one mid-drag deliberately does not.
    pub(super) fn of(response: &ResponseState) -> Option<Self> {
        Some(Self {
            cursor: response.pointer_local?,
        })
    }

    /// The pick this cursor makes, seen through `lens`.
    ///
    /// Everything that asks the scene a question about the cursor goes through
    /// here — the hover, the press and the click alike — so all three reach the
    /// same distance and none can be given a viewpoint the others were not.
    pub(super) fn aim(self, lens: Lens) -> Aim {
        lens.aim(self.cursor, HOVER_REACH)
    }
}

/// The viewport the view lays out at, or `None` where it has not arranged.
///
/// **The one place a response becomes a viewport.** Everything sized against the
/// screen is answered in one — a pick measures the cursor in it, and the
/// controls are cut in the world against what one of its pixels is worth — so
/// two spellings of it would be a gizmo built at one size and clicked at
/// another. It was spelt twice, four lines apart, and stayed harmless only
/// because both truncated the same way.
///
/// A free fn beside [`landing`] rather than something read off an [`Aimed`], and
/// for that one's reason: a viewport is not the pointer's. The drawing is cut
/// against one on every frame, including the frames where the pointer is
/// somewhere else entirely and there is no `Aimed` to ask.
pub(super) fn viewport(response: &ResponseState) -> Option<Viewport> {
    let rect = response.layout_rect?;
    Some(Viewport::new(UVec2::new(
        rect.size.w as u32,
        rect.size.h as u32,
    )))
}

/// Where the cursor lands on `motion` seen through `lens`, or `None` if it
/// cannot say.
///
/// A motion the cursor cannot resolve against — a plane gone edge-on — answers
/// with nothing rather than jumping, which is what makes turning the view
/// mid-drag survivable and what keeps a click across an edge-on sketch from
/// putting a point somewhere nobody asked for. So is a view that has not
/// arranged yet, which is what the lens being absent says.
///
/// The cursor arrives already read, and **unfiltered** — which is the one thing
/// separating it from the `over` a frame also holds. A drag that outruns the
/// view keeps hold of what it grabbed, and a click is already the view's by the
/// time palantir calls it one, so neither may be dropped for the pointer having
/// left; hovering and grabbing are the opposite case and take the filtered one.
///
/// A free fn rather than a method on [`Aimed`], because what it threads is two
/// `Option`s that come from different places — the cursor and the lens — and a
/// method would leave the caller unwrapping one of them at every call.
pub(super) fn landing(aimed: Option<Aimed>, lens: Option<Lens>, motion: Motion) -> Option<Vec3> {
    motion.resolve(&aimed?.aim(lens?))
}

/// What a harness reaches past a response for.
///
/// Sweeping candidate cursors is how a test finds something to grab without a
/// press to ask through, and a sweep has no `Response` to read a position off —
/// it is trying positions. So the one thing a response supplies arrives
/// directly, and everything the pick then does is the view's own.
#[cfg(test)]
mod sweeping {
    use glam::Vec2;

    use crate::scene_view::aimed::Aimed;

    impl Aimed {
        /// The pointer at `cursor`, in the same logical pixels [`Aimed::of`]
        /// reads.
        pub(crate) fn at(cursor: Vec2) -> Self {
            Self { cursor }
        }
    }
}
