//! The patch a corner three picks do not agree about is filled with.
//!
//! **Set back, where every other corner in §7.5 is not.** Three blends that
//! agree leave a hole a sphere spans and three chamfers leave none at all; three
//! that do not agree leave one no surface already written reaches — see
//! `.notes/VERTEX-BLENDS.md`, where each family is ruled out in turn. So the
//! blends are stopped short of the corner and a patch spans what they leave.
//!
//! **A height over one plane, blended from what its six sides hold it to.** The
//! opening is where each blend stops and the six places that bounds; the patch
//! spans it, tangent to each blend along a cross section and to each face along
//! a spring.

use std::f64::consts::{PI, TAU};

use crate::math::bisect;
use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::math::plane::Plane;
use crate::number::predicate;
use crate::number::tolerance::PLACED;
use crate::solid::buckets::Key;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::surface::Crossings;
use glam::{DVec2, DVec3};

/// What a patch raised at a corner no patch spans would be.
pub(crate) const PATCHED: &str = "a patch was raised at a corner it cannot span";

/// How many cells the walk of the patch's own box takes each way.
const BOXED: u32 = 12;

/// How many cells the probe of a triangle's straying takes each way.
///
/// **Four, which is fifteen places over the triangle.** The patch is smooth,
/// so what a flat triangle leaves under it is a bowl with one low place.
/// Measured on the notch's step corner at a stride of `1.05e-2`, the probe
/// reads within `1.5e-4` of one taken at thirty-two — and a probe at eight
/// reads no closer, what it misses being where the lattice falls rather than
/// how fine it is. Every place added costs the mesher a reading of the height.
const PROBED: u32 = 4;

/// How many steps a ray is walked over before its crossings are closed on.
const WALKED: u32 = 64;

/// How many places each of the six sides is walked at — see
/// [`Patched::walked`].
const RIMMED: u32 = 24;

/// How many shells in from the rim the reading is taken over, drawn in toward
/// the middle by squares — see [`Patched::walked`].
const STEPPED: u32 = 8;

/// What share of the reach a middle difference is taken over — see
/// [`Patched::bent`].
const BENT: f64 = 1e-3;

/// How far in from a circle's own radius a side is hushed — a ninth, which is a
/// third of the way in. See [`Sided::hushed`].
const HUSHED: f64 = 1.0 / 81.0;

/// The corner three blends of one reach leave, and the setback that opens it.
///
/// **Wound rather than paired**, which is what lets the six places come out in
/// one order: `blends[i]` and `blends[i + 1]` are the two that share
/// `faces[i]`, so a walk round the corner alternates a blend and a face without
/// asking which meets which.
///
/// **Every axis runs away from the corner.** A cylinder says where its blend
/// lies and not which way the edge under it runs, and the setback is measured
/// along that edge — so the caller hands the axes over pointing the way the
/// blend goes, and the sign is not worked out again here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Vertexed {
    /// Each blend's own axis, in the order the three run round the corner.
    ///
    /// **A line and not a cylinder**, the three sharing one reach and none of
    /// them wanting a frame: what a blend is here is where its axis runs and
    /// how far off it the surface stands, and a `Surface` is copied by value on
    /// every path a frame walks.
    axes: [Line; 3],
    /// Which way the face each neighbouring pair shares points: `facing[i]` is
    /// the normal of the one both `blends[i]` and `blends[i + 1]` run out onto.
    ///
    /// **A normal and not a plane**, all three faces running through the corner
    /// — so the corner and the normal are the whole of one. Each points *out of
    /// the material*, the way its own face of the body does, which is what
    /// [`Vertexed::flattening`] adds up.
    facing: [DVec3; 3],
    /// The corner of the body the three swallow.
    at: DVec3,
    /// The one reach all three were raised at.
    reach: f64,
    /// How far along its own edge each blend stops short of that corner.
    setback: f64,
    /// How hard the patch bends: the larger of its height's two principal
    /// second derivatives, at the worst place a walk of the opening finds.
    ///
    /// **Carried rather than asked for.** It is what a mesh's own stride comes
    /// off, and the walk that finds it is by far the largest thing this surface
    /// does — so it is worked out once, where the surface is made, and never
    /// again. See [`Vertexed::new`], which is the only way one is made, and
    /// [`Patched::stride`], which is the only thing that reads it.
    bending: f64,
}

/// The six places the opening runs between.
///
/// **Two per blend and two per face**, which is the same six read either way: a
/// blend stops on a cross section whose two ends stand on the two faces it
/// divides, and a face carries the two ends of the two blends that reach it.
/// `made[i]` is where `blends[i]` stops, its end on `faces[i - 1]` first and
/// its end on `faces[i]` second — so a face's own pair is `made[i][1]` and
/// `made[i + 1][0]`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Opened {
    pub(crate) made: [[DVec3; 2]; 3],
    /// How far every one of the six stands from the corner.
    ///
    /// One number and not six — see [`Vertexed::opened`], which refuses a
    /// corner whose blends do not agree about it.
    pub(crate) reach: f64,
}

/// One side of the opening: the circle it lies on, and the stretch of that
/// circle it takes.
///
/// **Six sides and one shape between them.** A blend stops on a plane section
/// of its own cylinder and a face carries an arc of the sphere about the
/// corner, and a circle is what each of those is — so nothing here has to know
/// which of the two it came from.
///
/// Not ascending: which end is the greater is which way the circle was framed,
/// and a reader wanting the sweep takes the difference.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Side {
    pub(crate) circle: Circle,
    pub(crate) bounds: [f64; 2],
}

/// The stretch of `circle` from `from` round to `to`, taken the way that holds
/// `through`.
///
/// **The way round is never the near one by default.** A spring on a face the
/// corner turns past a half over runs the long way and a spring on any other
/// runs the short way, so what picks between them is a place the arc has to
/// hold rather than a rule about which is smaller.
fn arced(circle: Circle, from: DVec3, to: DVec3, through: DVec3) -> Side {
    let start = circle.axis.angle_of(from);
    let sweep = (circle.axis.angle_of(to) - start).rem_euclid(TAU);
    let held = (circle.axis.angle_of(through) - start).rem_euclid(TAU);
    let taken = match held <= sweep {
        true => sweep,
        false => sweep - TAU,
    };
    Side {
        circle,
        bounds: [start, start + taken],
    }
}

/// What the patch's height is held to where it meets one of its own sides.
///
/// **A value and a whole gradient, not a value and a slope across.** A height
/// field's normal is `(−h_x, −h_y, 1)` in its own plane's frame, so a normal
/// prescribed along a side fixes *both* readings of the gradient there — the
/// one across the side and the one along it. What is left for the middle of the
/// patch is an interpolation and not a guess.
#[derive(Debug, Clone, Copy)]
struct Heighted {
    /// How far the place stands off the plane.
    height: f64,
    /// How fast that changes, in the plane's own two directions.
    slope: DVec2,
}

impl Vertexed {
    /// The surface a corner leaves, or `None` where it leaves none a body
    /// holds.
    ///
    /// **The only way one is made**, because a surface here carries how hard it
    /// bends — see [`Vertexed::bending`] — and that is not a number a caller
    /// could hand over.
    pub(crate) fn new(
        axes: [Line; 3],
        facing: [DVec3; 3],
        at: DVec3,
        reach: f64,
        setback: f64,
    ) -> Option<Self> {
        // The frame the walk runs over does not read the bending, so the nought
        // stands only until the walk has answered.
        let mut held = Self {
            axes,
            facing,
            at,
            reach,
            setback,
            bending: 0.0,
        };
        held.bending = held.patched()?.walked();
        Some(held)
    }

    /// Where the blend at `which` stops on the face it shares with the blend
    /// after it, or before it where `ahead` is false.
    ///
    /// **Read off the rail rather than off the edge.** A blend is tangent to
    /// each face it divides along one line, and that line is its own axis
    /// dropped onto the face — the two standing a reach apart is what tangency
    /// is. The corner's foot on that line is where the blend would run to, and
    /// the setback carries it back along the way the axis points.
    fn stopped(&self, which: usize, ahead: bool) -> DVec3 {
        let axis = self.axes[which];
        let normal = self.facing[match ahead {
            true => which,
            false => (which + 2) % 3,
        }];
        let rail = axis.origin - normal * (axis.origin - self.at).dot(normal);
        rail + axis.direction * ((self.at - rail).dot(axis.direction) + self.setback)
    }

    /// The six places the opening runs between, or `None` where the corner is
    /// not one this spans.
    ///
    /// **All six stand at one distance or the corner is refused.** A rail
    /// stands `d` off the edge along its own face, so a stopped place stands
    /// `√(t² + d²)` from the corner — and where the three blends share a `d`
    /// that is one number for the six, which is what lets each spring be an arc
    /// of the one sphere about the corner. Three faces meeting square share it;
    /// three at arbitrary angles do not, and those want a rule that interpolates
    /// rather than one sphere. See `.notes/VERTEX-BLENDS.md` §5.
    ///
    /// **A setback under the reach is refused too.** At `t = d` the two places
    /// on a face fall together and the spring between them is nothing, which is
    /// the corner as it stands without a setback at all.
    pub(crate) fn opened(&self) -> Option<Opened> {
        let mut made = [[DVec3::ZERO; 2]; 3];
        for (which, pair) in made.iter_mut().enumerate() {
            for (side, ahead) in [false, true].into_iter().enumerate() {
                pair[side] = self.stopped(which, ahead);
            }
        }
        let reach = self.at.distance(made[0][0]);
        for stop in made.into_iter().flatten() {
            if !predicate::touching((self.at.distance(stop) - reach).abs(), PLACED) {
                return None;
            }
        }
        // The two places on a face fall together at a setback of the rail's own
        // offset, and a spring between one place and itself spans nothing.
        if predicate::touching(made[0][1].distance(made[1][0]), PLACED) {
            return None;
        }
        Some(Opened { made, reach })
    }

    /// The cross section the blend at `which` stops on, from its end on the
    /// face before it round to its end on the face after.
    ///
    /// **Square to its own axis**, which is what makes it a circle of the
    /// blend's own reach rather than a section of some other plane: the setback
    /// is measured along the edge, so what it cuts is the section the edge's
    /// own direction is normal to.
    ///
    /// The way round is the one holding the place the blend faces the edge
    /// from, which is the quarter of the cylinder the blend actually raises.
    pub(crate) fn crossed(&self, opened: &Opened, which: usize) -> Side {
        let axis = self.axes[which];
        let ends = opened.made[which];
        let middle = axis.at((ends[0] - axis.origin).dot(axis.direction));
        let circle = Circle {
            axis: Axis::new(middle, axis.direction, (ends[0] - middle).normalize()),
            radius: self.reach,
        };
        let toward = self.at - axis.origin;
        arced(
            circle,
            ends[0],
            ends[1],
            middle + toward - axis.direction * toward.dot(axis.direction),
        )
    }

    /// The key several of these are filed under — see
    /// [`Natural::key`](super::natural::Natural), which is where the argument
    /// for it is.
    ///
    /// The word carries on from the naturals' four, the torus's fifth and the
    /// ruled patch's sixth, so no two surfaces of the whole set collide on it.
    pub(crate) fn key(&self) -> u64 {
        let mut key = Key::default()
            .word(6)
            .place(self.at)
            .float(self.reach)
            .float(self.setback);
        for (axis, facing) in self.axes.into_iter().zip(self.facing) {
            key = key.place(axis.origin).place(axis.direction).place(facing);
        }
        key.done()
    }

    /// The plane the patch is a height over.
    ///
    /// **Its normal is the three faces' own, added up.** A patch is a graph
    /// over a plane exactly where its normal never turns square to that plane's
    /// — and the patch is tangent to a face along each spring and to a blend
    /// along each cross section, so what its normal does over the whole
    /// boundary is swing between those three faces. Their sum is the direction
    /// that swing leans about. Held on the notch's step corner, where the whole
    /// boundary reads `0.577` against it at worst, and where a search over four
    /// hundred thousand directions finds none better.
    ///
    /// **Which is what buys the contract back.** A place inverts by flattening
    /// on to this plane, so `uv` is closed form after all; the domain is that
    /// plane's own two, so no hexagon is wanted and no split into quads; and
    /// there are no seams, so no curve with a tangent at both ends and no
    /// vertex enclosure. The face is the hexagonal *region* of the domain,
    /// trimmed as every face here is.
    ///
    /// `None` where the three faces face away from one another and add to
    /// nothing, which is no corner a patch spans either.
    fn flattening(&self) -> Option<Plane> {
        let leaning: DVec3 = self.facing.into_iter().sum();
        Some(Axis::about(self.at, leaning.try_normalize()?).plane())
    }

    /// Whether the material at the blend at `which` lies *outside* its own
    /// cylinder, which is what a blend filled into a concave edge does and one
    /// cut into a convex edge does not.
    ///
    /// **Read off which side of a face the axis stands.** A blend stands its
    /// axis a reach off each face it divides — on the material's own side where
    /// it was cut into the material, and on the far side where it was filled
    /// into the void.
    fn filled(&self, which: usize) -> bool {
        (self.axes[which].origin - self.at).dot(self.facing[which]) > 0.0
    }

    /// The patch itself, or `None` where the corner leaves none to read.
    pub(crate) fn patched(&self) -> Option<Patched> {
        let opened = self.opened()?;
        let over = self.flattening()?;
        let middle = over.flatten(self.middle(&opened)?);
        let spread = opened
            .made
            .into_iter()
            .flatten()
            .map(|made| over.flatten(made).distance(middle))
            .fold(0.0_f64, f64::max);
        // Alternating a blend and the face after it, which is the order the six
        // places come out in — see [`Opened::made`].
        let sides = std::array::from_fn(|at| {
            let which = at / 2;
            match at % 2 {
                0 => Sided::new(
                    over,
                    self.crossed(&opened, which),
                    Along::Blend {
                        filled: self.filled(which),
                    },
                    spread,
                ),
                _ => Sided::new(
                    over,
                    self.sprung(&opened, which),
                    Along::Face(self.facing[which]),
                    spread,
                ),
            }
        });
        Some(Patched {
            over,
            up: over.normal(),
            sides,
            at: self.at,
            reach: opened.reach,
            middle,
            bending: self.bending,
        })
    }

    /// The middle of the opening, which the patch stands out toward.
    ///
    /// **On the same sphere as its six corners**, so that every one of the nine
    /// sides and seams is an arc of one thing: the six places all stand a reach
    /// from the corner — see [`Vertexed::opened`] — and the middle is put out
    /// the way they lean, at that same distance.
    ///
    /// `None` where they lean nowhere between them, which is a corner with no
    /// middle to lean to and no patch to span.
    fn middle(&self, opened: &Opened) -> Option<DVec3> {
        let leaning: DVec3 = opened
            .made
            .into_iter()
            .flatten()
            .map(|at| at - self.at)
            .sum();
        let way = leaning.try_normalize()?;
        Some(self.at + way * opened.reach)
    }

    /// The spring the face at `which` carries, from the end of the blend before
    /// it round to the end of the blend after.
    ///
    /// **An arc of the sphere about the corner**, which every one of the six
    /// places stands on — see [`Vertexed::opened`], which refuses a corner
    /// where they do not.
    ///
    /// **The way round is which side the two blends stand on.** Both stand a
    /// reach off the face they share; a pair that disagrees stands them on
    /// opposite sides of it and leaves a corner the face turns a quarter over,
    /// and a pair that agrees stands them on one side and leaves one it turns
    /// three quarters over. So the second wants the long way round, and a
    /// straight run between the two ends would leave the face altogether.
    pub(crate) fn sprung(&self, opened: &Opened, which: usize) -> Side {
        let after = (which + 1) % 3;
        let (from, to) = (opened.made[which][1], opened.made[after][0]);
        let normal = self.facing[which];
        let circle = Circle {
            axis: Axis::new(self.at, normal, (from - self.at).normalize()),
            radius: opened.reach,
        };
        let sides = [which, after].map(|at| (self.axes[at].origin - self.at).dot(normal).signum());
        let ways = [self.axes[which].direction, self.axes[after].direction];
        let toward = match sides[0] == sides[1] {
            true => -(ways[0] + ways[1]),
            false => ways[0] + ways[1],
        };
        arced(circle, from, to, self.at + toward)
    }
}

/// Which way the patch faces along one of its own sides.
#[derive(Debug, Clone, Copy)]
enum Along {
    /// A cross section: out of the blend's own cylinder, or into it where the
    /// blend was filled into the void rather than cut into the material.
    ///
    /// **The cylinder's axis is not carried.** A cross section is centred on
    /// that axis, so the way out of the cylinder at a place of the section is
    /// the section's own radial there.
    Blend { filled: bool },
    /// A spring: the way its face does, which is one direction for the whole
    /// of it.
    Face(DVec3),
}

/// Where one side is read at, where it is weighed from, and how far past
/// itself the reading was taken.
///
/// **Two places and not one.** A side's own height and slope are read where the
/// query's bearing puts it on that side's *circle*, which moves smoothly and
/// runs on past the side's own ends; how far the side stands is read from the
/// place held to its stretch, so the run past those ends weighs less. Read at
/// one place alone, the patch is either kinked at every end or snapped to
/// wherever a side's circle happens to run through the middle.
///
/// **And `past` is what stops the holding from kinking.** A place the bearing
/// leaves a side's stretch by half a turn is held to whichever end is nearer,
/// and which end that is turns over on the corner's own mirror — so the held
/// place is as far either way and leans the other, which is a ridge in the
/// patch. See [`Sided::taper`], which weighs such a reading out before the
/// turnover can reach it.
///
/// **And `near` is what stops the bearing itself from spinning.** A bearing is
/// taken about the flattened circle's own middle, and at that middle every
/// place of the circle is as near as every other — so the reading spins there,
/// and beside it the patch bends by thousands. The three springs are arcs of
/// one sphere about the corner, and the corner falls on the opening's own rim,
/// so this is a place the mesh reads and not a corner case. See
/// [`Sided::hushed`].
#[derive(Debug, Clone, Copy)]
struct Footed {
    /// Where the side is read: its height, its facing and its own place.
    at: DVec3,
    facing: DVec3,
    /// That place in the patch's own plane.
    flat: DVec2,
    /// The place the side is weighed from, in that plane.
    held: DVec2,
    past: f64,
    near: f64,
}

/// A side's circle as the patch's own plane sees it: an ellipse about
/// `middle`, reached by `middle + across·cos θ + up·sin θ`.
#[derive(Debug, Clone, Copy)]
struct Flattened {
    middle: DVec2,
    across: DVec2,
    up: DVec2,
}

impl Flattened {
    /// How a `circle` lies in the plane `over`.
    fn new(over: Plane, circle: Circle) -> Self {
        let axis = circle.axis;
        let middle = over.flatten(axis.origin);
        Self {
            middle,
            across: over.flatten(axis.origin + axis.reference * circle.radius) - middle,
            up: over.flatten(axis.origin + axis.quarter() * circle.radius) - middle,
        }
    }
}

/// One side of the opening as the patch reads it.
#[derive(Debug, Clone, Copy)]
struct Sided {
    circle: Circle,
    /// The circle's own quarter turn, worked out once rather than crossed out
    /// of its frame at every reading.
    quarter: DVec3,
    /// The stretch of that circle the side actually takes.
    ///
    /// **Weighed from, and not merely recorded.** A circle runs on past its
    /// own side and its flattened image runs on through the middle of the
    /// opening — so a place inside could stand *on* the continuation of a side
    /// it is nowhere near, and the blend would snap to that side's own reading
    /// there. The place a side is weighed from is held to this stretch, so a
    /// bearing past it stands as far off as the end itself does.
    bounds: [f64; 2],
    /// How far past either end of `bounds` this side still speaks — see
    /// [`Sided::taper`].
    ///
    /// **A quarter of the way to the turnover.** A place whose bearing leaves
    /// an arc is held to whichever end is nearer, and half a turn from the
    /// arc's middle the nearer end changes over — so the held place is as far
    /// either way and leans the other, and the patch has a ridge along that
    /// whole locus. At four margins the weight is a sixty-five-thousandth, so
    /// the ridge is there and weighs nothing.
    ///
    /// **And no wider than the opening**, so that a side never outvotes the one
    /// a place actually stands against.
    margin: f64,
    /// The side's own circle as the patch's plane sees it, worked out once
    /// rather than at every reading.
    flat: Flattened,
    along: Along,
}

impl Sided {
    /// One side of the opening, as seen from the plane `over`, in an opening
    /// whose corners stand `spread` from its middle at the furthest.
    fn new(over: Plane, side: Side, along: Along, spread: f64) -> Self {
        let [from, to] = side.bounds;
        let turn = (PI - (to - from).abs() / 2.0).max(0.0) * side.circle.radius;
        Self {
            circle: side.circle,
            quarter: side.circle.axis.quarter(),
            bounds: side.bounds,
            margin: spread.min(turn / 4.0),
            flat: Flattened::new(over, side.circle),
            along,
        }
    }

    /// The place on this side the query at `uv` reads it through, in the
    /// patch's own plane.
    ///
    /// **The angle about its own middle, and no search at all.** Flattening is
    /// affine, so a circle flattens to `A + B·cos θ + C·sin θ` — and a place of
    /// that ellipse has coordinates `(cos θ, sin θ)` in the basis `B` and `C`.
    /// So reading `uv` in that basis and taking its bearing hands back the
    /// side's own parameter where `uv` stands on the side, and a smooth
    /// reading everywhere else.
    ///
    /// **Which is what the blend needs of it and the nearest place is not.**
    /// The nearest place on a curve *jumps* where two of them are equally near,
    /// and a footing that jumps is a height that jumps — a surface no cell size
    /// meshes. This one turns nowhere.
    ///
    /// `None` where the ellipse has collapsed to a segment, the side's own
    /// plane standing square to the patch's, which no side of an opening does.
    fn footing(&self, uv: DVec2) -> Option<Footed> {
        let Flattened { middle, across, up } = self.flat;
        let spread = across.perp_dot(up);
        if spread == 0.0 {
            return None;
        }
        let out = uv - middle;
        let near = out.length_squared() / spread.abs();
        let angle = (across.perp_dot(out) / spread).atan2(out.perp_dot(up) / spread);
        let [from, to] = self.bounds;
        let angle = branch::nearest(angle, (from + to) / 2.0);
        let stood = angle.clamp(from.min(to), from.max(to));
        let (sin, cos) = angle.sin_cos();
        let radial = self.radial(angle);
        let flat = middle + across * cos + up * sin;
        Some(Footed {
            at: self.circle.axis.origin + radial * self.circle.radius,
            facing: self.facing(radial),
            flat,
            held: match stood == angle {
                true => flat,
                false => self.turned(stood),
            },
            past: (angle - stood).abs() * self.circle.radius,
            near,
        })
    }

    /// How much a reading taken `past` a side's own end still counts.
    ///
    /// **A side speaks about the places it bounds and about their
    /// neighbourhood.** The weight is whole over the side itself and falls away
    /// over its own [`margin`](Sided::margin) beyond either end.
    ///
    /// **It falls away and never reaches nought.** A weight that came to
    /// nothing would leave the places past every side with no reading at all,
    /// and those are the places a mesh reads when it lays its grid over the box
    /// the opening bounds. It falls as the eighth power, so the turnover the
    /// margin is measured against carries a sixty-five-thousandth of a side's
    /// own say — under what a place is written down to — while the fall stays
    /// gentle enough that a far side is never simply the one that wins.
    fn taper(&self, past: f64) -> f64 {
        if self.margin <= 0.0 {
            return 0.0;
        }
        let share = past / self.margin;
        let squared = share * share;
        1.0 / (1.0 + squared * squared * squared * squared)
    }

    /// Where the side stands, a `share` of the way along its own stretch.
    fn along(&self, share: f64) -> DVec2 {
        let [from, to] = self.bounds;
        self.turned(from + (to - from) * share)
    }

    /// Where the side's flattened circle stands at the turn `angle`.
    fn turned(&self, angle: f64) -> DVec2 {
        self.flat.middle + self.flat.across * angle.cos() + self.flat.up * angle.sin()
    }

    /// The places the side's flattened arc reaches furthest at, in each of the
    /// plane's two directions.
    ///
    /// **Solved and not walked.** A flattened circle is
    /// `m + a·cos θ + u·sin θ`, so one of its coordinates is furthest where
    /// `tan θ = u/a` in that coordinate, and twice round the circle. The two
    /// ends stand in for any of those four the stretch does not reach, so the
    /// six places always bound the arc and never more than it.
    fn extremes(&self) -> [DVec2; 6] {
        let [from, to] = self.bounds;
        let (low, high) = (from.min(to), from.max(to));
        let ends = [self.along(0.0), self.along(1.0)];
        let flat = self.flat;
        let mut held = [ends[0], ends[1], ends[0], ends[0], ends[1], ends[1]];
        for (which, (one, two)) in [(flat.across.x, flat.up.x), (flat.across.y, flat.up.y)]
            .into_iter()
            .enumerate()
        {
            for (turn, at) in [0.0, PI].into_iter().zip([2, 4]) {
                let angle = branch::nearest(two.atan2(one) + turn, (low + high) / 2.0);
                if angle >= low && angle <= high {
                    held[at + which] = self.turned(angle);
                }
            }
        }
        held
    }

    /// How much a side says at a place `near` its own flattened middle.
    ///
    /// **Nothing at the middle itself, and everything a third of the way out.**
    /// `near` is the place's distance from that middle over the circle's own
    /// mean radius, squared — so the hush is a ratio of polynomials in the
    /// place, and smooth. Its zero is of the fourth order, which is what leaves
    /// the product of the hush and the spinning reading flat there.
    fn hushed(near: f64) -> f64 {
        let squared = near * near;
        squared / (HUSHED + squared)
    }

    /// The way out of the side's own circle at the turn `angle`, which is
    /// unit by how it is made.
    fn radial(&self, angle: f64) -> DVec3 {
        let (sin, cos) = angle.sin_cos();
        self.circle.axis.reference * cos + self.quarter * sin
    }

    /// Which way the patch faces where the side's circle runs out `radial`.
    fn facing(&self, radial: DVec3) -> DVec3 {
        match self.along {
            Along::Face(normal) => normal,
            Along::Blend { filled: true } => -radial,
            Along::Blend { filled: false } => radial,
        }
    }
}

/// The patch itself: a height over one plane, blended from what its six sides
/// hold it to.
///
/// **Worked out once for a call that wants several places**, which is
/// [`Gusset`](super::gusset::Gusset)'s own rule: a walk of the patch reads
/// hundreds of places off the same six sides and the same plane, and finding
/// them again per place would be the whole cost.
///
/// **Blended rather than fitted.** Each side holds the height and its whole
/// gradient — see [`Heighted`] — so what a place inside takes is those six
/// readings carried to it and weighed by how near each side stands. A place
/// *on* a side reads that side's own numbers back, the weight of the rest
/// falling away as the square of the distance.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Patched {
    over: Plane,
    /// That plane's own normal, crossed and normalised once rather than at
    /// every side of every reading.
    up: DVec3,
    sides: [Sided; 6],
    /// The corner the three blends swallow, which is what the patch's own
    /// extent is measured about.
    at: DVec3,
    /// How far the opening stands from that corner, which is the only length
    /// the patch has to step by — see [`Patched::normal`].
    reach: f64,
    /// The middle of the opening in those parameters, which the walk of the
    /// patch's own curvature fans from — see [`Patched::walked`].
    middle: DVec2,
    /// How hard the patch bends, carried from the surface — see
    /// [`Patched::walked`], which is what worked it out.
    bending: f64,
}

impl Patched {
    /// What the height is held to at `at`, where the patch faces `facing`.
    ///
    /// `None` where the patch turns square to its own plane there, which is a
    /// corner no height over that plane spans — see [`Vertexed::flattening`],
    /// which is what keeps the rest of them clear of it.
    fn heighted(&self, at: DVec3, facing: DVec3) -> Option<Heighted> {
        let along = facing.dot(self.up);
        (along > 0.0).then(|| Heighted {
            height: (at - self.over.origin).dot(self.up),
            slope: -DVec2::new(facing.dot(self.over.x), facing.dot(self.over.y)) / along,
        })
    }

    /// How far the patch stands off its own plane at the domain place `uv`.
    fn height(&self, uv: DVec2) -> f64 {
        let (mut top, mut sum) = (0.0_f64, 0.0_f64);
        for side in self.sides {
            let Some(footed) = side.footing(uv) else {
                continue;
            };
            let Some(held) = self.heighted(footed.at, footed.facing) else {
                continue;
            };
            let local = held.height + held.slope.dot(uv - footed.flat);
            let apart = (uv - footed.held).length_squared();
            if apart <= f64::EPSILON {
                return local;
            }
            let weighed = side.taper(footed.past) * Sided::hushed(footed.near) / apart;
            top += weighed * local;
            sum += weighed;
        }
        debug_assert!(sum > 0.0, "no side of the opening speaks at {uv}");
        top / sum
    }

    /// Where the parameters `uv` land.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        self.over.point(uv) + self.up * self.height(uv)
    }

    /// Which parameters `at` stands at.
    ///
    /// **Flattened, and that is the whole of it.** The patch is a height over
    /// one plane, so inverting a place is dropping it on to that plane — no
    /// solve, where §4.7 had budgeted a Newton one for a free-form surface.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        self.over.flatten(at)
    }

    /// Which way the patch faces at `uv`.
    ///
    /// **Read off the height rather than written down.** A height field faces
    /// `(−h_x, −h_y, 1)` in its own plane's frame, and the blend's gradient is
    /// a reading: the weights move with the place and so do the six footings
    /// they weigh. Stepped either way rather than solved, so the reading is
    /// always the gradient of the height this patch actually has — which is
    /// what the checking holds a face to.
    ///
    /// **The step is a millionth of the reach**, which is where a central
    /// difference of an `f64` stands nearest the truth: the rounding falls as
    /// the step shrinks and the truncation rises as its square.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let step = self.reach * 1e-6;
        let slope = DVec2::new(
            (self.height(uv + DVec2::X * step) - self.height(uv - DVec2::X * step)) / (2.0 * step),
            (self.height(uv + DVec2::Y * step) - self.height(uv - DVec2::Y * step)) / (2.0 * step),
        );
        (self.up - self.over.x * slope.x - self.over.y * slope.y).normalize()
    }

    /// How far `at` stands from the patch, never signed.
    ///
    /// **The height's own reading, turned by how far the patch leans.** A place
    /// stands off a graph by the gap in height divided by the stretch the lean
    /// puts on it, which is what the normal's own reading against the plane is
    /// — exact where the patch is flat and first order elsewhere.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        let uv = self.uv(at);
        let gap = (at - self.over.origin).dot(self.up) - self.height(uv);
        gap.abs() * self.normal(uv).dot(self.up)
    }

    /// The place of the patch under `at`, read down its own plane's normal.
    ///
    /// **The reading and not the nearest**, which for a graph is the same to
    /// first order and never further than the lean: a place off the patch drops
    /// on to one parameter place and no other, [`Patched::uv`] being a
    /// flattening.
    pub(crate) fn nearest(&self, at: DVec3) -> DVec3 {
        self.at(self.uv(at))
    }

    /// The box the patch fills.
    ///
    /// **Walked, the blend writing no bound down.** A grid over the opening's
    /// own stretch of the domain is read and the places gathered, which is what
    /// every other reading of this tier does — see
    /// [`Gusset::fills`](super::gusset::Gusset).
    pub(crate) fn fills(&self) -> Bounds<DVec3> {
        let mut filled = Bounds::default();
        let laid = self.laid();
        let span = laid.high - laid.low;
        for down in 0..=BOXED {
            for across in 0..=BOXED {
                let share = DVec2::new(
                    f64::from(across) / f64::from(BOXED),
                    f64::from(down) / f64::from(BOXED),
                );
                filled.hold(self.at(laid.low + span * share));
            }
        }
        filled
    }

    /// Whether the patch gets into the box `fills`, allowing `slack`.
    pub(crate) fn spans(&self, fills: Bounds<DVec3>, slack: f64) -> bool {
        self.fills().meets(fills, slack)
    }

    /// How far a normal read back at `at` may turn from the patch's own, as a
    /// sine, where `at` may stand as much as `off` from the patch.
    ///
    /// **Read rather than derived**, the blend writing no curvature down. A
    /// place `off` the patch names a parameter place `off` away along either of
    /// the plane's own two — the inversion being a flattening — so the room is
    /// a square of that width, and what is read is how far the normal turns
    /// over its corners.
    pub(crate) fn wavering(&self, at: DVec3, off: f64) -> f64 {
        debug_assert!(off >= 0.0, "a distance is a magnitude, not {off}");
        let uv = self.uv(at);
        let here = self.normal(uv);
        let mut most = 0.0_f64;
        for across in [-off, off] {
            for down in [-off, off] {
                most = most.max(here.distance(self.normal(uv + DVec2::new(across, down))));
            }
        }
        most
    }

    /// How far the flat triangle on the parameters `corners` strays from the
    /// patch at its furthest.
    ///
    /// **Probed, as the blend leaves nothing to derive.** The triangle's own
    /// plane is read against a grid of the patch over it, which is the reading
    /// §7.7 already spends on one side of a ruled patch — here it is the whole
    /// of the answer rather than one term of it.
    pub(crate) fn straying(&self, corners: [DVec2; 3]) -> f64 {
        let made = corners.map(|uv| self.at(uv));
        let normal = (made[1] - made[0]).cross(made[2] - made[0]);
        let Some(normal) = normal.try_normalize() else {
            return 0.0;
        };
        let mut most = 0.0_f64;
        for down in 0..=PROBED {
            for across in 0..=(PROBED - down) {
                let (one, two) = (
                    f64::from(across) / f64::from(PROBED),
                    f64::from(down) / f64::from(PROBED),
                );
                let uv =
                    corners[0] + (corners[1] - corners[0]) * one + (corners[2] - corners[0]) * two;
                most = most.max((self.at(uv) - made[0]).dot(normal).abs());
            }
        }
        most
    }

    /// How far apart the parameter lines of a face's grid must stand, to leave
    /// no triangle straying further than `sagitta`.
    ///
    /// **Both parameters alike**, the patch having no direction it bends less
    /// in: its domain is a plane's own two and neither is an angle.
    ///
    /// **Worked out of the curvature and not searched for.** A quadratic stands
    /// off the plane through a triangle's three corners by at most an eighth of
    /// its curvature times the longest side squared, and a cell's longest side
    /// is its diagonal — so `κ·stride²/4` is the straying, and the stride that
    /// holds it to the sagitta is `√(4·sagitta/κ)`. A search over cells reads
    /// what the grid would do at one stride and says nothing about the next,
    /// and it costs a walk of the whole opening at every halving.
    pub(crate) fn strides(&self, sagitta: f64) -> DVec2 {
        DVec2::splat(self.stride(sagitta))
    }

    /// How far apart those parameter lines stand, both being alike.
    fn stride(&self, sagitta: f64) -> f64 {
        debug_assert!(sagitta > 0.0, "a sagitta of {sagitta} cuts nothing");
        match self.bending > 0.0 {
            true => (4.0 * sagitta / self.bending).sqrt(),
            false => {
                let laid = self.laid();
                laid.high.distance(laid.low)
            }
        }
    }

    /// The stretch of its own parameters the opening fills, solved off the six
    /// arcs — see [`Sided::extremes`].
    ///
    /// **Asked for rather than carried.** Only the laying out of a grid wants
    /// it, and a reading of one place is what the mesher asks for a thousand
    /// times over — so the six solves stay out of that path.
    fn laid(&self) -> Bounds<DVec2> {
        self.sides.iter().flat_map(Sided::extremes).collect()
    }

    /// How hard the patch bends, walked over the whole of the opening.
    ///
    /// **Run once, where the surface is made** — see [`Vertexed::new`]. It is
    /// what every stride comes off and it costs some thousands of readings of
    /// the height, so a mesher asking twice would pay it twice.
    ///
    /// **The larger of the height's two principal second derivatives**, read
    /// off a middle difference in each parameter and across the two, and taken
    /// as the largest of what the walk below finds.
    ///
    /// **Walked over the rim and in to the middle.** The patch bends hardest
    /// against its own boundary, so the walk runs the six sides themselves and
    /// draws in from each place it reaches — and a walk that stopped at the
    /// chords of those sides would miss exactly that, an arc standing off its
    /// own chord.
    fn walked(&self) -> f64 {
        let step = self.reach * BENT;
        let mut most = 0.0_f64;
        for side in &self.sides {
            for around in 0..RIMMED {
                let out = side.along(f64::from(around) / f64::from(RIMMED)) - self.middle;
                for along in 0..=STEPPED {
                    let inward = f64::from(along) / f64::from(STEPPED);
                    let share = 1.0 - inward * inward;
                    most = most.max(self.bent(self.middle + out * share, step));
                }
            }
        }
        most
    }

    /// How hard the patch bends at `uv`, read off differences a `step` wide.
    ///
    /// **Six readings and not nine.** The two along the parameters are middle
    /// differences, and the one across them is a corner difference off the same
    /// three the others already took.
    fn bent(&self, uv: DVec2, step: f64) -> f64 {
        let (x, y) = (DVec2::X * step, DVec2::Y * step);
        let middle = self.height(uv);
        let (ahead, behind) = (self.height(uv + x), self.height(uv - x));
        let (above, below) = (self.height(uv + y), self.height(uv - y));
        let square = step * step;
        let along = (ahead - 2.0 * middle + behind) / square;
        let across = (above - 2.0 * middle + below) / square;
        let between = (self.height(uv + x + y) - ahead - above + middle) / square;
        let half = (along - across) / 2.0;
        ((along + across) / 2.0).abs() + half.hypot(between)
    }

    /// The patch laid out as a grid of places, and how many stand along each
    /// of its two parameters.
    ///
    /// **Square, both parameters bending alike** — see [`Patched::strides`],
    /// which is what the count comes off. Written row by row, the first
    /// parameter running fastest.
    pub(crate) fn netted(&self, sagitta: f64, into: &mut Vec<DVec3>) -> usize {
        debug_assert!(sagitta > 0.0, "a sagitta of {sagitta} lays out nothing");
        let laid = self.laid();
        let span = laid.high - laid.low;
        let steps = ((span / self.stride(sagitta)).max_element().ceil().max(1.0) as usize) + 1;
        for down in 0..steps {
            for across in 0..steps {
                let share = DVec2::new(across as f64, down as f64) / (steps - 1) as f64;
                into.push(self.at(laid.low + span * share));
            }
        }
        steps
    }

    /// Where a ray from `from` running `way` crosses the patch.
    ///
    /// **Counted rather than solved**, which is the bargain §4.7 strikes for a
    /// surface no closed form answers: the gap between the ray's own height and
    /// the patch's changes sign at every crossing, so a walk of the stretch the
    /// ray runs near the corner finds them by their signs and closes on each by
    /// halving. Nothing is guessed and nothing is missed that the walk is fine
    /// enough to bracket.
    ///
    /// **Bounded by the patch's own reach**, which every other surface here
    /// cannot be: a ray is walked only where it stands within two reaches of
    /// the corner, and outside that the blend answers about nothing.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Crossings {
        let gap = |along: f64| {
            let at = from + way * along;
            (at - self.over.origin).dot(self.up) - self.height(self.uv(at))
        };
        let out = from - self.at;
        let (half, apart) = (out.dot(way), out.length_squared());
        let under = half * half - apart + 4.0 * self.reach * self.reach;
        let mut found = Crossings::none();
        if under <= 0.0 {
            return found;
        }
        let (lo, hi) = (-half - under.sqrt(), -half + under.sqrt());
        let mut last = gap(lo);
        for step in 1..=WALKED {
            let along = lo + (hi - lo) * f64::from(step) / f64::from(WALKED);
            let here = gap(along);
            if last.is_sign_negative() != here.is_sign_negative()
                && let Some(at) = bisect::root(along - (hi - lo) / f64::from(WALKED), along, gap)
            {
                found.push(at);
            }
            last = here;
        }
        found.sorted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a square corner the notch leaves would be.
    const SPANS: &str = "a square corner spans a patch";
    use std::f64::consts::PI;

    /// The notch's step corner, in the frame `.notes/VERTEX-BLENDS.md` reads it
    /// in: `u` across the floor, `v` along the reflex edge and `w` up the wall,
    /// with the corner at nought.
    ///
    /// The fill runs the reflex edge, its axis a reach into the void the notch
    /// leaves; the two cuts run the edges the cap makes with the floor and the
    /// wall, each a reach into the material.
    fn notch(reach: f64, setback: f64) -> Option<Vertexed> {
        let axes = [
            Line {
                origin: DVec3::new(-reach, 0.0, -reach),
                direction: DVec3::Y,
            },
            Line {
                origin: DVec3::new(0.0, reach, reach),
                direction: DVec3::NEG_X,
            },
            Line {
                origin: DVec3::new(reach, reach, 0.0),
                direction: DVec3::NEG_Z,
            },
        ];
        // The floor is shared by the fill and the first cut, the cap by the two
        // cuts, and the wall by the second cut and the fill.
        // Each pointing out of the material: the notch holds `v >= 0`, and
        // `u >= 0` or `w >= 0`.
        let facing = [DVec3::NEG_Z, DVec3::NEG_Y, DVec3::NEG_X];
        Vertexed::new(axes, facing, DVec3::ZERO, reach, setback)
    }

    /// **The six places are hand-computed off the rails.** A rail stands one
    /// reach off its edge along each face, and the setback carries it that far
    /// again along the edge — so the fill's place on the floor is `(−r, t, 0)`
    /// and its place on the wall is `(0, t, −r)`, and the two cuts read the
    /// same one turn round apiece.
    ///
    /// **And all six stand `√(t² + r²)` from the corner**, which is what the
    /// springs are arcs of one sphere about. Held at three reaches and two
    /// setbacks, the six agreeing to the last bit.
    #[test]
    fn the_six_places_of_the_opening_stand_at_one_distance() {
        for (reach, setback) in [(0.5, 1.0), (0.5, 0.75), (1.0, 1.5), (0.25, 0.5)] {
            let (r, t) = (reach, setback);
            let opened = notch(r, t)
                .expect(SPANS)
                .opened()
                .expect("a square corner opens");
            // Each blend's end on the face before it first, and on the face
            // after it second — see [`Opened::made`].
            let want = [
                [DVec3::new(0.0, t, -r), DVec3::new(-r, t, 0.0)],
                [DVec3::new(-t, r, 0.0), DVec3::new(-t, 0.0, r)],
                [DVec3::new(r, 0.0, -t), DVec3::new(0.0, r, -t)],
            ];
            for (which, (got, pair)) in opened.made.iter().zip(&want).enumerate() {
                for (side, (got, want)) in got.iter().zip(pair).enumerate() {
                    assert!(
                        got.abs_diff_eq(*want, 1e-12),
                        "blend {which} side {side} stops at {got} where {want} is \
                         the rail",
                    );
                }
            }
            let want = (t * t + r * r).sqrt();
            assert!(
                (opened.reach - want).abs() < 1e-12,
                "the six stand {} from the corner where {want} is `√(t² + r²)`",
                opened.reach,
            );
        }
    }

    /// **Every side runs between the two places it is named by**, and lies on
    /// the shape it was cut from: a cross section a reach off its own blend's
    /// axis, a spring a reach off the corner and flat on its own face.
    #[test]
    fn the_six_sides_run_between_the_six_places() {
        let (r, t) = (0.5, 1.0);
        let corner = notch(r, t).expect(SPANS);
        let opened = corner.opened().expect("a square corner opens");
        for which in 0..3 {
            let side = corner.crossed(&opened, which);
            let ends = [side.bounds[0], side.bounds[1]].map(|at| side.circle.at(at));
            for (got, want) in ends.iter().zip(opened.made[which]) {
                assert!(
                    got.abs_diff_eq(want, 1e-12),
                    "{got} is not the place {want}"
                );
            }
            for step in 0..=8 {
                let along =
                    side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 8.0;
                let off = corner.axes[which].off(side.circle.at(along));
                assert!(
                    (off - r).abs() < 1e-12,
                    "the cross section stands {off} off"
                );
            }
        }
        for which in 0..3 {
            let side = corner.sprung(&opened, which);
            let ends = [side.bounds[0], side.bounds[1]].map(|at| side.circle.at(at));
            let want = [opened.made[which][1], opened.made[(which + 1) % 3][0]];
            for (got, want) in ends.iter().zip(want) {
                assert!(
                    got.abs_diff_eq(want, 1e-12),
                    "{got} is not the place {want}"
                );
            }
            let normal = corner.facing[which];
            for step in 0..=8 {
                let along =
                    side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 8.0;
                let at = side.circle.at(along);
                let flat = (at - corner.at).dot(normal).abs();
                assert!(flat < 1e-12, "the spring stands {flat} off its own face");
                assert!(
                    (corner.at.distance(at) - opened.reach).abs() < 1e-12,
                    "the spring stands {} off the corner",
                    corner.at.distance(at),
                );
            }
        }
    }

    /// **The spring on the face the two cuts share takes the long way round.**
    /// Both stand a reach off it on the one side, so the corner it keeps there
    /// turns three quarters and the near way would run out through the notch's
    /// own void. The other two faces have a blend either side and take the
    /// short way.
    #[test]
    fn the_spring_on_the_face_that_turns_past_a_half_runs_the_long_way() {
        let (r, t) = (0.5, 1.0);
        let corner = notch(r, t).expect(SPANS);
        let opened = corner.opened().expect("a square corner opens");
        let sweeps = [0, 1, 2].map(|which| {
            let side = corner.sprung(&opened, which);
            (side.bounds[1] - side.bounds[0]).abs()
        });
        assert!(sweeps[0] < PI, "the floor turns {} over", sweeps[0]);
        assert!(sweeps[2] < PI, "the wall turns {} over", sweeps[2]);
        assert!(sweeps[1] > PI, "the cap turns only {} over", sweeps[1]);

        // The cap is the notch's own L: everything but the quarter its void
        // takes, which is where a straight run between the two ends would go.
        let side = corner.sprung(&opened, 1);
        for step in 0..=32 {
            let along = side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 32.0;
            let at = side.circle.at(along);
            assert!(
                at.x >= -1e-12 || at.z >= -1e-12,
                "the spring stands at {at}, in the void the notch leaves",
            );
        }
    }

    /// **The middle leans the way the six corners do, hand-computed.** At a
    /// reach of a half and a setback of one the six stand at `(0, 1, −½)`,
    /// `(−½, 1, 0)`, `(−1, ½, 0)`, `(−1, 0, ½)`, `(½, 0, −1)` and `(0, ½, −1)`,
    /// which add to `(−2, 3, −2)` — so the middle stands `√(t² + r²)` out that
    /// way. It is its own mirror in `u` against `w`, as the corner is.
    ///
    /// **The seams that run in to it are not written**, and a sphere arc is not
    /// what they are: the patch's tangent plane at a corner of the opening is
    /// the face's own, so every curve leaving there lies in that face — and an
    /// arc of the sphere leaves almost straight through its normal. See
    /// `.notes/VERTEX-BLENDS.md` §5.
    #[test]
    fn the_middle_is_the_way_the_corners_lean() {
        let (r, t) = (0.5, 1.0);
        let corner = notch(r, t).expect(SPANS);
        let opened = corner.opened().expect("a square corner opens");
        let middle = corner.middle(&opened).expect("six corners lean somewhere");
        let want = DVec3::new(-2.0, 3.0, -2.0).normalize() * opened.reach;
        assert!(middle.abs_diff_eq(want, 1e-12), "{middle} is not {want}");
        assert!(
            (middle.x - middle.z).abs() < 1e-12,
            "{middle} is not its own mirror",
        );
    }

    /// **Every ray from the corner meets the opening once**: a spring stands the
    /// reach off the corner outright, and a cross section dips inside that and
    /// never to nothing.
    ///
    /// **Which is not enough to read the patch off the corner.** A graph about
    /// a place reads its own normal along the radial and never nought, and the
    /// radial at a spring lies *in* the face the spring is on — the corner
    /// being on that face too. So a graph about the corner is tangent to no
    /// face at all, and whatever the patch is read off stands away from it. See
    /// `.notes/VERTEX-BLENDS.md` §5. Held over the whole boundary at three reaches and two
    /// setbacks — and the notch's step corner reads `1.0212` at its nearest
    /// where the springs read `1.1180`.
    #[test]
    fn the_opening_stands_clear_of_the_corner_all_the_way_round() {
        for (r, t) in [(0.5, 1.0), (0.5, 0.75), (1.0, 1.5), (0.25, 0.5)] {
            let corner = notch(r, t).expect(SPANS);
            let opened = corner.opened().expect("a square corner opens");
            let mut least = f64::INFINITY;
            for which in 0..3 {
                for side in [
                    corner.crossed(&opened, which),
                    corner.sprung(&opened, which),
                ] {
                    for step in 0..=16 {
                        let along = side.bounds[0]
                            + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 16.0;
                        least = least.min(corner.at.distance(side.circle.at(along)));
                    }
                }
            }
            assert!(least > 0.0, "the opening runs through the corner itself");
            assert!(
                least <= opened.reach + 1e-12,
                "the opening stands {least} out where the springs stand {}",
                opened.reach,
            );
            if (r, t) == (0.5, 1.0) {
                assert!(
                    (least - 1.0212_f64).abs() < 1e-4,
                    "the notch's step corner reads {least} at its nearest",
                );
            }
        }
    }

    /// **The patch is a height over one plane, which is what the whole surface
    /// rests on.** It is a graph over a plane exactly where its own normal
    /// never turns square to that plane's — and the patch takes the blend's
    /// normal along each cross section and the face's along each spring, so
    /// what has to hold is that those six readings keep one sign.
    ///
    /// Held over the whole boundary at three reaches and two setbacks. On the
    /// notch's step corner the worst reading is `1/√3`, the three faces' own
    /// normals adding to the corner's own diagonal.
    #[test]
    fn the_patch_faces_one_way_all_the_way_round_its_boundary() {
        for (r, t) in [(0.5, 1.0), (0.5, 0.75), (1.0, 1.5), (0.25, 0.5)] {
            let patched = notch(r, t)
                .expect(SPANS)
                .patched()
                .expect("a square corner patches");
            let mut least = f64::INFINITY;
            for side in patched.sides {
                for step in 0..=16 {
                    let along =
                        side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 16.0;
                    least = least.min(side.facing(side.radial(along)).dot(patched.up));
                }
            }
            assert!(
                least > 0.0,
                "the patch turns square to its own plane, reading {least}",
            );
            assert!(
                (least - 1.0 / 3.0_f64.sqrt()).abs() < 1e-12,
                "the worst reading is {least} where a square corner gives `1/√3`",
            );
        }
    }

    /// **A normal read back off the height is the normal it was written
    /// from**, which is the whole of what the boundary data has to be: the
    /// height field's own normal is `(−h_x, −h_y, 1)` in the plane's frame, so
    /// the slope written down from a facing reads that facing back.
    ///
    /// Held over the whole boundary at three reaches and two setbacks, the
    /// round trip agreeing to the last bit.
    #[test]
    fn a_normal_read_back_off_the_slope_is_the_one_it_was_written_from() {
        for (r, t) in [(0.5, 1.0), (0.5, 0.75), (1.0, 1.5), (0.25, 0.5)] {
            let patched = notch(r, t)
                .expect(SPANS)
                .patched()
                .expect("a square corner patches");
            let over = patched.over;
            let mut asked = 0;
            for side in patched.sides {
                for step in 0..=16 {
                    let along =
                        side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 16.0;
                    let at = side.circle.at(along);
                    let facing = side.facing(side.radial(along));
                    let held = patched
                        .heighted(at, facing)
                        .expect("the patch stands clear of square");
                    let read =
                        (patched.up - over.x * held.slope.x - over.y * held.slope.y).normalize();
                    assert!(
                        read.abs_diff_eq(facing, 1e-12),
                        "the slope reads {read} back where {facing} was written",
                    );
                    assert!(
                        (held.height - (at - over.origin).dot(over.normal())).abs() < 1e-12,
                        "the height is not how far the place stands off",
                    );
                    asked += 1;
                }
            }
            assert_eq!(asked, 102, "the whole boundary was not read");
        }
    }

    /// **The patch meets its own boundary, in place and in facing.** A place on
    /// a side reads that side's own height back, the other five weighing
    /// nothing against it; and the normal read off the height there is the one
    /// the blend or the face holds it to, which is the tangency the whole
    /// corner exists for.
    #[test]
    fn the_patch_meets_every_side_it_was_written_from() {
        let patch = notch(0.5, 1.0)
            .expect(SPANS)
            .patched()
            .expect("a square corner is patched");
        let (mut off, mut turned) = (0.0_f64, 0.0_f64);
        for side in patch.sides {
            for step in 1..8 {
                let along =
                    side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 8.0;
                let at = side.circle.at(along);
                let uv = patch.uv(at);
                off = off.max(patch.at(uv).distance(at));
                turned = turned.max(patch.normal(uv).distance(side.facing(side.radial(along))));
            }
        }
        assert!(off < 1e-12, "the patch stands {off} off its own boundary");
        assert!(
            turned < 1e-6,
            "the patch turns {turned} from its own facing"
        );
    }

    /// **And it stays a graph inside**, which is what the whole surface rests
    /// on: a place read anywhere within the opening faces the same way its own
    /// plane does, so the domain names one place of the patch and not two.
    ///
    /// Walked from the middle out to each of the six corners, where the patch
    /// is at its most turned.
    #[test]
    fn the_patch_faces_its_own_plane_inside_the_opening_too() {
        let (r, t) = (0.5, 1.0);
        let corner = notch(r, t).expect(SPANS);
        let opened = corner.opened().expect("a square corner opens");
        let over = corner.flattening().expect("three faces that add up");
        let middle = over.flatten(corner.middle(&opened).expect("a middle"));
        let patch = corner.patched().expect("a square corner is patched");
        let mut least = f64::INFINITY;
        for made in opened.made.into_iter().flatten() {
            let toward = over.flatten(made);
            for step in 0..=8 {
                let uv = middle + (toward - middle) * f64::from(step) / 8.0;
                least = least.min(patch.normal(uv).dot(over.normal()));
                assert!(
                    patch.uv(patch.at(uv)).abs_diff_eq(uv, 1e-12),
                    "a place of the patch does not read its own parameters back",
                );
            }
        }
        assert!(
            least > 0.0,
            "the patch turns square to its own plane, at {least}"
        );
    }

    /// **A setback of the reach is the corner with no setback at all**, which
    /// is what the refusal is for: the two places on each face fall together
    /// there, the spring between them is nothing, and what is left is the three
    /// rail crossings the star already runs its legs from.
    #[test]
    fn a_setback_of_the_reach_leaves_no_spring_to_span() {
        let reach = 0.5;
        assert!(
            notch(reach, reach).is_none(),
            "a setback of the reach spans a patch, where its springs come to nothing",
        );
        assert!(
            notch(reach, reach + PLACED * 10.0).is_some(),
            "a setback past the reach leaves a spring and was refused",
        );
    }
}
