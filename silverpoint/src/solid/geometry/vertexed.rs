//! The patch a corner three picks do not agree about is filled with.
//!
//! **Set back, where every other corner in §7.5 is not.** Three blends that
//! agree leave a hole a sphere spans and three chamfers leave none at all; three
//! that do not agree leave one no surface already written reaches — see
//! `.notes/VERTEX-BLENDS.md`, where each family is ruled out in turn. So the
//! blends are stopped short of the corner and a patch spans what they leave.
//! Where each stops is the rounding's own business — see `Setback` in
//! `solid::rounding::setback`, which hands the six sides over.
//!
//! **A height over one plane, blended from what its six sides hold it to.** The
//! patch is tangent to each blend along a cross section and to each face along
//! a spring, and everything it is is worked out once, where it is made: a
//! reading of the height is what a mesher asks for a thousand times over, and
//! what the six sides come to is not something to find again for each.

use std::f64::consts::{PI, TAU};

use crate::math::bisect;
use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::math::plane::Plane;
use crate::solid::buckets::Key;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::surface::Crossings;
use glam::{DVec2, DVec3};

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
/// [`Vertexed::walked`].
const RIMMED: u32 = 48;

/// How many shells in from the rim the reading is taken over, drawn in toward
/// the corner by cosines — see [`Vertexed::walked`].
const STEPPED: u32 = 16;

/// What share of the reach a middle difference is taken over — see
/// [`Vertexed::bent`].
const BENT: f64 = 1e-3;

/// The fourth power of the radius at which a spring speaks with half its say —
/// see [`Sided::hushed`].
///
/// **A half, which puts that radius at `0.84` of the spring's own.** Measured on
/// the notch's step corner over a grid of the whole opening, at setbacks of one
/// and a half, two, three and six reaches, the patch bends hardest at `18.6`,
/// `15.8`, `12.5` and `18.4` over a reach of a half; a quarter reads `17.4`,
/// `20.2`, `15.9` and `15.3`, and one reads `20.3`, `17.2`, `15.8` and `24.6`.
/// The rounding sets back two reaches, where the half wins outright. A hush
/// that plateaued a third of the way out, which is what stood here first, read
/// `48` and `59` at two and three reaches: the patch had to bend from what the
/// three sections say at the corner to what all six say over a third of the
/// radius.
const HUSHED: f64 = 0.5;

/// One side of the opening: the circle it lies on, the stretch of that circle
/// it takes, and what the patch is held to along it.
///
/// **Six sides and one shape between them.** A blend stops on a plane section
/// of its own cylinder and a face carries an arc of the sphere about the
/// corner, and a circle is what each of those is — so nothing here has to know
/// which of the two it came from beyond which way the patch faces there.
///
/// Not ascending: which end is the greater is which way the circle was framed,
/// and a reader wanting the sweep takes the difference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Side {
    pub(crate) circle: Circle,
    pub(crate) bounds: [f64; 2],
    pub(crate) along: Along,
}

impl Side {
    /// The stretch of `circle` from `from` round to `to`, taken the way that
    /// holds `through`.
    ///
    /// **The way round is never the near one by default.** A spring on a face
    /// the corner turns past a half over runs the long way and a spring on any
    /// other runs the short way, so what picks between them is a place the arc
    /// has to hold rather than a rule about which is smaller.
    pub(crate) fn arced(
        circle: Circle,
        from: DVec3,
        to: DVec3,
        through: DVec3,
        along: Along,
    ) -> Self {
        let start = circle.axis.angle_of(from);
        let sweep = (circle.axis.angle_of(to) - start).rem_euclid(TAU);
        let held = (circle.axis.angle_of(through) - start).rem_euclid(TAU);
        let taken = match held <= sweep {
            true => sweep,
            false => sweep - TAU,
        };
        Self {
            circle,
            bounds: [start, start + taken],
            along,
        }
    }

    /// Where the side starts and where it ends.
    pub(crate) fn ends(&self) -> [DVec3; 2] {
        self.bounds.map(|at| self.circle.at(at))
    }
}

/// Which way the patch faces along one of its own sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Along {
    /// A cross section: out of the blend's own cylinder, or into it where the
    /// blend was filled into the void rather than cut into the material.
    ///
    /// **The cylinder's axis is not carried.** A cross section is centred on
    /// that axis, so the way out of the cylinder at a place of the section is
    /// the section's own radial there.
    Blend { filled: bool },
    /// A spring: the way its face does, which is the spring's own axis — the
    /// circle lies in the face, and the face's normal is what it was framed
    /// about.
    Face,
}

/// What the patch's height is held to where it meets one of its own sides.
///
/// **A value and a whole gradient, not a value and a slope across.** A height
/// field's normal is `(−h_x, −h_y, 1)` in its own plane's frame, so a normal
/// prescribed along a side fixes *both* readings of the gradient there — the
/// one across the side and the one along it. What is left for the middle of the
/// patch is an interpolation and not a guess.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Heighted {
    /// How far the place stands off the plane.
    height: f64,
    /// How fast that changes, in the plane's own two directions.
    slope: DVec2,
}

impl Heighted {
    /// What the height over `over`, whose normal is `up`, is held to at `at`
    /// where the patch faces `facing`.
    ///
    /// `None` where the patch turns square to its own plane there, which is a
    /// corner no height over that plane spans — see [`Vertexed::new`], which
    /// is what keeps the rest of them clear of it.
    fn of(over: Plane, up: DVec3, at: DVec3, facing: DVec3) -> Option<Self> {
        let along = facing.dot(up);
        (along > 0.0).then(|| Self {
            height: (at - over.origin).dot(up),
            slope: -DVec2::new(facing.dot(over.x), facing.dot(over.y)) / along,
        })
    }
}

/// A side's circle as the patch's own plane sees it: an ellipse about
/// `middle`, reached by `middle + across·cos θ + up·sin θ`.
///
/// **And the frame every reading of the side is taken in.** A place of the
/// plane reads as `middle + ξ·across + η·up`, and in those two the ellipse is
/// the unit circle: its bearing is the side's own parameter, and how far a
/// place stands from it is read off `√(ξ² + η²)` alone. See [`Sided::footing`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flattened {
    middle: DVec2,
    across: DVec2,
    up: DVec2,
    /// `across` crossed with `up`, which every reading of `ξ` and `η` divides
    /// by.
    spread: f64,
}

impl Flattened {
    /// How a `circle` lies in the plane `over`, or `None` where it lies edge
    /// on and flattens to a line.
    fn new(over: Plane, circle: Circle) -> Option<Self> {
        let axis = circle.axis;
        let middle = over.flatten(axis.origin);
        let across = over.flatten(axis.origin + axis.reference * circle.radius) - middle;
        let up = over.flatten(axis.origin + axis.quarter() * circle.radius) - middle;
        let spread = across.perp_dot(up);
        (spread != 0.0).then_some(Self {
            middle,
            across,
            up,
            spread,
        })
    }

    /// Where the flattened circle stands at the turn `angle`.
    fn turned(&self, angle: f64) -> DVec2 {
        let (sin, cos) = angle.sin_cos();
        self.middle + self.across * cos + self.up * sin
    }
}

/// What a side reads back at one place of the plane.
///
/// **Two places and not one.** A side's height and slope are read where the
/// place's bearing puts it on that side's *circle*, which moves smoothly and
/// runs on past the side's own ends; how far the side stands is read to the
/// place held to its stretch, so the run past those ends weighs less. Read at
/// one place alone, the patch is either kinked at every end or snapped to
/// wherever a side's circle happens to run through the opening.
#[derive(Debug, Clone, Copy)]
struct Footed {
    /// How far the place stands from the side's own middle, in radii of the
    /// flattened circle.
    near: f64,
    /// The bearing of the place about that middle, on the turn nearest the
    /// side's own stretch.
    turn: f64,
    /// That bearing held to the side's stretch.
    stood: f64,
    /// How far the bearing left the stretch by, in radians.
    past: f64,
    /// How far the place stands from the held place, in radii of the flattened
    /// circle — see [`Sided::footing`], where the metric is argued.
    gap: f64,
}

/// What one side holds the patch to, worked out once for the whole of it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Reading {
    /// A cross section: the blend's cylinder, read where the bearing puts the
    /// place on the section and carried over as a tangent plane.
    ///
    /// `room` is how far past either end, in radians, the section runs before
    /// the cylinder turns square to the patch's own plane — see
    /// [`Sided::saturated`], which holds every reading short of it.
    Blend { room: f64 },
    /// A spring: the face's own plane, which is one height function for the
    /// whole of it, `level + slope·uv`.
    ///
    /// **No footing at all.** A tangent plane carried from any place of a plane
    /// is that plane again, so where the bearing puts the place on the spring
    /// changes nothing about what the spring says — only how much.
    Face { level: f64, slope: DVec2 },
}

/// One side of the opening as the patch reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sided {
    /// The side as it was handed over.
    ///
    /// **Its stretch is weighed from, and not merely recorded.** A circle runs
    /// on past its own side and its flattened image runs on through the middle
    /// of the opening — so a place inside could stand *on* the continuation of
    /// a side it is nowhere near, and the blend would snap to that side's own
    /// reading there. The place a side is weighed from is held to the stretch,
    /// so a bearing past it stands as far off as the end itself does.
    side: Side,
    /// The circle's own quarter turn, worked out once rather than crossed out
    /// of its frame at every reading.
    quarter: DVec3,
    /// How far past either end of the stretch this side still speaks — see
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
    reading: Reading,
}

impl Sided {
    /// One side of the opening, as seen from the plane `over` with the normal
    /// `up`, in an opening whose corners stand `spread` from the corner at the
    /// furthest — or `None` where the side cannot be read over that plane.
    fn new(over: Plane, up: DVec3, side: Side, spread: f64) -> Option<Self> {
        let [from, to] = side.bounds;
        let axis = side.circle.axis;
        let quarter = axis.quarter();
        let turn = (PI - (to - from).abs() / 2.0).max(0.0) * side.circle.radius;
        let reading = match side.along {
            Along::Face => {
                let held = Heighted::of(over, up, side.circle.at(from), axis.direction)?;
                Reading::Face {
                    level: held.height - held.slope.dot(over.flatten(side.circle.at(from))),
                    slope: held.slope,
                }
            }
            Along::Blend { .. } => Reading::Blend {
                room: Self::room(up, side, quarter)?,
            },
        };
        Some(Self {
            side,
            quarter,
            margin: spread.min(turn / 4.0),
            flat: Flattened::new(over, side.circle)?,
            reading,
        })
    }

    /// How far past either end of a cross section the blend keeps facing the
    /// patch's plane, in radians, or `None` where the section itself does not.
    ///
    /// **Solved off the cylinder's own turn.** Its radial reads
    /// `a·cos θ + b·sin θ` against `up`, which is `A·cos(θ − θ*)`: nought a
    /// quarter turn either side of `θ*` and every half turn after. The room is
    /// the way from each end to the first nought beyond it — and where those
    /// two and the section's own sweep add to more than a half turn, a nought
    /// stands inside the section, which is a patch that is no height over
    /// this plane at all.
    fn room(up: DVec3, side: Side, quarter: DVec3) -> Option<f64> {
        let [from, to] = side.bounds;
        let axis = side.circle.axis;
        let (a, b) = (axis.reference.dot(up), quarter.dot(up));
        let toward = b.atan2(a) + PI / 2.0;
        let way = (to - from).signum();
        let ahead = (way * (toward - to)).rem_euclid(PI);
        let behind = (-way * (toward - from)).rem_euclid(PI);
        let facing = match side.along {
            Along::Blend { filled: true } => -1.0,
            _ => 1.0,
        };
        let clear = a.hypot(b) > 0.0
            && facing * axis.radial(from).dot(up) > 0.0
            && ahead + behind + (to - from).abs() < 1.5 * PI;
        clear.then_some(ahead.min(behind))
    }

    /// Where the query at `uv` reads this side.
    ///
    /// **The bearing about its own middle, and no search at all.** Flattening is
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
    /// **And the distance is read in that basis too.** The place a side is
    /// weighed from is held to its stretch, so past either end the distance is
    /// to the end itself and within it the distance is along the bearing —
    /// and the two have to join without a kink on the line through the end,
    /// or the patch creases along that whole line. In the plane's own metric
    /// they do not: a bearing runs along the ellipse's affine radial, which is
    /// square to the ellipse only where it is a circle, and the springs
    /// flatten to ellipses of `1.08` by `0.71`. Measured on the notch's step
    /// corner, the slope jumped by `0.4` across the line through a spring's
    /// end where the slope itself was `2`. In the ellipse's own basis the
    /// radial *is* the normal, both distances have the gradient `−∇√(ξ² + η²)`
    /// on the line, and the jump reads `0.003` — which is the second
    /// difference over the straddle, and no jump at all.
    fn footing(&self, uv: DVec2) -> Footed {
        let Flattened {
            middle,
            across,
            up,
            spread,
        } = self.flat;
        let out = uv - middle;
        let xi = out.perp_dot(up) / spread;
        let eta = across.perp_dot(out) / spread;
        let near = xi.hypot(eta);
        let [from, to] = self.side.bounds;
        let turn = branch::nearest(eta.atan2(xi), (from + to) / 2.0);
        let stood = turn.clamp(from.min(to), from.max(to));
        let past = (turn - stood).abs();
        let gap = match past == 0.0 {
            true => (1.0 - near).abs(),
            false => {
                let (sin, cos) = stood.sin_cos();
                (xi - cos).hypot(eta - sin)
            }
        };
        Footed {
            near,
            turn,
            stood,
            past,
            gap,
        }
    }

    /// The turn a cross section is read at for the footing `footed`, held short
    /// of where the blend turns square to the patch's plane.
    ///
    /// **Past the ends the reading slows and stops, smoothly.** A cross section
    /// is read off its cylinder at the bearing's own place, and that place runs
    /// on round the cylinder as the bearing leaves the section — round to where
    /// the cylinder faces along the plane and a height read off it means
    /// nothing. So the reading follows the bearing for half the room there is,
    /// then closes on three quarters of it and never arrives: a reading dropped
    /// outright where it turned square would be a side that stops speaking
    /// mid-sentence, and a reading held at the end outright is a tangent plane
    /// that stops moving, which kinks the patch along the line through the end.
    fn saturated(footed: Footed, room: f64) -> f64 {
        if footed.past == 0.0 {
            return footed.turn;
        }
        let (half, quarter) = (room / 2.0, room / 4.0);
        let held = match footed.past <= half {
            true => footed.past,
            false => half + quarter * ((footed.past - half) / quarter).tanh(),
        };
        footed.stood + held.copysign(footed.turn - footed.stood)
    }

    /// How much a reading taken `past` a side's own end still counts, `past`
    /// being a length along the circle.
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
        let share = past / self.margin;
        let squared = share * share;
        1.0 / (1.0 + squared * squared * squared * squared)
    }

    /// How much this side says at a place `near` radii out from its own
    /// flattened middle.
    ///
    /// **Nothing at the middle itself, for a spring.** The three springs are
    /// arcs of one sphere about the corner, so all three flatten to ellipses
    /// about the corner's own image — which stands inside the opening, and at
    /// it every place of a spring is as near as every other. The bearing spins
    /// there and so does the held end, and a weight read off either has a
    /// crease through the corner that the mesh reads. So a spring's say is
    /// `r⁴ / (½ + r⁴)` in radii out: a zero of the fourth order, which leaves
    /// the product of the say and the spinning reading flat, and half its say
    /// at `0.84` of the way out. See [`HUSHED`], where the half is measured.
    ///
    /// **Everything, for a cross section.** A section's circle is centred on
    /// its blend's own axis, which stands a reach off the faces and flattens
    /// outside the opening — so nothing inside is near its middle, and hushing
    /// it would only hand the middle of the patch to the springs.
    fn hushed(&self, near: f64) -> f64 {
        match self.side.along {
            Along::Blend { .. } => 1.0,
            Along::Face => {
                let fourth = (near * near) * (near * near);
                fourth / (HUSHED + fourth)
            }
        }
    }

    /// Where the side stands, a `share` of the way along its own stretch.
    fn along(&self, share: f64) -> DVec2 {
        let [from, to] = self.side.bounds;
        self.flat.turned(from + (to - from) * share)
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
        let [from, to] = self.side.bounds;
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
                    held[at + which] = flat.turned(angle);
                }
            }
        }
        held
    }

    /// The way out of the side's own circle at the turn `angle`, which is
    /// unit by how it is made.
    fn radial(&self, angle: f64) -> DVec3 {
        let (sin, cos) = angle.sin_cos();
        self.side.circle.axis.reference * cos + self.quarter * sin
    }

    /// Which way the patch faces where the side's circle runs out `radial`.
    fn facing(&self, radial: DVec3) -> DVec3 {
        match self.side.along {
            Along::Face => self.side.circle.axis.direction,
            Along::Blend { filled: true } => -radial,
            Along::Blend { filled: false } => radial,
        }
    }
}

/// The patch itself: a height over one plane, blended from what its six sides
/// hold it to.
///
/// **Carried whole, sides and all.** A `Surface` is copied by value on every
/// path a frame walks, and this is the largest arm of it by five: what it
/// holds is the six sides as the plane sees them, each with its own frame,
/// stretch, margin and reading worked out. That is the price of a reading of
/// the height being six bearings and nothing else — see `.notes/VERTEX-BLENDS.md`
/// §5, where the alternative of finding the six again at every query is
/// measured and refused.
///
/// **Blended rather than fitted.** Each side holds the height and its whole
/// gradient — see [`Heighted`] — so what a place inside takes is those six
/// readings carried to it and weighed by how near each side stands. A place
/// *on* a side reads that side's own numbers back, the weight of the rest
/// falling away as the square of the distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Vertexed {
    /// The plane the patch is a height over, whose origin is the corner.
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
    over: Plane,
    /// That plane's own normal, crossed and normalised once rather than at
    /// every side of every reading.
    up: DVec3,
    /// Alternating a blend and the face after it, which is the order the six
    /// come in.
    sides: [Sided; 6],
    /// The corner the three blends swallow, which is what the patch's own
    /// extent is measured about — and the origin of its plane, so the corner
    /// reads as nought in the domain.
    at: DVec3,
    /// How far the opening stands from that corner at the furthest, which is
    /// the only length the patch has to step by — see [`Vertexed::normal`].
    reach: f64,
    /// How hard the patch bends: the larger of its height's two principal
    /// second derivatives, at the worst place a walk of the opening finds.
    ///
    /// **Carried rather than asked for.** It is what a mesh's own stride comes
    /// off, and the walk that finds it is by far the largest thing this surface
    /// does — so it is worked out once, where the surface is made, and never
    /// again. See [`Vertexed::walked`].
    bending: f64,
}

impl Vertexed {
    /// The patch spanning the six `sides` about the corner `at`, or `None`
    /// where no height over one plane spans them.
    ///
    /// **The only way one is made**, because everything it carries is derived
    /// from the six: the plane, each side's frame in it, and how hard the
    /// whole thing bends.
    pub(crate) fn new(at: DVec3, sides: [Side; 6]) -> Option<Self> {
        let leaning: DVec3 = sides
            .iter()
            .filter(|side| side.along == Along::Face)
            .map(|side| side.circle.axis.direction)
            .sum();
        let over = Axis::about(at, leaning.try_normalize()?).plane();
        let up = over.normal();
        let (mut reach, mut spread) = (0.0_f64, 0.0_f64);
        for end in sides.iter().flat_map(Side::ends) {
            reach = reach.max(at.distance(end));
            spread = spread.max(over.flatten(end).length());
        }
        let mut read = [None; 6];
        for (held, side) in read.iter_mut().zip(sides) {
            *held = Some(Sided::new(over, up, side, spread)?);
        }
        let mut made = Self {
            over,
            up,
            sides: read.map(|side| side.expect("every side was read")),
            at,
            reach,
            bending: 0.0,
        };
        made.bending = made.walked();
        Some(made)
    }

    /// The key several of these are filed under — see
    /// [`Natural::key`](super::natural::Natural), which is where the argument
    /// for it is.
    ///
    /// The word carries on from the naturals' four, the torus's fifth and the
    /// ruled patch's sixth, so no two surfaces of the whole set collide on it.
    /// Keyed off what defines the patch — the corner and the six sides — and
    /// not off what was derived from them.
    pub(crate) fn key(&self) -> u64 {
        let mut key = Key::default().word(6).place(self.at);
        for read in &self.sides {
            let side = read.side;
            key = side
                .circle
                .axis
                .keyed(key)
                .float(side.circle.radius)
                .float(side.bounds[0])
                .float(side.bounds[1])
                .word(match side.along {
                    Along::Face => 0,
                    Along::Blend { filled: false } => 1,
                    Along::Blend { filled: true } => 2,
                });
        }
        key.done()
    }

    /// The side at `which`, in the order the six were handed over.
    pub(crate) fn side(&self, which: usize) -> Side {
        self.sides[which].side
    }

    /// How far the patch stands off its own plane at the domain place `uv`.
    fn height(&self, uv: DVec2) -> f64 {
        let (mut top, mut sum) = (0.0_f64, 0.0_f64);
        for read in &self.sides {
            let circle = read.side.circle;
            let footed = read.footing(uv);
            let local = match read.reading {
                Reading::Face { level, slope } => level + slope.dot(uv),
                Reading::Blend { room } => {
                    let turn = Sided::saturated(footed, room);
                    let radial = read.radial(turn);
                    let at = circle.axis.origin + radial * circle.radius;
                    let held = Heighted::of(self.over, self.up, at, read.facing(radial))
                        .expect("a reading held short of square faces the plane");
                    held.height + held.slope.dot(uv - read.flat.turned(turn))
                }
            };
            let apart = footed.gap * circle.radius;
            let squared = apart * apart;
            if squared <= f64::EPSILON {
                return local;
            }
            let weighed =
                read.taper(footed.past * circle.radius) * read.hushed(footed.near) / squared;
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
    /// on to one parameter place and no other, [`Vertexed::uv`] being a
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
    /// **Fanned from the corner out to the rim.** Every ray from the corner
    /// meets the opening once — held by `Setback`'s own rows, in
    /// `solid::rounding::setback` — and flattening keeps that, so every place
    /// of the opening lies on a run from the corner's image to one of the
    /// rim's. The walk runs the six sides
    /// themselves and draws in from each place it reaches toward the corner,
    /// the shells spaced by a cosine so that they cluster at both ends: at the
    /// rim, where the arcs stand off their chords and the lean is worst, and
    /// at the corner, where the springs fall silent and the sections are left
    /// to bend the patch between them. Measured against a grid of the whole
    /// opening on the notch's step corner at four setbacks, the fan reads
    /// within a twentieth of it.
    fn walked(&self) -> f64 {
        let step = self.reach * BENT;
        let mut most = 0.0_f64;
        for side in &self.sides {
            for around in 0..RIMMED {
                let out = side.along(f64::from(around) / f64::from(RIMMED));
                for along in 0..=STEPPED {
                    let share = (1.0 + (PI * f64::from(along) / f64::from(STEPPED)).cos()) / 2.0;
                    most = most.max(self.bent(out * share, step));
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
    /// **Square, both parameters bending alike** — see [`Vertexed::strides`],
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
    use crate::math::winding;
    use crate::solid::geometry::line::Line;

    /// What a square corner the notch leaves would be.
    const SPANS: &str = "a square corner spans a patch";

    /// The six sides the notch's step corner leaves, in the frame
    /// `.notes/VERTEX-BLENDS.md` reads it in: `u` across the floor, `v` along
    /// the reflex edge and `w` up the wall, with the corner at nought.
    ///
    /// **Written down rather than derived.** The fill runs the reflex edge,
    /// its axis a reach into the void the notch leaves; the two cuts run the
    /// edges the cap makes with the floor and the wall, each a reach into the
    /// material. A rail stands one reach off its edge along each face and the
    /// setback carries it that far again along the edge — so the fill stops on
    /// `(0, t, −r)` and `(−r, t, 0)`, and the two cuts read the same one turn
    /// round apiece. Each cross section is square to its own axis at the
    /// setback and holds the place of it that faces the edge; each spring is
    /// an arc of the sphere `√(t² + r²)` about the corner in its own face, the
    /// cap's holding `(1, 0, 1)` so that it runs the long way round the void.
    fn notch(reach: f64, setback: f64) -> Option<Vertexed> {
        let (r, t) = (reach, setback);
        let rho = t.hypot(r);
        let axes = [
            Line {
                origin: DVec3::new(-r, 0.0, -r),
                direction: DVec3::Y,
            },
            Line {
                origin: DVec3::new(0.0, r, r),
                direction: DVec3::NEG_X,
            },
            Line {
                origin: DVec3::new(r, r, 0.0),
                direction: DVec3::NEG_Z,
            },
        ];
        let made = [
            [DVec3::new(0.0, t, -r), DVec3::new(-r, t, 0.0)],
            [DVec3::new(-t, r, 0.0), DVec3::new(-t, 0.0, r)],
            [DVec3::new(r, 0.0, -t), DVec3::new(0.0, r, -t)],
        ];
        let facing = [DVec3::NEG_Z, DVec3::NEG_Y, DVec3::NEG_X];
        let filled = [true, false, false];
        let through = [
            [DVec3::new(0.0, t, 0.0), DVec3::new(-1.0, 1.0, 0.0)],
            [DVec3::new(-t, 0.0, 0.0), DVec3::new(1.0, 0.0, 1.0)],
            [DVec3::new(0.0, 0.0, -t), DVec3::new(0.0, 1.0, -1.0)],
        ];
        let mut sides = [None; 6];
        for which in 0..3 {
            let axis = axes[which];
            let ends = made[which];
            let middle = axis.at((ends[0] - axis.origin).dot(axis.direction));
            let circle = Circle {
                axis: Axis::new(middle, axis.direction, (ends[0] - middle).normalize()),
                radius: r,
            };
            sides[2 * which] = Some(Side::arced(
                circle,
                ends[0],
                ends[1],
                through[which][0],
                Along::Blend {
                    filled: filled[which],
                },
            ));
            let (from, to) = (made[which][1], made[(which + 1) % 3][0]);
            let circle = Circle {
                axis: Axis::new(DVec3::ZERO, facing[which], from.normalize()),
                radius: rho,
            };
            sides[2 * which + 1] = Some(Side::arced(
                circle,
                from,
                to,
                through[which][1],
                Along::Face,
            ));
        }
        Vertexed::new(DVec3::ZERO, sides.map(|side| side.expect("six sides")))
    }

    /// The rim of the flattened opening, walked as a closed run of corners for
    /// [`winding::holds`].
    fn rim(patch: &Vertexed) -> Vec<DVec2> {
        let mut walked = Vec::new();
        for read in &patch.sides {
            for step in 0..64 {
                walked.push(read.along(f64::from(step) / 64.0));
            }
        }
        walked
    }

    /// **The six sides are the ones handed over, and each runs between the
    /// two places it was named by.** A side is read back as it was given, and
    /// every one lies on the shape it was cut from: a cross section a reach
    /// off its own blend's axis, a spring a reach off the corner and flat on
    /// its own face.
    #[test]
    fn the_six_sides_read_back_as_they_were_given() {
        let (r, t) = (0.5, 1.0);
        let patch = notch(r, t).expect(SPANS);
        let rho = t.hypot(r);
        let want = [
            [DVec3::new(0.0, t, -r), DVec3::new(-r, t, 0.0)],
            [DVec3::new(-r, t, 0.0), DVec3::new(-t, r, 0.0)],
            [DVec3::new(-t, r, 0.0), DVec3::new(-t, 0.0, r)],
            [DVec3::new(-t, 0.0, r), DVec3::new(r, 0.0, -t)],
            [DVec3::new(r, 0.0, -t), DVec3::new(0.0, r, -t)],
            [DVec3::new(0.0, r, -t), DVec3::new(0.0, t, -r)],
        ];
        for (which, pair) in want.into_iter().enumerate() {
            let side = patch.side(which);
            for (got, want) in side.ends().into_iter().zip(pair) {
                assert!(
                    got.abs_diff_eq(want, 1e-12),
                    "side {which} ends at {got}, not {want}"
                );
            }
            for step in 0..=8 {
                let along =
                    side.bounds[0] + (side.bounds[1] - side.bounds[0]) * f64::from(step) / 8.0;
                let at = side.circle.at(along);
                match side.along {
                    Along::Blend { .. } => {
                        let off = side.circle.axis.off(at);
                        assert!(
                            (off - r).abs() < 1e-12,
                            "the cross section stands {off} off"
                        );
                    }
                    Along::Face => {
                        let flat = at.dot(side.circle.axis.direction).abs();
                        assert!(flat < 1e-12, "the spring stands {flat} off its own face");
                        assert!(
                            (at.length() - rho).abs() < 1e-12,
                            "the spring stands {} off the corner",
                            at.length(),
                        );
                    }
                }
            }
        }
        assert_eq!(patch.reach, rho, "the reach is how far the six stand");
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
            let patch = notch(r, t).expect(SPANS);
            let mut least = f64::INFINITY;
            for read in patch.sides {
                let [from, to] = read.side.bounds;
                for step in 0..=16 {
                    let along = from + (to - from) * f64::from(step) / 16.0;
                    least = least.min(read.facing(read.radial(along)).dot(patch.up));
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
            let patch = notch(r, t).expect(SPANS);
            let over = patch.over;
            let mut asked = 0;
            for read in patch.sides {
                let [from, to] = read.side.bounds;
                for step in 0..=16 {
                    let along = from + (to - from) * f64::from(step) / 16.0;
                    let at = read.side.circle.at(along);
                    let facing = read.facing(read.radial(along));
                    let held = Heighted::of(over, patch.up, at, facing)
                        .expect("the patch stands clear of square");
                    let read =
                        (patch.up - over.x * held.slope.x - over.y * held.slope.y).normalize();
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
        for (r, t) in [(0.5, 1.0), (0.5, 1.5)] {
            let patch = notch(r, t).expect(SPANS);
            let (mut off, mut turned) = (0.0_f64, 0.0_f64);
            for read in patch.sides {
                let [from, to] = read.side.bounds;
                for step in 1..8 {
                    let along = from + (to - from) * f64::from(step) / 8.0;
                    let at = read.side.circle.at(along);
                    let uv = patch.uv(at);
                    off = off.max(patch.at(uv).distance(at));
                    turned = turned.max(patch.normal(uv).distance(read.facing(read.radial(along))));
                }
            }
            assert!(off < 1e-12, "the patch stands {off} off its own boundary");
            assert!(
                turned < 1e-6,
                "the patch turns {turned} from its own facing"
            );
        }
    }

    /// **And it stays a graph inside**, which is what the whole surface rests
    /// on: a place read anywhere within the opening faces the same way its own
    /// plane does, so the domain names one place of the patch and not two.
    ///
    /// Walked from the corner's own image out to each of the six corners of the
    /// opening, where the patch is at its most turned, and over a grid of the
    /// whole opening besides.
    #[test]
    fn the_patch_faces_its_own_plane_inside_the_opening_too() {
        for (r, t) in [(0.5, 1.0), (0.5, 1.5)] {
            let patch = notch(r, t).expect(SPANS);
            let mut least = f64::INFINITY;
            let mut held = |uv: DVec2| {
                least = least.min(patch.normal(uv).dot(patch.up));
                assert!(
                    patch.uv(patch.at(uv)).abs_diff_eq(uv, 1e-12),
                    "a place of the patch does not read its own parameters back",
                );
            };
            for side in patch.sides {
                let toward = side.along(0.0);
                for step in 0..=8 {
                    held(toward * f64::from(step) / 8.0);
                }
            }
            let laid = patch.laid();
            let span = laid.high - laid.low;
            let rim = rim(&patch);
            for down in 0..=24 {
                for across in 0..=24 {
                    let uv =
                        laid.low + span * DVec2::new(f64::from(across), f64::from(down)) / 24.0;
                    if winding::holds(&rim, uv) {
                        held(uv);
                    }
                }
            }
            assert!(
                least > 0.0,
                "the patch turns square to its own plane, at {least}"
            );
        }
    }

    /// **The patch is smooth across the line through every spring's end.** A
    /// place whose bearing has left a spring is weighed from the spring's end,
    /// and one whose bearing has not is weighed from the bearing's own place
    /// on the spring — and where the two meet, on the line from the corner
    /// through the end, the weight has to join without a kink. Read in the
    /// plane's own metric it did not, and the slope jumped by `0.4` across
    /// that line on the notch's step corner. Read in the ellipse's own basis,
    /// what is left across a straddle of `2ε` is the second difference alone,
    /// which turns the normal by `2ε` times the bending and no more — see
    /// [`Sided::footing`].
    ///
    /// Held at three places along the line inside the opening, at two
    /// setbacks, against twice the bending the walk found.
    #[test]
    fn the_patch_is_smooth_across_the_line_through_each_end() {
        for (r, t) in [(0.5, 1.0), (0.5, 1.5)] {
            let patch = notch(r, t).expect(SPANS);
            let straddle = patch.reach * 1e-4;
            for (which, read) in patch.sides.iter().enumerate() {
                for end in [read.along(0.0), read.along(1.0)] {
                    let across = end.perp().normalize();
                    for share in [0.3, 0.6, 0.9] {
                        let uv = end * share;
                        let turned = patch
                            .normal(uv + across * straddle)
                            .distance(patch.normal(uv - across * straddle));
                        assert!(
                            turned <= 2.0 * straddle * patch.bending * 2.0,
                            "the normal turns {turned} across the line through an end of side \
                             {which}, {share} of the way out, where the bending is {}",
                            patch.bending,
                        );
                    }
                }
            }
        }
    }

    /// **The patch bends no harder than its reach and its lean account for.**
    /// Along a cross section the patch is a circle of the reach, and as a
    /// height over a plane that circle reads `κ·√(1 + |∇h|²)·(1 + (∇h·t)²)`,
    /// which at the corners of the opening — where the normal reads `1/√3`
    /// against the plane — is up to `5.2/r`. What the blend adds inside is
    /// under twice that: measured over a grid of the whole opening, the patch
    /// bends at most `7.9/r`, `6.2/r`, `9.3/r` and `9.2/r` at setbacks of
    /// two, three, one and a half and six reaches. Before the springs were
    /// hushed by their radius and weighed in their own basis it read `24/r`
    /// and `29/r` at two and three reaches over the same grid — see
    /// [`HUSHED`].
    ///
    /// **And the walk finds it.** The bending carried on the surface is what
    /// every stride comes off. Against a grid of two hundred and forty each
    /// way the fan reads within a twentieth; against the coarser one below,
    /// which a row can afford, it is held to a tenth — and to no upper bound,
    /// the fan being the denser of the two at the rim.
    #[test]
    fn the_patch_bends_no_harder_than_its_reach_and_lean_account_for() {
        for (r, t) in [(0.5, 1.0), (0.5, 1.5), (0.5, 0.75), (0.5, 3.0)] {
            let patch = notch(r, t).expect(SPANS);
            assert!(
                patch.bending * r <= 10.0,
                "the patch bends {} times harder than its reach at a setback of {t}",
                patch.bending * r,
            );
            let laid = patch.laid();
            let span = laid.high - laid.low;
            let step = patch.reach * BENT;
            let rim = rim(&patch);
            let mut most = 0.0_f64;
            for down in 0..=64 {
                for across in 0..=64 {
                    let uv =
                        laid.low + span * DVec2::new(f64::from(across), f64::from(down)) / 64.0;
                    if winding::holds(&rim, uv) {
                        most = most.max(patch.bent(uv, step));
                    }
                }
            }
            assert!(
                patch.bending >= most * 0.9,
                "the walk reads {} where the grid reads {most} at a setback of {t}",
                patch.bending,
            );
        }
    }

    /// **The patch bends differently at every setback**, which is what makes
    /// the bending a reading of this patch and not a constant.
    #[test]
    fn the_bending_is_the_patch_s_own() {
        let bent =
            [(0.5, 1.0), (0.5, 1.5), (0.5, 0.75)].map(|(r, t)| notch(r, t).expect(SPANS).bending);
        assert!(bent[0] != bent[1] && bent[1] != bent[2], "{bent:?}");
    }

    /// **The blend reads every side at every place**, which is what keeps the
    /// box the mesh lays its grid over readable: a place of the box outside the
    /// opening is weighed by tapers that never reach nought, and a cross
    /// section's reading is held short of where its cylinder turns square.
    #[test]
    fn every_place_of_the_box_reads_finite() {
        let patch = notch(0.5, 1.0).expect(SPANS);
        let laid = patch.laid();
        let span = laid.high - laid.low;
        for down in 0..=32 {
            for across in 0..=32 {
                let uv = laid.low + span * DVec2::new(f64::from(across), f64::from(down)) / 32.0;
                let at = patch.at(uv);
                assert!(at.is_finite(), "the patch reads {at} at {uv}");
            }
        }
        let filled = patch.fills();
        assert!(
            filled.low.is_finite() && filled.high.is_finite(),
            "{filled:?}"
        );
    }
}
