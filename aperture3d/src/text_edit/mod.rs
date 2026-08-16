//! A line of text typed into at a point in the world.

use crate::aim::Aim;
use crate::hit::{Hit, HitAt, Precedence};
use crate::primitive::Primitive;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::{Vec2, Vec3};
use palantir::{GlyphFont, Rect, TextGlyphs};
use std::cell::{Cell, RefCell};
use std::ops::Range;

/// How wide the caret is drawn, in logical pixels.
///
/// One pixel, and not scaled with the type: a caret is a mark on the surface
/// rather than a letter, and the same rule that keeps a stroke a pixel and a
/// half wide however far the camera pulls back keeps this the width it was
/// drawn at.
const CARET_WIDTH: f32 = 1.0;

/// Room left between the surround and the text, as a fraction of the type size.
///
/// A fraction rather than a length, because it is the one measurement here that
/// is *about* the type: a field at 24px with the padding of a field at 12 reads
/// as a box the text is falling out of.
const PADDING: Vec2 = Vec2::new(0.4, 0.25);

/// How wide a field is drawn however little it says, as a fraction of the type
/// size.
///
/// About two digits. An empty field with no floor under its width is a box the
/// size of its own padding, which is a thing you can neither read as a field
/// nor reliably click into — and a field is at its emptiest exactly when it is
/// waiting to be typed in.
const MIN_WIDTH: f32 = 2.0;

/// A field in the scene: a line of text with a caret in it, anchored at a point
/// in the world and drawn at a size the zoom cannot change.
///
/// The editable sibling of [`Text`](crate::Text), and an
/// [overlay](crate#overlays) on the same terms — unlit, unculled, and sized in
/// *logical pixels*, so a field holds its size however far the camera pulls
/// back. What it adds over a label is a surround to read the text against, a
/// caret, a run picked out, and the [editing](TextEdit::seek) that moves those.
///
/// **Always one line, and always left-aligned.** Not a simplification waiting to
/// be lifted: what a field in a drawing is for is a number, a length or a
/// formula, and every one of those is a line that reads from its left edge. A
/// wrap has nothing to wrap to — a field hugs its own content — and an alignment
/// would only decide where the text sat in the slack under
/// [`min_width`](Self::min_width), which is room for the *next* character and so
/// belongs after the text rather than before it.
///
/// **A field puts its glyphs exactly where a label would.** Its
/// [`anchor`](Self::anchor) is a fraction of the *text*, the same quantity
/// [`Text::anchor`](crate::Text::anchor) is, and the surround grows outward from
/// there — so a label swapped for a field of the same content, font and anchor
/// does not move by a pixel, and the box appears around the number rather than
/// under a number that has just jumped. That is the whole reason the anchor is
/// not a fraction of the box, which is the obvious way to write it and is wrong:
/// the box's size depends on the padding and on
/// [`min_width`](Self::min_width), so every one of those would shift the text.
///
/// **Reads no input**, like everything else in this crate: palantir owns the
/// pointer and the keyboard, so an application maps a keystroke onto
/// [`insert`](Self::insert), [`seek`](Self::seek) and the rest, and a click onto
/// [`byte_at`](Self::byte_at). What lives here is where the caret is and what
/// that makes the field look like — never how it got there.
///
/// **Always the field being typed in.** There is no unfocused state, so the
/// caret is always drawn: a value merely being *shown* is a
/// [`Text`](crate::Text), and one being typed into is this. An application
/// swaps the one for the other, which by the rule above costs no movement.
///
/// **Depth-tested like any other overlay**, which is worth knowing because a
/// field is a thing to be used rather than read: it is hidden by a solid in
/// front of its anchor and crossed by a stroke nearer the eye, exactly as a
/// label is. Nothing here lifts it out of the scene, and nothing should — where
/// a field can be seen from is the same question as where it *is*, and only
/// whoever placed it can answer that. Anchor it toward the viewer.
///
/// `Default` draws nothing: an empty line in a font of no size, in a box with
/// no room in it.
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// Where the field is anchored in the world.
    pub position: Vec3,
    pub font: GlyphFont,
    /// Linear-RGB, the ink and the caret alike — a caret is where the next
    /// letter goes, so it is drawn in the colour that letter will be.
    pub color: Vec3,
    /// Linear-RGB the surround is filled with.
    ///
    /// Opaque, like every colour in this crate: a field is read against its own
    /// surround rather than against whatever the model puts behind it, which is
    /// the whole reason it has one.
    pub background: Vec3,
    /// Linear-RGB the run picked out is washed with, behind the ink.
    pub selection: Vec3,
    /// Where in the **text's** own box [`TextEdit::position`] sits: `(0, 0)` its
    /// top-left corner, `(0.5, 0.5)` its middle, `(1, 1)` its bottom-right.
    ///
    /// Exactly [`Text::anchor`](crate::Text::anchor), measuring exactly the same
    /// box — which is what makes a label and the field that replaces it land on
    /// the same pixels. The surround is *not* in it: it hangs off the text by
    /// [`padding`](Self::padding), outward, and moves nothing.
    ///
    /// A fraction rather than a named alignment, for the reason `Text::anchor`
    /// gives — the useful anchors are not a short list, and every one of them is
    /// a pair of numbers either way.
    pub anchor: Vec2,
    /// Logical pixels the surround reaches past the text, across and down.
    ///
    /// Outward from the text rather than inward from a box, so that what it
    /// changes is how much room the field takes and never where the text is.
    pub padding: Vec2,
    /// Narrowest the surround gives the text room for, in logical pixels, before
    /// the padding is added.
    ///
    /// Taken up on the *right*, because the field is left-aligned and the slack
    /// is room for the next character. A floor and nothing more: a field says
    /// more than this and the surround grows to fit it.
    ///
    /// What it buys is that a field is still a thing you can see and aim at when
    /// it has been emptied — which is exactly when it is waiting to be typed in.
    pub min_width: f32,
    /// What this is for, which decides what a click meant for two things at
    /// once lands on. See [`Precedence`].
    pub precedence: Precedence,
    /// What a pick that lands here reports. See [picking](crate#picking).
    ///
    /// A field is never drawn a second time in a highlight's look, however it is
    /// named: the surround is opaque and covers the whole box, so a copy of it
    /// in one colour would hide the field rather than pick it out. What a field
    /// looks like in each of its states is its own — write
    /// [`background`](Self::background) and [`color`](Self::color) and it is
    /// said exactly.
    pub tag: Option<Tag>,
    /// The plane this field lies on, as a unit normal, when it lies on one. See
    /// [overlays](crate#overlays).
    pub plane_normal: Option<Vec3>,
    /// What it says.
    ///
    /// Private where a label's is public, because the caret is an offset into it
    /// and the two have to move together: a caller that could write the string
    /// could leave the caret past its end or inside a character. Written through
    /// [`set_content`](Self::set_content), which puts the caret back where it
    /// still fits.
    content: String,
    /// Where the caret is, as a byte offset into the content, always at a
    /// character boundary.
    caret: usize,
    /// The other end of the run picked out, on the same terms. Equal to the
    /// caret when nothing is.
    ///
    /// Two offsets rather than a range, because the pair says which end is
    /// moving and a range does not — and which end is moving is the whole of
    /// what shift-arrow means.
    mark: usize,
    /// How far the text reaches on screen, in logical pixels — remembered from
    /// the last time the field was laid out, and zero where nothing has laid it
    /// out.
    ///
    /// The very quantity [`Text::extent`](crate::Text::extent) holds, taken from
    /// the same call on the same shaper, because the two are compared: the
    /// anchor is a fraction of *this*, so a label and a field measuring
    /// differently would place their glyphs differently.
    ///
    /// A memo, for the reason `Text::extent` gives: what a string measures is
    /// the shaper's answer, not the caller's, and recording it through `&mut`
    /// would mark the batch, so every laid-out field would ask to be laid out
    /// again on the next frame forever.
    extent: Cell<Vec2>,
    /// Where a caret sits at every character boundary in the content, in
    /// logical pixels from the text's own left edge — remembered alongside the
    /// extent and on the same terms.
    ///
    /// A [`RefCell`] rather than a [`Cell`] only because the answer is a list;
    /// the argument for writing it through a shared reference is the extent's,
    /// unchanged.
    ///
    /// This is also the whole of what hit-testing a click reads, which is what
    /// keeps [`byte_at`](Self::byte_at) free of the shaper: a pick happens
    /// wherever the application handles a press, and the shaper is the
    /// renderer's.
    stops: RefCell<Vec<Stop>>,
}

/// Where a caret sits at one boundary in a field's content.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stop {
    /// Byte offset into the content — 0 for the first, the content's length for
    /// the last, and every character boundary between them.
    byte: usize,
    /// Logical pixels from the text's own left edge.
    x: f32,
}

/// Where a caret is being sent. See [`TextEdit::seek`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seek {
    /// One character back, and nowhere from the start.
    Left,
    /// One character on, and nowhere from the end.
    Right,
    Start,
    End,
    /// A byte offset — what [`TextEdit::byte_at`] answers a click with. Moved
    /// back to the character boundary it falls in and clamped to the content,
    /// so nothing a caller can pass puts the caret somewhere it cannot be.
    Byte(usize),
}

/// Whether a caret being moved drags the selection along with it.
///
/// An enum rather than a `bool`, because both are what a caller writes at the
/// call site and only one of them says which is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selecting {
    /// Leave nothing picked out — an arrow key on its own.
    Drop,
    /// Take the text between where the caret was and where it lands — an arrow
    /// key with shift.
    Extend,
}

/// Where a field's parts sit relative to its anchor, in logical pixels.
///
/// Worked out by the field because every bit of it is the field's own
/// arithmetic — where the text hangs off the anchor, how far the surround
/// reaches past it, and how far along the text the caret and the selection
/// fall. A renderer deciding any of that would be a second statement of what a
/// field looks like, free to drift from this one.
///
/// `text` is the one that is not the field's alone: it is what a
/// [`Text`](crate::Text) of the same content and anchor would work out for
/// itself, and the two have to agree — see [`TextEdit::anchor`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Parts {
    /// The surround, and the whole of what the field occupies.
    pub(crate) surround: Rect,
    /// Where the text's own origin sits: its left edge, top of the line box.
    pub(crate) text: Vec2,
    /// The run picked out, or `None` where nothing is.
    pub(crate) wash: Option<Rect>,
    /// The caret, which a field always has — see [`TextEdit`].
    pub(crate) caret: Rect,
}

impl Default for TextEdit {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            font: GlyphFont::new(0.0),
            color: Vec3::ONE,
            background: Vec3::ZERO,
            selection: Vec3::ZERO,
            anchor: Vec2::ZERO,
            padding: Vec2::ZERO,
            min_width: 0.0,
            precedence: Precedence::default(),
            tag: None,
            plane_normal: None,
            content: String::new(),
            caret: 0,
            mark: 0,
            extent: Cell::new(Vec2::ZERO),
            stops: RefCell::new(Vec::new()),
        }
    }
}

impl TextEdit {
    /// `content` in a field anchored at `position`, in white on black at
    /// `size_px` logical pixels, with the caret at the end of what it says.
    ///
    /// The caret lands at the end rather than the start because a field arrives
    /// holding a value someone is about to replace or extend, and the end is
    /// where both of those begin.
    pub fn new(position: Vec3, content: impl Into<String>, size_px: f32) -> Self {
        let content = content.into();
        let at = content.len();
        Self {
            position,
            font: GlyphFont::new(size_px),
            padding: PADDING * size_px,
            min_width: MIN_WIDTH * size_px,
            content,
            caret: at,
            mark: at,
            ..Self::default()
        }
    }

    /// Put `position` at this fraction of the field's own box — see
    /// [`TextEdit::anchor`].
    pub fn anchored(mut self, anchor: Vec2) -> Self {
        self.anchor = anchor;
        self
    }

    /// Declare the plane the field lies on. See [`TextEdit::plane_normal`].
    pub fn in_plane(mut self, normal: Vec3) -> Self {
        self.plane_normal = Some(normal.normalize_or_zero());
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    /// Where the caret is, as a byte offset into the content.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The run picked out, low end first, and empty where nothing is.
    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.mark)..self.caret.max(self.mark)
    }

    /// What the run picked out says, and `""` where nothing is.
    pub fn selected(&self) -> &str {
        &self.content[self.selection()]
    }

    /// Say what the field holds, keeping the caret wherever it still fits.
    ///
    /// What re-seeding a field from the model every frame goes through, which is
    /// what makes one a *view* of the value it shows: an undo, a drag that moved
    /// it, or a different dimension being picked all reach the field without
    /// anything having to notice. A call that changes nothing does nothing, so
    /// it costs a comparison on the frames a field is merely being shown.
    ///
    /// The caret is clamped rather than reset, so the two ways a field is
    /// written — this and a keystroke — do not fight over it: text arriving from
    /// the model while someone types would otherwise send the caret home on
    /// every frame.
    pub fn set_content(&mut self, content: &str) {
        if self.content == content {
            return;
        }
        self.content.clear();
        self.content.push_str(content);
        self.caret = self.boundary(self.caret);
        self.mark = self.boundary(self.mark);
    }

    /// Say what `other` says, with its caret and its selection, keeping this
    /// field's own placement and look.
    ///
    /// For an application whose draft lives somewhere else — where the caret is
    /// is the session's, and where the field is drawn is the drawing's, so the
    /// copy in the scene is written from both. Cheaper than cloning and
    /// deliberately so: it reuses the string this field already has, which is
    /// what keeps a field redrawn on every keystroke off the heap.
    pub fn mirror(&mut self, other: &TextEdit) {
        self.set_content(&other.content);
        self.caret = other.caret;
        self.mark = other.mark;
    }

    /// Send the caret somewhere, taking the selection with it or dropping it.
    pub fn seek(&mut self, to: Seek, selecting: Selecting) {
        let picked = self.selection();
        // A step with a run picked out and no shift lands on the end it was
        // moving toward rather than one past it: what Left means with something
        // selected is "leave it, at this end", which is the one case where a
        // step moves the caret no characters at all.
        let collapsing = selecting == Selecting::Drop && !picked.is_empty();
        self.caret = match to {
            Seek::Left if collapsing => picked.start,
            Seek::Right if collapsing => picked.end,
            Seek::Left => self.before(self.caret),
            Seek::Right => self.after(self.caret),
            Seek::Start => 0,
            Seek::End => self.content.len(),
            Seek::Byte(byte) => self.boundary(byte),
        };
        if selecting == Selecting::Drop {
            self.mark = self.caret;
        }
    }

    /// Pick out the whole line, with the caret at its end.
    pub fn select_all(&mut self) {
        self.mark = 0;
        self.caret = self.content.len();
    }

    /// Put `text` in, in place of whatever is picked out, and leave the caret
    /// after it.
    ///
    /// The one way anything is added, so typing a character, committing what an
    /// input method composed, and pasting a line are one path — and all three
    /// replace a selection, which is the behaviour that would otherwise have to
    /// be remembered three times.
    pub fn insert(&mut self, text: &str) {
        let over = self.selection();
        let at = over.start + text.len();
        self.content.replace_range(over, text);
        self.caret = at;
        self.mark = at;
    }

    /// Take out the character before the caret, or the run picked out.
    pub fn delete_back(&mut self) {
        let picked = self.selection();
        let over = if picked.is_empty() {
            self.before(self.caret)..self.caret
        } else {
            picked
        };
        self.take_out(over);
    }

    /// Take out the character after the caret, or the run picked out.
    pub fn delete_forward(&mut self) {
        let picked = self.selection();
        let over = if picked.is_empty() {
            self.caret..self.after(self.caret)
        } else {
            picked
        };
        self.take_out(over);
    }

    /// How far the *text* reaches on screen, in logical pixels, or zero where
    /// nothing has laid it out.
    ///
    /// [`Text::extent`](crate::Text::extent) exactly — the same measurement of
    /// the same string, and the box [`anchor`](Self::anchor) is a fraction of.
    /// What the whole field occupies is larger by the padding, and is
    /// [`surround`](Self::surround).
    ///
    /// The height falls back to the font's line height once the field is empty,
    /// which is the one place the two part company: a run that says nothing
    /// measures nothing, and a label with nothing to say is simply not drawn
    /// where a field still has a caret to stand up.
    pub fn extent(&self) -> Vec2 {
        let measured = self.extent.get();
        if measured.y > 0.0 {
            measured
        } else {
            Vec2::new(measured.x, self.font.line_height_px)
        }
    }

    /// How far the whole field reaches on screen, in logical pixels — the
    /// surround, which is the text grown by the padding and floored at
    /// [`min_width`](Self::min_width).
    ///
    /// This is what a click has to land in, and what the field covers.
    pub fn surround(&self) -> Vec2 {
        let text = self.extent();
        Vec2::new(
            text.x.max(self.min_width) + self.padding.x * 2.0,
            text.y + self.padding.y * 2.0,
        )
    }

    /// Where in the content a cursor landed, or `None` where the field is not
    /// drawn at all — what a click that places the caret asks.
    ///
    /// The nearest character boundary rather than the character the cursor fell
    /// in, which is what a caret does: clicking the left half of a digit puts
    /// the caret before it and the right half puts it after.
    ///
    /// Clamped to the content, so a cursor short of the first character answers
    /// the start and one past the last answers the end. A field nothing has laid
    /// out yet has no boundaries to answer with and says its start.
    pub fn byte_at(&self, aim: &Aim) -> Option<usize> {
        let surround = self.box_on_screen(aim)?;
        let along = aim.cursor.x - surround.min.x - self.padding.x;
        let stops = self.stops.borrow();
        Some(
            stops
                .iter()
                .min_by(|a, b| (a.x - along).abs().total_cmp(&(b.x - along).abs()))
                .map_or(0, |stop| stop.byte),
        )
    }

    /// Where every part of the field sits relative to its anchor.
    pub(crate) fn parts(&self) -> Parts {
        let extent = self.extent();
        // Where the text's top-left sits relative to the anchor — the one line
        // that has to read the same as a label's, and does: it is the same
        // fraction of the same measurement. See [`TextEdit::anchor`].
        let text = -self.anchor * extent;
        let picked = self.selection();
        Parts {
            // Outward from the text, so nothing here can move it. The floor
            // under the width is taken up on the right alone, which is the side
            // a left-aligned field grows toward.
            surround: Rect::new(
                text.x - self.padding.x,
                text.y - self.padding.y,
                extent.x.max(self.min_width) + self.padding.x * 2.0,
                extent.y + self.padding.y * 2.0,
            ),
            text,
            wash: (!picked.is_empty()).then(|| {
                let (from, to) = (self.x_at(picked.start), self.x_at(picked.end));
                Rect::new(text.x + from, text.y, to - from, extent.y)
            }),
            caret: Rect::new(
                text.x + self.x_at(self.caret),
                text.y,
                CARET_WIDTH,
                extent.y,
            ),
        }
    }

    /// Whether the cursor landed on this field.
    ///
    /// Anywhere inside the box counts, and counts equally — the same rule a
    /// label answers by, and for a stronger reason: a field is a thing to be
    /// clicked into, so every pixel of it is the target.
    pub(crate) fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let screen = aim.reach_to_box(self.box_on_screen(aim)?);
        (screen <= aim.radius)
            .then(|| aim.hit(tag, HitAt::Field, self.precedence, self.position, screen))
    }

    /// The field's surround on screen, in logical pixels from the view's
    /// top-left corner, or `None` where there is none to have.
    ///
    /// `None` is an anchor the projection does not draw, or a surround with no
    /// area — a field of no size, which is one in a font of none.
    fn box_on_screen(&self, aim: &Aim) -> Option<Rect> {
        let surround = self.surround();
        if surround.x <= 0.0 || surround.y <= 0.0 {
            return None;
        }
        // Off the *text's* anchor and then out by the padding, so the box a
        // click lands in is the box that was drawn — see [`TextEdit::parts`].
        let text = aim.screen_of(self.position)? - self.anchor * self.extent();
        Some(Rect::new(
            text.x - self.padding.x,
            text.y - self.padding.y,
            surround.x,
            surround.y,
        ))
    }

    /// Where the boundary at `byte` sits, in logical pixels from the text's own
    /// left edge.
    ///
    /// Zero where `byte` is not among the stops, which is only ever a field
    /// nothing has laid out yet: the stops carry every boundary in the content,
    /// the caret and the mark are always at one, and the renderer lays a field
    /// out before it reads any of this.
    fn x_at(&self, byte: usize) -> f32 {
        let stops = self.stops.borrow();
        stops
            .binary_search_by_key(&byte, |stop| stop.byte)
            .map_or(0.0, |at| stops[at].x)
    }

    /// Take out `over`, leaving the caret where it began and nothing picked
    /// out.
    fn take_out(&mut self, over: Range<usize>) {
        let at = over.start;
        self.content.replace_range(over, "");
        self.caret = at;
        self.mark = at;
    }

    /// The character boundary before `byte`, and the start where there is
    /// none.
    fn before(&self, byte: usize) -> usize {
        self.content[..byte]
            .char_indices()
            .next_back()
            .map_or(0, |(at, _)| at)
    }

    /// The character boundary after `byte`, and `byte` itself at the end.
    fn after(&self, byte: usize) -> usize {
        self.content[byte..]
            .chars()
            .next()
            .map_or(byte, |ch| byte + ch.len_utf8())
    }

    /// `byte` clamped to the content and moved back to the character boundary
    /// it falls in.
    fn boundary(&self, byte: usize) -> usize {
        let mut at = byte.min(self.content.len());
        // Terminates at nothing worse than the start, which is a boundary in
        // every string there is.
        while !self.content.is_char_boundary(at) {
            at -= 1;
        }
        at
    }
}

/// A field is an overlay like the other four, and not a [`Flatten`]: what it
/// comes to is a surround, a wash, a caret and however many glyphs the shaper
/// makes of what it says — and only the first three of those are the field's to
/// count. See [`Flatten`].
///
/// [`Flatten`]: crate::primitive::Flatten
impl Primitive for TextEdit {
    fn tag(&self) -> Option<Tag> {
        self.tag
    }

    fn reaches(&self, mut include: impl FnMut(Vec3)) {
        include(self.position);
    }
}

impl Styled for TextEdit {
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

/// Take every field's caret stops from `shaper`, so that what has been laid out
/// knows how far each of its boundaries reaches.
///
/// Reads the fields and writes only their memo, which is why it takes them
/// shared — see [`TextEdit::stops`]. A pass that needed `&mut` would mark the
/// batch it measured, and a marked batch is one the renderer lays out again.
///
/// Measured unscaled, so the answers are in logical pixels like every other
/// overlay's size.
///
/// **One measurement per character**, where a label costs one for the whole
/// run. That is what a caret needs and what nothing cheaper gives: a shaper
/// hands back where each *glyph* was placed, and there is no glyph to point at
/// for the caret at the end of a line, nor any way to tell which glyphs one
/// character came to. Measuring each prefix asks about boundaries directly, and
/// a field is short by construction — a length, a number, a formula — so the
/// walk is a few dozen shapings of a few dozen characters, on the frames an edit
/// lands rather than on every frame.
///
/// What it costs in exactness is a kern: the caret between two letters sits at
/// the advance of the first measured alone, where the pair shaped together may
/// close up by a fraction of a pixel. The glyphs are drawn from one shaping of
/// the whole line and so are spaced correctly; it is only the caret that can
/// stand a hair off, and only in a face that kerns the digits.
pub(crate) fn measure_all(fields: &[TextEdit], glyphs: &mut TextGlyphs<'_>) {
    for field in fields {
        // The whole string first, and through the very call
        // [`text::measure_all`](crate::text::measure_all) makes: this is the box
        // the anchor is a fraction of, so a field measuring a hair differently
        // from the label it replaced would place its glyphs a hair differently.
        let whole = glyphs.measure(&field.content, field.font, 1.0);
        field.extent.set(Vec2::new(whole.w, whole.h));

        let mut stops = field.stops.borrow_mut();
        stops.clear();
        stops.reserve_exact(field.content.chars().count() + 1);
        // The caret before the first character, which every field has and an
        // empty one has only.
        stops.push(Stop { byte: 0, x: 0.0 });
        for (at, ch) in field.content.char_indices() {
            let byte = at + ch.len_utf8();
            let x = glyphs.measure(&field.content[..byte], field.font, 1.0).w;
            stops.push(Stop { byte, x });
        }
    }
}

/// Standing in for the renderer, which is the only thing that lays a field out.
///
/// `cfg(test)` rather than the `internals` feature, on the same terms as
/// [`Text::measured`](crate::Text): a caller outside the crate gets its stops
/// the honest way, by having the field drawn. What this is for is asking what a
/// *pick* or a *caret* does about a box, which wants a box and no GPU.
#[cfg(test)]
impl TextEdit {
    /// Lay the field out as though every character were `advance` wide on a
    /// line `line` tall.
    pub(crate) fn measured(self, advance: f32, line: f32) -> Self {
        let mut stops = vec![Stop { byte: 0, x: 0.0 }];
        for (nth, (at, ch)) in self.content.char_indices().enumerate() {
            stops.push(Stop {
                byte: at + ch.len_utf8(),
                x: (nth + 1) as f32 * advance,
            });
        }
        self.extent
            .set(Vec2::new(stops.last().expect("the first stop").x, line));
        *self.stops.borrow_mut() = stops;
        self
    }
}

#[cfg(test)]
mod tests;
