//! A run of text drawn at a point in the world, at a size the zoom cannot
//! change.

use crate::aim::{self, Aim};
use crate::hit::{Hit, HitAt, Precedence};
use crate::primitive::Primitive;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::text::turn::Facing;
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
    fn reach_from(&self, aim: &Aim) -> Option<Reach> {
        let extent = self.extent.get();
        if extent.x <= 0.0 || extent.y <= 0.0 {
            return None;
        }
        let origin = self.origin();
        let box_of = Rect::new(origin.x, origin.y, extent.x, extent.y);
        let Facing::Turned(turn) = self.facing else {
            let from_anchor = aim.cursor - aim.screen_of(self.position)?;
            return Some(Reach {
                screen: aim::reach_to_box(from_anchor, box_of),
                at: self.touched(aim, self.position),
            });
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
        let hangs = self.position + turn.lift_world() * step;
        let from_anchor = aim.cursor - aim.screen_of(hangs)?;
        // Settled at the run's *unlifted* anchor, both of them, because that is
        // where the vertex shader settles them: it has the anchor's own clip
        // position and nothing else. A lifted point is a few pixels off and
        // would be marginally the better linearization, and disagreeing with
        // what was drawn would cost more than it bought.
        let axes = turn.axes(self.position, aim.view_proj, aim.viewport);
        let here = aim.view_proj * self.position.extend(1.0);
        let across = aim
            .viewport
            .screen_tangent(axes.advance * step, here, aim.view_proj);
        let down = aim
            .viewport
            .screen_tangent(axes.down * step, here, aim.view_proj);
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
        Some(Reach {
            screen: (across * past.x + down * past.y).length(),
            at: self.touched(aim, hangs),
        })
    }

    /// Where the cursor meets the run, in the world, given the point the run's
    /// box hangs off.
    ///
    /// **A run is a box, and a box on a surface seen at an angle is not all at
    /// one depth.** Where the cursor falls on it decides what is in front of it,
    /// so a run answering from its anchor answers about a place the cursor is
    /// not — and on a drawing lying flat that is the whole lower half of every
    /// label. The face the drawing encloses is coplanar with the label and so
    /// nearer than the label's *centre* everywhere below it: measured from the
    /// centre, the bottom half of every number read as being behind the sheet it
    /// is drawn on and could not be clicked, while the empty space above it
    /// could.
    ///
    /// Read off [`Facing::normal`] rather than off which way the run is set,
    /// because this is the depth question and that is what the surface a run's
    /// depth follows is for — the same surface the shader takes its corners'
    /// depth from, whether it lays them in a plane or slides a screen-space quad
    /// along one.
    ///
    /// A run belonging to no surface has only its anchor's depth, which is also
    /// what the shader gives every corner of one.
    fn touched(&self, aim: &Aim, hangs: Vec3) -> Vec3 {
        let Some(normal) = self.facing.normal() else {
            return hangs;
        };
        let ray = aim.ray();
        let along = normal.dot(ray.direction);
        if along.abs() <= GRAZING {
            return hangs;
        }
        ray.origin + ray.direction * ((hangs - ray.origin).dot(normal) / along)
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
        let reach = self.reach_from(aim)?;
        (reach.screen <= aim.radius)
            .then(|| aim.hit(tag, HitAt::Text, self.precedence, reach.at, reach.screen))
    }
}

/// Where a run stands in the world, and how far the cursor fell from the box it
/// stands in.
///
/// The two come out of one measurement and are wanted together: the depth a hit
/// is ordered and occluded by is a depth *where the run is*, and where the run
/// is is what the lift decided on the way to answering the reach. Two calls for
/// them would be two chances to take the depth from the point a run names
/// rather than from the run.
///
/// [`Reach::at`] is what a run standing clear of its geometry makes worth
/// naming. For a run square to the viewer it is the anchor and nothing has
/// happened; for one laid in a plane and lifted off the point it names, the
/// anchor is a place the run is *about* and can be a long way from where it was
/// drawn — a whole label's width along the plane, which under a grazing view is
/// most of the depth between the drawing and whatever surface it lies on.
#[derive(Debug, Clone, Copy)]
struct Reach {
    /// How far outside the run's box the cursor fell, in screen pixels, and
    /// zero anywhere within it.
    screen: f32,
    /// Where the run's box hangs — the point it names, carried by the lift.
    at: Vec3,
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

/// How square the surface a run's depth follows has to be to the cursor's ray
/// before the depth is read off that surface rather than off the run's anchor.
///
/// The cosine between the two, so a thousandth is within a twentieth of a degree
/// of the ray lying *in* the surface — where the intersection runs off to
/// infinity and the anchor's own depth is the only honest answer left. See
/// [`Text::touched`].
const GRAZING: f32 = 1e-3;

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

pub(crate) mod turn;

#[cfg(test)]
mod tests;
