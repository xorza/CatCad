//! A cut across a face, and what a region is asked about one.

use crate::loops::Loops;
use crate::math::bisect;
use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::math::plane::Plane;
use crate::math::quadratic;
use crate::number::predicate;
use crate::number::tolerance::{ALIGNED, PLACED};
use crate::solid::boolean::splitting::bough::Bough;
use crate::solid::boolean::splitting::bow::{Bow, Bowed};
use crate::solid::boolean::splitting::corner::{Came, Corner};
use crate::solid::boolean::splitting::flare::Flare;
use crate::solid::boolean::splitting::oval::Oval;
use crate::solid::boolean::splitting::reading::Reading;
use crate::solid::boolean::splitting::ripple::Ripple;
use crate::solid::boolean::splitting::straight::Straight;
use crate::solid::boolean::splitting::traced::Traced;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// How finely a closed cut is flattened, as a fraction of its longer half.
///
/// **A classification tolerance and not a geometry one**, which is what lets it
/// be this coarse. What the corners are for is saying which region a place
/// falls in and how much one covers, and the body's own curve comes from the
/// meeting rather than from them — so what this has to be fine enough for is a
/// sample point landing on the right side of a hole, not for a face. Taken off
/// the tolerance ladder instead it would be seventy thousand chords to a
/// circle, for an answer no better.
pub(super) const ROUNDED: f64 = 1e-3;

/// A cut across a surface's own parameters, with a side to keep.
///
/// The side kept is always the *left* of the way the cut runs, which is what
/// makes cutting both ways one operation asked twice — see [`Cut::turned`].
///
/// Seven shapes, and what they have in common is that each divides the whole of
/// a face rather than a stretch of one. What a cut is *not* is a segment —
/// every stage downstream needs each region to be wholly one thing or the
/// other, and a cut that stopped part way would leave a region straddling it.
/// See `.notes/KERNEL.md` §7.4.
///
/// **Borrowed for the one call that splits by it**, which is what the fifth
/// shape asks and what nothing here loses by: a cut is built by `imprinted`
/// and read by [`Splitting::split`](super::Splitting::split), and no stage
/// keeps one. See [`Traced`], which is the arm that carries the borrow.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Cut<'a> {
    /// A straight cut, the left of its own `along` kept — see [`Straight`].
    Straight(Straight),
    /// A closed cut round an ellipse, the inside kept where its own `inward`
    /// says — see [`Oval`], which is the whole of what one is.
    Round(Oval),
    /// A cut along a cosine of the angle, kept above or below as its own
    /// `above` says — see [`Ripple`].
    Wave(Ripple),
    /// A cut along a root of a sine of the angle, which is closed or open as
    /// its own [`Bow::closed`] says — see [`Bow`], which is the one shape here
    /// that is either.
    Bow(Bow),
    /// A cut along one branch of an open conic, kept above or below as its own
    /// `above` says — see [`Bough`], which carries the parabola and the
    /// hyperbola alike.
    Bough(Bough),
    /// A cut along a plane's own section of a cone, which is every one of them
    /// — see [`Flare`].
    Flare(Flare),
    /// A cut traced from a curve's own places, which is what any curve with no
    /// closed form above gets — see [`Traced`]. A marched run and a general
    /// quartic both land here.
    Traced(Traced<'a>),
}

impl Cut<'static> {
    /// `along` in `on`'s own parameters, or `None` where it cannot be carried
    /// there.
    ///
    /// **The one place a curve of the world becomes a cut**, and the reason it is a
    /// table rather than a rule: a curve lying *on* a surface has a description in
    /// that surface's parameters only when the pair happen to suit each other, and
    /// which pairs do is a short list rather than a formula. A circle square to a
    /// cylinder's axis is a straight line in its `(θ, v)`; the same circle tilted is
    /// a sinusoid, and there is no honest way to write one down as a [`Cut`].
    ///
    /// `None` is a refusal and never "it misses": the caller already knows the two
    /// surfaces meet along this curve, so what this cannot carry is a face that
    /// would really have been divided — see
    /// [`Combining::cut`](crate::solid::boolean::combining::Combining), which
    /// turns that into a walk, and a refusal of the whole boolean where even that
    /// cannot be seeded.
    ///
    /// **A marched meeting is not here**, and the reason is that it is not one
    /// curve: it comes in pieces and one cut carries all of them, so what makes it
    /// is a pair of surfaces rather than a curve — see
    /// [`Combining::trace`](crate::solid::boolean::combining::Combining).
    ///
    /// `run` is the run the curve was given, and `None` where it was given none
    /// because it is a straight line — see
    /// [`Imprints`](crate::solid::boolean::imprints::Imprints). The round arms want one
    /// and the straight arms do not, which is exactly the two states of that
    /// argument.
    pub(crate) fn of(
        on: Surface,
        along: Curve,
        run: Option<u32>,
        laid: Bounds<DVec2>,
    ) -> Option<Self> {
        let about = laid.middle();
        match (on, along) {
            // A line on a plane is a line in its parameters.
            (Surface::Natural(Natural::Plane(plane)), Curve::Line(line)) => {
                let at = plane.flatten(line.origin);
                Some(Cut::Straight(Straight {
                    origin: at,
                    along: (plane.flatten(line.origin + line.direction) - at).normalize(),
                    run: None,
                }))
            }
            // A circle lying *in* a plane keeps its frame whole: the centre
            // flattens and the radius is a length, which a plane's parameters keep.
            // **Inward**, so what is kept first is the disc — the splitter cuts both
            // ways round and each side is read by where it stands, so which is
            // asked first says nothing about the answer.
            (Surface::Natural(Natural::Plane(plane)), Curve::Circle(circle)) => {
                Some(Cut::Round(Oval {
                    middle: plane.flatten(circle.axis.origin),
                    along: DVec2::X,
                    half: DVec2::splat(circle.radius),
                    inward: true,
                    run: run.expect("a circle is numbered"),
                }))
            }
            // A circle on a cylinder or a cone square to its axis is a *straight*
            // cut in that surface's own parameters: every place on it stands the
            // same distance along the axis, so it is the line `v = that`. Which is
            // what the end of a block does to a bore through it, and the reason a
            // bore needs no cut shape a plane did not already need.
            //
            // **One arm for the two**, because how far out the parameter reaches is
            // the whole of what separates them and a cut in parameter space never
            // asks. A cone's `v` is measured from its apex where a cylinder's is
            // measured from its origin, which is the axis each states.
            //
            // Square to the axis or not at all: a circle on either that is not is
            // no circle at all, and one whose plane is tilted meets it in an
            // ellipse, which arrives here as [`Curve::Ellipse`] and falls through.
            //
            // **Both of a cone's nappes arrive here**, a cone being both. The
            // circle of the far one lands at a `v` of the other sign, where a face
            // covering one nappe has nothing for it to cross, and the splitter
            // leaves that face whole. It is a cut that cuts nothing rather than a
            // case to keep out.
            (
                Surface::Natural(
                    Natural::Cylinder(Cylinder { axis, .. }) | Natural::Cone(Cone { axis, .. }),
                ),
                Curve::Circle(circle),
            ) if predicate::parallel(circle.axis.direction, axis.direction) => {
                Some(Cut::Straight(Straight {
                    origin: DVec2::new(0.0, axis.along(circle.axis.origin)),
                    along: DVec2::X,
                    run,
                }))
            }
            // **A circle on a sphere square to its axis is a straight cut too**, and
            // in the same parameter — but that one is an *angle* up from the
            // equator where a cylinder's is a height along the axis, so the two
            // cannot share an arm however alike they read. Every place of such a
            // circle stands at one angle up, so it is the line `v = that`.
            //
            // Square to the axis or not at all. A circle on a sphere that is not
            // square to it is one no straight line in these parameters holds: it
            // runs `v = ψ(u) ± acos(…)` for an amplitude and a phase that both move
            // with `u`, and nothing writes that down. Which costs nothing but the
            // sampling — the caller walks the curve instead, see
            // [`Combining::walked`](crate::solid::boolean::combining::Combining).
            (Surface::Natural(Natural::Sphere(sphere)), Curve::Circle(circle))
                if predicate::parallel(circle.axis.direction, sphere.axis.direction) =>
            {
                Some(Cut::Straight(Straight {
                    origin: DVec2::new(0.0, sphere.uv(circle.at(0.0)).y),
                    along: DVec2::X,
                    run,
                }))
            }
            // **Every section of a cone is one shape in its own parameters** — see
            // [`flared`], where that is derived. Which conic it is decides nothing
            // here: what the cut reads is the plane, and each of the three carries
            // the plane's own frame in its axis.
            //
            // A *circle* is the one section left out, and on purpose: square across
            // the axis it is the line `v = that`, which the arm above writes down
            // exactly where this one would chord it.
            (Surface::Natural(Natural::Cone(cone)), Curve::Ellipse(of)) => {
                Some(flared(cone, of.axis, of.at(0.0), laid, run))
            }
            (Surface::Natural(Natural::Cone(cone)), Curve::Parabola(of)) => {
                Some(flared(cone, of.axis, of.at(0.0), laid, run))
            }
            (Surface::Natural(Natural::Cone(cone)), Curve::Hyperbola(of)) => {
                Some(flared(cone, of.axis, of.at(0.0), laid, run))
            }
            // **An open conic on a plane is a graph about its own vertex** — see
            // [`boughed`], which is where the pair of them come to one shape. A
            // parabola's vertex is its own origin and it has no eccentricity to
            // spare; a branch stands its own `major` off the centre the two of them
            // share, and `ε` there is `b²/a²`.
            (Surface::Natural(Natural::Plane(plane)), Curve::Parabola(bent)) => Some(boughed(
                plane,
                bent.axis.origin,
                bent.axis.reference,
                bent.latus(),
                0.0,
                run,
            )),
            (Surface::Natural(Natural::Plane(plane)), Curve::Hyperbola(branch)) => Some(boughed(
                plane,
                branch.axis.origin + branch.axis.reference * branch.major,
                branch.axis.reference,
                branch.latus(),
                (branch.minor / branch.major).powi(2),
                run,
            )),
            // A ruling line on a cylinder is a cut at a constant angle, which is a
            // straight cut in a parameter that *wraps*: `θ = that`, and which turn
            // of it decides whether the face is divided at all. A face may not wrap
            // — `.notes/KERNEL.md` §4.4 — so its own range covers less than a whole
            // turn and at most one of them falls inside it: the one nearest the
            // middle it was laid out about. Where none does the cut misses, and a
            // cut that misses leaves the region whole, which is the right answer
            // rather than a refusal.
            //
            // What a plane parallel to an axis does to a shaft, which is a flat, a
            // keyway or a D — and the edges it leaves are straight in the world, a
            // ruling of a cylinder being a straight line, so it carries no imprint.
            (Surface::Natural(Natural::Cylinder(tube)), Curve::Line(line))
                if predicate::parallel(line.direction, tube.axis.direction) =>
            {
                let angle = tube.axis.angle_of(line.origin);
                Some(Cut::Straight(Straight {
                    origin: DVec2::new(branch::nearest(angle, about.x), 0.0),
                    along: DVec2::Y,
                    run: None,
                }))
            }
            // **A ruling on a cone is one straight cut across both nappes**, which
            // its parameters make of a line through the apex: `u = that`, the same
            // number either side. A place at a negative `v` is measured from the
            // apex *back* along the ray — see [`Cone::uv`] — so the angle the ray
            // going one way stands at is the angle the ray going the other way
            // stands at, and the ruling is one line of the chart rather than two.
            //
            // What a plane through the apex leaves — see [`Meeting::apexed`](crate::solid::meeting::Meeting) — and
            // what cutting a turned part down its own axis reaches. The angle
            // wraps, so which turn is the one nearest the middle the region was
            // laid out about, exactly as a cylinder's ruling below.
            //
            // Straight in the world, so it carries no imprint.
            (Surface::Natural(Natural::Cone(cone)), Curve::Line(line)) => {
                let on = line.origin + line.direction;
                debug_assert!(
                    predicate::touching(cone.off(on), PLACED),
                    "{line:?} is no ruling of {cone:?}",
                );
                let angle = cone.uv(on).x;
                Some(Cut::Straight(Straight {
                    origin: DVec2::new(branch::nearest(angle, about.x), 0.0),
                    along: DVec2::Y,
                    run: None,
                }))
            }
            // An ellipse lying *in* a plane keeps its frame whole, as a circle
            // does: the centre and both halves flatten, and what a plane's
            // parameters do to lengths is nothing. Which way round the two halves
            // turn is the one thing to settle — a plane's own uv may hand the
            // ellipse over mirrored, and [`Cut::Round`] reads its shorter half as a
            // quarter turn *ahead* of its longer one.
            (Surface::Natural(Natural::Plane(plane)), Curve::Ellipse(oval)) => {
                let middle = plane.flatten(oval.axis.origin);
                let major = plane.flatten(oval.at(0.0)) - middle;
                let minor = plane.flatten(oval.at(FRAC_PI_2)) - middle;
                let along = major.normalize();
                Some(Cut::Round(Oval {
                    middle,
                    along: if along.perp().dot(minor) < 0.0 {
                        -along
                    } else {
                        along
                    },
                    half: DVec2::new(oval.major, oval.minor),
                    inward: true,
                    run: run.expect("an ellipse is numbered"),
                }))
            }
            // And on the cylinder that same ellipse is a *wave* — see [`Cut::Wave`].
            // Every place of it stands where the ellipse's own plane cuts the
            // cylinder's ruling at that angle: `n·p = n·c` with
            // `p = O + r·radial(θ) + d·v` gives `v` as a cosine of `θ`, whose
            // amplitude is how far the plane leans and whose phase is which way it
            // leans.
            (Surface::Natural(Natural::Cylinder(tube)), Curve::Ellipse(oval)) => {
                let axis = tube.axis;
                let normal = oval.axis.direction;
                let leaning = normal.dot(axis.direction);
                // An ellipse at all means the plane leans on the axis: square to it
                // the crossing is a circle, and along it two lines.
                debug_assert!(
                    !predicate::touching(leaning.abs(), ALIGNED),
                    "{oval:?} lies square to {axis:?} and is no ellipse on it",
                );
                let across = DVec2::new(
                    normal.dot(axis.reference) * tube.radius,
                    normal.dot(axis.quarter()) * tube.radius,
                );
                Some(Cut::Wave(Ripple {
                    level: normal.dot(oval.axis.origin - axis.origin) / leaning,
                    swing: -across.length() / leaning,
                    phase: across.y.atan2(across.x),
                    above: true,
                    run: run.expect("an ellipse is numbered"),
                }))
            }
            // **A saddle is a bow on either of the two cylinders it was cut
            // from**, and which of them the face stands on decides the regime and
            // nothing else — see [`Bow`], where the shape is derived. The wider
            // cylinder, which the saddle is written on, sees a closed loop; the
            // narrower one sees a single branch of a cut that runs right round it.
            //
            // Which turn of the loop, for the wider one, is the question a ruling
            // line answers above: the angle wraps and a face may not, so the turn
            // taken is the one nearest the middle the face was laid out about.
            (Surface::Natural(Natural::Cylinder(tube)), Curve::Saddle(saddle)) => {
                let axis = tube.axis;
                // Where the two axes come nearest, read along whichever of them
                // this face stands on — which is the same reading either way, the
                // saddle's own origin being that place.
                let level = (saddle.axis.origin - axis.origin).dot(axis.direction);
                // The wider cylinder by exact equality, and sound for the reason
                // [`Combining::against`](crate::solid::boolean::combining::Combining) tells
                // two faces of one surface apart that
                // way: the saddle was *given* this direction rather than working
                // one out.
                if axis.direction == saddle.axis.direction {
                    let phase = axis.bearing(saddle.axis.reference);
                    return Some(Cut::Bow(Bow {
                        across: saddle.across,
                        reach: saddle.reach,
                        phase: branch::nearest(phase, about.x),
                        off: saddle.off,
                        level,
                        // The loop is both branches, so there is no branch to pick.
                        upper: true,
                        inward: true,
                        run: run.expect("a saddle is numbered"),
                    }));
                }
                // The narrower cylinder reads the same numbers the other way round:
                // the offset is the same length square to both axes, and which
                // branch this loop is is which way the narrower axis was taken —
                // see [`Saddle`](crate::solid::geometry::saddle::Saddle), where the two loops are that one flip.
                let upper = saddle.axis.reference.dot(axis.direction) > 0.0;
                Some(Cut::Bow(Bow {
                    across: saddle.reach,
                    reach: saddle.across,
                    phase: axis.bearing(saddle.axis.direction),
                    off: if upper { saddle.off } else { -saddle.off },
                    level,
                    upper,
                    inward: true,
                    run: run.expect("a saddle is numbered"),
                }))
            }
            // **A circle on a torus is a straight cut in its own parameters**, and
            // which of the two it holds constant is which of them the circle turns
            // about. One sharing the axis stands at a single angle round the tube,
            // so it is that angle; one round the tube itself stands at a single
            // angle about the axis, so it is that one. Both parameters wrap, so
            // both take the turn nearest the middle the face was laid out about —
            // the question a ruling line answers above, asked twice over.
            //
            // Neither, and the circle crosses both parameters at once: Villarceau's
            // do, and no cut is written for them, so a boolean that met one is
            // refused rather than answered wrongly.
            (Surface::Fitted(Fitted::Torus(torus)), Curve::Circle(circle)) => {
                let axis = torus.axis;
                let uv = torus.uv(circle.at(0.0));
                if predicate::parallel(circle.axis.direction, axis.direction) {
                    Some(Cut::Straight(Straight {
                        origin: DVec2::new(0.0, branch::nearest(uv.y, about.y)),
                        along: DVec2::X,
                        run,
                    }))
                } else if predicate::square(circle.axis.direction, axis.direction) {
                    Some(Cut::Straight(Straight {
                        origin: DVec2::new(branch::nearest(uv.x, about.x), 0.0),
                        along: DVec2::Y,
                        run,
                    }))
                } else {
                    None
                }
            }
            // Everything else.
            _ => None,
        }
    }
}

impl<'a> Cut<'a> {
    /// The same cut with the other side kept.
    pub(super) fn turned(self) -> Self {
        match self {
            Self::Straight(straight) => Self::Straight(straight.turned()),
            Self::Round(oval) => Self::Round(Oval {
                inward: !oval.inward,
                ..oval
            }),
            Self::Wave(ripple) => Self::Wave(Ripple {
                above: !ripple.above,
                ..ripple
            }),
            Self::Bow(bow) => Self::Bow(Bow {
                inward: !bow.inward,
                ..bow
            }),
            Self::Bough(bough) => Self::Bough(Bough {
                above: !bough.above,
                ..bough
            }),
            Self::Flare(flare) => Self::Flare(Flare {
                under: !flare.under,
                ..flare
            }),
            Self::Traced(traced) => Self::Traced(traced.turned()),
        }
    }

    /// What the corners this cut puts down at `at` are marked with.
    ///
    /// A straight imprint needs nothing remembered about it — a line between
    /// two places is the same line whoever drew it — so only a circle is
    /// numbered.
    ///
    /// **Asked of a place, because a marched cut is several curves.** A meeting
    /// walked rather than written down comes in pieces and the cut is the whole
    /// of it, so which curve a corner lies on is which piece it stands on — see
    /// [`Traced`]. Every other shape is one curve and reads nothing, so a run
    /// of corners along one asks this once and carries the answer.
    ///
    /// `at` has to be a place *on* the cut: a marched one finds the piece
    /// nearest it, and a place off the cut answers with whichever piece it
    /// happens to lie nearest.
    pub(super) fn came(self, at: DVec2) -> Came {
        match self {
            Self::Straight(Straight { run: Some(run), .. })
            | Self::Round(Oval { run, .. })
            | Self::Wave(Ripple { run, .. })
            | Self::Bow(Bow { run, .. })
            | Self::Bough(Bough { run, .. })
            | Self::Flare(Flare { run, .. }) => Came::Arc(run),
            Self::Traced(traced) => traced.came(at),
            Self::Straight(Straight { run: None, .. }) => Came::Edge,
        }
    }

    /// Which piece of it the parameter `at` runs along.
    ///
    /// **One piece for every cut but a traced one**, which is the only shape
    /// that comes in disjoint curves — see [`Traced::piece`]. A meeting written
    /// down as one circle, one wave or one bow is one curve, however far round
    /// itself it goes.
    pub(super) fn piece(self, at: f64) -> usize {
        match self {
            Self::Traced(traced) => traced.piece(at),
            _ => 0,
        }
    }

    /// Whether it is a loop in its own right rather than a line across
    /// everything.
    pub(super) fn closed(self) -> bool {
        match self {
            Self::Round(_) => true,
            Self::Bow(bow) => bow.closed(),
            Self::Traced(traced) => traced.closed(),
            Self::Straight(_) | Self::Wave(_) | Self::Bough(_) | Self::Flare(_) => false,
        }
    }

    /// Whether a region every corner of which lies on this cut is kept.
    ///
    /// **Only a closed cut shuts anything in.** A region every corner of which
    /// lies on a line has no width and bounds nothing on either side of it, so
    /// the open shapes answer `false` without reading anything.
    ///
    /// A shape reads its own middle. A marched cut has places rather than a
    /// middle, so it reads which way the piece the region lies on winds —
    /// `anywhere` is any corner of the region, one being as good as another
    /// where every one of them is on the cut.
    pub(super) fn keeps_its_inside(self, anywhere: Option<DVec2>) -> bool {
        match self {
            Self::Round(oval) => self.side(oval.middle) > 0.0,
            Self::Bow(bow) => bow.closed() && self.side(bow.middle()) > 0.0,
            Self::Traced(traced) => traced.closed() && anywhere.is_some_and(|at| traced.holds(at)),
            Self::Straight(_) | Self::Wave(_) | Self::Bough(_) | Self::Flare(_) => false,
        }
    }

    /// How far off the cut `point` stands, positive on the side being kept.
    pub(super) fn side(self, point: DVec2) -> f64 {
        match self {
            Self::Straight(straight) => straight.side(point),
            // **How far off along the ray from the middle**, which is what a
            // radius is to a circle and the nearest thing an ellipse has to
            // one. A true distance to an ellipse is a quartic; this agrees
            // with it exactly where the two halves are equal, and everywhere
            // else it is the same sign and the same nought, which is all
            // [`Side::of`] reads and all the walk asks.
            Self::Round(oval) => {
                let off = oval.reach(point);
                if oval.inward { off } else { -off }
            }
            // Straight up, which is a distance in `v` and an overstatement of
            // the distance to the wave itself by however steeply it runs. The
            // sign and the nought are exact, and those are what [`Side::of`]
            // and the walk read.
            Self::Wave(ripple) => {
                let off = point.y - ripple.crest(point.x);
                if ripple.above { off } else { -off }
            }
            // Two measures in one arm, a bow being closed or open — see
            // [`Bow::side`], where each is argued.
            Self::Bow(bow) => bow.side(point),
            // The wave above read in a frame that leans — see [`Bough::side`],
            // where the same overstatement is argued.
            Self::Bough(bough) => bough.side(point),
            // Linear in the distance along the cone's axis, and scaled by how
            // flat the plane lies against the ruling there — see
            // [`Flare::side`].
            Self::Flare(flare) => flare.side(point),
            // The true distance to the *other surface*, which is the one shape
            // here that has one to give — see [`Traced::side`].
            Self::Traced(traced) => traced.side(point),
        }
    }

    /// How far along the cut `point` stands.
    ///
    /// A distance for a line and an angle for a circle, and what the two have
    /// in common is the only thing read off them: they increase the way the cut
    /// runs, so ordering by this is ordering along the cut — see
    /// `Splitting::close`, which reassembles by it and by nothing else.
    ///
    /// Read backwards by [`Cut::at`], which is why both are written:
    /// `down(at(x)) == x` and `at(down(p))` is `p` back on the cut, so the walk
    /// can measure where it met the cut and the reassembly can put corners back
    /// along it without either spelling out which way round a circle runs.
    pub(super) fn down(self, point: DVec2) -> f64 {
        match self {
            Self::Straight(straight) => straight.down(point),
            Self::Round(oval) => {
                let off = oval.frame(point - oval.middle) / oval.half;
                let turned = off.y.atan2(off.x).rem_euclid(TAU);
                // Counterclockwise keeps the disc on the left, so keeping
                // everything *but* it runs the other way round.
                if oval.inward { turned } else { TAU - turned }
            }
            // The angle itself, the wave being a graph over it. Keeping what is
            // above puts that on the left of a walk running the way the angle
            // grows; keeping what is below runs the other way.
            Self::Wave(ripple) => {
                if ripple.above {
                    point.x
                } else {
                    -point.x
                }
            }
            // An angle round the loop where it is closed and the cylinder's own
            // angle where it is not, which is the two above under one call.
            Self::Bow(bow) => bow.down(point),
            // The branch's own first parameter, which is a length rather than
            // an angle — see [`Bough::down`].
            Self::Bough(bough) => bough.down(point),
            // The cone's own angle, which the cut is a graph over.
            Self::Flare(flare) => flare.down(point),
            // How far round the run it was walked as, measured from a place
            // the face does not hold — see [`Traced::down`].
            Self::Traced(traced) => traced.down(point),
        }
    }

    /// Whether any of it runs through the box `fills`.
    ///
    /// **What says a region is not worth walking.** A face cut by *n* surfaces
    /// is walked again by each cut after the first, and most of those cuts come
    /// nowhere near most of those regions — a hundred and twenty-eight walls
    /// against a block's face leave a hundred and twenty-eight slices, and the
    /// next wall crosses two of them. A region no cut of it reaches is whole to
    /// one side of it, so it survives the cut as it stands and keeps the corners
    /// it was written with — which four comparisons settle where a walk of them
    /// settled it before.
    ///
    /// **Sound because the boxes decide containment as well as crossing.** A
    /// closed cut lying wholly inside a region, or a region swallowed whole by
    /// the disc one bounds, both put one box inside the other — so boxes that
    /// do not meet leave the cut clear of the region *and* the region clear of
    /// what it shuts in, and which side the region is on is then the same for
    /// every corner of it.
    ///
    /// **Every arm answers off its own shape.** A line and an ellipse have a
    /// box; a wave and a bow have a band in the height alone, being graphs over
    /// an angle that wraps; a marched run has the boxes of its pieces. Coarse
    /// where a band is all there is, and not wrong: what a cull owes is to drop
    /// work and never an answer.
    pub(super) fn reaches(self, fills: Bounds<DVec2>) -> bool {
        match self {
            Self::Straight(straight) => straight.reaches(fills),
            // The box an ellipse fills, which is its middle plus how far each
            // of its two halves reaches along each axis.
            Self::Round(oval) => {
                let across = oval.along.perp();
                let reach = DVec2::new(
                    (oval.half.x * oval.along.x).hypot(oval.half.y * across.x),
                    (oval.half.x * oval.along.y).hypot(oval.half.y * across.y),
                );
                Bounds {
                    low: oval.middle - reach,
                    high: oval.middle + reach,
                }
                .meets(fills, 0.0)
            }
            // Both of these are a graph over the angle where they are answered
            // at all, so they run the whole width of a face and what bounds
            // them is `v` alone. A wave swings its own `swing` either side of
            // its level; a bow's two numbers are the other cylinder's radius
            // and how far off its axis a place stands, and the sum of their
            // squares is that radius the whole way round — see [`Bow::turn`].
            Self::Wave(ripple) => banded(ripple.level, ripple.swing, fills),
            Self::Bow(bow) => banded(bow.level, bow.across, fills),
            // Its own frame rather than a band, a branch being a graph over a
            // *length* that does not wrap — see [`Bough::reaches`].
            Self::Bough(bough) => bough.reaches(fills),
            // Half a band, a flare running away from the apex without bound on
            // the one nappe — see [`Flare::reaches`].
            Self::Flare(flare) => flare.reaches(fills),
            Self::Traced(traced) => traced.reaches(fills),
        }
    }

    /// Where the stretch of boundary leaving `from` and reaching `to` crosses
    /// it.
    ///
    /// **On the run that stretch walks, and on the straight line between the
    /// two corners only where it walks none** — see [`Reading`], which argues
    /// what the difference between the two is worth.
    ///
    /// **`None` where the curve the stretch walks does not cross after all**,
    /// which the two corners standing either side of the cut says it must. What
    /// the corners say is where the *straight run* between them stands, and the
    /// walk goes along the curve — so a stretch whose ends read a hair off the
    /// cut once carried onto the curve is one the bisection reads as a graze
    /// and hands nothing back for. Refused rather than guessed at, on the terms
    /// every other unanswerable case here takes — see
    /// [`Boolean::combine`](crate::solid::boolean::Boolean), which lists them.
    ///
    /// **A corner at a place the surface names with every angle at once is not
    /// one of them.** A cone's apex and a sphere's pole are written twice — see
    /// [`Face::flatten`](crate::solid::topology::face::Face) — and both
    /// writings stand at the one place, so no cut puts the pair of them on
    /// opposite sides of itself. What each stretch leaving one walks is the
    /// curve its own mark names, which is why the marks are doubled along with
    /// the corners — see
    /// [`Face::doubled`](crate::solid::topology::face::Face).
    pub(super) fn crossing(self, from: Corner, to: Corner, reading: Reading<'_>) -> Option<DVec2> {
        if let Came::Arc(run) = from.came
            && let Some(curve) = reading.curved(run)
        {
            return self.met_along(curve, from, to, reading);
        }
        Some(self.met_across(from.at, to.at))
    }

    /// Where the stretch leaving `from` crosses it, walking `curve` between the
    /// two corners rather than the straight run between them — see [`Reading`],
    /// which argues why that is the difference between a body and a refusal.
    ///
    /// `None` where the walk finds no crossing, which is [`Cut::crossing`]'s
    /// refusal and where the reason for it is.
    fn met_along(
        self,
        curve: Curve,
        from: Corner,
        to: Corner,
        reading: Reading<'_>,
    ) -> Option<DVec2> {
        let start = reading.along(curve, from.at);
        let end = reading.along(curve, to.at);
        let middle = (from.at + to.at) / 2.0;
        // **Which way round the stretch runs, measured rather than assumed.** A
        // curve answers where a place stands on it in one turn, so the two
        // corners give the same pair of answers whichever way the boundary
        // walks between them. What tells the two apart is the stretch itself:
        // the way round whose middle stands nearer the middle of the straight
        // run between the corners is the way the boundary goes.
        //
        // Taking the near way round instead is right for a flattening, whose
        // corners are a chord apart, and wrong for the case that has no near
        // way — a face wrapping a whole cylinder is two corners half a turn
        // apart, and half a turn is the same distance both ways.
        let near = start + (end - start + PI).rem_euclid(TAU) - PI;
        let far = near - TAU.copysign(near - start);
        let strays = |ended: f64| {
            reading
                .at(curve, (start + ended) / 2.0, middle)
                .distance(middle)
        };
        let end = if strays(far) < strays(near) {
            far
        } else {
            near
        };
        // Read on the branch the loop itself runs on, which is what the
        // straight run between the corners is still good for: the face's
        // parameters are unwrapped along a loop — see
        // [`Face::flatten`](crate::solid::topology::face::Face) — so a curve's
        // own answer is as near the wrong end of a long stretch as the right
        // one.
        let place = |part: f64| {
            let along = start + (end - start) * part;
            reading.at(curve, along, from.at.lerp(to.at, part))
        };
        let part = bisect::root(0.0, 1.0, |part| self.side(place(part)))?;
        Some(place(part))
    }

    /// The same, across the straight run from `from` to `to`.
    ///
    /// The two have to be on opposite sides, which every caller has just
    /// established — so exactly one root of the two a closed shape answers lies
    /// on the run.
    fn met_across(self, from: DVec2, to: DVec2) -> DVec2 {
        let [from, to] = ordered(from, to);
        match self {
            Self::Straight(straight) => straight.crossing(from, to),
            Self::Traced(traced) => traced.crossing(from, to),
            Self::Flare(flare) => flare.crossing(from, to),
            Self::Round(_) | Self::Wave(_) | Self::Bow(_) | Self::Bough(_) => {
                let along = self
                    .met(from, to)
                    .into_iter()
                    .find(|&along| (0.0..=1.0).contains(&along))
                    .expect("the run crosses the cut");
                from.lerp(to, along)
            }
        }
    }

    /// Where the straight run from `from` to `to` crosses it *twice*, both ends
    /// standing on the same side.
    ///
    /// The case a bent cut has and a straight one cannot: a run whose ends are
    /// both outside an ellipse can still pass through it, and one whose ends
    /// both stand above a wave can still dip below it — so what this finds is a
    /// boundary crossing the cut and back between two of its corners, which the
    /// walk would otherwise step straight over. A line has no such case, and
    /// [`Cut::met`] answers with nothing for one.
    pub(super) fn grazes(self, from: DVec2, to: DVec2) -> Option<[DVec2; 2]> {
        match self {
            Self::Traced(traced) => traced.grazes(from, to),
            // Against the chords it lays down rather than against the reading,
            // which is what a traced cut does and for the same reason — see
            // [`Flare::grazes`].
            Self::Flare(flare) => flare.grazes(from, to),
            // **Two, or the run went across rather than dipping.** One crossing
            // is a boundary that ends on the far side and the walk has it
            // already; none at all, or the one a straight cut always answers, is
            // nothing to find here. A graze is a miss for the reason
            // [`roots`](crate::math::quadratic::roots) argues one dimension up.
            Self::Straight(_) | Self::Round(_) | Self::Wave(_) | Self::Bow(_) | Self::Bough(_) => {
                let [first, second]: [f64; 2] = self.met(from, to).all().try_into().ok()?;
                let inside = |along: f64| (PLACED..=1.0 - PLACED).contains(&along);
                (inside(first) && inside(second))
                    .then(|| [from.lerp(to, first), from.lerp(to, second)])
            }
        }
    }

    /// The place the parameter `along` stands at, which is [`Cut::down`] read
    /// backwards.
    ///
    /// **Not a marched cut**, which has places rather than a formula to read
    /// them off — see [`Traced::between`]. Both callers answer that shape
    /// before they reach here, so a marched one arriving is a walk that lost
    /// its way rather than a state to report.
    fn at(self, along: f64) -> DVec2 {
        match self {
            Self::Straight(straight) => straight.at(along),
            Self::Round(oval) => oval.at(along),
            Self::Wave(ripple) => ripple.at(along),
            Self::Bough(bough) => bough.at(along),
            Self::Flare(flare) => flare.at(along),
            Self::Bow(bow) => bow.at(along),
            Self::Traced(_) => unreachable!("a marched cut is read off its own places"),
        }
    }

    /// How many chords a stretch of `sweep` parameter is worth.
    ///
    /// **One for a straight cut, which lays no corners of its own.** A stretch
    /// of a line between two points *is* the straight run between them, which
    /// whatever closes the loop has already got — so one chord is the whole of
    /// it and [`Cut::between`] writes nothing. A circle's stretch is not, and a
    /// loop closed without its corners cuts the corner with a chord: a quarter
    /// disc coming back as the triangle under it.
    ///
    /// Not a marched cut, on the terms [`Cut::at`] states.
    fn steps(self, sweep: f64) -> usize {
        match self {
            Self::Round(oval) => oval.steps(sweep),
            Self::Wave(ripple) => ripple.steps(sweep),
            Self::Bough(bough) => bough.steps(sweep),
            Self::Flare(flare) => flare.steps(sweep),
            Self::Bow(bow) => bow.steps(sweep),
            Self::Straight(_) => 1,
            Self::Traced(_) => unreachable!("a marched cut is chorded as it was walked"),
        }
    }

    /// The corners of the cut between two places along it, in the direction it
    /// runs, exclusive of both.
    ///
    /// **Round the turn where the cut closes on itself and straight along it
    /// where it does not** — see [`Cut::closed`], which is that one question
    /// and is asked here rather than answered a shape at a time. A wave, a
    /// branch and a flare all run from one edge of the face to the other and
    /// `down` grows the whole way; a circle and a shut bow come back to where
    /// they began.
    ///
    /// **Nothing for a straight cut**, and that is not an oversight — see
    /// [`Cut::steps`], which answers one chord for one and says why.
    ///
    /// `false` where there is no such stretch, which only a marched cut has —
    /// see [`Traced::between`].
    pub(super) fn between(self, from: f64, to: f64, into: &mut Vec<Corner>) -> bool {
        // The piece's own places rather than a shape read at a step, a marched
        // curve having no formula to read — see [`Traced::lay`].
        if let Self::Traced(traced) = self {
            return traced.between(from, to, into);
        }
        let sweep = match self.closed() {
            true => (to - from).rem_euclid(TAU),
            false => to - from,
        };
        let count = self.steps(sweep);
        let came = self.came(self.at(from));
        into.extend((1..count).map(|step| Corner {
            at: self.at(from + sweep * step as f64 / count as f64),
            came,
        }));
        true
    }

    /// The cut as loops of corners, each wound so the side kept is on its left.
    ///
    /// **Appends nothing for a cut that is not closed**, which is not a loop
    /// and cannot bound anything on its own.
    ///
    /// Flattened, and this is the one place in the boolean that flattens
    /// anything. What these corners are for is saying which region a place
    /// falls in and how much one covers; the *body* takes its curve from the
    /// meeting that produced the cut and never from here — see
    /// `.notes/KERNEL.md` §7.4.
    pub(super) fn walk(self, into: &mut Loops<Corner>) {
        // **Several loops rather than one**, a marched meeting coming in
        // pieces — see [`Traced`].
        if let Self::Traced(traced) = self {
            return traced.walk(into);
        }
        if !self.closed() {
            return;
        }
        into.add(|write| {
            let count = self.steps(TAU);
            let came = self.came(self.at(0.0));
            write.reserve_exact(count);
            write.extend((0..count).map(|step| Corner {
                at: self.at(TAU * step as f64 / count as f64),
                came,
            }));
        });
    }

    /// Where along the run from `from` to `to` the cut is met, in order.
    ///
    /// Nothing for a straight cut, which is not that it is never met: a line is
    /// met once and [`Cut::crossing`] has a better reading of where, so what
    /// this is for — a boundary that crosses the cut and comes back — a line
    /// does not have.
    ///
    /// And nothing for a marched one or for a flare, neither of which has a
    /// closed form to solve: both of the callers reach [`Traced`] and
    /// [`Flare`] before they reach here.
    fn met(self, from: DVec2, to: DVec2) -> Bowed {
        let mut met = Bowed::none();
        match self {
            Self::Straight(_) | Self::Traced(_) | Self::Flare(_) => {}
            Self::Round(oval) => {
                // In the frame the ellipse is the unit circle in, where the run
                // is still a straight run and the meeting is still a quadratic.
                let run = oval.frame(to - from) / oval.half;
                let start = oval.frame(from - oval.middle) / oval.half;
                let roots = quadratic::roots(
                    run.length_squared(),
                    2.0 * run.dot(start),
                    start.length_squared() - 1.0,
                );
                for along in roots.into_iter().flatten() {
                    met.push(along);
                }
            }
            Self::Wave(ripple) => {
                for along in ripple.crested(from, to) {
                    met.push(along);
                }
            }
            Self::Bow(bow) => met = bow.bowed(from, to),
            Self::Bough(bough) => {
                for along in bough.crossed(from, to) {
                    met.push(along);
                }
            }
        }
        met
    }
}

/// The cut a plane makes on a cone, read off the plane's own frame.
///
/// `on` is the frame the section carries — every conic here holds the plane's
/// normal as its direction and a place of the plane as its origin — and `laid`
/// is the stretch of the cone's parameters the face covers.
///
/// **Derived rather than fitted.** A place of a cone is
/// `apex + v·a + v·tan α·radial(θ)`, so `n·(x − o) = 0` carries one `v` in
/// every term: what is left is `v·(level + swing·cos(θ − phase)) = apart` for
/// `level = n·a`, `swing = tan α·|n − a(n·a)|` and a phase where the normal
/// leans out. See [`Flare`], which is that reading and nothing else.
///
/// **Which way the normal points decides nothing**, and that is worth checking
/// rather than assuming: turning it over flips `apart`, `level` and the phase
/// by half a turn together, so the reading turns over — and so does which side
/// of the cut the apex is on, which turns it back.
///
/// **`at` says which nappe, and it has to be a place of the section.** A plane
/// past a cone's rulings cuts *two* arcs, one on each nappe, and the two carry
/// the identical reading — the same plane, read the same way. What tells them
/// apart is the nappe, which is what [`Flare::reaches`] culls the far one by: a
/// face lies on one, so the arc on the other divides it nowhere and the cut is
/// put aside whole.
///
/// **Read off the arc and not off the face**, which is what the two sides of
/// one edge have to agree on: a face cut by the one arc that reaches it carries
/// that arc's own run, and the face across the edge breaks it along the same
/// one. Read off the face, both arcs cut it under two runs and the second
/// wrote the marks.
///
/// **And `laid` says how far**, which is the size the chording is held
/// against: the end of the face's own `v` that stands furthest from the apex.
fn flared(cone: Cone, on: Axis, at: DVec3, laid: Bounds<DVec2>, run: Option<u32>) -> Cut<'static> {
    let normal = on.direction;
    let out = normal - cone.axis.direction * normal.dot(cone.axis.direction);
    Cut::Flare(Flare {
        level: normal.dot(cone.axis.direction),
        swing: cone.half_angle.tan() * out.length(),
        phase: cone.axis.bearing(out),
        apart: (on.origin - cone.axis.origin).dot(normal),
        upward: cone.axis.along(at) > 0.0,
        under: true,
        reach: laid.low.y.abs().max(laid.high.y.abs()),
        run: run.expect("a section of a cone is numbered"),
    })
}

/// The cut an open conic makes on the plane holding it, about the vertex it
/// stands at and the direction it opens along.
///
/// **One shape for the parabola and the hyperbola's branch**, which is what the
/// vertex form buys: every conic reads `ε·y² + 2L·y − x² = 0` there, so a
/// semi-latus rectum and an `ε` are the whole of the difference — see
/// [`Bough`]. A plane holds the curve, so its frame flattens whole: the vertex
/// is a place, the opening is a direction, and the two numbers are lengths,
/// which a plane's parameters keep.
///
/// **Framed off the opening direction**, so which way the plane's own two axes
/// wind decides nothing. What a cut needs is the side it keeps on the left of
/// the way it runs, and the branch is even about its own axis — so the first
/// parameter is set a quarter turn back from the second and either sign of it
/// draws the same curve.
fn boughed(
    plane: Plane,
    vertex: DVec3,
    opening: DVec3,
    latus: f64,
    bend: f64,
    run: Option<u32>,
) -> Cut<'static> {
    let at = plane.flatten(vertex);
    let up = plane.flatten(vertex + opening) - at;
    Cut::Bough(Bough {
        at,
        across: DVec2::new(up.y, -up.x),
        latus,
        bend,
        above: true,
        run: run.expect("an open conic is numbered"),
    })
}

/// Whether a cut running the whole width of the angle, and reaching `swing`
/// either side of `level`, gets into the box `fills`.
///
/// **A band rather than a box**, which is what a graph over a parameter that
/// wraps comes to: it is somewhere at every angle, so the angle bounds nothing
/// and only the height does. That is still most of what a cull wants — a face
/// cut at a constant height has its regions stacked in exactly that direction.
fn banded(level: f64, swing: f64, fills: Bounds<DVec2>) -> bool {
    let reach = swing.abs();
    fills.low.y <= level + reach && level - reach <= fills.high.y
}

/// The two ends of one stretch in one order, whichever way round the walk
/// handed them over.
///
/// **What makes a crossing the same place from either side of it.** A cut is
/// taken twice over the region it divides — once keeping each side — and a
/// later cut meets the two halves of the stretch it left walking opposite
/// ways. Interpolated from one end, a crossing rounds a little differently from
/// the same crossing interpolated from the other, so the two halves come back
/// carrying places an ulp apart and nothing downstream can tell that they are
/// one place. See `.notes/KERNEL.md` §9.3, where what that costs is argued.
///
/// Any total order does, so long as it is the same one on both sides. This is
/// [`f64::total_cmp`] over the two coordinates in turn, which orders every pair
/// of places a walk can hand over and reads nothing but the values themselves.
fn ordered(from: DVec2, to: DVec2) -> [DVec2; 2] {
    let by = from.x.total_cmp(&to.x).then(from.y.total_cmp(&to.y));
    match by.is_le() {
        true => [from, to],
        false => [to, from],
    }
}

#[cfg(test)]
mod tests;
