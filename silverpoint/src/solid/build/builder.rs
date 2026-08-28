//! A region of a drawing, carried off its plane into a body.

use crate::math::plane::Plane;
use crate::number::tolerance::EXACT;
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::strip::{Strip, Strips};
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::line::Line;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::meeting::Meeting;
use crate::solid::named::{Named, Step};
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::lump::Lump;
use crate::solid::topology::shell::Shell;
use crate::solid::topology::validity::Checking;
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::{DVec2, DVec3};

/// A region of a drawing and how far it is carried off the plane it was drawn
/// on.
///
/// Borrowed and [`Copy`], like the readings a drawing hands out: what it holds
/// is an arrangement somebody else owns, a position in it, and two values. A
/// caller makes one, hands it to a [`Builder`], and lets it go.
///
/// Which region by *position*, unlike everything a feature keeps. Turning a
/// durable name into one of these is the caller's, and happens once per
/// rebuild.
///
/// The distance is signed, so which way it grows needs no second field: a
/// negative distance is the same solid on the other side of the plane, and
/// every winding below follows the sign rather than being fixed one way.
#[derive(Debug, Clone, Copy)]
pub struct Extrusion<'a> {
    of: &'a Arrangement,
    at: usize,
    plane: Plane,
    distance: f64,
    /// Which of the caller's steps this is, so the faces it raises say which
    /// feature grew them and not merely what of it they are.
    by: Step,
}

impl<'a> Extrusion<'a> {
    /// The region at `at` in `of`, carried `distance` along `plane`'s normal.
    ///
    /// `at` has to be one of `of`'s faces, and `plane` the one the drawing it
    /// came from lies on — a face names its edges by where they fall in the
    /// arrangement that cut them, so neither travels to another.
    pub fn new(of: &'a Arrangement, at: usize, plane: Plane, distance: f64, by: Step) -> Self {
        Self {
            of,
            at,
            plane,
            distance,
            by,
        }
    }
}

/// Raises bodies, keeping the room it works in.
///
/// Held across calls rather than stood up for each, like the
/// [`Filler`](crate::Filler) and the [`Mesher`](crate::Mesher) it sits between:
/// a solid is rebuilt on every frame of a drag through the drawing under it,
/// and comes out the shape it was last time. Together with a body refilled
/// rather than replaced — see [`Body::clear`] — that is what keeps a drag off
/// the heap entirely.
///
/// [`Body::clear`]: crate::Body
#[derive(Debug, Default)]
pub struct Builder {
    strips: Strips,
    /// Both vertices raised at each corner, the base one first, or `None` where
    /// no strip reaches that corner.
    corners: Vec<Option<[VertexId; 2]>>,
    /// The lateral edge climbing from each corner, in step with `corners`.
    climbing: Vec<Option<EdgeId>>,
    /// The wall raised off each strip, in step with [`Strips::all`].
    walls: Vec<FaceId>,
    /// The base edge and the far edge each strip sweeps, in step with the walls
    /// and indexed by whether the far end is wanted.
    spans: Vec<[EdgeId; 2]>,
    /// The room the validity check works in, held for the reason everything
    /// else here is: it runs after every build, which on the path a drag takes
    /// is every frame.
    checking: Checking,
}

/// What the passes of one build hand each other.
///
/// Read-only and [`Copy`], so the scratch a [`Builder`] holds and the body
/// being filled can both be written while this is being read.
#[derive(Debug, Clone, Copy)]
struct Raising {
    plane: Plane,
    distance: f64,
    by: Step,
    /// The direction the plane faces, unit — the way the solid grows where the
    /// distance is positive and the way it grows back where it is not.
    normal: DVec3,
    /// Whether it grows along that normal rather than against it.
    along: bool,
    base: FaceId,
    far: FaceId,
}

/// A surface a strip sweeps, and which side of it the material is on.
#[derive(Debug, Clone, Copy)]
struct Walled {
    surface: Surface,
    outward: bool,
}

/// A curve an edge runs along, and the stretch of it that edge covers.
#[derive(Debug, Clone, Copy)]
struct Running {
    curve: Curve,
    bounds: [f64; 2],
}

impl Builder {
    /// Raise `of` into `into`, emptying whatever was there.
    ///
    /// Exact throughout: every surface raised below is the surface rather than
    /// a fit to one, and nothing here flattens anything. What a body is *drawn*
    /// as is [`Mesher`](crate::Mesher)'s, and how finely is its caller's.
    ///
    /// A distance of nothing leaves a body with no faces. There is no solid, so
    /// there is nothing to draw, to pick or to build on — and six faces
    /// enclosing nothing would be a worse answer than none.
    ///
    /// Four passes, and the order is forced: an edge names the two faces that
    /// use it, so every face has to exist before any edge is made, and a face's
    /// loops cannot be written until its edges are.
    pub fn extrude(&mut self, of: &Extrusion<'_>, into: &mut Body) {
        into.clear();
        if of.distance == 0.0 {
            return;
        }
        self.strips.lay(of.of, of.at);
        let normal = of.plane.normal();
        let along = of.distance > 0.0;
        // The caps face away from the solid, so the far end faces the way it
        // grew and the base faces back along it. The base lies on the sketch's
        // own plane — it *is* that plane — which is what lets a datum taken on
        // it carry the drawing's frame rather than a copy of one.
        // In this order and before anything else, because the order faces are
        // made is the order their names come back in — and a caller writing one
        // drawable per name relies on that not moving between rebuilds. See
        // [`Body::names`].
        let base = Self::cap(into, of.plane, of.by.grew(Grown::Base), !along);
        let far = Self::cap(
            into,
            Plane {
                origin: of.plane.origin + normal * of.distance,
                ..of.plane
            },
            of.by.grew(Grown::Far),
            along,
        );
        let raising = Raising {
            plane: of.plane,
            distance: of.distance,
            by: of.by,
            normal,
            along,
            base,
            far,
        };
        self.raise_walls(raising, into);
        self.raise_edges(raising, into);
        self.write_loops(raising, into);
        self.gather(raising, into);
        if cfg!(debug_assertions) {
            self.checking.run(into);
        }
    }

    /// One of the two ends: the region itself, lying flat.
    fn cap(into: &mut Body, plane: Plane, name: Named, outward: bool) -> FaceId {
        into.named(name);
        into.topology_mut().add_face(Face {
            surface: Surface::Natural(Natural::Plane(plane)),
            outward,
            // Filled by the loop pass, which is the only one that can know: a
            // face's loops are a stretch of the body's own buffer, and where
            // that stretch falls is decided when it is written.
            loops: 0..0,
            name,
            tolerance: EXACT,
        })
    }

    /// A wall per strip, empty of loops.
    fn raise_walls(&mut self, raising: Raising, into: &mut Body) {
        self.walls.clear();
        for at in 0..self.strips.all().len() {
            let strip = self.strips.all()[at];
            let name = raising.by.grew(Grown::Side(strip.bound));
            let Walled { surface, outward } = self.wall_of(raising, strip);
            into.named(name);
            let wall = into.topology_mut().add_face(Face {
                surface,
                outward,
                loops: 0..0,
                name,
                tolerance: EXACT,
            });
            self.walls.push(wall);
        }
    }

    /// What a strip sweeps, and which side of it the material is on.
    ///
    /// The material side is the region's own outward direction carried up, and
    /// it is read off the walk rather than off the extrusion: which way a wall
    /// faces is a fact about the drawing, so it does not turn over when the
    /// solid is grown the other way.
    fn wall_of(&self, raising: Raising, strip: Strip) -> Walled {
        let Some(turn) = strip.turn else {
            let origin = self.base_at(raising, strip.from);
            let running = (self.base_at(raising, strip.to) - origin).normalize();
            // Its own `u` runs along the walk and its `v` up the normal, which
            // puts `∂u × ∂v` on the region's outward side — so the wall of a
            // straight strip always faces out of its own parameters.
            return Walled {
                surface: Surface::Natural(Natural::Plane(Plane {
                    origin,
                    x: running,
                    y: raising.normal,
                })),
                outward: true,
            };
        };
        Walled {
            surface: Surface::Natural(Natural::Cylinder(Cylinder {
                axis: Self::turning(raising, turn.center),
                radius: turn.radius,
            })),
            // A cylinder faces away from its axis, which is the material side
            // exactly when the region keeps the arc on its left — that is, when
            // the walk runs counterclockwise.
            outward: turn.sweep > 0.0,
        }
    }

    /// The frame a circle of the drawing turns in, carried into the world.
    ///
    /// The plane's own axes, so an angle here and an angle in the drawing are
    /// the same number — which is what lets a wall's parameters be read
    /// straight off the arc that swept it.
    fn turning(raising: Raising, center: DVec2) -> Axis {
        Axis::new(raising.plane.point(center), raising.normal, raising.plane.x)
    }

    /// Every vertex, and every edge between them.
    fn raise_edges(&mut self, raising: Raising, into: &mut Body) {
        let corners = self.strips.corners().len();
        self.corners.clear();
        self.corners.resize(corners, None);
        self.climbing.clear();
        self.climbing.resize(corners, None);
        self.spans.clear();
        for at in 0..self.strips.all().len() {
            self.along_strip(raising, at, into);
        }
        for loop_ in 0..self.strips.loops() {
            let run = self.strips.run(loop_);
            for at in run.clone() {
                // The corner between this strip and the next around the loop is
                // where the two walls meeting there climb.
                let next = if at + 1 == run.end { run.start } else { at + 1 };
                let corner = self.strips.all()[at].to;
                self.climb(raising, corner, [self.walls[at], self.walls[next]], into);
            }
        }
    }

    /// The base and far edges one strip sweeps, and the vertices at their ends.
    fn along_strip(&mut self, raising: Raising, at: usize, into: &mut Body) {
        let strip = self.strips.all()[at];
        let wall = self.walls[at];
        self.corner(raising, strip.from, into);
        self.corner(raising, strip.to, into);
        let base = self.span(raising, strip, false, [raising.base, wall], into);
        let far = self.span(raising, strip, true, [raising.far, wall], into);
        self.spans.push([base, far]);
    }

    /// One of the two edges a strip sweeps, at the base or at the far end.
    fn span(
        &mut self,
        raising: Raising,
        strip: Strip,
        far: bool,
        between: [FaceId; 2],
        into: &mut Body,
    ) -> EdgeId {
        let lift = if far { raising.distance } else { 0.0 };
        let Running { curve, bounds } = self.running(raising, strip, lift);
        let ends = usize::from(far);
        let [from, to] = [strip.from, strip.to]
            .map(|corner| self.corners[corner].expect("every corner of a strip is raised")[ends]);
        into.topology_mut().add_edge(Edge {
            curve,
            bounds,
            from,
            to,
            between,
            // A cap and a wall never lie on one surface — a wall runs along the
            // normal the cap is square to — so an edge between them is always a
            // crease.
            artificial: false,
            tolerance: EXACT,
        })
    }

    /// The curve a strip runs along, `lift` above the base plane.
    fn running(&self, raising: Raising, strip: Strip, lift: f64) -> Running {
        let rise = raising.normal * lift;
        let Some(turn) = strip.turn else {
            let origin = self.base_at(raising, strip.from) + rise;
            let to = self.base_at(raising, strip.to) + rise;
            return Running {
                curve: Curve::Line(Line {
                    origin,
                    direction: (to - origin).normalize(),
                }),
                bounds: [0.0, origin.distance(to)],
            };
        };
        let mut axis = Self::turning(raising, turn.center);
        axis.origin += rise;
        Running {
            curve: Curve::Circle(Circle {
                axis,
                radius: turn.radius,
            }),
            bounds: [turn.start, turn.end()],
        }
    }

    /// The edge climbing from `corner`, between the two walls that meet there.
    fn climb(&mut self, raising: Raising, corner: usize, between: [FaceId; 2], into: &mut Body) {
        debug_assert!(
            self.climbing[corner].is_none(),
            "corner {corner} is the end of two strips of one loop",
        );
        let [from, to] = self.corners[corner].expect("every corner of a strip is raised");
        let origin = self.base_at(raising, corner);
        // No crease where the two walls lie on one surface: either side of a
        // split cylinder, two arcs of one circle the drawing was cut between,
        // or two segments drawn straight through a corner. A box's upright
        // corner is not that, and is not flagged.
        //
        // Asked of the *surfaces* rather than of their descriptions, because the
        // last of those three is two planes that are one plane and not the same
        // `Plane` — different origins, one surface.
        let topology = into.topology();
        let [one, two] = between.map(|face| topology.face(face).surface);
        let smooth = Meeting::of(&one, &two) == Meeting::Same;
        let climbing = into.topology_mut().add_edge(Edge {
            curve: Curve::Line(Line {
                origin,
                direction: raising.normal,
            }),
            bounds: [0.0, raising.distance],
            from,
            to,
            between,
            artificial: smooth,
            tolerance: EXACT,
        });
        self.climbing[corner] = Some(climbing);
    }

    /// Raise both vertices at `corner`, unless something already has.
    fn corner(&mut self, raising: Raising, corner: usize, into: &mut Body) {
        if self.corners[corner].is_some() {
            return;
        }
        let base = self.base_at(raising, corner);
        // **What the drawing knew this corner to**, which is nought wherever
        // nothing folded into it — see
        // [`Arrangement::reached`](crate::Arrangement). The ceiling on
        // `.notes/KERNEL.md` §4.1's claim is per corner rather than a blanket
        // over the body: a drawing whose curves meet where they were drawn
        // raises vertices that are exact.
        let tolerance = self.strips.reached()[corner];
        let raised = [base, base + raising.normal * raising.distance]
            .map(|at| into.topology_mut().add_vertex(Vertex { at, tolerance }));
        self.corners[corner] = Some(raised);
    }

    /// Where a corner of the drawing stands in the world.
    fn base_at(&self, raising: Raising, corner: usize) -> DVec3 {
        raising.plane.point(self.strips.corners()[corner])
    }

    /// Write the loops of every face.
    ///
    /// Each is wound counterclockwise about its own face's outward normal, an
    /// outline and its holes opposite ways round. That one rule is what makes
    /// every edge come out walked once each way — which is exactly what
    /// `Body::check` asks, so getting it wrong is loud rather than subtle.
    ///
    /// A face at a time rather than a loop at a time, because a face keeps the
    /// *stretch* of the body's buffer its loops occupy, and a stretch has to be
    /// contiguous.
    fn write_loops(&mut self, raising: Raising, into: &mut Body) {
        self.cap_loops(raising, false, into);
        self.cap_loops(raising, true, into);
        for at in 0..self.walls.len() {
            self.wall_loop(raising, at, into);
        }
    }

    /// Every loop of one cap: the region's outline, then each of its holes.
    ///
    /// The two caps face opposite ways, so exactly one of them walks its strips
    /// the way the drawing did — and which one turns over with the sign of the
    /// distance, because that is what decides which end of the solid is on
    /// which side of its plane.
    fn cap_loops(&mut self, raising: Raising, far: bool, into: &mut Body) {
        let forward = far == raising.along;
        let from = into.topology().loops_added();
        for loop_ in 0..self.strips.loops() {
            let run = self.strips.run(loop_);
            let spans = &self.spans;
            into.topology_mut().add_loop(|walk| {
                // The buffer a loop is written into holds every other loop of
                // the body as well, so a reversal reaches only what this one
                // just put in it.
                let from = walk.len();
                walk.extend(run.map(|at| Coedge {
                    edge: spans[at][usize::from(far)],
                    forward,
                }));
                if !forward {
                    walk[from..].reverse();
                }
            });
        }
        let to = into.topology().loops_added();
        let face = if far { raising.far } else { raising.base };
        into.topology_mut().face_mut(face).loops = from..to;
    }

    /// The one loop of one wall: along the base, up, back along the far end,
    /// and down.
    fn wall_loop(&mut self, raising: Raising, at: usize, into: &mut Body) {
        let strip = self.strips.all()[at];
        let [base, far] = self.spans[at];
        let climbing = [strip.to, strip.from]
            .map(|corner| self.climbing[corner].expect("every corner of a strip climbs"));
        let along = raising.along;
        let from = into.topology_mut().add_loop(|walk| {
            let wrote = walk.len();
            walk.extend([
                Coedge {
                    edge: base,
                    forward: true,
                },
                Coedge {
                    edge: climbing[0],
                    forward: true,
                },
                Coedge {
                    edge: far,
                    forward: false,
                },
                Coedge {
                    edge: climbing[1],
                    forward: false,
                },
            ]);
            if !along {
                // Grown the other way, the same four edges are walked the other
                // way round — which is what keeps the loop counterclockwise
                // about a normal that has not moved. Only this loop's own four,
                // which is what the buffer is indexed from.
                walk[wrote..].reverse();
                for coedge in &mut walk[wrote..] {
                    *coedge = coedge.turned();
                }
            }
        });
        into.topology_mut().face_mut(self.walls[at]).loops = from..from + 1;
    }

    /// Gather every face into the one shell around the one lump.
    fn gather(&mut self, raising: Raising, into: &mut Body) {
        let topology = into.topology_mut();
        let from = topology.faces_shelled();
        for face in [raising.base, raising.far] {
            topology.add_shelled(face);
        }
        for &wall in &self.walls {
            topology.add_shelled(wall);
        }
        let to = topology.faces_shelled();
        let shell = topology.add_shell(Shell { faces: from..to });
        topology.add_lump(Lump {
            outer: shell,
            voids: 0..0,
        });
    }
}

/// Raising a body without keeping the room to raise the next one in.
///
/// Everything that draws a solid holds a [`Builder`] and hands it a body to
/// refill, because a drag rebuilds one on every frame. Neither a test nor a
/// bench fixture raised once outside every window wants either, and saying so
/// once here keeps the published surface to what the application actually
/// calls — see `.notes/KERNEL.md` §6.
#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::solid::build::builder::{Builder, Extrusion};
    use crate::solid::topology::body::Body;

    impl Extrusion<'_> {
        /// Build it into a body of its own.
        pub fn body(&self) -> Body {
            let mut body = Body::default();
            Builder::default().extrude(self, &mut body);
            body
        }
    }
}
