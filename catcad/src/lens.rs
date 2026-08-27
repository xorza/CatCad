//! Where the drawing is looked at from, and how big it comes out.

use aperture::{Aim, Camera, Viewport};
use glam::{Vec2, Vec3};
use palantir::Rect;

/// The camera and the room it is answered in, read together.
///
/// **Two halves of one question, which is what a pixel is worth here.** A camera
/// alone cannot say: how much world one pixel covers, where a point lands on
/// screen and which ray a cursor casts are all answers in a *viewport*, and every
/// call in aperture that gives one asks for both. They were passed apart across
/// seven signatures, which is seven places a camera could meet a viewport it was
/// never measured against — and a control built at one size and clicked at
/// another is a control that misses.
///
/// Gathered rather than passed one by one, because they arrive together and mean
/// one thing between them — the same reason [`Shown`](crate::hud::Shown) and
/// [`Made`](crate::paint::layout::Made) are gathered.
///
/// [`Copy`], and costing what a reference costs: a [`Camera`] is a handful of
/// scalars the document hands back by value, and a [`Viewport`] is two.
///
/// Made where the two are known and never held: the camera is the document's and
/// the viewport is the view's, so a lens kept across frames would be a third copy
/// of both, free to fall behind either.
///
/// Compared, though: the controls' stamp keeps one to say what they were last
/// built against. That is a record of what was drawn rather than somewhere the
/// camera is read from, which is the distinction the paragraph above draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Lens {
    camera: Camera,
    viewport: Viewport,
}

impl Lens {
    /// The drawing seen through `camera`, in a view `viewport` big.
    pub(crate) fn new(camera: Camera, viewport: Viewport) -> Self {
        Self { camera, viewport }
    }

    /// How many world units one logical pixel covers at `at`.
    ///
    /// What everything sized in *pixels* but built in the *world* is grown from
    /// — a control's outline, a mark's clearance, a dimension's gaps. Depends on
    /// where it is asked, and only under perspective: there a pixel covers more
    /// world the further off it is.
    pub(crate) fn world_per_pixel(self, at: Vec3) -> f32 {
        self.camera.world_per_pixel(at, self.viewport)
    }

    /// The point the eye orbits, which is the middle of what is on screen.
    ///
    /// The one distance a decision taken for a whole frame can be measured at.
    /// A solid's faces stand at as many depths as it is deep, and something
    /// chorded once for the picture has to pick one of them — see
    /// [`Chorded`](crate::paint::Chorded).
    pub(crate) fn focus(self) -> Vec3 {
        self.camera.target
    }

    /// Where `at` lands on the view, in logical pixels down from its top-left
    /// corner, or `None` where the projection draws none of it.
    ///
    /// One position, and it builds a whole view-projection to answer. A caller
    /// with a run of them wants [`Lens::footprint`], which builds one for all of
    /// them.
    pub(crate) fn screen_of(self, at: Vec3) -> Option<Vec2> {
        self.camera.screen_of(at, self.viewport)
    }

    /// The screen rectangle a run of world positions covers, or `None` where the
    /// projection draws none of it.
    ///
    /// **The projection is built once and every position read through it**,
    /// rather than asked of the camera apiece: a region's boundary is dozens of
    /// corners and a rim is as many again, and [`Camera::screen_of`] builds a
    /// whole view-projection per call.
    ///
    /// Positions the projection drops are skipped and the rest still answer,
    /// because a shape half off screen still covers the half that was drawn.
    /// All of them dropped is `None`, which is a shape the view is not showing
    /// at all — see [`Stands::Beside`](crate::prompt::Stands), which is what
    /// reads this and what that means for a form.
    pub(crate) fn footprint(self, at: impl Iterator<Item = Vec3>) -> Option<Rect> {
        let view_proj = self.camera.view_proj(self.viewport.aspect());
        // The span carries "nothing drawn yet" itself, rather than a pair of
        // sentinels with a flag beside them saying whether to believe it — an
        // empty run then has one spelling instead of two that have to agree.
        at.filter_map(|corner| self.viewport.pixel_of(view_proj * corner.extend(1.0)))
            .fold(None, |span: Option<[Vec2; 2]>, corner| {
                Some(span.map_or([corner, corner], |[low, high]| {
                    [low.min(corner), high.max(corner)]
                }))
            })
            .map(|[low, high]| Rect::new(low.x, low.y, high.x - low.x, high.y - low.y))
    }

    /// The pick a cursor at `cursor` makes, reaching `radius` logical pixels.
    ///
    /// Everything that asks the scene a question about the cursor comes through
    /// here, so the hover, the press and the click cannot be given viewpoints
    /// the others were not.
    pub(crate) fn aim(self, cursor: Vec2, radius: f32) -> Aim {
        Aim::new(&self.camera, cursor, self.viewport, radius)
    }

    /// The world step that slides the picture by `screen` logical pixels.
    ///
    /// `screen` names where the *viewport* goes rather than where the model
    /// does — see [`Camera::pan_step`].
    pub(crate) fn pan_step(self, screen: Vec2) -> Vec3 {
        self.camera.pan_step(screen, self.viewport)
    }

    /// The unit direction the view looks along.
    ///
    /// What a flat outline is turned square to so that it reads at full size
    /// from wherever the camera is. The one answer here the viewport has no say
    /// in, and here all the same: it is a fact about how the drawing is being
    /// looked at, which is the whole of what this is.
    pub(crate) fn facing(self) -> Vec3 {
        self.camera.facing()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::UVec2;

    /// A lens over a view big enough that a handful of points around the origin
    /// all land inside it.
    fn lens() -> Lens {
        Lens::new(Camera::default(), Viewport::new(UVec2::new(800, 600)))
    }

    /// **A footprint is the box the same points land in when each is projected
    /// on its own.**
    ///
    /// Two readings of one question — [`Lens::screen_of`] builds a
    /// view-projection per point and [`Lens::footprint`] builds one for the run
    /// — so what is pinned is that they *agree*, rather than either matching
    /// numbers written out again beside it.
    ///
    /// The points are in no useful order along either axis, which is what makes
    /// the box's four sides four separate claims: a corner that only ever grew
    /// one way would leave it short on the other, and a run that happened to be
    /// sorted would not notice.
    #[test]
    fn a_footprint_is_the_box_the_same_points_project_into() {
        let lens = lens();
        let at = [
            Vec3::new(1.0, 0.5, -1.0),
            Vec3::new(-2.0, -1.5, 0.5),
            Vec3::new(0.25, 2.0, 1.5),
            Vec3::new(-0.5, -0.25, -2.0),
        ];
        let seen: Vec<Vec2> = at
            .iter()
            .map(|&corner| lens.screen_of(corner).expect("in view"))
            .collect();
        let low = seen.iter().copied().reduce(Vec2::min).unwrap();
        let high = seen.iter().copied().reduce(Vec2::max).unwrap();
        // Every one of the four is a different point, or the sweep above would
        // be four readings of one corner and the box would be a dot.
        assert!(
            (high - low).min_element() > 1.0,
            "the fixture projects to {low:?}..{high:?}, which is nothing to bound"
        );

        let covered = lens.footprint(at.into_iter()).expect("in view");
        assert!(
            (covered.min - low).abs().max_element() < 1e-3,
            "the box starts at {:?} where the points start at {low:?}",
            covered.min,
        );
        assert!(
            (Vec2::new(covered.size.w, covered.size.h) - (high - low))
                .abs()
                .max_element()
                < 1e-3,
            "the box is {:?} across where the points span {:?}",
            covered.size,
            high - low,
        );
    }

    /// **A point the projection drops is skipped, and all of them dropped is
    /// nothing at all.**
    ///
    /// The two halves of what a caller reads: a shape half off screen still
    /// covers the half that was drawn, and one the view is showing none of is
    /// what puts a form away rather than standing it in a corner — see
    /// [`Stands::Beside`](crate::prompt::Stands).
    #[test]
    fn a_footprint_skips_what_is_not_drawn_and_is_nothing_where_none_is() {
        let lens = lens();
        // Well behind the eye, so the near plane clips it — which is the one
        // way a position reaches here and answers with nothing.
        let behind = lens.camera.eye() - lens.facing() * 100.0;
        assert_eq!(lens.screen_of(behind), None, "the fixture is still drawn");
        let front = [Vec3::new(1.0, 0.5, -1.0), Vec3::new(-2.0, -1.5, 0.5)];

        let drawn = lens.footprint(front.into_iter()).expect("both are in view");
        assert_eq!(
            lens.footprint(front.into_iter().chain([behind])),
            Some(drawn),
            "a point the projection dropped moved the box"
        );
        assert_eq!(lens.footprint([behind, behind].into_iter()), None);
        assert_eq!(lens.footprint(std::iter::empty()), None);
    }
}
