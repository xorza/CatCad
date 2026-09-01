//! A rectangle of the view, and how what is drawn for one lands in another.

use crate::viewport::Viewport;
use glam::{IVec2, Mat4, UVec2, Vec2, Vec4};

/// A rectangle of the view, in physical pixels from the view's own top-left
/// corner.
///
/// **A shape rather than a role, because two different things are one.** The
/// target is a tile of the view: palantir allocates a `GpuView`'s target for
/// what is on screen rather than for the whole widget — a rect is allowed to
/// reach past the window, and a scroll can put most of one outside its pane —
/// so the target is sometimes a window onto the view rather than the view
/// itself. A [`Pane`](crate::Pane) covers a tile of the view too, and the two
/// are the same tile only when one pane has the whole view to itself.
///
/// Whole pixels, because the scissor that confines a pane counts in them. A
/// tile ending half way through one would be skewed to a boundary its own
/// scissor could not cut on.
///
/// The corner is signed and the extent is not. A pane pinned into a view too
/// small to hold it starts left of the view's own corner, which is a place; a
/// rectangle reaching backwards is not a shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Tile {
    /// Where the tile begins — [`Rect::min`](palantir::Rect::min)'s
    /// counterpart, and named for it so the crate has one word for a corner.
    pub(super) min: IVec2,
    pub(super) size: UVec2,
}

impl Tile {
    /// The middle of the tile, in the view's own pixels.
    fn middle(self) -> Vec2 {
        self.min.as_vec2() + self.size.as_vec2() * 0.5
    }

    /// What a camera drawing into this tile frames for.
    pub(super) fn viewport(self) -> Viewport {
        Viewport::new(self.size)
    }

    /// The clip-space transform that lands what was drawn for this tile where
    /// `target` shows it.
    ///
    /// A scale about `target`'s middle, in NDC. The translation is folded
    /// into the `w` column rather than applied after the divide, because these
    /// are clip coordinates and a constant added there would move a vertex by
    /// less the further off it was.
    ///
    /// Y is negated because a tile is measured down from the view's top where
    /// NDC counts up from its middle — the same flip every conversion between
    /// the two makes.
    ///
    /// **The tile is allowed to reach outside `target`, and the answer still
    /// holds there.** A pane pinned to a corner of a view that a scroll has
    /// slid off the target is exactly that. What confines it to the target is
    /// [`Tile::scissor`], which clips — where a viewport rect could not, wgpu
    /// refusing one that leaves the attachment.
    pub(super) fn onto(self, target: Tile) -> Mat4 {
        let scale = self.size.as_vec2() / target.size.as_vec2();
        let off = (self.middle() - target.middle()) / (target.size.as_vec2() * 0.5);
        Mat4::from_cols(
            Vec4::new(scale.x, 0.0, 0.0, 0.0),
            Vec4::new(0.0, scale.y, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(off.x, -off.y, 0.0, 1.0),
        )
    }

    /// The part of this tile that lands inside `target`, in `target`'s own
    /// pixels — and `None` where none of it does.
    ///
    /// What a pane is clipped to, and why a pane wholly off the target costs a
    /// skipped draw rather than a refused scissor.
    pub(super) fn within(self, target: Tile) -> Option<Tile> {
        let low = self.min.max(target.min);
        let high = (self.min + self.size.as_ivec2()).min(target.min + target.size.as_ivec2());
        let size = high - low;
        (size.x > 0 && size.y > 0).then(|| Tile {
            min: low - target.min,
            size: size.as_uvec2(),
        })
    }

    /// Confine what is drawn next to this tile.
    ///
    /// Only a tile of the target may be handed here, which is what
    /// [`Tile::within`] answers: wgpu refuses a scissor that leaves the
    /// attachment, and a corner left of it would wrap rather than being
    /// refused.
    pub(super) fn scissor(self, pass: &mut wgpu::RenderPass<'_>) {
        debug_assert!(
            self.min.min_element() >= 0,
            "{self:?} is outside its target"
        );
        pass.set_scissor_rect(
            self.min.x as u32,
            self.min.y as u32,
            self.size.x,
            self.size.y,
        );
    }
}

#[cfg(test)]
mod tests;
