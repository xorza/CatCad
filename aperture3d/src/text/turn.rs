//! How a run is laid in a plane: which surface, which way round on it, and how
//! far off the point it names.
//!
//! Apart from [`Text`](crate::Text) because it stands on its own: [`Turn::axes`]
//! is where a laid run's frame is settled, the box is built along what it
//! answers and a pick brings the cursor onto the same pair — and the vertex
//! shader cannot call it, so it builds the same two rules instead.

use crate::viewport::Viewport;
use glam::{Mat4, Vec2, Vec3};

/// What a run is set against: the screen, or a plane of the world.
///
/// Two states rather than a normal beside a direction, because the third
/// combination is meaningless — a run turned into a plane it takes no depth from
/// would be lettering lying on a surface and fighting it — and an enum is how a
/// meaningless combination stops being expressible.
///
/// Both are sized in *logical pixels*. Turning a run changes the direction its
/// box runs in and nothing else: it does not foreshorten, and the zoom cannot
/// reach it. See [`Text`](crate::Text).
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

    /// The turn this run is laid along, where there is one to lay it along.
    ///
    /// `None` for a run square to the viewer *and* for one turned into a plane
    /// it has no direction in — see [`Turn::laid`] — because the shader draws
    /// the second as the first. Asking it once is what keeps a run from being
    /// measured in a frame it was not laid out in: everything that has to
    /// agree with the vertex stage takes its turn from here.
    pub(crate) fn laid_turn(self) -> Option<Turn> {
        match self {
            Self::Screen { .. } => None,
            Self::Turned(turn) => turn.laid().then_some(turn),
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
        self.laid_turn().map(|turn| turn.right)
    }

    /// How far the run's box floats off the point it names, as a world
    /// displacement per logical pixel of it. See [`Turn::lift_world`].
    ///
    /// A bare vector rather than an `Option` like the two above, because there
    /// is no such thing as *no* lift to tell apart from a lift of nothing: a run
    /// square to the viewer sits on its point, and so does a laid one that was
    /// given none.
    pub fn lift_world(self) -> Vec3 {
        match self {
            Self::Screen { .. } => Vec3::ZERO,
            Self::Turned(turn) => turn.lift_world(),
        }
    }
}

/// How a run is laid in a plane: which surface, which way round on it, and how
/// far off the point it names.
///
/// A direction and a normal rather than the plane's two axes. Naming a second
/// axis would read as though which way the *box* runs were the caller's too, and
/// it is not: it is derived, deliberately, so that a run cannot come out
/// mirrored or sheared — and derived inside this crate, because the deriving is
/// what the renderer and a pick have to agree about and neither answer is one a
/// caller could usefully hold.
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
    ///
    /// Unit and in the plane, which [`Turn::new`] is what makes true — or zero
    /// where what it was handed named no direction the plane has. A run with
    /// nothing to advance along is drawn across the screen *and picked there*:
    /// [`Facing::right`] answers `None`, and the shader reads that same absence
    /// off its record as a zero vector.
    pub right: Vec3,
    /// The plane's unit normal: what the run's depth follows, and what says
    /// which surface `right` is a direction *of*.
    ///
    /// Its sign is nobody's business. Depth reads a plane as a surface to take
    /// a gradient over rather than as a side to be on, and which side the eye is
    /// on decides nothing: what settles the run's own frame reads the projected
    /// pair, not where the eye stands.
    pub normal: Vec3,
    /// How far the run's box floats off the point it names, in logical pixels
    /// across and down the plane's axes **as authored** — along `right`, and
    /// along `normal × right`.
    ///
    /// **Nothing the projection decides reaches it, and that is the whole of
    /// what it is for.** An offset written into
    /// [`Text::anchor`](crate::Text::anchor) rides in the run's *own* frame,
    /// and both rules that settle that frame move it: the mirror that keeps a
    /// run readable from behind its plane, and the half turn that keeps it the
    /// right way up. Either swings the box off whatever it was standing clear
    /// of. Stated here it is fixed in the plane, so a run that comes round to
    /// stay readable only changes direction.
    ///
    /// Resolved in the world rather than against the run's own settled axes,
    /// because those carry *two* camera-dependent signs — the mirror and the
    /// half turn — and a lift that went through them would pick up both.
    ///
    /// Which leaves one thing for the caller: a box hung off a *centred* anchor
    /// is mapped onto itself by that half turn, so its place holds outright.
    /// One hung off any other fraction is reflected through the lifted point,
    /// which is a real answer but rarely the wanted one — see
    /// [`Text::anchor`](crate::Text::anchor).
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
        let normal = normal.normalize_or_zero();
        // Brought *into* the plane rather than checked against it. What `right`
        // says is which way round the run is set, and a direction a rounding off
        // the plane says it as clearly as one exactly in it — where refusing the
        // first would fire on a caller's arithmetic, and taking it as given
        // would lean the run's own down axis out of the surface it letters.
        //
        // A direction the plane has no part of — one along the normal, or either
        // of them zero — leaves nothing to lay a run along and comes back zero.
        // See [`Turn::right`] for what is then drawn.
        let right = (right - normal * right.dot(normal)).normalize_or_zero();
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

    /// Whether there is a direction here to lay a run along.
    ///
    /// **The one statement of it**, read by everything that has to agree with
    /// the shader: [`Facing::right`] answers `None` through it, which is what
    /// puts a zero in the record and sends the vertex stage down its
    /// screen-facing branch, and picking asks it so that a run is measured in
    /// the frame it was drawn in. See [`Turn::right`].
    pub(super) fn laid(self) -> bool {
        self.right != Vec3::ZERO
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
    /// The box is built along these and a pick brings the cursor onto them, so
    /// what is drawn and what is clicked cannot disagree. The vertex shader
    /// cannot call it and builds the same two rules — the same arrangement
    /// [`MIN_RUN_PX`](crate::viewport::MIN_RUN_PX) is under, where one number is stated in
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
    /// area its box comes to, which is what a pick refuses on.
    pub(crate) fn axes(self, at: Vec3, view_proj: Mat4, viewport: Viewport) -> Axes {
        let here = view_proj * at.extend(1.0);
        let across = self.normal.cross(self.right);
        let along = viewport.screen_tangent(self.right, here, view_proj);
        let sideways = viewport.screen_tangent(across, here, view_proj);
        // Which of the plane's two ways to run down is the one that winds the
        // way the screen does, as the sign that picks it.
        let winding = if along.perp_dot(sideways) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        // And the half turn, which is a sign on both rather than a second frame
        // — which is what makes it a rotation and so leaves the choice above
        // where it was.
        let upright = if along.x < 0.0 { -1.0 } else { 1.0 };
        // Both signs at once, since the down axis takes both and the advance
        // takes only the second.
        let downward = winding * upright;
        Axes {
            advance: self.right * upright,
            down: across * downward,
            // The tangents that settled those signs, carried out under them
            // rather than thrown away: `screen_tangent` is linear in the
            // direction it is asked about, so a tangent of the settled axis is
            // the tangent of the authored one under the same sign, and asking
            // the projection again would be three more products for an answer
            // already in hand.
            advance_px: along * upright,
            down_px: sideways * downward,
        }
    }
}

/// The plane directions a run is laid along: the way it advances, the way its
/// own box runs down, and how far each of them carries on screen.
///
/// World directions, both unit and square to each other, both in the run's
/// plane. Where a run *sits* on those axes is the anchor's and how far it
/// reaches along them is the shaping's; what is here is which way they point and
/// how fast they run, which is the half the projection has a say in. See
/// [`Turn::axes`].
///
/// The advance is [`Turn::right`] *as settled* and so is sometimes its negation
/// — named apart for that reason, since the two are a half turn out from each
/// other exactly when it matters and a box built on the wrong one hangs off the
/// wrong side of its anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Axes {
    pub(crate) advance: Vec3,
    pub(crate) down: Vec3,
    /// Pixels per world unit along [`Axes::advance`], at the point the axes
    /// were settled at, with y running down the screen.
    ///
    /// The projection's own tangent — see
    /// [`Viewport::screen_tangent`](crate::Viewport::screen_tangent). Carried
    /// because settling the signs above had to ask for it, and because the one
    /// reader that inverts the pair to bring a cursor into the run's frame
    /// would otherwise ask for it a second time.
    pub(crate) advance_px: Vec2,
    /// The same along [`Axes::down`].
    pub(crate) down_px: Vec2,
}
