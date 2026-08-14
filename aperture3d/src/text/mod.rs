//! A run of text drawn at a point in the world, at a size the zoom cannot
//! change.

use crate::aim::Aim;
use crate::hit::{Hit, HitAt};
use crate::primitive::Primitive;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::{Vec2, Vec3};
use palantir::{GlyphFont, Rect, Size, TextGlyphs};
use std::cell::Cell;

/// Something written in the scene: a label on a point, a dimension on a
/// drawing.
///
/// An [overlay](crate#overlays) like a stroke or a marker — unlit, unculled,
/// and sized in *logical pixels*, so a label holds its size however far the
/// camera pulls back. Legibility is the whole point of text, and text that
/// shrank with the model would be saying something about the model instead.
///
/// Where it is *anchored* is a world position; how far it reaches from there is
/// a screen measurement, which nothing outside a renderer can take — see
/// [`Text::extent`].
///
/// `Default` draws nothing: an empty string in a font of no size.
#[derive(Debug, Clone)]
pub struct Text {
    /// Where the run is anchored in the world.
    pub position: Vec3,
    /// What it says. Owned rather than borrowed, so a scene outlives whatever
    /// produced it; written in place through
    /// [`refill`](crate::Batch::refill) so a label rewritten every frame keeps
    /// its allocation.
    pub content: String,
    pub font: GlyphFont,
    /// Linear-RGB.
    pub color: Vec3,
    /// Where in the run's own box [`Text::position`] sits: `(0, 0)` its
    /// top-left corner, `(0.5, 0.5)` its middle, `(1, 1)` its bottom-right.
    ///
    /// A fraction rather than a named alignment, because the useful anchors are
    /// not a short list — a dimension centres on its line, a leader hangs off
    /// one end, a note clears a corner — and every one of them is a pair of
    /// numbers either way.
    pub anchor: Vec2,
    /// Depth-test bias in steps of depth-buffer resolution, positive toward
    /// the viewer. See [overlays](crate#overlays).
    pub z_offset: i32,
    /// What a pick that lands here reports. See [picking](crate#picking).
    pub tag: Option<Tag>,
    /// The plane this run lies on, as a unit normal, when it lies on one. See
    /// [overlays](crate#overlays).
    pub plane_normal: Option<Vec3>,
    /// How far the run reaches on screen, in logical pixels — remembered from
    /// the last time it was laid out.
    ///
    /// Private because it is not the caller's to state: what a string measures
    /// depends on the faces it is shaped in, which only the renderer's shaper
    /// knows. It is filled when the run is laid out and read by picking, which
    /// is why a run that has never been drawn cannot be picked — it has no box
    /// on screen to have been clicked in.
    ///
    /// A [`Cell`] because it is a memo and not state. What a run *is* — where it
    /// is anchored, what it says, how it is styled — is the caller's, and a
    /// [`Batch`] marks itself whenever any of that is written so the renderer
    /// knows to flatten it again. How wide the run came out is the answer to
    /// that flatten rather than a part of it, and recording it through `&mut`
    /// would mark the batch, so every measured batch would ask to be measured
    /// again on the next frame, forever. Writing it through a shared reference
    /// is what keeps the mark meaning what it says.
    extent: Cell<Vec2>,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            content: String::new(),
            font: GlyphFont::new(0.0),
            color: Vec3::ONE,
            anchor: Vec2::ZERO,
            z_offset: 0,
            tag: None,
            plane_normal: None,
            extent: Cell::new(Vec2::ZERO),
        }
    }
}

impl Text {
    /// `content` anchored at `position`, in white, at `size_px` logical pixels.
    pub fn new(position: Vec3, content: impl Into<String>, size_px: f32) -> Self {
        Self {
            position,
            content: content.into(),
            font: GlyphFont::new(size_px),
            ..Self::default()
        }
    }

    /// How far the run reaches on screen, in logical pixels, or zero where it
    /// has not been laid out.
    pub fn extent(&self) -> Vec2 {
        self.extent.get()
    }

    /// The run's box on screen, in logical pixels from the top-left corner, or
    /// `None` where there is none to have.
    ///
    /// `None` covers both ways a run can fail to be somewhere: an anchor the
    /// projection does not draw, and an extent nothing has measured — a run
    /// that has never been laid out, or one that says nothing.
    fn box_on_screen(&self, aim: &Aim) -> Option<Rect> {
        let extent = self.extent.get();
        if extent.x <= 0.0 || extent.y <= 0.0 {
            return None;
        }
        let top_left = aim.screen_of(self.position)? - self.anchor * extent;
        Some(Rect::new(top_left.x, top_left.y, extent.x, extent.y))
    }

    /// Whether the cursor landed on this run.
    ///
    /// Anywhere inside the box counts, and counts equally: a label is an opaque
    /// thing, not an outline to be aimed at, so the whole of it is the target.
    /// Outside it, the distance to the nearest edge is what the reach is
    /// measured against, which is what lets a cursor a pixel off a small label
    /// still find it.
    pub(crate) fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let screen = distance_to(self.box_on_screen(aim)?, aim.cursor);
        (screen <= aim.radius).then(|| aim.hit(tag, HitAt::Text, self.position, screen))
    }
}

/// A run is an overlay like the other three, and not a [`Flatten`]: how many
/// glyphs it comes to is the shaper's answer rather than the run's. See
/// [`Flatten`].
///
/// [`Flatten`]: crate::primitive::Flatten
impl Primitive for Text {
    fn tag(&self) -> Option<Tag> {
        self.tag
    }

    fn extend_bounds(&self, mut include: impl FnMut(Vec3)) {
        include(self.position);
    }
}

impl Text {
    /// Put `position` at this fraction of the run's own box — see
    /// [`Text::anchor`].
    pub fn anchored(mut self, anchor: Vec2) -> Self {
        self.anchor = anchor;
        self
    }

    /// Bias the run this many steps of depth-buffer resolution toward the
    /// viewer. See [overlays](crate#overlays).
    pub fn z_offset(mut self, z_offset: i32) -> Self {
        self.z_offset = z_offset;
        self
    }

    /// Declare the plane the run lies on. See [`Text::plane_normal`].
    pub fn in_plane(mut self, normal: Vec3) -> Self {
        self.plane_normal = Some(normal.normalize());
        self
    }
}

impl Styled for Text {
    fn color_mut(&mut self) -> &mut Vec3 {
        &mut self.color
    }

    fn tag_mut(&mut self) -> &mut Option<Tag> {
        &mut self.tag
    }
}

/// How far `point` fell outside `rect`, and zero anywhere within.
///
/// Per axis, how far past an edge the point sits — negative between them, which
/// the floor at zero discards. The length of what survives is the distance to
/// the nearest corner when the point is diagonally out, and to the nearest edge
/// when it is out on one axis alone, both of which fall out of the same two
/// lines.
///
/// Here rather than on [`Rect`], which is palantir's and answers
/// [`contains`](Rect::contains) but not this. A pick needs how far *outside* as
/// well, because a cursor a pixel off a small label should still find it.
fn distance_to(rect: Rect, point: Vec2) -> f32 {
    let past = (rect.min - point).max(point - rect.max());
    past.max(Vec2::ZERO).length()
}

/// Take every run's extent from `shaper`, so that what has been laid out knows
/// how far it reaches.
///
/// Reads the runs and writes only their memo, which is why it takes them shared
/// — see [`Text::extent`]. A measuring pass that needed `&mut` would be a pass
/// that marked the batch it measured, and a marked batch is one the renderer
/// lays out again.
///
/// Measured unscaled, so the answer is in logical pixels like every other
/// overlay's size — a label's extent is what it will be drawn at, and how many
/// device pixels that is belongs to whoever rasterizes it.
///
/// Takes the caller's lease rather than the shaper, because measuring is half
/// of laying a run out and the other half needs the same one: a lease is the
/// shaper's exclusive borrow, so a caller that opened one to place glyphs cannot
/// hand over a shaper for this to open another from.
pub(crate) fn measure_all(texts: &[Text], glyphs: &mut TextGlyphs<'_>) {
    for text in texts {
        let Size { w, h } = glyphs.measure(&text.content, text.font, 1.0);
        text.extent.set(Vec2::new(w, h));
    }
}

/// Standing in for the renderer, which is the only thing that lays a run out.
///
/// `cfg(test)` rather than the `internals` feature: a caller outside the crate
/// gets its extents the honest way, by having the run drawn. What this is for is
/// asking what a *pick* does about a box, which wants a box and no GPU.
#[cfg(test)]
impl Text {
    pub(crate) fn measured(self, extent: Vec2) -> Self {
        self.extent.set(extent);
        self
    }
}

#[cfg(test)]
mod tests;
