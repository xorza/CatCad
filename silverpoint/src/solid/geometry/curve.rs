//! What an edge is a piece of.

use crate::math::arc;
use crate::solid::buckets::Key;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::ellipse::Ellipse;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::marchings::Marched;
use crate::solid::geometry::quartic::Quartered;
use crate::solid::geometry::saddle::Saddle;
use glam::DVec3;

/// One of the curves an edge may lie on.
///
/// Three conics, because three is what a sketch and the reducible meetings off
/// it make: a line, a circle or an ellipse. The fourth is the first curve that
/// is none of those — the quartic a cross drilling leaves, which
/// [`Saddle`] carries for the one pair that produces it.
/// The general quartic a general pair of quadrics gives arrives with the
/// routine that parameterizes it — see `.notes/KERNEL.md` §7.3.
///
/// Untrimmed, like a [`Surface`](super::surface::Surface): where a curve starts
/// and stops belongs to the [`Edge`](crate::solid::topology::edge::Edge) on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Curve {
    Line(Line),
    Circle(Circle),
    Ellipse(Ellipse),
    Saddle(Saddle),
    /// A curve of the fitted tier, laid down as places rather than written
    /// down — see [`Marched`], and `.notes/KERNEL.md` §4.1 for the tier.
    ///
    /// What builds one is the boolean, meeting a pair it has to march.
    Marched(Marched),
    /// The curve a general pair of quadrics meets in, written down exactly —
    /// see [`Quartered`], and `.notes/KERNEL.md` §7.3 for the route.
    ///
    /// What builds one is the boolean, meeting a pair no row of the reducible
    /// table answers: a cone drilled off its own axis is the first. That
    /// arrives with the cut the splitter can make from one, which is what the
    /// arm still waits on — `.notes/KERNEL.md` §9.1, and
    /// [`Meeting::Algebraic`](crate::solid::meeting::Meeting), which is the
    /// refusal in the meantime.
    #[allow(dead_code)]
    Quartic(Quartered),
}

impl Curve {
    /// The key several of these are filed under — see
    /// [`Buckets`](crate::solid::buckets::Buckets).
    ///
    /// Over the numbers the curve is made of, and sound for the reason
    /// [`Surface::key`](super::surface::Surface::key) is: a crossing met from
    /// either side is one call answering the identical value both times, so
    /// the two key alike bit for bit.
    pub(crate) fn key(&self) -> u64 {
        match self {
            Self::Line(line) => Key::default()
                .word(0)
                .place(line.origin)
                .place(line.direction)
                .done(),
            Self::Circle(circle) => circle
                .axis
                .keyed(Key::default().word(1))
                .float(circle.radius)
                .done(),
            Self::Ellipse(ellipse) => ellipse
                .axis
                .keyed(Key::default().word(2))
                .float(ellipse.major)
                .float(ellipse.minor)
                .done(),
            Self::Saddle(saddle) => saddle
                .axis
                .keyed(Key::default().word(3))
                .float(saddle.reach)
                .float(saddle.across)
                .float(saddle.off)
                .done(),
            // Worked out where the run was laid down and carried since — see
            // [`Marched::key`], which says why it is not read off the places.
            Self::Marched(marched) => marched.key,
            Self::Quartic(of) => of.key,
        }
    }

    /// Which parameter puts the curve at `at`, which is [`Curve::at`] read
    /// backwards.
    ///
    /// **The place has to be on the curve**, which every caller has: what asks
    /// is an edge being given the stretch of curve it covers, and the two
    /// places it runs between are places the curve was cut at. A point off the
    /// curve answers with the parameter of the nearest place on it that shares
    /// its bearing, which is a wrong answer rather than no answer — so this is
    /// not a projection and must not be used as one.
    pub(crate) fn along(&self, at: DVec3, carried: &Carried) -> f64 {
        match self {
            Self::Line(line) => (at - line.origin).dot(line.direction),
            Self::Circle(circle) => circle.axis.angle_of(at),
            // **Not the bearing**, which is what an axis answers and what a
            // circle's parameter happens to be. An ellipse sweeps its frame —
            // see [`Ellipse`] — so the parameter is the bearing of the place
            // with each half divided out, and reading the bearing itself would
            // give a `t` that [`Curve::at`] sends somewhere else entirely.
            Self::Ellipse(ellipse) => {
                let out = at - ellipse.axis.origin;
                (out.dot(ellipse.axis.quarter()) / ellipse.minor)
                    .atan2(out.dot(ellipse.axis.reference) / ellipse.major)
            }
            Self::Saddle(saddle) => saddle.along(at),
            Self::Marched(of) => carried.marched.along(of.run, at),
            Self::Quartic(of) => carried.quartics.along(of.run, at),
        }
    }

    /// How far the pieces it is made of stray from the curve itself.
    ///
    /// **Nought for every curve of the exact tier**, which is written down
    /// rather than laid down: a place read off one is the curve's own place to
    /// a rounding. A marched curve is a run of chords and answers what its
    /// walk measured, which is the bound `.notes/KERNEL.md` §4.1 says a fitted
    /// result carries — and what the edge on it stands for.
    pub(crate) fn strays(&self, carried: &Carried) -> f64 {
        match self {
            // A quartic is written down rather than walked, so it strays
            // nowhere at all — which is what puts it in the exact tier.
            Self::Line(_)
            | Self::Circle(_)
            | Self::Ellipse(_)
            | Self::Saddle(_)
            | Self::Quartic(_) => 0.0,
            Self::Marched(of) => carried.marched.strayed(of.run).most,
        }
    }

    /// How large the numbers evaluating it at `t` works in.
    ///
    /// **Not how large the answer is.** A place on a curve can land next to the
    /// origin off terms a hundred million wide — a line reaching back from far
    /// away is the plain case — and what a check has to allow the machine is a
    /// proportion of *those* rather than of what came out, cancellation having
    /// thrown the size of them away. See
    /// [`slack`](crate::number::predicate::slack).
    ///
    /// A round curve's parameter is an angle and carries no size of its own, so
    /// only the straight one reads `t` at all.
    pub(crate) fn reach(&self, t: f64) -> f64 {
        match self {
            Self::Line(line) => line.origin.length() + t.abs(),
            Self::Circle(circle) => circle.axis.origin.length() + circle.radius,
            Self::Ellipse(ellipse) => ellipse.axis.origin.length() + ellipse.major,
            // Both radii, the loop standing one out from the axis it is
            // written on and the other along it.
            Self::Saddle(saddle) => saddle.axis.origin.length() + saddle.reach + saddle.across,
            Self::Marched(of) => of.reach,
            Self::Quartic(of) => of.reach,
        }
    }

    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64, carried: &Carried) -> DVec3 {
        match self {
            Self::Line(line) => line.at(t),
            Self::Circle(circle) => circle.at(t),
            Self::Ellipse(ellipse) => ellipse.at(t),
            Self::Saddle(saddle) => saddle.at(t),
            Self::Marched(of) => carried.marched.at(of.run, t),
            Self::Quartic(of) => carried.quartics.at(of.run, t),
        }
    }

    /// How many straight pieces a stretch of `span` parameter is worth,
    /// flattened no further than `sagitta` from the true curve.
    ///
    /// Straight is exact however coarsely it is cut, so only a round curve is
    /// asked — see [`arc::chords`], which is where the rule lives and why it is
    /// one rule rather than one per caller.
    ///
    /// **An ellipse is asked with its longer half**, which is the radius of the
    /// circle it bends no harder than: how far a chord over a parameter step
    /// strays is set by the second derivative, and an ellipse's is at most its
    /// major semi-axis. So the same rule bounds it, conservatively at the flat
    /// ends and exactly at the sharp ones.
    pub(crate) fn steps(&self, span: f64, sagitta: f64, carried: &Carried) -> usize {
        match self {
            Self::Line(_) => 1,
            Self::Circle(circle) => arc::chords(circle.radius, span, sagitta),
            Self::Ellipse(ellipse) => arc::chords(ellipse.major, span, sagitta),
            // **Its own bound rather than a radius**, a saddle having no
            // circle it bends no harder than — see [`Saddle::bending`].
            Self::Saddle(saddle) => arc::chords(saddle.bending(), span, sagitta),
            // **The chords it has, whatever is asked of it.** A run cannot be
            // laid down again — see [`Marchings::steps`] — and how far its own
            // stray from the curve is what the edge on it carries.
            Self::Marched(of) => carried.marched.steps(of.run, span),
            // Its own measured bound rather than a radius, a quartic having
            // no circle it bends no harder than and no closed form for one.
            Self::Quartic(of) => carried.quartics.steps(of.run, span, sagitta),
        }
    }
}
