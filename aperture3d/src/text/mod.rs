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
    /// that is what lets a run laid in a plane be tested by the same rectangle
    /// as one square to the viewer. Its box is that rectangle projected, which
    /// is a parallelogram; taking the projection's own linearization at the
    /// anchor turns it back into the rectangle for the price of a two-by-two
    /// inverse, where a polygon test would be a second idea of what a run's box
    /// is.
    ///
    /// Exact under parallel rays, and under perspective wrong only by however
    /// much the projection curves across a box a few dozen pixels wide — far
    /// inside the pixel a reach is compared in.
    ///
    /// **The overshoot comes back out to the screen before it is measured.** A
    /// laid run's own pixels are foreshortened, so a cursor two pixels under a
    /// raked box is further than two in the box's frame — and what the answer is
    /// compared against is a radius in *screen* pixels.
    fn reach_from(&self, aim: &Aim) -> Option<f32> {
        let extent = self.extent.get();
        if extent.x <= 0.0 || extent.y <= 0.0 {
            return None;
        }
        let origin = self.origin();
        let box_of = Rect::new(origin.x, origin.y, extent.x, extent.y);
        let Facing::Turned(turn) = self.facing else {
            let from_anchor = aim.cursor - aim.screen_of(self.position)?;
            return Some(aim::reach_to_box(from_anchor, box_of));
        };
        // What one logical pixel of the run reaches on screen: it is sized
        // against the screen and built in the world, so this is the step between
        // the two.
        let step = aim.world_per_pixel(self.position);
        // A lift is not a fourth thing to place the box by — it moves the point
        // the box hangs off, and everything after it is what it always was. It
        // holds still through the mirror and the half turn alike by being
        // resolved in the plane's own axes rather than the run's — see
        // [`Turn::lift_world`].
        //
        // Bailing here rather than later is what earns the axes below: they are
        // meaningless for a point the projection does not draw.
        let from_anchor = aim.cursor - aim.screen_of(self.position + turn.lift_world() * step)?;
        // Settled at the run's *unlifted* anchor, both of them, because that is
        // where the vertex shader settles them: it has the anchor's own clip
        // position and nothing else. A lifted point is a few pixels off and
        // would be marginally the better linearization, and disagreeing with
        // what was drawn would cost more than it bought.
        let axes = turn.axes(self.position, aim.view_proj, aim.viewport);
        let here = aim.view_proj * self.position.extend(1.0);
        let across = screen_tangent(axes.advance * step, here, aim.view_proj, aim.viewport);
        let down = screen_tangent(axes.down * step, here, aim.view_proj, aim.viewport);
        // How much screen the box covers against how much it would cover face
        // on, which for a plane merely tilted is the cosine of the tilt. Under
        // the floor there is nothing to have been clicked in — see [`EDGE_ON`],
        // which is a floor on what can be seen rather than on what can be
        // divided by.
        let area = across.perp_dot(down);
        if area.abs() <= EDGE_ON {
            return None;
        }
        let local = Vec2::new(from_anchor.perp_dot(down), across.perp_dot(from_anchor)) / area;
        // Back out through the same pair, so what is measured is a screen
        // distance however the box is foreshortened.
        let past = aim::into_box(local, box_of);
        Some((across * past.x + down * past.y).length())
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

/// How a run is laid in a plane: which surface, which way round on it, and how
/// far off the point it names.
///
/// A direction and a normal rather than the plane's two axes. Naming a second
/// axis would read as though which way the *box* runs were the caller's too, and
/// it is not: it is derived, deliberately, so that a run cannot come out
/// mirrored or sheared. See [`Turn::axes`].
///
/// A direction and not just the plane, because a normal alone says which surface
/// and not which way round on it: the same plane carries lettering at any angle,
/// and which one it is at is the caller's — a sketch may set its marks along its
/// own +x, or a dimension along the span it measures.
///
/// `right` and `normal` are expected to be unit length, and `right` to lie in
/// the plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turn {
    /// World direction the run advances along, which is the whole of what
    /// decides the angle it is set at.
    pub right: Vec3,
    /// The plane's unit normal: what the run's depth follows, and what says
    /// which surface `right` is a direction *of*.
    ///
    /// Its sign is nobody's business. Depth reads a plane as a surface to take
    /// a gradient over rather than as a side to be on, and which side the eye
    /// is on decides nothing here — see [`Turn::axes`].
    pub normal: Vec3,
    /// How far the run's box floats off the point it names, in logical pixels
    /// across and down the plane's axes **as authored** — along `right`, and
    /// along `normal × right`.
    ///
    /// **Nothing the projection decides reaches it, and that is the whole of
    /// what it is for.** An offset written into [`Text::anchor`] rides in the
    /// run's *own* frame, and both rules that settle that frame move it: the
    /// mirror that keeps a run readable from behind its plane, and the half turn
    /// that keeps it the right way up. Either swings the box off whatever it was
    /// standing clear of. Stated here it is fixed in the plane, so a run that
    /// comes round to stay readable only changes direction.
    ///
    /// Resolved in the world rather than against [`Turn::axes`] — see
    /// [`Turn::carried`] — because those carry *two* camera-dependent signs, the
    /// mirror and the half turn, and a lift that went through them would pick up
    /// both.
    ///
    /// Which leaves one thing for the caller: a box hung off a *centred* anchor
    /// is mapped onto itself by that half turn, so its place holds outright. One
    /// hung off any other fraction is reflected through the lifted point, which
    /// is a real answer but rarely the wanted one — see [`Text::anchor`].
    ///
    /// In pixels rather than world units, like everything else about a laid
    /// run's size: how far a mark stands off the line it measures is a thing you
    /// read at a glance, not a thing the model has an opinion about.
    pub lift: Vec2,
}

impl Turn {
    /// A run set along `right`, on the plane `normal` names — each normalized —
    /// and sitting on the point it names.
    pub fn new(right: Vec3, normal: Vec3) -> Self {
        let (right, normal) = (right.normalize_or_zero(), normal.normalize_or_zero());
        debug_assert!(
            right.dot(normal).abs() < 1e-3,
            "{right:?} does not lie in the plane {normal:?} names"
        );
        Self {
            right,
            normal,
            lift: Vec2::ZERO,
        }
    }

    /// Float the run's box this far off that point. See [`Turn::lift`].
    pub fn lifted(mut self, lift: Vec2) -> Self {
        self.lift = lift;
        self
    }

    /// [`Turn::lift`] as a world displacement, per logical pixel of it.
    ///
    /// The plane's axes as authored, so this owes the projection nothing: a run
    /// hangs off the point it names by exactly this however the camera comes
    /// round, which is what leaves a lifted run still while it turns.
    ///
    /// Down is `normal × right` rather than a second axis the caller states,
    /// for the reason [`Turn`] has no second axis at all.
    pub fn lift_world(self) -> Vec3 {
        self.right * self.lift.x + self.normal.cross(self.right) * self.lift.y
    }

    /// The plane directions the run is laid along where it is anchored at `at`,
    /// with both signs settled.
    ///
    /// **The whole of how a laid run is placed, and the one statement of it.**
    /// Three readers agree by reading this: the box is built along these, a pick
    /// brings the cursor onto them, and an application standing something of its
    /// own over the run measures from them. The vertex shader is a fourth and
    /// cannot call it, so it builds the same two rules — the same arrangement
    /// [`MIN_RUN_PX`](crate::Viewport) is under, where one number is stated in
    /// Rust and handed to the shader.
    ///
    /// `at` is expected to be somewhere the projection draws. Behind the eye
    /// there is no screen direction to settle a sign against and what comes back
    /// means nothing — ask
    /// [`Camera::screen_of`](crate::Camera::screen_of) first, which every caller
    /// here is doing anyway to find where the run's anchor landed.
    ///
    /// Two rules, and in the plane both of them are real — where a run set in
    /// *screen* space could derive its down as the advance's perpendicular and
    /// so could never mirror at all, the down here is a world direction and
    /// which of the two it is decides whether the glyphs come out backwards.
    ///
    /// **Un-mirrored**: of the plane's two ways to run down, take the one whose
    /// projection winds the way the screen does. That is what turns a run seen
    /// from behind its plane the right way round, and it is read off the
    /// projected pair rather than from where the eye is, so no camera position
    /// is wanted.
    ///
    /// **Upright**: where the advance would point into the left half of the
    /// screen, turn the whole pair a half turn *in the plane*. At ninety degrees
    /// nothing happens and a degree further it comes round, so a sketch worked at
    /// any angle keeps its numbers the right way up rather than half of them
    /// upside down. A proper rotation, which is why it can follow the mirror
    /// without undoing it.
    ///
    /// Neither needs a guard for the degenerate case, which is the plane seen
    /// edge-on: the tests are a winding and a sign, both of which a collapsed
    /// projection answers deterministically, and a run whose plane covers no
    /// screen is one nobody can read or click either way. What refuses it is the
    /// area its box comes to — see [`Text::pick`].
    pub fn axes(self, at: Vec3, view_proj: Mat4, viewport: Viewport) -> Axes {
        let here = view_proj * at.extend(1.0);
        let across = self.normal.cross(self.right);
        let along = screen_tangent(self.right, here, view_proj, viewport);
        let sideways = screen_tangent(across, here, view_proj, viewport);
        // Of the plane's two ways to run down, the one that winds the way the
        // screen does.
        let down = if along.perp_dot(sideways) >= 0.0 {
            across
        } else {
            -across
        };
        // And the half turn, which is a sign on both rather than a second frame
        // — which is what makes it a rotation and so leaves the choice above
        // where it was.
        let upright = if along.x < 0.0 { -1.0 } else { 1.0 };
        Axes {
            advance: self.right * upright,
            down: down * upright,
        }
    }
}

/// The plane directions a run is laid along: the way it advances, and the way
/// its own box runs down.
///
/// World directions, both unit and square to each other, both in the run's
/// plane. Where a run *sits* on those axes is the anchor's and how far it
/// reaches along them is the shaping's; this is only which way they point, which
/// is the half the projection has a say in. See [`Turn::axes`].
///
/// The advance is [`Turn::right`] *as settled* and so is sometimes its negation
/// — named apart for that reason, since the two are a half turn out from each
/// other exactly when it matters and a box built on the wrong one hangs off the
/// wrong side of its anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axes {
    pub advance: Vec3,
    pub down: Vec3,
}

/// How little of its face-on area a laid run's box may cover before there is
/// nothing on screen to have been clicked in.
///
/// Its box covers one screen pixel per logical pixel when the plane faces the
/// viewer and less as the plane turns away — for a plain tilt, exactly the
/// cosine of it. So this is a fraction rather than an area: a thousandth is
/// within a twentieth of a degree of edge-on, where the whole run has collapsed
/// to a line and no cursor is on it.
///
/// A policy rather than an arithmetic guard, though it serves as one. The
/// arithmetic degrades gracefully: as the box thins, what comes back goes on
/// being the distance to it, and only exactly edge-on is there a zero to divide
/// by. What it would mean is a mark covering a hundredth of a pixel answering
/// clicks along its whole length, which is a mark you cannot see and can grab.
///
/// A pick's floor and not the drawing's. Type laid in a plane goes on
/// foreshortening all the way to nothing, which is what being in the plane means
/// and what the drawing should show.
const EDGE_ON: f32 = 1e-3;

/// How far a step along `world` carries on screen where a point of clip
/// position `here` is drawn: pixels per world unit, with y running down.
///
/// The tangent of the projection itself, by the quotient rule on
/// `ndc = clip.xy / clip.w`, rather than a step taken along the direction and
/// projected. A step has a size, the size changes the answer under perspective,
/// and every reader of the rule would then have to be handed the same one; this
/// has no size to agree about.
///
/// A rate rather than only a bearing, because both are wanted and from one
/// call: settling the signs of a run's axes reads the bearing, and picking one
/// needs the two rates its box is built on to invert them.
fn screen_tangent(world: Vec3, here: Vec4, view_proj: Mat4, viewport: Viewport) -> Vec2 {
    let there = view_proj * world.extend(0.0);
    let ndc = (there.xy() * here.w - here.xy() * there.w) / (here.w * here.w);
    // NDC counts y up from the middle and the framebuffer counts it down from
    // the top, which for a difference is the flip and nothing else; and it spans
    // two units across the whole target, which is the half.
    ndc * Vec2::new(1.0, -1.0) * viewport.extent() * 0.5
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
