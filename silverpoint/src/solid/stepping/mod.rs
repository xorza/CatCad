//! A body written out as ISO 10303-21, the exchange file every kernel reads.
//!
//! **What §4.1's exactness was for.** A face here carries an analytic surface
//! and a boundary of analytic curves, which is what STEP asks for — so what
//! goes out is the surface itself rather than a mesh of it or a spline fitted
//! to it. A cylinder leaves as a cylinder, and a reader on the other side gets
//! the drawing rather than a rendering of it.
//!
//! **And nothing is refused.** A curve this kernel walked goes out through the
//! very places the march laid down, so the file says what the body says. The
//! two the format has no entity for — the quartic a general pair of quadrics
//! meets in and the saddle a cross drilling leaves — go out chorded at a
//! sagitta the *caller* names, because §1 says every flattening here is the
//! caller's.
//!
//! **What makes that honest is the declaring.** A chording costs an error the
//! body did not carry, so the file's own accuracy carries it — and a body of
//! nothing but analytic curves claims no slack it never spent. That is §1's
//! third requirement kept rather than dodged: nothing downstream inherits an
//! approximation this file did not state.
//!
//! Text into a caller's own `String`, the way every routine here writes into a
//! caller's own body: this crate knows nothing about files, and a document
//! deciding where one goes is the application's business.

use crate::number::tolerance::PLACED;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::curve::{Curve, Sampled};
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::gusset::Gusset;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::geometry::vertexed::{PATCHED, Patched};
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::EdgeId;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::shell::ShellId;
use crate::solid::topology::vertex::VertexId;
use glam::DVec3;
use std::f64::consts::TAU;
use std::fmt::Write;

/// How closely a reader may take two places to be one, in the file's own units.
///
/// **What the body says it strays, and never less than the machine.** An exact
/// body would honestly carry nought, and a nought here is a claim no reader
/// believes — every one of them wants a positive number to weld its vertices
/// by. So the floor is the tolerance this kernel already works to and the
/// answer is the wider of the two.
const WELD: f64 = PLACED;

/// How many places hold one ruling of a net.
///
/// **Two, and never more.** A ruling is a straight line, so a degree of one
/// across it says the patch itself rather than a fit of it — which is why the
/// net is chorded along the turn alone. See [`Stepping::netted`].
const RULING: usize = 2;

/// The stamp every file carries in place of the hour it was written.
///
/// **Fixed on purpose.** Two exports of one document have to be the same file
/// or the format cannot be diffed, which is the argument the document's own
/// text format makes one shelf up. Nothing downstream reads it.
const STAMPED: &str = "1970-01-01T00:00:00";

/// Writes a body out as an exchange file, keeping the room it works in.
///
/// **Held across calls**, like every other routine here: an export runs off a
/// menu rather than off a frame, but the side tables are the same shape the
/// rest of the crate keeps and cost nothing after the first body.
#[derive(Debug, Default)]
pub struct Stepping {
    /// The entity each vertex, edge and face of the body became, by slot.
    ///
    /// Written once and named many times, which is what an exchange file is: a
    /// vertex is an entity two edges point at rather than a place written twice.
    corners: Vec<u32>,
    edges: Vec<u32>,
    /// One curve's places on their way into a polyline — see
    /// [`Stepping::polylined`], which is the one thing here that lays them down.
    places: Vec<Sampled>,
    /// One ruled patch's places on their way into a net — see
    /// [`Stepping::netted`].
    net: Vec<DVec3>,
    /// Every polyline already written, and the entity each became.
    ///
    /// Only the curves with no entity of their own are here. The analytic ones
    /// are a line apiece, and telling two of them apart would cost more than
    /// writing one twice.
    laid: Vec<Laid>,
    /// The last entity number handed out.
    last: u32,
    /// What an entity's own list gathers before the entity that names it can
    /// be written: a curve's places, a loop's oriented edges, a face's bounds,
    /// a shell's faces and a lump's cavities.
    ///
    /// **One buffer and not five, because the five nest.** A shell writes its
    /// faces, each of which writes its bounds, each of which writes its edges —
    /// so every level takes the length as its mark, pushes above it and cuts
    /// back to it. A stack is what that is, and five of them would be five
    /// chances to gather into the wrong one.
    ///
    /// **Held rather than built per entity**, which is the whole of what this
    /// type is for — see [`Stepping::opened`], which makes the same argument
    /// about the entities themselves.
    gathered: Vec<u32>,
    /// Every solid of the body, which the shape representation names together.
    solids: Vec<u32>,
}

impl Stepping {
    /// Write `body` into `into` as an ISO 10303-21 file named `called`.
    ///
    /// **Every body writes.** A surface goes out as the analytic entity it is,
    /// and so does a curve wherever the format has one. The rest go out as the
    /// polyline they either already are — a curve this kernel walked, through
    /// the very places the march laid down — or are chorded to.
    ///
    /// `sagitta` is how far a chord may stand from the curve it replaces, and it
    /// is the caller's because `.notes/KERNEL.md` §1 says every flattening here
    /// is. It is spent only where a curve has no entity, and whatever it costs
    /// is declared: the file's own accuracy is the widest of what the body
    /// strays, what this chorded, and the tolerance the kernel works to.
    pub fn write(&mut self, body: &Body, called: &str, sagitta: f64, into: &mut String) {
        assert!(sagitta > 0.0, "a sagitta of {sagitta} chords nothing");
        into.clear();
        let topology = body.topology();
        self.last = 0;
        self.corners.clear();
        self.corners.resize(topology.vertex_slots(), 0);
        self.edges.clear();
        self.edges.resize(topology.edge_slots(), 0);
        self.laid.clear();
        self.solids.clear();
        self.header(called, into);
        // What the file says a reader may weld by: the widest of what the body
        // already strays and what the chording below will cost it.
        let slack = match chorded(topology) {
            true => body.strays().max(sagitta),
            false => body.strays(),
        };
        let context = self.preamble(called, slack, into);
        for (_, lump) in topology.lumps() {
            let solid = self.lump(topology, lump.outer, topology.voids_of(lump), sagitta, into);
            self.solids.push(solid);
        }
        self.shape(context, into);
        into.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    }

    /// The file's own head: what it is, what wrote it, and which schema it is
    /// to be read against.
    fn header(&mut self, called: &str, into: &mut String) {
        into.push_str("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((");
        quoted(into, called);
        into.push_str("),'2;1');\nFILE_NAME(");
        quoted(into, called);
        let _ = writeln!(into, ",'{STAMPED}',(''),(''),'silverpoint','','');");
        into.push_str(
            "FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n",
        );
    }

    /// The product a body is a shape of, and the units and the accuracy every
    /// place in the file is read against.
    ///
    /// **Boilerplate, and every reader wants it.** A file of pure geometry with
    /// no product behind it opens in nothing: what a reader looks for first is
    /// a `SHAPE_DEFINITION_REPRESENTATION`, and what that names on either side
    /// is this.
    fn preamble(&mut self, called: &str, slack: f64, into: &mut String) -> [u32; 2] {
        let context = self.opened(into);
        into.push_str("APPLICATION_CONTEXT('automotive design')");
        shut(into);

        let _ = self.opened(into);
        let _ = write!(
            into,
            "APPLICATION_PROTOCOL_DEFINITION('','automotive_design',2000,#{context})"
        );
        shut(into);

        let held = self.opened(into);
        let _ = write!(into, "PRODUCT_CONTEXT('',#{context},'mechanical')");
        shut(into);

        let product = self.opened(into);
        into.push_str("PRODUCT(");
        quoted(into, called);
        into.push(',');
        quoted(into, called);
        let _ = write!(into, ",'',(#{held}))");
        shut(into);

        let form = self.opened(into);
        let _ = write!(into, "PRODUCT_DEFINITION_FORMATION('','',#{product})");
        shut(into);

        let made = self.opened(into);
        let _ = write!(into, "PRODUCT_DEFINITION_CONTEXT('',#{context},'design')");
        shut(into);

        let defined = self.opened(into);
        let _ = write!(into, "PRODUCT_DEFINITION('','',#{form},#{made})");
        shut(into);

        let shaped = self.opened(into);
        let _ = write!(into, "PRODUCT_DEFINITION_SHAPE('','',#{defined})");
        shut(into);

        let metre = self.opened(into);
        into.push_str("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT($,.METRE.))");
        shut(into);

        let radian = self.opened(into);
        into.push_str("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))");
        shut(into);

        let solid = self.opened(into);
        into.push_str("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())");
        shut(into);

        let accuracy = self.opened(into);
        into.push_str("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(");
        // Floored at the tolerance this kernel works to — see [`WELD`].
        real(into, slack.max(WELD));
        let _ = write!(into, "),#{metre},'distance_accuracy_value','')");
        shut(into);

        let frame = self.opened(into);
        let _ = write!(
            into,
            "(GEOMETRIC_REPRESENTATION_CONTEXT(3)\
             GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{accuracy}))\
             GLOBAL_UNIT_ASSIGNED_CONTEXT((#{metre},#{radian},#{solid}))\
             REPRESENTATION_CONTEXT('',''))"
        );
        shut(into);
        [shaped, frame]
    }

    /// The one entity a reader opens: which product the shape below belongs to.
    fn shape(&mut self, context: [u32; 2], into: &mut String) {
        let [shaped, frame] = context;
        let placed = self.placement(Axis::new(DVec3::ZERO, DVec3::Z, DVec3::X), into);
        let shape = self.opened(into);
        let _ = write!(into, "ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{placed}");
        for solid in &self.solids {
            let _ = write!(into, ",#{solid}");
        }
        let _ = write!(into, "),#{frame})");
        shut(into);

        let _ = self.opened(into);
        let _ = write!(into, "SHAPE_DEFINITION_REPRESENTATION(#{shaped},#{shape})");
        shut(into);
    }

    /// One lump: its outer shell, and a cavity apiece for what it shuts in.
    fn lump(
        &mut self,
        topology: &Topology,
        outer: ShellId,
        voids: &[ShellId],
        sagitta: f64,
        into: &mut String,
    ) -> u32 {
        let held = self.shell(topology, outer, sagitta, into);
        if voids.is_empty() {
            let made = self.opened(into);
            let _ = write!(into, "MANIFOLD_SOLID_BREP('',#{held})");
            shut(into);
            return made;
        }
        let from = self.gathered.len();
        for &shell in voids {
            let cavity = self.shell(topology, shell, sagitta, into);
            // Turned over, which is what a cavity is: the same closed surface
            // read with its material on the other side.
            let turned = self.opened(into);
            let _ = write!(into, "ORIENTED_CLOSED_SHELL('',*,#{cavity},.F.)");
            shut(into);
            self.gathered.push(turned);
        }
        let made = self.opened(into);
        let _ = write!(into, "BREP_WITH_VOIDS('',#{held},(");
        listed(into, &self.gathered[from..]);
        into.push_str("))");
        shut(into);
        self.gathered.truncate(from);
        made
    }

    /// One closed shell, and every face on it.
    fn shell(
        &mut self,
        topology: &Topology,
        shell: ShellId,
        sagitta: f64,
        into: &mut String,
    ) -> u32 {
        let from = self.gathered.len();
        for &face in topology.faces_of(shell) {
            let made = self.face(topology, face, sagitta, into);
            self.gathered.push(made);
        }
        let held = self.opened(into);
        into.push_str("CLOSED_SHELL('',(");
        listed(into, &self.gathered[from..]);
        into.push_str("))");
        shut(into);
        self.gathered.truncate(from);
        held
    }

    /// One face: its surface, its outline, and a bound per hole punched out of
    /// it.
    ///
    /// **The sense flag is the body's own.** `Face::outward` says whether the
    /// material lies where the surface's normal points, which is exactly what
    /// STEP asks an `ADVANCED_FACE` — so the two need no reconciling.
    fn face(&mut self, topology: &Topology, at: FaceId, sagitta: f64, into: &mut String) -> u32 {
        let face = topology.face(at);
        let surface = self.surface(&face.surface, sagitta, into);
        let from = self.gathered.len();
        for (which, walk) in topology.loops_of(face).enumerate() {
            let held = self.walk(topology, walk, sagitta, into);
            let bound = self.opened(into);
            let kind = match which {
                0 => "FACE_OUTER_BOUND",
                _ => "FACE_BOUND",
            };
            let _ = write!(into, "{kind}('',#{held},.T.)");
            shut(into);
            self.gathered.push(bound);
        }
        let made = self.opened(into);
        into.push_str("ADVANCED_FACE('',(");
        listed(into, &self.gathered[from..]);
        let _ = write!(into, "),#{surface},{})", flag(face.outward));
        shut(into);
        self.gathered.truncate(from);
        made
    }

    /// One loop of one face, as the edges it walks and which way it takes each.
    fn walk(
        &mut self,
        topology: &Topology,
        walk: &[Coedge],
        sagitta: f64,
        into: &mut String,
    ) -> u32 {
        let from = self.gathered.len();
        for coedge in walk {
            let edge = self.edge(topology, coedge.edge, sagitta, into);
            let held = self.opened(into);
            let _ = write!(
                into,
                "ORIENTED_EDGE('',*,*,#{edge},{})",
                flag(coedge.forward)
            );
            shut(into);
            self.gathered.push(held);
        }
        let made = self.opened(into);
        into.push_str("EDGE_LOOP('',(");
        listed(into, &self.gathered[from..]);
        into.push_str("))");
        shut(into);
        self.gathered.truncate(from);
        made
    }

    /// One edge, written once however many loops walk it.
    ///
    /// **The sense flag is which way its bounds run.** An edge here runs from
    /// the first of its bounds to the second, and a curve runs the way its own
    /// parameter grows — so an edge whose bounds fall is one read against its
    /// curve, which is exactly what STEP's flag says.
    fn edge(&mut self, topology: &Topology, at: EdgeId, sagitta: f64, into: &mut String) -> u32 {
        if self.edges[at.slot()] != 0 {
            return self.edges[at.slot()];
        }
        let edge = topology.edge(at);
        let ends = [edge.from, edge.to].map(|end| self.corner(topology, end, into));
        let curve = self.curve(&edge.curve, topology.carried(), sagitta, into);
        let made = self.opened(into);
        let _ = write!(
            into,
            "EDGE_CURVE('',#{},#{},#{curve},{})",
            ends[0],
            ends[1],
            flag(edge.bounds[1] > edge.bounds[0]),
        );
        shut(into);
        self.edges[at.slot()] = made;
        made
    }

    /// One corner, written once however many edges end at it.
    fn corner(&mut self, topology: &Topology, at: VertexId, into: &mut String) -> u32 {
        if self.corners[at.slot()] != 0 {
            return self.corners[at.slot()];
        }
        let place = self.point(topology.vertex(at).at, into);
        let made = self.opened(into);
        let _ = write!(into, "VERTEX_POINT('',#{place})");
        shut(into);
        self.corners[at.slot()] = made;
        made
    }

    /// One surface, as the analytic entity it is.
    ///
    /// `sagitta` is spent on the one surface with no such entity — the ruled
    /// patch a corner two picks do not agree about is filled with, which
    /// [`Stepping::netted`] lays out.
    fn surface(&mut self, of: &Surface, sagitta: f64, into: &mut String) -> u32 {
        match of {
            Surface::Fitted(Fitted::Gusset(gusset)) => self.netted(gusset, sagitta, into),
            Surface::Fitted(Fitted::Vertexed(vertexed)) => {
                self.gridded(&vertexed.patched().expect(PATCHED), sagitta, into)
            }
            Surface::Natural(Natural::Plane(plane)) => {
                let placed = self.placement(Axis::new(plane.origin, plane.normal(), plane.x), into);
                let made = self.opened(into);
                let _ = write!(into, "PLANE('',#{placed})");
                shut(into);
                made
            }
            Surface::Natural(Natural::Cylinder(cylinder)) => {
                let placed = self.placement(cylinder.axis, into);
                self.wrote(into, "CYLINDRICAL_SURFACE", placed, &[cylinder.radius])
            }
            // **Measured at the apex**, where the radius is nought: STEP takes a
            // cone as a placement, the radius there and the half angle, and this
            // kernel already keeps the apex as the frame's own origin.
            Surface::Natural(Natural::Cone(cone)) => {
                let placed = self.placement(cone.axis, into);
                self.wrote(into, "CONICAL_SURFACE", placed, &[0.0, cone.half_angle])
            }
            Surface::Natural(Natural::Sphere(sphere)) => {
                let placed = self.placement(sphere.axis, into);
                self.wrote(into, "SPHERICAL_SURFACE", placed, &[sphere.radius])
            }
            Surface::Fitted(Fitted::Torus(torus)) => {
                let placed = self.placement(torus.axis, into);
                self.wrote(
                    into,
                    "TOROIDAL_SURFACE",
                    placed,
                    &[torus.major, torus.minor],
                )
            }
        }
    }

    /// One curve, as the analytic entity it is.
    ///
    /// **Five of the eight are entities and three are not.** A line, a circle,
    /// an ellipse, a hyperbola and a parabola each name a STEP curve outright;
    /// a saddle, a marched run and a quartic name none, and go out as the
    /// polylines [`Stepping::polylined`] lays down.
    fn curve(&mut self, of: &Curve, carried: &Carried, sagitta: f64, into: &mut String) -> u32 {
        match of {
            Curve::Line(line) => {
                let from = self.point(line.origin, into);
                let way = self.direction(line.direction, into);
                let along = self.opened(into);
                let _ = write!(into, "VECTOR('',#{way},1.0)");
                shut(into);
                let made = self.opened(into);
                let _ = write!(into, "LINE('',#{from},#{along})");
                shut(into);
                made
            }
            Curve::Circle(circle) => {
                let placed = self.placement(circle.axis, into);
                self.wrote(into, "CIRCLE", placed, &[circle.radius])
            }
            Curve::Ellipse(ellipse) => {
                let placed = self.placement(ellipse.axis, into);
                self.wrote(into, "ELLIPSE", placed, &[ellipse.major, ellipse.minor])
            }
            Curve::Hyperbola(of) => {
                let placed = self.placement(of.axis, into);
                self.wrote(into, "HYPERBOLA", placed, &[of.major, of.minor])
            }
            Curve::Parabola(of) => {
                let placed = self.placement(of.axis, into);
                self.wrote(into, "PARABOLA", placed, &[of.focal])
            }
            Curve::Marched(_) | Curve::Saddle(_) | Curve::Quartic(_) => {
                self.polylined(of, sagitta, carried, into)
            }
        }
    }

    /// A ruled patch, as the net of chords the format has no entity for.
    ///
    /// **Degree one each way, and one of the two is exact.** Every ruling is a
    /// straight line already, so a degree of one across it says the patch
    /// itself; the turn along it is chorded, and a smoother fit there would
    /// read better and claim more. That is the argument
    /// [`Stepping::polylined`] makes one dimension down.
    ///
    /// **The last ruling has closed to a point**, the patch shutting at the
    /// place its two blends touch — so the net's last row is one place written
    /// twice, which is the degenerate row a conical face already hands every
    /// reader.
    ///
    /// Written afresh per face rather than filed and named again: two faces on
    /// one patch would want one net only if they were also asked for at one
    /// sagitta, and a body holds a handful of these.
    fn netted(&mut self, of: &Gusset, sagitta: f64, into: &mut String) -> u32 {
        let from = self.walked(
            |net| {
                of.netted(sagitta, net);
            },
            into,
        );
        self.splined(from, RULING, "RULED_SURF", into)
    }

    /// One corner patch, as the net it goes out as.
    ///
    /// **A grid where a ruled patch is two rulings**, the corner patch bending
    /// both ways — see [`Patched::netted`], which lays it out to the sagitta
    /// asked for.
    fn gridded(&mut self, of: &Patched, sagitta: f64, into: &mut String) -> u32 {
        let mut across = 0;
        let from = self.walked(
            |net| {
                across = of.netted(sagitta, net);
            },
            into,
        );
        self.splined(from, across, "UNSPECIFIED", into)
    }

    /// Walk a surface into this type's own room and write the places out,
    /// answering where in `gathered` they start.
    ///
    /// **Taken out and put back**, so the walk writes into that room while the
    /// writing holds the rest of this type.
    fn walked(&mut self, walk: impl FnOnce(&mut Vec<DVec3>), into: &mut String) -> usize {
        let mut net = std::mem::take(&mut self.net);
        net.clear();
        walk(&mut net);
        let from = self.gathered.len();
        for &at in &net {
            let place = self.point(at, into);
            self.gathered.push(place);
        }
        self.net = net;
        from
    }

    /// The places gathered from `from` on, as the bilinear B-spline surface
    /// they go out as, `across` of them to the row.
    ///
    /// **Degree one either way, which is what makes it honest**: the entity
    /// claims exactly what was laid down and no more.
    fn splined(&mut self, from: usize, across: usize, form: &str, into: &mut String) -> u32 {
        let held = self.gathered.len() - from;
        debug_assert_eq!(held % across, 0, "a row of the net lost an end");
        let down = held / across;
        // Two rows and two columns at the very least, or the multiplicities
        // below spell a knot vector the places do not fill.
        debug_assert!(
            down > 1 && across > 1,
            "a net of {down} by {across} spans nothing",
        );
        let made = self.opened(into);
        into.push_str("B_SPLINE_SURFACE_WITH_KNOTS('',1,1,(");
        for (at, row) in self.gathered[from..].chunks(across).enumerate() {
            if at > 0 {
                into.push(',');
            }
            into.push('(');
            listed(into, row);
            into.push(')');
        }
        into.push_str("),.");
        into.push_str(form);
        into.push_str(".,.F.,.F.,.U.,");
        doubled(into, down);
        into.push(',');
        doubled(into, across);
        into.push(',');
        counted(into, down);
        into.push(',');
        counted(into, across);
        into.push_str(",.UNSPECIFIED.)");
        shut(into);
        self.gathered.truncate(from);
        made
    }

    /// One curve with no entity of its own, as the polyline it goes out as.
    ///
    /// **Two readings and one routine**, which is what [`Curve::sample`] already
    /// keeps apart. A curve this kernel *walked* is a run of chords laid to a
    /// sagitta the body declares, and it hands its own places back — so what
    /// goes out is what the march laid down, and the file says what the body
    /// says. A curve written down exactly and having no entity — the quartic a
    /// general pair of quadrics meets in, and the saddle a cross drilling
    /// leaves — is chorded to the sagitta the caller named, which costs an error
    /// the body did not carry. See [`Stepping::write`], where that is declared.
    ///
    /// **Degree one either way**, and that is what makes it honest: a smoother
    /// fit would read better and claim more, where this claims exactly what was
    /// laid down.
    ///
    /// **The whole turn**, so a curve that closes goes out whole and the
    /// vertices trim it, exactly as a circle does. One that does not comes back
    /// from its own two ends, and the entity says which it is.
    ///
    /// **Written once and named by every edge on it.** A curve is cut into
    /// several edges wherever the faces it divides are split — §4.4 — and a
    /// polyline of hundreds of places repeated per edge would be a file many
    /// times the size for nothing. Scanned rather than filed under a handle,
    /// because a saddle has none: a body holds a handful of these, and a walk
    /// over that handful beats a key over every curve in it.
    fn polylined(&mut self, of: &Curve, sagitta: f64, carried: &Carried, into: &mut String) -> u32 {
        if let Some(laid) = self.laid.iter().find(|laid| laid.of == *of) {
            return laid.entity;
        }
        // Taken out and put back, so the sampling writes into this type's own
        // room while the writing below holds the rest of it.
        let mut places = std::mem::take(&mut self.places);
        places.clear();
        of.sample(TAU, sagitta, carried, &mut places);
        let from = self.gathered.len();
        for sampled in &places {
            let place = self.point(sampled.at, into);
            self.gathered.push(place);
        }
        self.places = places;
        let entity = self.polyline(from, of.closed(), into);
        self.laid.push(Laid { of: *of, entity });
        entity
    }

    /// The places gathered from `from` on, as a polyline through every one.
    ///
    /// **Degree one, its ends doubled** — the one knot vector a degree of one
    /// takes, over knots that are the places counted off. `closed` is the
    /// curve's own answer: a run that comes back lays its first place down
    /// again at the end, and one that does not stops at its second corner.
    fn polyline(&mut self, from: usize, closed: bool, into: &mut String) -> u32 {
        let held = self.gathered.len() - from;
        // Two places would give a knot vector the multiplicities below do not
        // spell.
        debug_assert!(held > 1, "a run of {held} places has no chord");
        let made = self.opened(into);
        into.push_str("B_SPLINE_CURVE_WITH_KNOTS('',1,(");
        listed(into, &self.gathered[from..]);
        let _ = write!(into, "),.POLYLINE_FORM.,{},.U.,", flag(closed));
        doubled(into, held);
        into.push(',');
        counted(into, held);
        into.push_str(",.UNSPECIFIED.)");
        shut(into);
        self.gathered.truncate(from);
        made
    }

    /// An entity that is a placement and a run of numbers, which most of the
    /// analytic ones are.
    fn wrote(&mut self, into: &mut String, kind: &str, placed: u32, numbers: &[f64]) -> u32 {
        let made = self.opened(into);
        let _ = write!(into, "{kind}('',#{placed}");
        for &number in numbers {
            into.push(',');
            real(into, number);
        }
        into.push(')');
        shut(into);
        made
    }

    /// A frame: where it stands, which way it points, and where its angles
    /// begin.
    fn placement(&mut self, axis: Axis, into: &mut String) -> u32 {
        let at = self.point(axis.origin, into);
        let up = self.direction(axis.direction, into);
        let from = self.direction(axis.reference, into);
        let made = self.opened(into);
        let _ = write!(into, "AXIS2_PLACEMENT_3D('',#{at},#{up},#{from})");
        shut(into);
        made
    }

    fn point(&mut self, at: DVec3, into: &mut String) -> u32 {
        let made = self.opened(into);
        into.push_str("CARTESIAN_POINT('',(");
        triple(into, at);
        into.push_str("))");
        shut(into);
        made
    }

    fn direction(&mut self, way: DVec3, into: &mut String) -> u32 {
        let made = self.opened(into);
        into.push_str("DIRECTION('',(");
        triple(into, way);
        into.push_str("))");
        shut(into);
        made
    }

    /// Open a new entity and hand back the number it took, leaving the caller
    /// to write its body.
    ///
    /// **Straight into the caller's string**, which is what keeps a file of a
    /// hundred thousand entities off the heap: an entity built as a `String`
    /// first and pushed after would be an allocation apiece, on a walk that
    /// visits every vertex, edge and face of a body.
    fn opened(&mut self, into: &mut String) -> u32 {
        self.last += 1;
        let _ = write!(into, "#{} = ", self.last);
        self.last
    }
}

/// Close the entity being written.
fn shut(into: &mut String) {
    into.push_str(";\n");
}

/// The multiplicities a degree of one takes over `held` places: the ends
/// doubled, and everything between single.
///
/// **One spelling rather than two.** A polyline and a net ask for the same run
/// — see [`Stepping::polyline`] and [`Stepping::netted`] — and two spellings of
/// one relation is how they come to disagree.
fn doubled(into: &mut String, held: usize) {
    into.push_str("(2");
    for _ in 2..held {
        into.push_str(",1");
    }
    into.push_str(",2)");
}

/// The knots those multiplicities count off, which are the places numbered from
/// nought.
fn counted(into: &mut String, held: usize) {
    into.push('(');
    for at in 0..held {
        if at > 0 {
            into.push(',');
        }
        real(into, at as f64);
    }
    into.push(')');
}

/// A run of entity numbers, comma separated.
fn listed(into: &mut String, of: &[u32]) {
    for (at, number) in of.iter().enumerate() {
        if at > 0 {
            into.push(',');
        }
        let _ = write!(into, "#{number}");
    }
}

/// A string as the format spells one.
///
/// **Every quote doubled**, which is Part 21's own escape and the only one a
/// name can trip: a document called `Bob's bracket` would otherwise close the
/// string a word early and leave a file nothing reads.
fn quoted(into: &mut String, of: &str) {
    into.push('\'');
    for held in of.chars() {
        if held == '\'' {
            into.push('\'');
        }
        into.push(held);
    }
    into.push('\'');
}

/// One polyline already written, and the entity it became.
#[derive(Debug)]
struct Laid {
    of: Curve,
    entity: u32,
}

/// Whether anything in `topology` has to be chorded to be written.
///
/// **Asked before a line goes down**, because what it decides is the accuracy
/// the file's own head declares — and a body of nothing but analytic geometry
/// and walked runs must not claim a slack it never spent.
///
/// The two curves that cost one are the quartic a general pair of quadrics
/// meets in and the saddle a cross drilling leaves. The one surface that costs
/// one is the ruled patch a corner two picks do not agree about is filled with
/// — see [`Stepping::netted`]. All three are written down *exactly* here, so
/// what a chording costs is an error the body did not carry until this made
/// it, which is why the file has to say so. Everything else either has an
/// entity or was already walked to a bound of its own.
fn chorded(topology: &Topology) -> bool {
    topology
        .edges()
        .any(|(_, edge)| matches!(edge.curve, Curve::Saddle(_) | Curve::Quartic(_)))
        || topology
            .faces()
            .any(|(_, face)| matches!(face.surface, Surface::Fitted(Fitted::Gusset(_))))
}

/// A boolean as the format spells one.
fn flag(of: bool) -> &'static str {
    match of {
        true => ".T.",
        false => ".F.",
    }
}

/// Three reals, comma separated.
fn triple(into: &mut String, at: DVec3) {
    for (which, number) in [at.x, at.y, at.z].into_iter().enumerate() {
        if which > 0 {
            into.push(',');
        }
        real(into, number);
    }
}

/// One real, in the form Part 21 reads.
///
/// **Every real carries a decimal point**, which is the whole of the format's
/// rule and the one thing Rust's own shortest-round-trip spelling drops: a
/// magnitude small enough to be written with an exponent comes out `1e-7`,
/// where the format wants `1.0E-7`. Written straight into the caller's string
/// and mended in place, so a file of a hundred thousand numbers asks the heap
/// for nothing.
fn real(into: &mut String, of: f64) {
    let from = into.len();
    let _ = write!(into, "{of:?}");
    let Some(at) = into[from..].find('e') else {
        return;
    };
    let at = from + at;
    into.replace_range(at..at + 1, "E");
    if !into[from..at].contains('.') {
        into.insert_str(at, ".0");
    }
}

#[cfg(test)]
mod tests;
