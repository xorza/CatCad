//! A run of text drawn at a point in the world, at a size the zoom cannot
//! change.

use crate::aim::{self, Aim};
use crate::hit::{Hit, HitAt, Precedence};
use crate::primitive::Primitive;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::viewport::Viewport;
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
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
/// Which *way* it reaches is a third thing again, and the one a run can say
/// something about the world with: a run can run across the screen, or be
/// turned into a plane so it reads as lettering on a drawing rather than as a
/// note pinned over one. Turning changes the direction alone — see [`Facing`].
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
    /// What this is for, which decides what a click meant for two things at
    /// once lands on. See [`Precedence`].
    pub precedence: Precedence,
    /// What a pick that lands here reports. See [picking](crate#picking).
    pub tag: Option<Tag>,
    /// Which way the run is set, and what surface it takes its depth from. See
    /// [`Facing`].
    pub facing: Facing,
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
            precedence: Precedence::default(),
            tag: None,
            facing: Facing::default(),
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

    /// Where the run's top-left corner sits relative to its anchor, in logical
    /// pixels along the run's own axes.
    ///
    /// The whole of what [`Text::anchor`] decides, and asked by both halves of
    /// putting a run somewhere: the renderer folds it into every glyph it
    /// places, and a pick opens the box it measures the cursor against. Two
    /// spellings of it would be a run drawn in one place and clicked in
    /// another.
    ///
    /// Zero where the run has never been laid out, which is the nothing
    /// [`Text::extent`] answers with — and neither caller reaches here without
    /// having asked about that first.
    pub(crate) fn origin(&self) -> Vec2 {
        -self.anchor * self.extent()
    }

    /// How far the cursor fell from the run's box, in logical pixels, or `None`
    /// where there is no box to have fallen from.
    ///
    /// `None` covers both ways a run can fail to be somewhere: an anchor the
    /// projection does not draw, and an extent nothing has measured — a run
    /// that has never been laid out, or one that says nothing.
    ///
    /// Measured against the run's **own axes** rather than the screen's, and
    /// that is what lets a run turned into a plane be tested by the same
    /// rectangle as one square to the viewer: turning is a rotation, a rotation
    /// is its own inverse transposed, so bringing the cursor onto those axes
    /// costs two dot products and leaves the box alone. A box built the other
    /// way round would be a rotated rectangle on screen and would want a
    /// polygon test — a second idea of what a run's box is, for no gain.
    fn reach_from(&self, aim: &Aim) -> Option<f32> {
        let extent = self.extent.get();
        if extent.x <= 0.0 || extent.y <= 0.0 {
            return None;
        }
        let from_anchor = aim.cursor - aim.screen_of(self.position)?;
        let cursor = match self.facing {
            Facing::Screen { .. } => from_anchor,
            Facing::Turned(turn) => {
                let axes = turn.axes(self.position, aim.view_proj, aim.viewport);
                Vec2::new(from_anchor.dot(axes.advance), from_anchor.dot(axes.down))
            }
        };
        let origin = self.origin();
        Some(aim::reach_to_box(
            cursor,
            Rect::new(origin.x, origin.y, extent.x, extent.y),
        ))
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
        let screen = self.reach_from(aim)?;
        (screen <= aim.radius)
            .then(|| aim.hit(tag, HitAt::Text, self.precedence, self.position, screen))
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

    fn reaches(&self, mut include: impl FnMut(Vec3)) {
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

    /// Set which way the run is turned. See [`Facing`].
    pub fn facing(mut self, facing: Facing) -> Self {
        self.facing = facing;
        self
    }
}

/// What a run is set against: the screen, or a plane of the world.
///
/// Two states rather than a normal beside a direction, because the third
/// combination is meaningless — a run turned into a plane it takes no depth from
/// would be lettering lying on a surface and fighting it — and an enum is how a
/// meaningless combination stops being expressible.
///
/// Both are sized in *logical pixels*. Turning a run changes the direction its
/// box runs in and nothing else: it does not foreshorten, and the zoom cannot
/// reach it. See [`Text`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Facing {
    /// Square to the viewer, running across the screen.
    ///
    /// `on` is the surface the run's depth follows, as a unit normal, where it
    /// lies on one — a wide label is wide enough for the surface under it to
    /// rise through, and this is what gives its corners the surface's own depth
    /// rather than the anchor's. See [overlays](crate#overlays).
    Screen { on: Option<Vec3> },
    /// Turned into a plane: the run advances along the plane's own axis as the
    /// projection draws it, at the size it would have had square to the viewer.
    ///
    /// What lettering on a drawing is, as against a note pinned over one. The
    /// surface its depth follows is the plane it is turned into, so there is
    /// nothing separate to declare.
    Turned(Turn),
}

impl Default for Facing {
    /// Square to the viewer and belonging to no surface, which is what a run
    /// that has said nothing about either is.
    ///
    /// Hand-written because `derive` can default only to a unit variant, and
    /// the state wanted here carries a field.
    fn default() -> Self {
        Self::Screen { on: None }
    }
}

impl Facing {
    /// The surface the run's depth follows, as a unit normal, where it follows
    /// one.
    ///
    /// One question both states answer, so that whatever is deciding depth asks
    /// it once rather than matching on how the run happens to be turned — the
    /// two are separate decisions and only one of them is depth's.
    pub fn normal(self) -> Option<Vec3> {
        match self {
            Self::Screen { on } => on,
            Self::Turned(turn) => Some(turn.normal),
        }
    }

    /// The direction the run advances along, where it is turned into a plane,
    /// and `None` where it runs across the screen.
    ///
    /// Beside [`Facing::normal`] and answered on the same terms, because the
    /// two are the pair a [`Turn`] is made of: whatever is unpacking one is
    /// unpacking both, and it should not have to know which state it is looking
    /// at to do either.
    pub fn right(self) -> Option<Vec3> {
        match self {
            Self::Screen { .. } => None,
            Self::Turned(turn) => Some(turn.right),
        }
    }
}

/// The plane a run is turned into, and which way round in it the run is set.
///
/// Both, because a normal alone says which surface and not which way round on
/// it: the same plane carries lettering at any angle, and which one it is at is
/// the caller's — a sketch sets its marks along its own +x, and a dimension
/// could later be set along the span it measures instead.
///
/// One direction and a normal, rather than the plane's two axes. Naming a
/// second axis would read as though which way the *box* runs were the caller's
/// too, and it is not: it is derived, deliberately, so that a run cannot come
/// out mirrored or sheared. See [`Turn::axes`].
///
/// Both are expected to be unit length, and `right` to lie in the plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turn {
    /// World direction the run advances along, which is the whole of what
    /// decides the angle it is set at — the plane's own +x, for a sketch.
    pub right: Vec3,
    /// The plane's unit normal: what the run's depth follows, and what says
    /// which surface `right` is a direction *of*.
    ///
    /// Its sign is nobody's business. Depth reads a plane as a surface to take
    /// a gradient over rather than as a side to be on, and which side the eye
    /// is on decides nothing here — see [`Turn::frame`].
    pub normal: Vec3,
}

impl Turn {
    /// A run set along `right`, on the plane `normal` names — each normalized.
    pub fn new(right: Vec3, normal: Vec3) -> Self {
        let (right, normal) = (right.normalize_or_zero(), normal.normalize_or_zero());
        debug_assert!(
            right.dot(normal).abs() < 1e-3,
            "{right:?} does not lie in the plane {normal:?} names"
        );
        Self { right, normal }
    }

    /// The screen axes the run is set along where it is anchored at `at`.
    ///
    /// **The whole of how a turned run is placed, and the one statement of
    /// it.** Three readers agree by reading this: the box hangs along these, a
    /// pick brings the cursor into them, and an application standing something
    /// of its own over the run measures from them. The vertex shader is a fourth
    /// and cannot call it, so it builds the same rule — the same arrangement
    /// [`MIN_RUN_PX`](crate::Viewport) is under, where one number is stated in
    /// Rust and handed to the shader.
    ///
    /// `at` is expected to be somewhere the projection draws. Behind the eye
    /// there is no screen direction to answer with and what comes back means
    /// nothing — ask
    /// [`Camera::screen_of`](crate::Camera::screen_of) first, which every caller
    /// here is doing anyway to find where the run's anchor landed.
    ///
    /// **Along the plane's own advance, the way up that reads.** Where the
    /// plane's `right` goes on screen, taken with whichever sign points into the
    /// right half of the screen: at ninety degrees nothing happens and a degree
    /// further the whole frame comes round, so a sketch worked at any angle
    /// keeps its numbers the right way up rather than half of them upside down.
    ///
    /// **Down is the perpendicular**, not the plane's own down projected. That
    /// is what makes the box a rotated rectangle rather than a sheared one — the
    /// run holds the size it would have had square to the viewer — and it is
    /// also, on its own, the whole of why a run seen from *behind* its plane
    /// reads rather than mirroring. A frame built on the perpendicular has the
    /// screen's own handedness at every angle, so there is no arrangement in
    /// which the glyphs come out backwards, and a side test would be a second
    /// rule with nothing left for it to decide. What is lost with the projected
    /// down is the foreshortening, which constant size had already ruled out.
    ///
    /// **Screen-horizontal** where the advance is pointing at the eye and has
    /// no direction on screen to be set along. The same frame an unturned run
    /// gets, and the right limit: a run advancing at the eye is a column of
    /// glyphs on edge, and no angle makes it legible.
    pub fn axes(self, at: Vec3, view_proj: Mat4, viewport: Viewport) -> Axes {
        let here = view_proj * at.extend(1.0);
        let along = screen_direction(self.right, here, view_proj, viewport);
        // The plane's other direction, and read only as the scale the advance is
        // measured against. Square to the advance, so the two cannot both be
        // pointing at the eye — which is what makes it a yardstick rather than
        // another thing to have collapsed.
        let across = screen_direction(self.normal.cross(self.right), here, view_proj, viewport);
        let advance = if along.length_squared() <= across.length_squared() * COLLAPSED {
            Vec2::X
        } else {
            let advance = along.normalize();
            if advance.x < 0.0 { -advance } else { advance }
        };
        Axes {
            advance,
            down: advance.perp(),
        }
    }
}

/// The screen axes a run is set along: which way it advances, and which way its
/// own box runs down.
///
/// Both, rather than the advance for a caller to turn: the down is one of two
/// perpendiculars and only one of them keeps the pair the way round that says
/// the run is not mirrored, so a caller that took the other would measure a box
/// the glyphs are not in. See [`Turn::axes`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axes {
    /// Unit, in pixels, along the run.
    pub advance: Vec2,
    /// Unit, in pixels, down the run's own box and square to the advance.
    pub down: Vec2,
}

/// How far the run's advance may collapse against the plane's other direction
/// before there is no direction left to set it along, as a ratio of their
/// squared screen lengths.
///
/// A ratio rather than a length in pixels, because both are projections of a
/// unit world direction and carry whatever scale the projection is at: what is
/// being asked is whether the advance has turned to face the eye, which is a
/// question about the two of them and not about how large the run is drawn.
///
/// A thousandth in length, which is an advance within about a twentieth of a
/// degree of the eye. Both collapsing at once would need two independent
/// directions pointing at one eye, which is the anchor being *at* the eye —
/// where nothing is drawn, and where this answers first and hands back the
/// fallback.
///
/// Reachable because the vertex shader asks the same question and must answer
/// it the same way — it is handed this at pipeline creation, the arrangement
/// [`MIN_RUN_PX`](crate::viewport::MIN_RUN_PX) is already under. A run that fell
/// back on one side and not the other would be drawn along one direction and
/// clicked along another.
pub(crate) const COLLAPSED: f32 = 1e-6;

/// Which way `world` runs on screen where a point of clip position `here` is
/// drawn, in pixels with y running down — up to a positive scale.
///
/// The tangent of the projection itself, by the quotient rule on
/// `ndc = clip.xy / clip.w`, rather than a step taken along the direction and
/// projected. A step has a size, the size changes the answer under perspective,
/// and every reader of the rule would then have to be handed the same one; this
/// has no size to agree about.
///
/// The scale left out is `clip.w²` from the quotient rule and the half that
/// turns an NDC span into a pixel one, both of them positive — so the direction
/// is exact and only the length is not, and length is read here only against
/// another answer from the same call.
fn screen_direction(world: Vec3, here: Vec4, view_proj: Mat4, viewport: Viewport) -> Vec2 {
    let there = view_proj * world.extend(0.0);
    let ndc = there.xy() * here.w - here.xy() * there.w;
    // NDC counts y up from the middle and the framebuffer counts it down from
    // the top, which for a difference is the flip and nothing else.
    ndc * Vec2::new(1.0, -1.0) * viewport.extent()
}

impl Styled for Text {
    fn color_mut(&mut self) -> &mut Vec3 {
        &mut self.color
    }

    fn tag_mut(&mut self) -> &mut Option<Tag> {
        &mut self.tag
    }

    fn precedence_mut(&mut self) -> &mut Precedence {
        &mut self.precedence
    }
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
