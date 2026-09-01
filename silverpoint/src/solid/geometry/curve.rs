//! What an edge is a piece of.

use crate::math::arc;
use crate::number::predicate;
use crate::number::predicate::ApproxEq;
use crate::number::tolerance::PLACED;
use crate::solid::buckets::Key;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::ellipse::Ellipse;
use crate::solid::geometry::hyperbola::Hyperbola;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::marchings::Marched;
use crate::solid::geometry::parabola::Parabola;
use crate::solid::geometry::quartic::Quartered;
use crate::solid::geometry::saddle::Saddle;
use glam::DVec3;

/// One place a curve was sampled at, and where along it that place stands.
///
/// What a cut laid off a curve puts its corners at — see [`Curve::sample`].
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Sampled {
    /// The curve's own parameter, which is an angle round a closed one.
    pub(crate) along: f64,
    pub(crate) at: DVec3,
}

/// One of the curves an edge may lie on.
///
/// **Every conic**, because a plane against a natural quadric makes every one
/// of them: a line and a circle come off a sketch, an ellipse off a plane
/// leaning on a cylinder or a cone, and the open pair off a plane leaning past
/// a cone's own rulings. Then the first curve that is no conic at all — the
/// quartic a cross drilling leaves, which [`Saddle`] carries for the one pair
/// that produces it.
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
    /// One branch of a hyperbola — see [`Hyperbola`], which says why a branch
    /// rather than the pair.
    Hyperbola(Hyperbola),
    Parabola(Parabola),
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
    /// table answers: a cone drilled off its own axis is the first.
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
            // The two branches of one meeting differ in the reference alone,
            // which an axis keys — see [`Axis::keyed`].
            Self::Hyperbola(of) => of
                .axis
                .keyed(Key::default().word(4))
                .float(of.major)
                .float(of.minor)
                .done(),
            Self::Parabola(of) => of.axis.keyed(Key::default().word(5)).float(of.focal).done(),
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
            Self::Hyperbola(of) => of.along(at),
            Self::Parabola(of) => of.along(at),
            Self::Saddle(saddle) => saddle.along(at),
            Self::Marched(of) => carried.marched.along(of.run, at),
            Self::Quartic(of) => carried.quartics.along(of.run, at),
        }
    }

    /// Whether it comes back to where it began.
    ///
    /// **What a walked cut asks before it walks one.** A traced cut samples a
    /// whole turn of the curve's own parameter and orders places by how far
    /// round they stand — see
    /// [`Traced`](crate::solid::boolean::splitting::traced::Traced) — and an
    /// open curve has neither a turn nor a way round. A line, a parabola and a
    /// hyperbola's branch are the three that run away and never return.
    pub(crate) fn closed(&self) -> bool {
        match self {
            Self::Line(_) | Self::Parabola(_) | Self::Hyperbola(_) => false,
            Self::Circle(_)
            | Self::Ellipse(_)
            | Self::Saddle(_)
            | Self::Marched(_)
            | Self::Quartic(_) => true,
        }
    }

    /// Whether two edges meeting at a corner are pieces of the one curve.
    ///
    /// **Asked of a pair that already shares a place**, which is what lets it
    /// be as cheap as it is: two lines through one point are the same line when
    /// they run the same way, and two circles through one point are the same
    /// circle when they turn about the one axis at the one radius. Nothing here
    /// looks for a shared place, and a caller that has not got one is asking a
    /// question this does not answer.
    ///
    /// **Not [`PartialEq`]**, which is stricter than the question: a boolean
    /// cutting a circle in two may hand each piece its own reference direction,
    /// and two arcs of one circle described from different marks are still two
    /// arcs of one circle.
    ///
    /// The two shapes a run of picked edges can be, and no others — see
    /// `.notes/KERNEL.md` §7.5, where the rest are refused.
    pub(crate) fn alike(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Line(one), Self::Line(two)) => predicate::parallel(one.direction, two.direction),
            (Self::Circle(one), Self::Circle(two)) => {
                one.axis.origin.approx_eq(two.axis.origin, PLACED)
                    && predicate::parallel(one.axis.direction, two.axis.direction)
                    && one.radius.approx_eq(two.radius, PLACED)
            }
            _ => false,
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
            | Self::Hyperbola(_)
            | Self::Parabola(_)
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
            // How far out the branch has run at `t`, which grows without bound
            // where a closed curve's does not.
            Self::Hyperbola(of) => {
                of.axis.origin.length() + of.major * t.cosh() + of.minor * t.sinh().abs()
            }
            Self::Parabola(of) => of.axis.origin.length() + of.focal * (t * t + 2.0 * t.abs()),
            // Both radii, the loop standing one out from the axis it is
            // written on and the other along it.
            Self::Saddle(saddle) => saddle.axis.origin.length() + saddle.reach + saddle.across,
            Self::Marched(of) => of.reach,
            Self::Quartic(of) => of.reach,
        }
    }

    /// The places a cut laid off this curve puts its corners at, in the order
    /// the curve runs.
    ///
    /// **A marched run hands back its own**, whatever is asked, on the same
    /// terms [`Curve::steps`] answers for one: the run *is* the curve, and
    /// reading it at even steps of the parameter would lay corners on the
    /// chords between its places rather than on the walk that made them — see
    /// [`Marchings::at`](super::marchings::Marchings), which interpolates by
    /// how far round a place stands. Every other curve is written down, and is
    /// walked at the chords it asks for.
    ///
    /// **Appended to `into` rather than handed back**, and appended rather than
    /// written over: a cut is laid off every piece of one meeting and reads
    /// them from one buffer, so each piece adds its own and names the stretch
    /// it added. A caller wanting the buffer to itself empties it first.
    pub(crate) fn sample(
        &self,
        span: f64,
        sagitta: f64,
        carried: &Carried,
        into: &mut Vec<Sampled>,
    ) {
        if let Self::Marched(of) = self {
            let walked = carried.marched.sampled(of.run);
            into.extend(walked.map(|(along, at)| Sampled { along, at }));
            return;
        }
        let steps = self.steps([0.0, span], sagitta, carried);
        into.reserve(steps + 1);
        for step in 0..=steps {
            let along = span * step as f64 / steps as f64;
            into.push(Sampled {
                along,
                at: self.at(along, carried),
            });
        }
    }

    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64, carried: &Carried) -> DVec3 {
        match self {
            Self::Line(line) => line.at(t),
            Self::Circle(circle) => circle.at(t),
            Self::Ellipse(ellipse) => ellipse.at(t),
            Self::Hyperbola(of) => of.at(t),
            Self::Parabola(of) => of.at(t),
            Self::Saddle(saddle) => saddle.at(t),
            Self::Marched(of) => carried.marched.at(of.run, t),
            Self::Quartic(of) => carried.quartics.at(of.run, t),
        }
    }

    /// How many straight pieces the stretch of parameter `bounds` is worth,
    /// flattened no further than `sagitta` from the true curve.
    ///
    /// Straight is exact however coarsely it is cut, so only a round curve is
    /// asked — see [`arc::chords`], which is where the rule lives and why it is
    /// one rule rather than one per caller.
    ///
    /// **The stretch and not its width**, because a curve need not bend the
    /// same everywhere along itself. Every closed curve here does and reads
    /// only the width; a hyperbola's branch bends harder the further out it is
    /// taken, so where the stretch *stands* decides — see
    /// [`Hyperbola::bending`].
    ///
    /// **An ellipse is asked with its longer half**, which is the radius of the
    /// circle it bends no harder than: how far a chord over a parameter step
    /// strays is set by the second derivative, and an ellipse's is at most its
    /// major semi-axis. So the same rule bounds it, conservatively at the flat
    /// ends and exactly at the sharp ones.
    pub(crate) fn steps(&self, bounds: [f64; 2], sagitta: f64, carried: &Carried) -> usize {
        let span = bounds[1] - bounds[0];
        match self {
            Self::Line(_) => 1,
            Self::Circle(circle) => arc::chords(circle.radius, span, sagitta),
            Self::Ellipse(ellipse) => arc::chords(ellipse.major, span, sagitta),
            Self::Hyperbola(of) => arc::chords(of.bending(bounds), span, sagitta),
            // Flat in the second derivative, which is what its parameter is
            // chosen for — see [`Parabola`].
            Self::Parabola(of) => arc::chords(2.0 * of.focal, span, sagitta),
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
