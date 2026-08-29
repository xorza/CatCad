//! One scene, seen from one camera, landed in one rect of the target.

use crate::camera::Camera;
use crate::scene::Scene;
use crate::tag::Tag;
use glam::Vec2;
use palantir::Rect;

/// Which corner of the view a pinned pane holds on to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Where a pane lands in the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// The whole view, which is what a viewport is.
    Fill,
    /// A box of a stated size, held to a corner of the view and inset from it.
    ///
    /// **Stated in logical pixels, so that furniture is the same size whatever
    /// the view is.** An orientation gizmo is a control rather than part of the
    /// drawing: it is read and pressed at a size a hand can hit, and one that
    /// grew with the window would be a different control on a different
    /// monitor.
    Pinned { at: Corner, size: Vec2, inset: Vec2 },
}

impl Placement {
    /// The rect this takes of a view `view` logical pixels across.
    ///
    /// Logical throughout, because that is what a pinned size is stated in and
    /// what a cursor arrives in. What a *frame* wants is the same rect in the
    /// target's own pixels, and it converts — so the placement is worked out
    /// once and the raster scale is spent in one place.
    ///
    /// A pane too large for the view it is pinned in keeps its corner and
    /// reaches past the far one, rather than being shrunk or refused: what
    /// confines it is the scissor, and a window too small for the furniture in
    /// it is a window rather than a mistake.
    pub fn rect(self, view: Vec2) -> Rect {
        match self {
            Self::Fill => Rect::new(0.0, 0.0, view.x, view.y),
            Self::Pinned { at, size, inset } => {
                let far = view - size - inset;
                let min = match at {
                    Corner::TopLeft => inset,
                    Corner::TopRight => Vec2::new(far.x, inset.y),
                    Corner::BottomLeft => Vec2::new(inset.x, far.y),
                    Corner::BottomRight => far,
                };
                Rect::new(min.x, min.y, size.x, size.y)
            }
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
    /// What a pick that lands here reports it landed in — see
    /// [`Renderer::pane_at`](crate::Renderer::pane_at). `None` for a pane
    /// nothing points at.
    pub tag: Option<Tag>,
}

impl Pane {
    /// A pane of `scene` placed by `placement`, seen from the default camera
    /// and answering to no tag.
    ///
    /// The two a pane cannot be built without. The camera and the tag are set
    /// after, being the two a caller usually leaves as they are.
    pub fn new(scene: Scene, placement: Placement) -> Self {
        Self {
            scene,
            camera: Camera::default(),
            placement,
            tag: None,
        }
    }
}

/// Which pane a point of the view falls in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneAt {
    /// Which pane it is, as [`Renderer::pane`](crate::Renderer::pane) takes
    /// one.
    pub nth: usize,
    /// What that pane answers to, carried here so a caller that only wants to
    /// know *which* pane need not go back for it.
    pub tag: Option<Tag>,
    /// Where the point falls in the pane, in logical pixels from the pane's own
    /// top-left corner — which is what a pick of that pane's scene aims with.
    pub local: Vec2,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every placement puts the pane where its own words say**, worked out in
    /// pixels by hand.
    ///
    /// A view 800 across and 600 down, and a pane 120 square inset 20 from
    /// whichever corner it holds. Held top-left it begins at 20, 20. Held
    /// bottom-right it begins at `800 − 120 − 20` across and `600 − 120 − 20`
    /// down, which is 660, 460 — and the other two take one of each.
    ///
    /// What this guards is the pair of subtractions, which is exactly the
    /// arithmetic that reads right and comes out mirrored: an inset counted
    /// from the far edge instead of the near one puts a gizmo 20 pixels off
    /// screen rather than 20 pixels in.
    #[test]
    fn every_placement_puts_the_pane_where_its_own_words_say() {
        let view = Vec2::new(800.0, 600.0);
        let size = Vec2::splat(120.0);
        let inset = Vec2::splat(20.0);
        assert_eq!(
            Placement::Fill.rect(view),
            Rect::new(0.0, 0.0, 800.0, 600.0)
        );
        for (at, min) in [
            (Corner::TopLeft, Vec2::new(20.0, 20.0)),
            (Corner::TopRight, Vec2::new(660.0, 20.0)),
            (Corner::BottomLeft, Vec2::new(20.0, 460.0)),
            (Corner::BottomRight, Vec2::new(660.0, 460.0)),
        ] {
            let rect = Placement::Pinned { at, size, inset }.rect(view);
            assert_eq!(rect, Rect::new(min.x, min.y, 120.0, 120.0), "{at:?}");
        }
    }

    /// A pane too large for the view keeps the corner it was pinned to and
    /// reaches past the other, rather than being shrunk to fit.
    ///
    /// A 300-pixel pane in a 200-pixel view, held bottom-right and inset 10:
    /// its corner is `200 − 300 − 10`, which is −110. Negative is the answer,
    /// not an error — the far corner is what the pin holds, and the near one is
    /// what gives.
    #[test]
    fn a_pane_larger_than_its_view_keeps_the_corner_it_holds() {
        let rect = Placement::Pinned {
            at: Corner::BottomRight,
            size: Vec2::splat(300.0),
            inset: Vec2::splat(10.0),
        }
        .rect(Vec2::splat(200.0));
        assert_eq!(rect.min, Vec2::splat(-110.0));
        assert_eq!(rect.max(), Vec2::splat(190.0), "the pin let go");
    }
}
