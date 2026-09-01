//! The ruled patch a corner two picks do not agree about is filled with.
//!
//! **Ruled because both joins are tangent**, which is one statement rather than
//! two: a patch meeting both blends tangentially along its own two edges has
//! every ruling lying in both tangent planes at once. So the ruling from a
//! place on the fillet is the line lying in the fillet's tangent plane there
//! and running tangent to the round's cylinder — two divisions and one root,
//! with no fit anywhere. `.notes/KERNEL.md` §9.6 is where no quadric is shown
//! to do the same job, and where the whole corner is argued.
#![allow(dead_code)]
// Kept ahead of its caller deliberately: the arm of
// [`Fitted`](super::fitted::Fitted) that would reach this wants a box round the
// patch before it can cull, which `.notes/KERNEL.md` §9.6 leaves open. The
// geometry below is settled and tested, and the route in `Rounding` that raises
// it lands with that arm.

use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::math::harmonic;
use crate::number::predicate;
use crate::number::predicate::ApproxEq;
use crate::number::tolerance::{EXACT, PLACED};
use crate::solid::buckets::Key;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::surface::Crossings;
use glam::{BVec2, DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::iter::zip;

/// How many chords the second edge is first walked in.
///
/// Small, because the doubling below reaches a fine walk in a handful of rounds
/// and most corners are small — where starting fine would pay for every corner
/// whose reach makes it coarse enough not to need it.
const FIRST: usize = 4;

/// How many times that walk may double before it hands back what it reached.
///
/// **A stray over the sagitta asked for is an answer rather than a failure.**
/// What a run carries is what it was walked to — see
/// [`Strayed::most`](super::marchings::Strayed) — and §4.1's tier is about
/// saying how far a fit stands rather than about always reaching a number.
/// Twelve doublings is sixteen thousand chords, which no corner wants.
const DOUBLINGS: usize = 12;

/// How many chords the box's own walk of the second edge takes.
///
/// **Coarse deliberately**, the box being grown by whatever that walk strayed —
/// so a finer one moves the box by less than the stray it saves. This runs per
/// surface per face a boolean culls, where the walk that *files* the edge runs
/// once and is as fine as its caller asked.
const BOXED: usize = 16;

/// How many angles the search for the nearest place reads at each round, and
/// how many rounds it narrows for.
///
/// Each round keeps the bracket either side of the best reading, so the arc it
/// searches shrinks by `2/SOUGHT` a round — sixteen and six leave a millionth
/// of it, which is finer than the walk that laid the patch's own edge down.
const SOUGHT: usize = 16;
const ROUNDS: usize = 6;

const _: () = assert!(
    SOUGHT.is_multiple_of(2),
    "an odd count leaves the last round's best off the next round's grid, so a \
     search could answer worse than a round it had already reached",
);

/// The patch between two blends of one reach whose picks do not agree.
///
/// **Parameterized by the fillet's own angle and the run along the ruling**, in
/// that order, so that `u` is what runs round it — the convention every curved
/// surface here keeps. `v` of nought is the edge on the fillet and `v` of one
/// the edge on the round.
///
/// **Four fields and no more.** Where the two blends touch is the middle of
/// their axes' common perpendicular, and the plane the first edge is the
/// section of comes off that and [`Gusset::from`] — so neither is held, and
/// neither can come to disagree with what it was derived from. A call wanting
/// either of them several times over works them out once into a [`Framing`]
/// and carries that down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Gusset {
    /// The blend filled into the concave edge. Every ruling leaves it, and the
    /// patch's first edge lies on it.
    pub(crate) filled: Cylinder,
    /// The blend cut into the convex edge. Every ruling lands on it, and the
    /// patch's second edge lies on it.
    pub(crate) cut: Cylinder,
    /// Where the first edge starts, on [`Gusset::filled`]: the corner its other
    /// ruling runs out to.
    pub(crate) from: DVec3,
    /// Which of the two lines through a place tangent to [`Gusset::cut`] the
    /// ruling is.
    ///
    /// **A bit rather than a reading**, because the two close on each other at
    /// the touch point and nothing measured at a place tells them apart there.
    /// Which way round each axis was framed decides it, and a caller does not
    /// choose that.
    pub(crate) turning: bool,
}

/// What every ruling of one [`Gusset`] shares.
///
/// **Worked out once for a call that wants several rulings, and never stored.**
/// A cast against the patch reads seven angles and then walks up to six
/// crossings, each of which builds two rulings — so the touch point and the
/// cutting plane would be found a score of times for a ray where they are the
/// same two values throughout. Carried as a value the caller makes rather than
/// as fields on the patch, which is [`Gusset`]'s own rule.
#[derive(Debug, Clone, Copy)]
struct Framing {
    /// Where the two blends touch — see [`Gusset::met`].
    met: DVec3,
    /// The normal of the plane the first edge is a section by — see
    /// [`Gusset::cutting`].
    cutting: DVec3,
    /// Where the first edge's own ellipse is centred, and how far it swings
    /// along each of the two directions the fillet's angle is read against.
    ///
    /// **The first edge written as an ellipse rather than a place at a time.**
    /// The fillet's section by the cutting plane is
    /// `middle + cos u · swung[0] + sin u · swung[1]`, which is one statement
    /// where two would be two — [`Gusset::headed`] reads a place off it and
    /// [`Gusset::fills`] boxes the whole of it without reading one at all.
    middle: DVec3,
    swung: [DVec3; 2],
    /// Which of the two tangents through a head the ruling is, as a sign.
    side: f64,
    /// The stretch of the fillet's angle the patch covers — see
    /// [`Gusset::bounds`], which is the reading, and [`Gusset::along`], which
    /// walks it.
    bounds: [f64; 2],
}

/// One ruling of a [`Gusset`], and how fast each of its ends moves.
///
/// The rates are here rather than worked out again because the normal wants
/// both and the place wants neither: everything below the ruling is one cross
/// product, and everything above it would be the same arithmetic twice.
#[derive(Debug, Clone, Copy)]
struct Ruling {
    head: DVec3,
    heading: DVec3,
    foot: DVec3,
    footing: DVec3,
}

impl Gusset {
    /// The patch between the two blends, its first edge starting at `from`.
    pub(crate) fn new(filled: Cylinder, cut: Cylinder, from: DVec3, turning: bool) -> Self {
        let gusset = Self {
            filled,
            cut,
            from,
            turning,
        };
        let room = predicate::slack(EXACT, filled.radius);
        debug_assert!(
            predicate::touching((filled.radius - cut.radius).abs(), room),
            "{gusset:?} blends one corner with two reaches",
        );
        debug_assert!(
            predicate::touching((filled.axis.off(from) - filled.radius).abs(), room),
            "{from} is not on the fillet the patch runs out of",
        );
        gusset
    }

    /// The key several of these are filed under — see
    /// [`Natural::key`](super::natural::Natural), which is where the argument
    /// for it is.
    ///
    /// The word carries on from the naturals' four and the torus's fifth, so no
    /// two surfaces of the whole set can collide on it.
    pub(crate) fn key(&self) -> u64 {
        let filled = self
            .filled
            .axis
            .keyed(Key::default().word(5))
            .float(self.filled.radius);
        self.cut
            .axis
            .keyed(filled)
            .float(self.cut.radius)
            .place(self.from)
            .word(u64::from(self.turning))
            .done()
    }

    /// The stretch of the fillet's own angle the patch covers, its first edge
    /// first and the tip second.
    ///
    /// **The near way round of the two**, which is the whole of the patch: a
    /// corner is a gap between two blends that touch, so what it spans is under
    /// a half turn and the far way round is the fillet itself. Read the other
    /// way it would run out of the wedge the two picks left.
    ///
    /// Not ascending — which end is the greater is which way the fillet was
    /// framed, and a reader wanting the span takes the difference.
    pub(crate) fn bounds(&self) -> [f64; 2] {
        self.framing().bounds
    }

    /// Which of the two parameters run round the patch, so that a face on it
    /// could wrap.
    ///
    /// **The first alone, and a face on this one never reaches round it
    /// anyway.** The fillet's angle is what `u` is, so it wraps as every angle
    /// here does; `v` is how far along the ruling a place stands, which runs
    /// from nought at the head to one at the foot and no further.
    pub(crate) fn round(&self) -> BVec2 {
        BVec2::new(true, false)
    }

    /// Whether the parameterization says nothing at `at`.
    ///
    /// **The tip alone.** Every ruling closes to nothing there — see
    /// [`Gusset::met`] — so the patch shuts the way a cone shuts at its apex,
    /// and `v` names no direction at that one place.
    pub(crate) fn singular(&self, at: DVec3) -> bool {
        at.approx_eq(self.met(), PLACED)
    }

    /// The patch's second edge, laid down in chords from its first edge round
    /// to the tip, and how far those chords stand from the edge itself.
    ///
    /// **Walked rather than written down**, which is what puts the patch in the
    /// fitted tier although both of its joins are exact. The first edge is a
    /// plane section of the fillet and so an exact ellipse; the second follows
    /// from it and is then not planar — over the whole pencil of planes through
    /// its two ends it stands between a seventeenth and a tenth of its own size
    /// out of flat, and never approaches nought. See `.notes/KERNEL.md` §9.6.
    ///
    /// **Probed rather than bounded**, exactly as a marched meeting is — see
    /// [`Marching`](crate::solid::meeting::marching::Marching), where the same
    /// reading is argued. Nothing writes this edge's curvature down, so no step
    /// count can be read off a radius the way
    /// [`arc::chords`](crate::math::arc::chords) reads one:
    /// instead the walk doubles until three places along the worst chord all
    /// stand within `sagitta` of the edge, and what comes back is what that
    /// walk measured.
    ///
    /// **Against the edge at the same share of the angle**, which overstates
    /// the distance to the curve where the two do not run in step — and
    /// overstating a stray is the safe way to be wrong about one.
    ///
    /// The tip is the last place written, so a caller sewing this edge finds
    /// the corner the two edges share where it expects it.
    pub(crate) fn chorded(&self, sagitta: f64, into: &mut Vec<DVec3>) -> f64 {
        debug_assert!(sagitta > 0.0, "a sagitta of {sagitta} chords nothing");
        let framing = self.framing();
        let mut steps = FIRST;
        let mut most = self.laid(steps, framing, into);
        while most > sagitta && steps < FIRST << DOUBLINGS {
            steps *= 2;
            most = self.laid(steps, framing, into);
        }
        most
    }

    /// The box the whole patch fills.
    ///
    /// **Half of it is written down and half of it is walked.** The first edge
    /// is a plane section of the fillet, so it is an ellipse in the world — see
    /// [`Framing::swung`] — and `a cos u + b sin u` never passes `√(a² + b²)`,
    /// which boxes it on each axis without reading a place off it. The second
    /// edge lies on the round and wants its extent *along* that axis, which is
    /// the quotient with no value at the tip — so it is walked, and the box is
    /// grown by what that walk strayed.
    ///
    /// **The rulings ask for nothing of their own.** Every one of them has both
    /// ends on those two edges, so the patch lies inside their convex hull and
    /// a box round the two is a box round the patch.
    ///
    /// **Coarse deliberately.** The ellipse is read over the fillet's whole
    /// turn rather than the arc the patch covers, and a finer walk of the
    /// second edge would move the box by less than the stray it saved. What a
    /// cull owes is to drop work and never an answer, which is the trade a
    /// torus's own box makes as well.
    pub(crate) fn fills(&self) -> Bounds<DVec3> {
        let framing = self.framing();
        let [one, two] = framing.swung;
        let swing = DVec3::new(one.x.hypot(two.x), one.y.hypot(two.y), one.z.hypot(two.z));
        let mut fills = Bounds {
            low: framing.middle - swing,
            high: framing.middle + swing,
        };
        let strayed = self.along(BOXED, framing, |at| fills.hold(at));
        Bounds {
            low: fills.low - strayed,
            high: fills.high + strayed,
        }
    }

    /// The nearest place of the patch to `at`.
    ///
    /// **Sought rather than solved**, which is what a ruled patch has where a
    /// quadric has a formula. The nearest place on the ruling at one angle is a
    /// projection onto a segment and closed form; *which* angle carries the
    /// nearest of all is a minimum over the arc, and nothing isolates its roots
    /// — the reading has a derivative, [`Ruling`] carrying the rate of both
    /// ends, but no fence to bracket one between. So the arc is read at
    /// [`SOUGHT`] angles, the bracket either side of the best is kept, and that
    /// is narrowed [`ROUNDS`] times.
    ///
    /// **Every round re-reads the last one's best**, an even [`SOUGHT`] putting
    /// it at the middle of the bracket it left — so a search never answers
    /// worse than a round it has already reached.
    ///
    /// **A reading and not a proof**, which is the whole difference §4.1 draws
    /// between the tiers — and what puts this patch in the fitted one although
    /// both of its joins are exact.
    ///
    /// A hundred rulings a call, which is what a caller asking per correction
    /// of a march should know it is spending.
    pub(crate) fn nearest(&self, at: DVec3) -> DVec3 {
        let framing = self.framing();
        let [mut from, mut to] = framing.bounds;
        let mut best = self.onto(from, framing, at);
        for _ in 0..ROUNDS {
            let step = (to - from) / SOUGHT as f64;
            let mut which = 0;
            let mut off = f64::INFINITY;
            for at_angle in 0..=SOUGHT {
                let onto = self.onto(from + step * at_angle as f64, framing, at);
                let reach = onto.distance_squared(at);
                if reach < off {
                    (off, which, best) = (reach, at_angle, onto);
                }
            }
            // Either side of the best, which is where the true one lies: the
            // distance to a ruling moves smoothly with the angle, so a reading
            // beaten by both its neighbours has the minimum between them.
            let near = from + step * (which.saturating_sub(1)) as f64;
            to = from + step * (which + 1).min(SOUGHT) as f64;
            from = near;
        }
        best
    }

    /// How far `at` stands from the patch, never signed.
    ///
    /// A reading, on the terms [`Gusset::nearest`] states.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        at.distance(self.nearest(at))
    }

    /// The nearest place of the ruling at `angle` to `at`.
    ///
    /// **The segment and not the line it runs along**, the patch running out
    /// where its two blends do — a place past either end of a ruling is nearest
    /// that end. At the tip the ruling has closed to nothing and the touch
    /// point is the whole of it.
    fn onto(&self, angle: f64, framing: Framing, at: DVec3) -> DVec3 {
        let ruling = self.ruled(angle, framing);
        let along = ruling.foot - ruling.head;
        let reach = along.length_squared();
        if reach == 0.0 {
            return ruling.head;
        }
        ruling.head + along * ((at - ruling.head).dot(along) / reach).clamp(0.0, 1.0)
    }

    /// Whether any of the patch passes within `slack` of `fills`.
    ///
    /// **Settled outright rather than narrowed.** What
    /// [`Surface::reaches`](super::surface::Surface) falls back on is halving
    /// the caller's box and reading a distance at each half, and a ruled patch
    /// has no distance written down to read — so a bounded surface answers off
    /// its own box instead. What that costs is [`Gusset::fills`]'s own coarse
    /// walk, against eight halvings and a distance apiece.
    pub(crate) fn spans(&self, fills: Bounds<DVec3>, slack: f64) -> bool {
        self.fills().meets(fills, slack)
    }

    /// Walk the second edge in `steps` chords, handing each place to `held`,
    /// and say how far the worst chord stands from the edge.
    ///
    /// **Handed over rather than laid down**, because the two callers want
    /// different things of one walk: one files the places as a run and one only
    /// takes their box, and a walk that answered with a list would make the
    /// second ask for a buffer it has nowhere to keep.
    ///
    /// Three places along each chord, which is what a marched curve is measured
    /// by and for the same reason: a smooth curve leaves its chord furthest
    /// near the middle, so three catch what one would and a leaning chord
    /// besides.
    ///
    /// **The tip is written rather than read.** Where the ruling has closed to
    /// nothing both edges are the touch point, and how far along the round's
    /// axis the ruling lands is nought over nought there — see
    /// `.notes/KERNEL.md` §9.6, which is where that limit is argued. The
    /// probing stops three quarters of a chord short of it, so nothing reads
    /// the quotient at the one angle it has no value at.
    fn along(&self, steps: usize, framing: Framing, mut held: impl FnMut(DVec3)) -> f64 {
        let foot = |u: f64| self.ruled(u, framing).foot;
        let [from, to] = framing.bounds;
        let step = (to - from) / steps as f64;
        let mut most = 0.0_f64;
        let mut last = foot(from);
        held(last);
        for at in 1..=steps {
            let began = from + step * (at as f64 - 1.0);
            let here = match at == steps {
                true => framing.met,
                false => foot(began + step),
            };
            for share in [0.25, 0.5, 0.75] {
                let along = last.lerp(here, share);
                most = most.max(along.distance(foot(began + step * share)));
            }
            held(here);
            last = here;
        }
        most
    }

    /// The same walk laid down in `into`, rather than handed over a place at a
    /// time — see [`Gusset::along`].
    fn laid(&self, steps: usize, framing: Framing, into: &mut Vec<DVec3>) -> f64 {
        into.clear();
        into.reserve_exact(steps + 1);
        self.along(steps, framing, |at| into.push(at))
    }

    /// Where the two blends touch, which is the tip the patch closes to.
    ///
    /// **The middle of the two axes' common perpendicular.** Both spines run a
    /// reach off the face the two picks share and run off it on opposite sides,
    /// so the axes stand two reaches apart and touch nothing but each other's
    /// tube at that one place — `.notes/KERNEL.md` §9.6.
    fn met(&self) -> DVec3 {
        let (one, two) = (self.filled.axis, self.cut.axis);
        let across = one.direction.cross(two.direction);
        let apart = two.origin - one.origin;
        let spread = across.length_squared();
        let head = one.origin + one.direction * (apart.cross(two.direction).dot(across) / spread);
        let foot = two.origin + two.direction * (apart.cross(one.direction).dot(across) / spread);
        (head + foot) / 2.0
    }

    /// What every ruling of this patch shares, worked out once — see
    /// [`Framing`].
    fn framing(&self) -> Framing {
        let axis = self.filled.axis;
        let reach = self.filled.radius;
        let met = self.met();
        let cutting = self.cutting(met);
        // The plane fixes how far along the axis a head stands and the angle
        // fixes the rest of it, so one division serves every ruling.
        let leaning = cutting.dot(axis.direction);
        let along = reach / leaning;
        Framing {
            met,
            cutting,
            middle: axis.origin + axis.direction * (cutting.dot(self.from - axis.origin) / leaning),
            swung: [axis.reference, axis.quarter()]
                .map(|way| way * reach - axis.direction * (along * cutting.dot(way))),
            side: match self.turning {
                true => 1.0,
                false => -1.0,
            },
            bounds: {
                let start = axis.angle_of(self.from);
                [start, branch::nearest(axis.angle_of(met), start)]
            },
        }
    }

    /// Where the parameters `uv` land: `u` radians round the fillet, `v` along
    /// the ruling from it.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        self.placed(uv, self.framing())
    }

    /// The same, against a framing the caller already has.
    fn placed(&self, uv: DVec2, framing: Framing) -> DVec3 {
        let ruling = self.ruled(uv.x, framing);
        ruling.head + (ruling.foot - ruling.head) * uv.y
    }

    /// How far the flat triangle on the parameters `corners` strays from the
    /// patch at its furthest.
    ///
    /// **Probed rather than bounded**, which is the third time this patch has
    /// had to be — see [`Gusset::along`] and [`Gusset::nearest`], and §4.1,
    /// which is what the tier means. Across `v` the patch is exactly linear, so
    /// a triangle standing on one ruling strays nowhere at all; across `u` it
    /// bends by both of its edges at once, and the second edge's rate about the
    /// round's axis is the one thing here nothing writes down.
    ///
    /// **Four places, and the three middles are what a leaning triangle
    /// wants.** A smooth patch leaves its plane furthest near the centre, so
    /// the centroid catches what one probe would; a triangle long in `u` and
    /// thin in `v` leaves it near an edge instead, which the edge middles
    /// catch. The same argument [`Marching`](crate::solid::meeting::marching)
    /// makes for three shares along a chord.
    ///
    /// Seven rulings a triangle, where a quadric answers this off a radius —
    /// which is what a face on this surface costs a mesher.
    pub(crate) fn straying(&self, corners: [DVec2; 3]) -> f64 {
        /// The barycentric shares each probe stands at.
        const PROBES: [[f64; 3]; 4] = [
            [0.5, 0.5, 0.0],
            [0.0, 0.5, 0.5],
            [0.5, 0.0, 0.5],
            [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
        ];
        let framing = self.framing();
        let places = corners.map(|uv| self.placed(uv, framing));
        let mut most = 0.0_f64;
        for share in PROBES {
            let uv: DVec2 = zip(corners, share).map(|(at, by)| at * by).sum();
            let flat: DVec3 = zip(places, share).map(|(at, by)| at * by).sum();
            most = most.max(self.placed(uv, framing).distance(flat));
        }
        most
    }

    /// Which way the surface faces at `uv`, the way its own parameters wind
    /// about.
    ///
    /// **A ruled surface turns along its ruling**, which is what the second
    /// term is: the place moves at one rate where the ruling leaves the fillet
    /// and at another where it lands, and a reading between the two moves at
    /// the blend of them. So the two edges read the two blends' own normals
    /// back, which is the tangency this patch exists to keep.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let ruling = self.ruled(uv.x, self.framing());
        let along = ruling.foot - ruling.head;
        let across = ruling.heading + (ruling.footing - ruling.heading) * uv.y;
        across.cross(along).normalize()
    }

    /// Which parameters `at` stands at, its angle read in `(-π, π]`.
    ///
    /// **Closed form, and the tangency is why.** Every ruling lies in a tangent
    /// plane of the fillet, and those planes are one family — so a place off
    /// that cylinder stands in exactly two of them, `(x − o)·m = r` being one
    /// harmonic in the angle. Which of the two carries the place is a reading
    /// rather than a bit, the two rulings standing well apart wherever the
    /// question is asked.
    ///
    /// The angle comes back off the ruling's own head rather than out of the
    /// harmonic, so a half turn reads the way every other surface here reads
    /// one — see [`Axis::bearing`](super::axis::Axis).
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        let axis = self.filled.axis;
        let to = at - axis.origin;
        let (out, round) = (to.dot(axis.reference), to.dot(axis.quarter()));
        let lean = (self.filled.radius / out.hypot(round))
            .clamp(-1.0, 1.0)
            .acos();
        let bearing = round.atan2(out);
        let framing = self.framing();
        let held = [bearing - lean, bearing + lean].map(|angle| self.ruled(angle, framing));
        let ruling = if held[0].strays(at) <= held[1].strays(at) {
            held[0]
        } else {
            held[1]
        };
        let along = ruling.foot - ruling.head;
        DVec2::new(
            axis.angle_of(ruling.head),
            (at - ruling.head).dot(along) / along.length_squared(),
        )
    }

    /// How far along a ray from `from` running `way` it meets this, in order.
    ///
    /// **Six at most, and the tip is why it is not eight.** A ray meets a ruled
    /// surface where it lies in a plane with a ruling, which here is one
    /// equation in the fillet's own angle: the direction the ray picks out of
    /// the fillet's tangent plane, held against the pair of directions there
    /// that run tangent to the round. Written out that is a harmonic of degree
    /// four — the two top harmonics cancel, which
    /// `.notes/KERNEL.md` §9.6 shows — and it carries a double root at the tip
    /// whatever the ray, the tangent plane there being the face the two blends
    /// share and every line in it touching the round. Divided out, what is left
    /// is a harmonic of degree three and six roots at the most.
    ///
    /// **The tip is never one of them.** The division takes the doubled root —
    /// a graze counts for none, which is the policy [`harmonic::angles`] states
    /// — and the ruling there has closed to nothing, so a ray reaching that far
    /// has no line left to cross.
    ///
    /// **Every root of the harmonic is a crossing of one of the two tangents,
    /// and only one of them is this patch's.** Which is settled by a comparison
    /// rather than a tolerance: the ray stands nearer the ruling it meets than
    /// the one it does not.
    ///
    /// **The ruling and not the line it runs along.** Every other surface here
    /// is unbounded where the faces on it are not, and a ray is answered about
    /// the whole of it; this one closes at the tip and runs out where its two
    /// blends do, so a crossing past either end of a ruling is a crossing of
    /// nothing. Leaving them in would answer places a hundred million reaches
    /// out, which no face holds and the inversion cannot read back.
    ///
    /// **A crossing on one of the patch's own two edges is a boundary place**,
    /// and which side of the edge a rounding puts it is the loops' business
    /// rather than this one's — the same ray meets the blend that edge is
    /// shared with on *its* boundary, where a cast is abandoned outright.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Crossings {
        let framing = self.framing();
        let tip = self.filled.axis.angle_of(framing.met);
        let start = tip + PI;
        let mut readings = [0.0; harmonic::READINGS];
        for (step, reading) in readings.iter_mut().enumerate() {
            let angle = start + TAU * step as f64 / harmonic::READINGS as f64;
            *reading = self.aimed(angle, from, way, framing) / (1.0 - (angle - tip).cos());
        }
        let mut found = Crossings::none();
        for angle in harmonic::angles(readings, start) {
            if let Some(along) = self.crossed(angle, from, way, framing) {
                found.push(along);
            }
        }
        found.sorted()
    }

    /// The ruling the fillet's angle `angle` carries, and how fast its two ends
    /// move.
    ///
    /// **The head is the fillet's own section by the plane the first edge is
    /// cut by**, and the foot is where the line through it in the fillet's
    /// tangent plane runs tangent to the round. The second is two statements:
    /// `(o − x)·m = −r` puts the landing angle where the ruling touches, one
    /// harmonic and so one root, and squaring the ruling against the fillet's
    /// normal puts it along the round's axis, which is linear.
    fn ruled(&self, angle: f64, framing: Framing) -> Ruling {
        let (axis, other) = (self.filled.axis, self.cut.axis);
        let reach = self.filled.radius;
        let cutting = framing.cutting;
        let (facing, turning) = (axis.radial(angle), axis.radial(angle + FRAC_PI_2));
        let leaning = cutting.dot(axis.direction);
        let running = -reach * cutting.dot(turning) / leaning;
        let head = self.headed(angle, framing);
        let heading = axis.direction * running + turning * reach;

        let to = other.origin - head;
        let toward = -heading;
        let across = to - other.direction * to.dot(other.direction);
        let crossing = toward - other.direction * toward.dot(other.direction);
        let spread = across.length_squared();
        let spreading = 2.0 * across.dot(crossing);
        let square = other.direction.cross(across);
        let squaring = other.direction.cross(crossing);
        let root = (spread - reach * reach).max(0.0).sqrt();
        let rooting = across.dot(crossing) / root;
        let over = across * -reach + square * (framing.side * root);
        let overing = crossing * -reach + (squaring * root + square * rooting) * framing.side;
        let lands = over / spread;
        let landing = (overing * spread - over * spreading) / (spread * spread);

        let slide = other.direction.dot(facing);
        let sliding = other.direction.dot(turning);
        let want = -(to.dot(facing) + reach * lands.dot(facing));
        let wanting = -(toward.dot(facing)
            + to.dot(turning)
            + reach * (landing.dot(facing) + lands.dot(turning)));
        let step = want / slide;
        let stepping = (wanting * slide - want * sliding) / (slide * slide);
        Ruling {
            head,
            heading,
            foot: other.origin + other.direction * step + lands * reach,
            footing: other.direction * stepping + landing * reach,
        }
    }

    /// Where the patch's first edge stands at the fillet's angle `angle`.
    ///
    /// Read off the ellipse the framing carries — see [`Framing::swung`], which
    /// is where the fillet's section by the cutting plane is derived.
    fn headed(&self, angle: f64, framing: Framing) -> DVec3 {
        let (sin, cos) = angle.sin_cos();
        framing.middle + framing.swung[0] * cos + framing.swung[1] * sin
    }

    /// How far the ray from `from` running `way` stands from meeting either of
    /// the two tangents the head at `angle` carries — nought where it meets
    /// one of them.
    ///
    /// **Both tangents at once, which is what makes this a polynomial.** The
    /// ray picks one direction out of the fillet's tangent plane, and the two
    /// tangents to the round are the roots of a quadratic form on that plane —
    /// so putting the ray's own direction into the form asks about both without
    /// taking the root that would tell them apart.
    fn aimed(&self, angle: f64, from: DVec3, way: DVec3, framing: Framing) -> f64 {
        let (axis, other) = (self.filled.axis, self.cut.axis);
        let turning = axis.radial(angle + FRAC_PI_2);
        let head = self.headed(angle, framing);
        let leaving = head - from;
        let (axial, across) = (
            leaving.cross(axis.direction).dot(way),
            leaving.cross(turning).dot(way),
        );
        let (square, slant) = (
            axis.direction.cross(other.direction),
            turning.cross(other.direction),
        );
        let to = other.origin - head;
        let reach = self.filled.radius;
        (across * to.dot(square) - axial * to.dot(slant)).powi(2)
            - reach
                * reach
                * (across * across * square.length_squared()
                    - 2.0 * across * axial * square.dot(slant)
                    + axial * axial * slant.length_squared())
    }

    /// How far along the ray from `from` running `way` it crosses the ruling at
    /// `angle`, or `None` where what it meets there is the other tangent.
    ///
    /// **A comparison and not a bound.** At a root of [`Gusset::aimed`] the ray
    /// meets one of the two tangents exactly and stands clear of the other, so
    /// which is which is read off the two distances rather than off a
    /// tolerance. A tangent running along the round's own axis carries no
    /// place, and answers nothing.
    ///
    /// **A tie is this patch's.** The two tangents meet each other at the head
    /// and nowhere else, so a ray through the head stands nought from both —
    /// and so does one meeting the single tangent a head on the round itself
    /// carries. Both are crossings of this patch, so only the *other* tangent
    /// standing strictly nearer turns one away. A head standing inside the
    /// round carries no tangent at all, and is refused before either is read.
    fn crossed(&self, angle: f64, from: DVec3, way: DVec3, framing: Framing) -> Option<f64> {
        let ruling = self.ruled(angle, framing);
        if self.cut.axis.off(ruling.head) < self.filled.radius {
            return None;
        }
        let apart = |ruling: &Ruling| {
            let along = ruling.foot - ruling.head;
            let across = along.cross(way);
            (ruling.head - from).dot(across).abs() / across.length()
        };
        let mine = apart(&ruling);
        if mine.is_nan() || apart(&self.ruled(angle, framing.turned())) < mine {
            return None;
        }
        let along = ruling.foot - ruling.head;
        let leaving = ruling.head - from;
        let across = way.cross(along);
        let run = leaving.cross(way).dot(across) / across.length_squared();
        (0.0..=1.0)
            .contains(&run)
            .then(|| leaving.cross(along).dot(across) / across.length_squared())
    }

    /// The normal of the plane the patch's first edge is the fillet's section
    /// by, unnormalized.
    ///
    /// **Through both ends of that edge, and square to the fillet's rulings at
    /// the touch point.** The second is what leaves a corner rather than a cusp
    /// where the patch's two edges meet: both leave that point in the shared
    /// face's plane, so an edge running out *along* its own blend would leave
    /// in its neighbour's direction. See `.notes/KERNEL.md` §9.6, where the
    /// choice is named as a choice.
    fn cutting(&self, met: DVec3) -> DVec3 {
        let axis = self.filled.axis;
        axis.direction
            .cross(met - axis.origin)
            .cross(self.from - met)
    }
}

impl Framing {
    /// The same, reading the other of the two tangents.
    fn turned(self) -> Self {
        Self {
            side: -self.side,
            ..self
        }
    }
}

impl Ruling {
    /// How far `at` stands from the line this runs along.
    fn strays(&self, at: DVec3) -> f64 {
        let along = (self.foot - self.head).normalize();
        let to = at - self.head;
        (to - along * to.dot(along)).length()
    }
}

#[cfg(test)]
mod tests;
