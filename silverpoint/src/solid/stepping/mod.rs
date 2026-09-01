//! A body written out as ISO 10303-21, the exchange file every kernel reads.
//!
//! **What §4.1's exactness was for.** A face here carries an analytic surface
//! and a boundary of analytic curves, which is what STEP asks for — so what
//! goes out is the surface itself rather than a mesh of it or a spline fitted
//! to it. A cylinder leaves as a cylinder, and a reader on the other side gets
//! the drawing rather than a rendering of it.
//!
//! **And it says where it stops.** A curve this kernel *walked* rather than
//! wrote down has no analytic entity to be, and a spline fitted to it here
//! would be the approximation §1 promises nothing downstream inherits. So a
//! body carrying one is refused, and the caller is told rather than handed a
//! file that quietly rounded — see [`Stepping::write`].
//!
//! Text into a caller's own `String`, the way every routine here writes into a
//! caller's own body: this crate knows nothing about files, and a document
//! deciding where one goes is the application's business.

use std::fmt::Write;

use glam::DVec3;

use crate::number::tolerance::PLACED;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::topology::Topology;
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::EdgeId;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::shell::ShellId;
use crate::solid::topology::vertex::VertexId;

/// How closely a reader may take two places to be one, in the file's own units.
///
/// **What the body says it strays, and never less than the machine.** An exact
/// body would honestly carry nought, and a nought here is a claim no reader
/// believes — every one of them wants a positive number to weld its vertices
/// by. So the floor is the tolerance this kernel already works to and the
/// answer is the wider of the two.
const WELD: f64 = PLACED;

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
    /// The last entity number handed out.
    last: u32,
    /// What each list of an entity gathers before the entity that names them
    /// can be written: a loop's oriented edges, a face's bounds, a shell's
    /// faces and a lump's cavities.
    ///
    /// **Held rather than built per entity**, which is the whole of what this
    /// type is for — see [`Stepping::opened`], which makes the same argument
    /// about the entities themselves.
    oriented: Vec<u32>,
    bounded: Vec<u32>,
    shelled: Vec<u32>,
    voided: Vec<u32>,
    /// Every solid of the body, which the shape representation names together.
    solids: Vec<u32>,
}

impl Stepping {
    /// Write `body` into `into` as an ISO 10303-21 file named `called`.
    ///
    /// `false`, with `into` emptied, where the body carries geometry the format
    /// has no analytic entity for: a curve this kernel marched, the quartic a
    /// general pair of quadrics meets in, or the saddle a cross drilling
    /// leaves. A refusal is an answer rather than a failure — what it says is
    /// that writing the body would mean fitting a spline to it, and
    /// `.notes/KERNEL.md` §1 promises nothing downstream inherits an
    /// approximation this kernel did not already declare.
    pub fn write(&mut self, body: &Body, called: &str, into: &mut String) -> bool {
        into.clear();
        let topology = body.topology();
        if !analytic(topology) {
            return false;
        }
        self.last = 0;
        self.corners.clear();
        self.corners.resize(topology.vertex_slots(), 0);
        self.edges.clear();
        self.edges.resize(topology.edge_slots(), 0);
        self.solids.clear();
        self.header(called, into);
        let context = self.preamble(called, body, into);
        for (_, lump) in topology.lumps() {
            let solid = self.lump(topology, lump.outer, topology.voids_of(lump), into);
            self.solids.push(solid);
        }
        self.shape(context, into);
        into.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        true
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
    fn preamble(&mut self, called: &str, body: &Body, into: &mut String) -> [u32; 2] {
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

        let slack = self.opened(into);
        into.push_str("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(");
        // What the body says it strays, floored at the tolerance this kernel
        // works to — see [`WELD`].
        real(into, body.strays().max(WELD));
        let _ = write!(into, "),#{metre},'distance_accuracy_value','')");
        shut(into);

        let frame = self.opened(into);
        let _ = write!(
            into,
            "(GEOMETRIC_REPRESENTATION_CONTEXT(3)\
             GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{slack}))\
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
        into: &mut String,
    ) -> u32 {
        let held = self.shell(topology, outer, into);
        if voids.is_empty() {
            let made = self.opened(into);
            let _ = write!(into, "MANIFOLD_SOLID_BREP('',#{held})");
            shut(into);
            return made;
        }
        self.voided.clear();
        for &shell in voids {
            let cavity = self.shell(topology, shell, into);
            // Turned over, which is what a cavity is: the same closed surface
            // read with its material on the other side.
            let turned = self.opened(into);
            let _ = write!(into, "ORIENTED_CLOSED_SHELL('',*,#{cavity},.F.)");
            shut(into);
            self.voided.push(turned);
        }
        let made = self.opened(into);
        let _ = write!(into, "BREP_WITH_VOIDS('',#{held},(");
        listed(into, &self.voided);
        into.push_str("))");
        shut(into);
        made
    }

    /// One closed shell, and every face on it.
    fn shell(&mut self, topology: &Topology, shell: ShellId, into: &mut String) -> u32 {
        let from = self.shelled.len();
        for &face in topology.faces_of(shell) {
            let made = self.face(topology, face, into);
            self.shelled.push(made);
        }
        let held = self.opened(into);
        into.push_str("CLOSED_SHELL('',(");
        listed(into, &self.shelled[from..]);
        into.push_str("))");
        shut(into);
        self.shelled.truncate(from);
        held
    }

    /// One face: its surface, its outline, and a bound per hole punched out of
    /// it.
    ///
    /// **The sense flag is the body's own.** `Face::outward` says whether the
    /// material lies where the surface's normal points, which is exactly what
    /// STEP asks an `ADVANCED_FACE` — so the two need no reconciling.
    fn face(&mut self, topology: &Topology, at: FaceId, into: &mut String) -> u32 {
        let face = topology.face(at);
        let surface = self.surface(&face.surface, into);
        let from = self.bounded.len();
        for (which, walk) in topology.loops_of(face).enumerate() {
            let held = self.walk(topology, walk, into);
            let bound = self.opened(into);
            let kind = match which {
                0 => "FACE_OUTER_BOUND",
                _ => "FACE_BOUND",
            };
            let _ = write!(into, "{kind}('',#{held},.T.)");
            shut(into);
            self.bounded.push(bound);
        }
        let made = self.opened(into);
        into.push_str("ADVANCED_FACE('',(");
        listed(into, &self.bounded[from..]);
        let _ = write!(into, "),#{surface},{})", flag(face.outward));
        shut(into);
        self.bounded.truncate(from);
        made
    }

    /// One loop of one face, as the edges it walks and which way it takes each.
    fn walk(&mut self, topology: &Topology, walk: &[Coedge], into: &mut String) -> u32 {
        let from = self.oriented.len();
        for coedge in walk {
            let edge = self.edge(topology, coedge.edge, into);
            let held = self.opened(into);
            let _ = write!(
                into,
                "ORIENTED_EDGE('',*,*,#{edge},{})",
                flag(coedge.forward)
            );
            shut(into);
            self.oriented.push(held);
        }
        let made = self.opened(into);
        into.push_str("EDGE_LOOP('',(");
        listed(into, &self.oriented[from..]);
        into.push_str("))");
        shut(into);
        self.oriented.truncate(from);
        made
    }

    /// One edge, written once however many loops walk it.
    ///
    /// **The sense flag is which way its bounds run.** An edge here runs from
    /// the first of its bounds to the second, and a curve runs the way its own
    /// parameter grows — so an edge whose bounds fall is one read against its
    /// curve, which is exactly what STEP's flag says.
    fn edge(&mut self, topology: &Topology, at: EdgeId, into: &mut String) -> u32 {
        if self.edges[at.slot()] != 0 {
            return self.edges[at.slot()];
        }
        let edge = topology.edge(at);
        let ends = [edge.from, edge.to].map(|end| self.corner(topology, end, into));
        let curve = self.curve(&edge.curve, into);
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
    fn surface(&mut self, of: &Surface, into: &mut String) -> u32 {
        match of {
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
    /// Every arm a body that reached here can hold: the three it cannot are
    /// what [`analytic`] turned away before a line was written.
    fn curve(&mut self, of: &Curve, into: &mut String) -> u32 {
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
            Curve::Saddle(_) | Curve::Marched(_) | Curve::Quartic(_) => {
                unreachable!("a body carrying a curve with no entity was turned away")
            }
        }
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

/// Whether every curve of `topology` is one the format has an entity for.
///
/// **Asked of the curves and not of the surfaces**, because the surfaces all
/// pass: STEP carries a torus as readily as a plane, so this kernel's own
/// fitted *tier* is no bar. What has no entity is a curve that was walked or
/// written as a quartic, and a body holding one is refused whole rather than
/// written with a gap in it.
fn analytic(topology: &Topology) -> bool {
    topology.edges().all(|(_, edge)| {
        !matches!(
            edge.curve,
            Curve::Saddle(_) | Curve::Marched(_) | Curve::Quartic(_)
        )
    })
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
