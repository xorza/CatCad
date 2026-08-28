//! The quartic two cylinders on square axes meet in.

use crate::solid::geometry::axis::Axis;
use glam::DVec3;

/// One loop of the meeting of two cylinders whose axes cross square, written
/// on the wider of the two.
///
/// **The first curve here that is not a conic**, and the shape a cross drilling
/// leaves. Two cylinders of one radius on crossing axes meet in two ellipses,
/// which [`Ellipse`](super::ellipse::Ellipse) already carries; where the radii
/// differ the meeting is a quartic in space and no conic describes it — see
/// `.notes/KERNEL.md` §7.3. The trade calls the shape a saddle, a pipe cut to
/// sit over another pipe being saddled to it, and the name is worth keeping
/// because the curve is exactly that cut.
///
/// **A frame and three lengths, and the frame carries the rest.** Its origin is
/// where the two axes come nearest, its direction is the wider axis, and its
/// reference is the narrower axis's own direction — so where each cylinder
/// stands, which way round the narrower one was taken, and where along the
/// wider one they meet are all read off one [`Axis`] rather than carried beside
/// it as three more numbers.
///
/// **Written on the wider cylinder, always.** Both cylinders describe the same
/// curve and neither describes it better, so which one carries it is a choice —
/// and taking the wider one makes every saddle a *closed loop* of that
/// cylinder's own parameters rather than sometimes one and sometimes a pair of
/// runs right round. That is what lets the parameter below be one angle.
///
/// **Nested cross-sections only.** The two radii and the offset have to satisfy
/// `across + |off| < reach`: the narrower cylinder passes wholly through the
/// wider one. Where they only overlap, the meeting is one loop that runs most
/// of the way round the wider cylinder and doubles back on itself in that
/// cylinder's angle, which no graph over an angle can hold — and
/// [`Meeting`](crate::solid::meeting::Meeting) hands that case to the algebraic
/// route instead.
///
/// Two loops to a meeting, an entry and an exit, and they are the same numbers
/// with the narrower axis taken the other way round: the reference reversed and
/// `off` negated. See [`Saddle::at`], which is what makes that true.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Saddle {
    /// Where the two axes come nearest, along the wider one, angles from the
    /// narrower one.
    pub(crate) axis: Axis,
    /// The wider cylinder's radius.
    pub(crate) reach: f64,
    /// The narrower cylinder's.
    pub(crate) across: f64,
    /// How far the narrower axis passes the wider one by, square to both.
    pub(crate) off: f64,
}

impl Saddle {
    /// Where the parameter `t` lands.
    ///
    /// **A whole turn to a lap, and the parameter is an angle of its own.**
    /// Every place of the loop stands `across` from the narrower axis, so the
    /// two numbers a place has against that cylinder — how far along the wider
    /// axis it is, and how far it stands off the narrower one square to that —
    /// run round a circle of radius `across` as the loop is walked. `t` is the
    /// angle of that circle, and it is what makes a loop whose two halves are a
    /// square root apart into one curve with no root left in it.
    ///
    /// Regular the whole way round, which the angle of the wider cylinder is
    /// not: a graph over that angle stands vertical where the loop turns back,
    /// and the same place is a plain quarter turn of `t`.
    pub(crate) fn at(self, t: f64) -> DVec3 {
        let (up, round) = t.sin_cos();
        let leaning = (self.across * round + self.off) / self.reach;
        self.axis.origin
            + self.axis.radial(leaning.asin()) * self.reach
            + self.axis.direction * (self.across * up)
    }

    /// Which parameter puts the curve at `at`, which is [`Saddle::at`] read
    /// backwards.
    ///
    /// The two numbers the note there names, read off the place and taken as an
    /// angle. Exact for anything on the loop, and the nearest bearing for
    /// anything off it — see [`Curve::along`](super::curve::Curve::along),
    /// which says why that is an inversion rather than a projection.
    pub(crate) fn along(self, at: DVec3) -> f64 {
        let leaning = self.reach * self.axis.angle_of(at).sin() - self.off;
        self.axis.along(at).atan2(leaning)
    }

    /// How hard the loop can turn, as a bound on the second derivative of the
    /// place with the parameter.
    ///
    /// **What a chord count is taken against** — see
    /// [`arc::chords`](crate::math::arc::chords), which reads its first argument
    /// as exactly this and is given a radius by every other curve because a
    /// circle's is its radius.
    ///
    /// Worked out rather than sampled. With `q = across/reach` and
    /// `s = (across + |off|)/reach`, the angle of the wider cylinder moves as
    /// `asin` of a cosine, so its first two derivatives are held by
    /// `q/√(1 − s²)` and `q/√(1 − s²) + s·q²/(1 − s²)^{3/2}`; the place is
    /// `reach` out at that angle and `across·sin t` along the axis, and the two
    /// halves add. Finite because the cross-sections are nested, which is what
    /// keeps `s` below one.
    pub(crate) fn bending(self) -> f64 {
        let quick = self.across / self.reach;
        let most = (self.across + self.off.abs()) / self.reach;
        let leaning = (1.0 - most * most).sqrt();
        let turn = quick / leaning;
        let bend = turn + most * quick * quick / (leaning * leaning * leaning);
        self.reach * (bend + turn * turn) + self.across
    }
}
