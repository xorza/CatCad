//! One scene, seen from one camera, landed in one rect of the target.

use crate::camera::Camera;
use crate::scene::Scene;
use glam::Vec2;
use palantir::Rect;

/// Where a pane lands in the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// The whole view, which is what a viewport is.
    Fill,
    /// Exactly this rect of it, in logical pixels down and right from the
    /// view's own top-left corner.
    ///
    /// **The caller places it, because the caller already has a layout.** A
    /// gizmo in the corner of a window sits above whatever else that corner
    /// holds, and how much room that leaves is a question the application's own
    /// layout answers. A placement that pinned itself to the corner here would
    /// be a second layout, agreeing with the first only until one of them
    /// changed.
    ///
    /// Logical pixels, because that is what a layout and a cursor are both
    /// measured in. The frame is the one place the raster scale is spent.
    At(Rect),
}

impl Placement {
    /// The rect this takes of a view `view` logical pixels across.
    ///
    /// `Fill` is the one arm that needs the view at all, and it is why the arm
    /// exists: a viewport would otherwise have to restate the view's own size
    /// every frame, and would be wrong for the frame after a resize.
    ///
    /// A rect reaching outside the view is answered as given rather than
    /// clamped: what confines a pane is the scissor, and a window too small for
    /// the furniture in it is a window rather than a mistake.
    pub fn rect(self, view: Vec2) -> Rect {
        match self {
            Self::Fill => Rect::new(0.0, 0.0, view.x, view.y),
            Self::At(rect) => rect,
        }
    }
}

/// One scene, seen from one camera, landed in one rect of the target.
///
/// **What a renderer draws is a list of these**, back to front. A viewport is
/// one pane with the whole view to itself; an orientation gizmo, an axis triad
/// or a thumbnail is another pinned into a corner of the same target. They
/// share the pipelines and the glyph sheet, and nothing else: each has its own
/// scene to be picked in, its own camera to be seen from, and its own slice of
/// the depth buffer, so what is drawn in one can neither occlude nor be
/// occluded by what is drawn in another.
///
/// Authored, where what the renderer flattens it into is derived — the fields
/// are public because every one of them is the caller's to say, and
/// nothing here is invalidated by saying it: an edit marks the batch it
/// touched, and moving the camera re-uploads nothing at all.
#[derive(Debug)]
pub struct Pane {
    pub scene: Scene,
    /// Where the scene is drawn from.
    ///
    /// Beside the scene rather than in it: a viewpoint is not part of what
    /// there is, and holding one inside the scene let a caller pick through it
    /// without ever naming it.
    pub camera: Camera,
    pub placement: Placement,
}

impl Pane {
    /// A pane of `scene` placed by `placement`, seen from the default camera.
    ///
    /// The two a pane cannot be built without. The camera is set after, being
    /// the one a caller usually leaves as it is.
    pub fn new(scene: Scene, placement: Placement) -> Self {
        Self {
            scene,
            camera: Camera::default(),
            placement,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A pane takes the rect its placement names**, and `Fill` is the only
    /// one the view's own size reaches.
    ///
    /// Worked by hand against a view 800 across and 600 down. `Fill` is that
    /// rect. A gizmo 120 square, held 20 clear of the bottom-right corner,
    /// begins at `800 − 120 − 20` across and `600 − 120 − 20` down — 660, 460 —
    /// and says so itself, the caller's layout having worked it out.
    #[test]
    fn a_pane_takes_the_rect_its_placement_names() {
        let view = Vec2::new(800.0, 600.0);
        assert_eq!(
            Placement::Fill.rect(view),
            Rect::new(0.0, 0.0, 800.0, 600.0)
        );
        let corner = Rect::new(660.0, 460.0, 120.0, 120.0);
        assert_eq!(Placement::At(corner).rect(view), corner);
        // And a view of another size moves neither of them the same way: the
        // first follows it and the second does not.
        let wider = Vec2::new(1600.0, 600.0);
        assert_eq!(Placement::Fill.rect(wider).max().x, 1600.0);
        assert_eq!(Placement::At(corner).rect(wider), corner);
    }

    /// A rect reaching past the view is answered as it was given.
    ///
    /// What a scroll does to a pinned gizmo, and what a window too small for
    /// its own furniture is. The scissor is what clips; a placement that
    /// clamped would move the pane instead of cutting it, and picking would
    /// then answer somewhere the drawing is not.
    #[test]
    fn a_rect_past_the_view_is_answered_as_it_was_given() {
        let over = Rect::new(-110.0, -110.0, 300.0, 300.0);
        assert_eq!(Placement::At(over).rect(Vec2::splat(200.0)), over);
    }
}
