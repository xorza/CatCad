//! How a run is laid in a plane: which surface, which way round on it, and how
//! far off the point it names.
//!
//! Apart from [`Text`](crate::Text) because it stands on its own: three readers
//! agree by reading [`Turn::axes`] — the box is built along it, a pick brings
//! the cursor onto it, and an application standing something of its own over a
//! run measures from it — and the vertex shader is a fourth that cannot call it
//! and builds the same two rules instead.

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
        let along = viewport.screen_tangent(self.right, here, view_proj);
        let sideways = viewport.screen_tangent(across, here, view_proj);
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
