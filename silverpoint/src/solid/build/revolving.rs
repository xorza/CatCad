//! A region of a drawing, spun a whole turn about a line in its own plane.

use crate::math::plane::Plane;
use crate::number::predicate;
use crate::number::tolerance::{EXACT, PLACED};
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::strip::{Strip, Strips, Turn};
use crate::solid::build::{Running, Walled, gathered};
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::sphere::Sphere;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::torus::Torus;
use crate::solid::grown::Grown;
use crate::solid::meeting::Meeting;
use crate::solid::named::Step;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

/// A region of a drawing and the line in its own plane it is spun about.
///
/// Borrowed and [`Copy`], like [`Extrusion`](super::builder::Extrusion) beside
/// it and for the same reason: what it holds is an arrangement somebody else
/// owns, a position in it, and a frame.
///
/// **A whole turn and no other**, which is what makes it one shape rather than
/// two. Spun part way a region has two ends, and those are caps of the same
/// kind an extrusion raises; spun the whole way it has none, and every wall
/// closes on itself instead. The second is what a ring, a washer and a ball
/// are, and it is the one written here.
#[derive(Debug, Clone, Copy)]
pub struct Revolution<'a> {
    of: &'a Arrangement,
    at: usize,
    plane: Plane,
    /// Somewhere on the line, in the plane's own coordinates.
    axis: DVec2,
    /// The way that line runs, in the same coordinates. Need not be unit.
    along: DVec2,
    /// Which of the caller's steps this is, so the faces it raises say which
    /// feature grew them and not merely what of it they are.
    by: Step,
}

impl<'a> Revolution<'a> {
    /// The region at `at` in `of`, spun a whole turn about the line through
    /// `axis` running `along`, both in `plane`'s own coordinates.
    ///
    /// `at` has to be one of `of`'s faces, and `plane` the one the drawing it
    /// came from lies on — a face names its edges by where they fall in the
    /// arrangement that cut them, so neither travels to another.
    pub fn new(
        of: &'a Arrangement,
        at: usize,
        plane: Plane,
        axis: DVec2,
        along: DVec2,
        by: Step,
    ) -> Self {
        Self {
            of,
            at,
            plane,
            axis,
            along,
            by,
        }
    }
}

/// The frame a revolve spins in, and what its passes hand each other.
///
/// Read-only and [`Copy`], so the room a [`Revolving`] holds and the body being
/// filled can both be written while this is being read.
#[derive(Debug, Clone, Copy)]
struct Spinning {
    plane: Plane,
    by: Step,
    /// The line in the world, framed so an angle of nought points at the
    /// region — which is what puts the drawing itself at the seam a whole turn
    /// is cut at.
    axis: Axis,
    /// Where that line stands in the drawing, and the unit way it runs.
    at: DVec2,
    along: DVec2,
    /// The unit way out of it toward the region, in the drawing.
    out: DVec2,
    /// Whether a loop is walked with the spin before the profile.
    ///
    /// **One flag for the whole revolve, not one per wall.** The spin and the
    /// profile make a face's own two parameters, and which order winds them
    /// counterclockwise about the material-free side turns only on whether the
    /// frame `(along, out)` reads the drawing the way it was drawn. Per wall it
    /// would differ, and two walls disagreeing would walk the circle between
    /// them the same way twice.
    forward: bool,
}

impl Spinning {
    /// Where `at` stands in the profile's own two: how far along the line, and
    /// how far out from it.
    fn profile(self, at: DVec2) -> DVec2 {
        let out = at - self.at;
        DVec2::new(self.along.dot(out), self.out.dot(out))
    }

    /// The same of a direction, which the line's own place does not move.
    fn turned(self, way: DVec2) -> DVec2 {
        DVec2::new(self.along.dot(way), self.out.dot(way))
    }

    /// Where the profile's `(along, out)` lands in the world at a spin of `u`.
    fn spun(self, at: DVec2, u: f64) -> DVec3 {
        self.axis.origin + self.axis.direction * at.x + self.axis.radial(u) * at.y
    }

    /// The line's own frame, moved `up` along it — which every circle a spin
    /// makes turns about and every surface of revolution is written on.
    fn about(self, up: f64) -> Axis {
        Axis::new(
            self.axis.origin + self.axis.direction * up,
            self.axis.direction,
            self.axis.reference,
        )
    }

    /// Whether `turn` stands clear of the line along the whole of it.
    ///
    /// **Asked of the arc and not of its ends**, which a straight run needs and
    /// an arc does not: an arc bulges away from its own chord, so one drawn
    /// from a place beside the line round to another can cross it in between
    /// and come back. What it sweeps there is a surface folded through itself.
    ///
    /// How far out an arc stands is a cosine of the angle round it, so the
    /// least of it is a radius inside the centre where the sweep reaches
    /// straight back at the line, and an end everywhere else.
    fn clears(self, turn: Turn) -> bool {
        let middle = self.profile(turn.center);
        let back = (self.out.to_angle() + PI - turn.start) * turn.sweep.signum();
        if back.rem_euclid(TAU) <= turn.sweep.abs() {
            return middle.y - turn.radius > 0.0;
        }
        let ends = [turn.start, turn.end()]
            .map(|angle| middle.y + self.out.dot(DVec2::from_angle(angle)) * turn.radius);
        ends[0] > 0.0 && ends[1] > 0.0
    }
}

/// Spins bodies, keeping the room it works in.
///
/// Held across calls for the reason [`Builder`](super::builder::Builder) is:
/// a solid is rebuilt on every frame of a drag through the drawing under it,
/// and comes out the shape it was last time.
#[derive(Debug, Default)]
pub(super) struct Revolving {
    /// Both vertices swept from each corner — at a nought turn and at a half
    /// one — or `None` where no strip of this region reaches that corner.
    corners: Vec<Option<[VertexId; 2]>>,
    /// The two halves of the circle each corner sweeps, in step.
    circling: Vec<Option<[EdgeId; 2]>>,
    /// The two faces raised off each strip, in step with [`Strips::all`].
    walls: Vec<[FaceId; 2]>,
    /// The two copies of each strip's own curve, at a nought turn and at a
    /// half one, in step with the walls.
    seams: Vec<[EdgeId; 2]>,
}

impl Revolving {
    /// Spin `of` into `into`, emptying whatever was there.
    ///
    /// **A body with no faces where it cannot be spun**, which is the answer
    /// an extrusion of no distance gives and means the same thing: there is no
    /// solid, so there is nothing to draw, to pick or to build on. Three things
    /// have no solid — a line with no direction, a region that touches or
    /// crosses the line, and an arc that reaches it, which would sweep a
    /// surface folded through itself.
    ///
    /// Four passes, and the order is forced for the reason the extrusion's is:
    /// an edge names the two faces that use it, so every face has to exist
    /// before any edge is made, and a face's loops cannot be written until its
    /// edges are.
    pub(super) fn raise(&mut self, of: &Revolution<'_>, strips: &mut Strips, into: &mut Body) {
        into.clear();
        strips.lay(of.of, of.at);
        let Some(spinning) = Self::framed(of, strips) else {
            return;
        };
        if !self.raise_walls(spinning, strips, into) {
            into.clear();
            return;
        }
        self.raise_edges(spinning, strips, into);
        self.write_loops(spinning, strips, into);
        self.gather(into);
    }

    /// The frame the region is spun in, or `None` where it cannot be.
    ///
    /// **Which side of the line the region stands on is read rather than
    /// given**, so a caller need not say — and reading it is the same walk that
    /// refuses a region straddling the line. A corner exactly on it is refused
    /// too: a corner there sweeps a point rather than a circle, which is a pole
    /// and not a vertex this writes.
    fn framed(of: &Revolution<'_>, strips: &Strips) -> Option<Spinning> {
        let along = of.along.try_normalize()?;
        let perp = along.perp();
        // The side the region stands on, off the first corner clear of the
        // line — and every corner after it has to agree.
        let mut side = 0.0_f64;
        for strip in strips.all() {
            for corner in [strip.from, strip.to] {
                let off = perp.dot(strips.corners()[corner] - of.axis);
                if off == 0.0 {
                    return None;
                }
                if side == 0.0 {
                    side = off.signum();
                } else if side != off.signum() {
                    return None;
                }
            }
        }
        // A region with no boundary at all, which stands nowhere and so on
        // neither side.
        if side == 0.0 {
            return None;
        }
        let out = perp * side;
        Some(Spinning {
            plane: of.plane,
            by: of.by,
            axis: Axis::new(
                of.plane.point(of.axis),
                borne(of.plane, along),
                borne(of.plane, out),
            ),
            at: of.axis,
            along,
            out,
            // The map from the drawing's own two to `(along, out)` reverses the
            // handedness exactly when the region stands to the right of the
            // line, and a face's parameters follow it.
            forward: side < 0.0,
        })
    }

    /// Two faces per strip, empty of loops.
    ///
    /// **Two and not one**, because a wall spun the whole way round covers its
    /// own surface and no face may — see `.notes/KERNEL.md` §4.4. Cut at the
    /// drawing's own seam and half a turn from it, so one of the two holds the
    /// profile exactly as it was drawn.
    ///
    /// `false` where a strip sweeps no surface this can write.
    fn raise_walls(&mut self, spinning: Spinning, strips: &Strips, into: &mut Body) -> bool {
        self.walls.clear();
        for at in 0..strips.all().len() {
            let strip = strips.all()[at];
            let Some(Walled { surface, outward }) = Self::wall_of(spinning, strips, strip) else {
                return false;
            };
            let name = spinning.by.grew(Grown::Side(strip.bound));
            into.named(name);
            let halves = [0, 1].map(|_| {
                into.topology_mut().add_face(Face {
                    surface,
                    outward,
                    loops: 0..0,
                    name,
                    tolerance: EXACT,
                })
            });
            self.walls.push(halves);
        }
        true
    }

    /// What a strip sweeps, and which side of it the material is on.
    ///
    /// **Five surfaces off two shapes**, which is the whole of what a revolve
    /// makes: a straight run parallel to the line sweeps a cylinder, one square
    /// across it an annulus of a plane, and one that leans a cone; an arc about
    /// a centre on the line sweeps a sphere, and one about a centre off it a
    /// torus.
    ///
    /// `None` for an arc that reaches the line, which sweeps no surface at all
    /// — and for one whose whole *circle* reaches it while the arc does not,
    /// which would spin into a torus passing through itself, a surface `Torus`
    /// refuses to be and a solid nothing bounds.
    fn wall_of(spinning: Spinning, strips: &Strips, strip: Strip) -> Option<Walled> {
        let profiled = |corner: usize| spinning.profile(strips.corners()[corner]);
        let surface = match strip.turn {
            None => {
                let (from, to) = (profiled(strip.from), profiled(strip.to));
                let way = spinning.spun(to, 0.0) - spinning.spun(from, 0.0);
                let way = way.try_normalize()?;
                if predicate::parallel(way, spinning.axis.direction) {
                    Surface::Natural(Natural::Cylinder(Cylinder {
                        axis: spinning.axis,
                        radius: from.y,
                    }))
                } else if predicate::square(way, spinning.axis.direction) {
                    Surface::Natural(Natural::Plane(spinning.about(from.x).plane()))
                } else {
                    // Where the run meets the line, which is what a cone is
                    // measured from — and the axis points from there toward the
                    // region, so the parameter along it is never negative.
                    let apex = from.x - from.y * (to.x - from.x) / (to.y - from.y);
                    let rise = from.x - apex;
                    Surface::Natural(Natural::Cone(Cone {
                        axis: Axis::new(
                            spinning.axis.origin + spinning.axis.direction * apex,
                            spinning.axis.direction * rise.signum(),
                            spinning.axis.reference,
                        ),
                        half_angle: (from.y / rise.abs()).atan(),
                    }))
                }
            }
            Some(turn) => {
                if !spinning.clears(turn) {
                    return None;
                }
                let middle = spinning.profile(turn.center);
                let axis = spinning.about(middle.x);
                // A centre nearer the line than a place stands for is on it,
                // and what it sweeps is a ball rather than a ring drawn to a
                // rounding. Refusing instead would refuse every sphere whose
                // centre was not constructed exactly.
                if predicate::touching(middle.y.abs(), PLACED) {
                    Surface::Natural(Natural::Sphere(Sphere {
                        axis,
                        radius: turn.radius,
                    }))
                } else {
                    if middle.y <= turn.radius {
                        return None;
                    }
                    Surface::Fitted(Fitted::Torus(Torus {
                        axis,
                        major: middle.y,
                        minor: turn.radius,
                    }))
                }
            }
        };
        // **The material is on the left of the walk**, the outline of a region
        // being counterclockwise — so the wall faces the other way, and asking
        // the surface itself where that is answers for all five at once.
        let along = laid(strips, strip, 0.5);
        let inward = borne(spinning.plane, along.way.perp());
        let place = spinning.plane.point(along.at);
        let outward = surface.normal(surface.uv(place)).dot(inward) < 0.0;
        Some(Walled { surface, outward })
    }

    /// Every vertex, every circle a corner sweeps, and every seam.
    fn raise_edges(&mut self, spinning: Spinning, strips: &Strips, into: &mut Body) {
        let corners = strips.corners().len();
        self.corners.clear();
        self.corners.resize(corners, None);
        self.circling.clear();
        self.circling.resize(corners, None);
        self.seams.clear();
        for at in 0..strips.all().len() {
            let strip = strips.all()[at];
            self.corner(spinning, strips, strip.from, into);
            self.corner(spinning, strips, strip.to, into);
            let halves = self.walls[at];
            let seams = [0, 1].map(|half| self.seam(spinning, strips, strip, half, halves, into));
            self.seams.push(seams);
        }
        for loop_ in 0..strips.loops() {
            let run = strips.run(loop_);
            for at in run.clone() {
                // The corner between this strip and the next around the loop
                // is where the two walls meeting there share an edge.
                let next = if at + 1 == run.end { run.start } else { at + 1 };
                let corner = strips.all()[at].to;
                self.circle(spinning, strips, corner, [at, next], into);
            }
        }
    }

    /// Raise both vertices at `corner`, unless something already has.
    fn corner(&mut self, spinning: Spinning, strips: &Strips, corner: usize, into: &mut Body) {
        if self.corners[corner].is_some() {
            return;
        }
        // What the drawing knew this corner to, which is nought wherever
        // nothing folded into it — see
        // [`Arrangement::reached`](crate::Arrangement).
        let tolerance = strips.reached()[corner];
        let at = spinning.profile(strips.corners()[corner]);
        let raised = [0.0, PI].map(|u| {
            into.topology_mut().add_vertex(Vertex {
                at: spinning.spun(at, u),
                tolerance,
            })
        });
        self.corners[corner] = Some(raised);
    }

    /// One copy of a strip's own curve, at the spin the walls part at `half`.
    fn seam(
        &mut self,
        spinning: Spinning,
        strips: &Strips,
        strip: Strip,
        half: usize,
        between: [FaceId; 2],
        into: &mut Body,
    ) -> EdgeId {
        let [from, to] = [strip.from, strip.to]
            .map(|corner| self.corners[corner].expect("every corner of a strip is raised")[half]);
        let Running { curve, bounds } = Self::running(spinning, strips, strip, PI * half as f64);
        into.topology_mut().add_edge(Edge {
            curve,
            bounds,
            from,
            to,
            between,
            // The two halves of one wall lie on one surface, so a seam between
            // them is what splitting a whole turn left behind rather than a
            // crease — see `.notes/KERNEL.md` §4.4.
            artificial: true,
            tolerance: EXACT,
        })
    }

    /// The curve a strip's own copy at a spin of `u` runs along.
    fn running(spinning: Spinning, strips: &Strips, strip: Strip, u: f64) -> Running {
        let placed = |corner: usize| spinning.spun(spinning.profile(strips.corners()[corner]), u);
        let Some(turn) = strip.turn else {
            let origin = placed(strip.from);
            let to = placed(strip.to);
            return Running {
                curve: Curve::Line(Line {
                    origin,
                    direction: (to - origin).normalize(),
                }),
                bounds: [0.0, origin.distance(to)],
            };
        };
        // The arc stands in the half-plane the spin has carried it into, so its
        // own frame is read there: out from the middle to where the walk begins,
        // and about whichever way makes the parameter run the way the walk does.
        let middle = spinning.spun(spinning.profile(turn.center), u);
        let start = placed(strip.from);
        let reference = (start - middle).normalize();
        let leaving = spinning.turned(DVec2::from_angle(turn.start).perp() * turn.sweep.signum());
        let ahead = spinning.axis.direction * leaving.x + spinning.axis.radial(u) * leaving.y;
        Running {
            curve: Curve::Circle(Circle {
                axis: Axis::new(middle, reference.cross(ahead).normalize(), reference),
                radius: turn.radius,
            }),
            bounds: [0.0, turn.sweep.abs()],
        }
    }

    /// The two halves of the circle `corner` sweeps, between the walls that
    /// meet there.
    fn circle(
        &mut self,
        spinning: Spinning,
        strips: &Strips,
        corner: usize,
        between: [usize; 2],
        into: &mut Body,
    ) {
        debug_assert!(
            self.circling[corner].is_none(),
            "corner {corner} is the end of two strips of one loop",
        );
        let at = spinning.profile(strips.corners()[corner]);
        let axis = spinning.about(at.x);
        // No crease where the two walls lie on one surface, which two arcs of
        // one circle the drawing was cut between and two segments drawn
        // straight through a corner both are.
        let topology = into.topology();
        let [one, two] = between.map(|wall| topology.face(self.walls[wall][0]).surface);
        let smooth = Meeting::of(&one, &two) == Meeting::Same;
        let raised = self.corners[corner].expect("every corner of a strip is raised");
        let halves = [0, 1].map(|half| {
            let faces = between.map(|wall| self.walls[wall][half]);
            let turn = PI * half as f64;
            into.topology_mut().add_edge(Edge {
                curve: Curve::Circle(Circle { axis, radius: at.y }),
                bounds: [turn, turn + PI],
                from: raised[half],
                to: raised[(half + 1) % 2],
                between: faces,
                artificial: smooth,
                tolerance: EXACT,
            })
        });
        self.circling[corner] = Some(halves);
    }

    /// Write the one loop of every face.
    fn write_loops(&mut self, spinning: Spinning, strips: &Strips, into: &mut Body) {
        for at in 0..self.walls.len() {
            let strip = strips.all()[at];
            for half in 0..2 {
                self.wall_loop(spinning.forward, strip, at, half, into);
            }
        }
    }

    /// The one loop of one half of one wall: along the profile at the near
    /// seam, round to the far one, back along the profile, and round again.
    fn wall_loop(&mut self, forward: bool, strip: Strip, at: usize, half: usize, into: &mut Body) {
        let seams = self.seams[at];
        let circling = [strip.to, strip.from]
            .map(|corner| self.circling[corner].expect("every corner of a strip sweeps a circle"));
        let from = into.topology_mut().add_loop(|walk| {
            let wrote = walk.len();
            walk.extend([
                Coedge {
                    edge: seams[half],
                    forward: true,
                },
                Coedge {
                    edge: circling[0][half],
                    forward: true,
                },
                Coedge {
                    edge: seams[(half + 1) % 2],
                    forward: false,
                },
                Coedge {
                    edge: circling[1][half],
                    forward: false,
                },
            ]);
            if !forward {
                // Read the other way round, which is what keeps the loop
                // counterclockwise about a face whose parameters the frame
                // reversed. Only this loop's own four, which is what the buffer
                // is indexed from.
                walk[wrote..].reverse();
                for coedge in &mut walk[wrote..] {
                    *coedge = coedge.turned();
                }
            }
        });
        into.topology_mut().face_mut(self.walls[at][half]).loops = from..from + 1;
    }

    /// Gather every face into the one shell around the one lump.
    fn gather(&mut self, into: &mut Body) {
        gathered(into, self.walls.iter().flatten().copied());
    }
}

/// Where a strip stands part way along it, and the way it runs there.
#[derive(Debug, Clone, Copy)]
struct Laid {
    at: DVec2,
    /// Unit, the way the walk runs — which is what says where the material is,
    /// a region lying on the left of its own boundary.
    way: DVec2,
}

/// The world direction a direction of the drawing points, which is
/// [`Plane::point`] without the origin under it.
fn borne(plane: Plane, way: DVec2) -> DVec3 {
    plane.x * way.x + plane.y * way.y
}

/// Where the strip stands `share` of the way along it, in the drawing.
fn laid(strips: &Strips, strip: Strip, share: f64) -> Laid {
    let (from, to) = (strips.corners()[strip.from], strips.corners()[strip.to]);
    let Some(turn) = strip.turn else {
        return Laid {
            at: from.lerp(to, share),
            way: (to - from).normalize(),
        };
    };
    let angle = turn.start + turn.sweep * share;
    let out = DVec2::from_angle(angle);
    Laid {
        at: turn.center + out * turn.radius,
        way: out.perp() * turn.sweep.signum(),
    }
}

/// Spinning a body without keeping the room to spin the next one in.
///
/// The reason [`Extrusion`](super::builder::Extrusion)'s twin gives, and the
/// same one: everything that draws a solid holds a
/// [`Builder`](super::builder::Builder) and hands it a body to refill, and a
/// fixture raised once outside every window wants neither.
#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::solid::build::builder::Builder;
    use crate::solid::build::revolving::Revolution;
    use crate::solid::topology::body::Body;

    impl Revolution<'_> {
        /// Spin it into a body of its own.
        pub fn body(&self) -> Body {
            let mut body = Body::default();
            Builder::default().revolve(self, &mut body);
            body
        }
    }
}
