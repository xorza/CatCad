//! A region of a drawing, spun about a line in its own plane.

use crate::math::plane::Plane;
use crate::number::predicate;
use crate::number::tolerance::{EXACT, PLACED};
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::strip::{Strip, Strips, Turn};
use crate::solid::build::{Running, Walled, shelled};
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
use crate::solid::named::{Named, Step};
use crate::solid::topology::body::Body;
use crate::solid::topology::coedge::Coedge;
use crate::solid::topology::edge::{Edge, EdgeId};
use crate::solid::topology::face::{Face, FaceId};
use crate::solid::topology::lump::Lump;
use crate::solid::topology::vertex::{Vertex, VertexId};
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};
use std::ops::Range;

/// The regions of a drawing and the line in their own plane they are spun
/// about.
///
/// Borrowed and [`Copy`], like [`Extrusion`](super::builder::Extrusion) beside
/// it and for the same reason: what it holds is an arrangement somebody else
/// owns, some positions in it, and a frame.
///
/// **Several regions and one solid**, on the terms the extrusion states: they
/// are faces of one arrangement and so are disjoint, and each raises lumps of
/// its own in the one body.
///
/// Being disjoint in the *drawing* is not enough here, which the extrusion does
/// not have to ask: on one side of the line a region maps to a radius and a
/// height without folding, so two of them stay apart — but a region mirrored
/// across the line sweeps the very same space. So every region has to stand on
/// the same side, which is the refusal a region *straddling* the line already
/// gets, asked of the profile instead.
///
/// **A whole turn and any part of one.** Spun the whole way a region has no
/// ends and every wall closes on itself, which is what a ring, a washer and a
/// ball are; spun part way it has two, and those are caps of the same kind an
/// extrusion raises. Which of the two it is follows from the [`Sector`] rather
/// than being asked for.
#[derive(Debug, Clone, Copy)]
pub struct Revolution<'a> {
    of: &'a Arrangement,
    at: &'a [usize],
    plane: Plane,
    /// Somewhere on the line, in the plane's own coordinates.
    axis: DVec2,
    /// The way that line runs, in the same coordinates. Need not be unit.
    along: DVec2,
    /// How much of a turn, and where it starts.
    sector: Sector,
    /// Which of the caller's steps this is, so the faces it raises say which
    /// feature grew them and not merely what of it they are.
    by: Step,
}

impl<'a> Revolution<'a> {
    /// The regions at `at` in `of`, spun through `sector` about the line
    /// through `axis` running `along`, both in `plane`'s own coordinates.
    ///
    /// Every one of `at` has to be one of `of`'s faces, and `plane` the one the
    /// drawing they came from lies on — a face names its edges by where they
    /// fall in the arrangement that cut them, so neither travels to another.
    pub fn new(
        of: &'a Arrangement,
        at: &'a [usize],
        plane: Plane,
        axis: DVec2,
        along: DVec2,
        sector: Sector,
        by: Step,
    ) -> Self {
        Self {
            of,
            at,
            plane,
            axis,
            along,
            sector,
            by,
        }
    }
}

/// How much of a turn a revolve sweeps, and where round the line it starts.
///
/// Both in radians, about the line's own direction and right-handed about it —
/// so which way it goes is the sign of the one number rather than a second
/// field, on the terms [`Extrusion`](super::builder::Extrusion) states for a
/// signed distance.
///
/// **An angle of nought is where the drawing itself stands.** The frame a
/// revolve spins in is built with the region at nought, so a sector starting
/// there puts one seam on the profile as it was drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sector {
    pub from: f64,
    /// Signed, and at most a whole turn: more than one would sweep the same
    /// space twice, which is not a solid.
    pub sweep: f64,
}

impl Sector {
    /// The whole way round from the drawing's own place.
    pub const WHOLE: Self = Self {
        from: 0.0,
        sweep: TAU,
    };
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
    /// How many faces each wall is cut into.
    ///
    /// **Every part spans at most a third of a turn**, which is the one rule —
    /// see [`MOST`], where it is argued. So a whole turn is three, a half turn
    /// two, and anything up to a third of one is a single face.
    parts: usize,
    /// Whether the turn closes on itself, so the seam past the last part is the
    /// first one again and there are no caps.
    closed: bool,
    /// Where the turn starts and how far it goes, in the line's own angle.
    from: f64,
    sweep: f64,
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
    ///
    /// **A place within [`PLACED`] of the line is *on* it**, and comes back at
    /// nought exactly. Two reasons, and the snap answers both: a corner there
    /// sweeps a point rather than a circle — see [`Revolving::corner`] — and
    /// the two ends of that turn have to be one place rather than two a
    /// rounding apart. A profile drawn to touch the line does so through the
    /// solver, which lands it within a place and not on the number.
    fn profile(self, at: DVec2) -> DVec2 {
        let out = at - self.at;
        let off = self.out.dot(out);
        let off = if predicate::touching(off.abs(), PLACED) {
            0.0
        } else {
            off
        };
        DVec2::new(self.along.dot(out), off)
    }

    /// Whether the corner at `at` stands on the line, so that what it sweeps is
    /// a point.
    fn poles(self, at: DVec2) -> bool {
        self.profile(at).y == 0.0
    }

    /// Where the wall parts at `part`, which is `part` steps of an equal cut
    /// round the sector.
    ///
    /// Reaches one past the parts, that being the seam the last of them ends
    /// at — see [`each_seam`].
    fn seamed(self, part: usize) -> f64 {
        self.from + self.sweep * part as f64 / self.parts as f64
    }

    /// The same of a direction, which the line's own place does not move.
    fn turned(self, way: DVec2) -> DVec2 {
        DVec2::new(self.along.dot(way), self.out.dot(way))
    }

    /// Where the profile's `(along, out)` lands in the world at a spin of `u`.
    fn spun(self, at: DVec2, u: f64) -> DVec3 {
        self.axis.origin + self.axis.direction * at.x + self.axis.radial(u) * at.y
    }

    /// Which seam of the turn one of its two ends stands at.
    fn ended(self, far: bool) -> usize {
        if far { self.parts } else { 0 }
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

    /// Whether `turn` keeps to one side of the line along the whole of it,
    /// with `ends` the two places it runs between.
    ///
    /// **Asked of the arc and not only of its ends**, which a straight run
    /// needs and an arc does not: an arc bulges away from its own chord, so one
    /// drawn from a place beside the line round to another can cross it in
    /// between and come back. What it sweeps there is a surface folded through
    /// itself.
    ///
    /// How far out an arc stands is a cosine of the angle round it, so the
    /// least of it is a radius inside the centre where the sweep reaches
    /// straight back at the line, and an end everywhere else.
    ///
    /// **An end *on* the line is a pole and is kept**, which is what a
    /// semicircle spun about its own diameter is — the commonest ball there is.
    /// An *interior* place on the line is not: the arc reaches the line and
    /// leaves it again, so what it sweeps folds through itself. The ends come
    /// in already snapped, a place within [`PLACED`] of the line being on it —
    /// see [`Spinning::profile`].
    fn clears(self, turn: Turn, ends: [DVec2; 2]) -> bool {
        let middle = self.profile(turn.center);
        let back = (self.out.to_angle() + PI - turn.start) * turn.sweep.signum();
        if back.rem_euclid(TAU) <= turn.sweep.abs() {
            return middle.y - turn.radius > 0.0;
        }
        ends[0].y >= 0.0 && ends[1].y >= 0.0
    }
}

/// Spins bodies, keeping the room it works in.
///
/// Held across calls for the reason [`Builder`](super::builder::Builder) is:
/// a solid is rebuilt on every frame of a drag through the drawing under it,
/// and comes out the shape it was last time.
#[derive(Debug, Default)]
pub(super) struct Revolving {
    /// The vertex each corner sweeps at every seam of the turn, or `None`
    /// where no strip of this region reaches that corner.
    ///
    /// One entry per seam of the turn, which is one more than the parts — see
    /// [`each_seam`].
    ///
    /// A corner *on* the line sweeps one place rather than a seam's worth of
    /// them, and holds that one vertex in every slot: every reader wants a slot
    /// per seam, and the slots being equal is what a pole is.
    corners: Vec<Option<[VertexId; MOST + 1]>>,
    /// The parts of the circle each corner sweeps, in step — and `None` for a
    /// corner on the line, which sweeps no circle at all.
    circling: Vec<Option<[EdgeId; MOST]>>,
    /// Whether a seam ends at each corner, which is the only thing a corner
    /// *on* the line is ever wanted for — see [`Revolving::corner`].
    seamed: Vec<bool>,
    /// The faces raised off each strip, in step with [`Strips::all`] — and
    /// `None` for a strip lying *on* the line, which sweeps nothing.
    walls: Vec<Option<Walls>>,
    /// One copy of each strip's own curve at every seam, in step with the
    /// walls — and `None` where the wall is and the turn closes.
    ///
    /// A strip lying *on* the line raises no wall and still has one of these
    /// where the turn is cut: the line itself, which is a side of both caps.
    seams: Vec<Option<[EdgeId; MOST + 1]>>,
    /// The two ends of a partial turn, and `None` where the turn closes on
    /// itself and has none.
    ///
    /// One pair for the region in hand rather than a list, the passes running
    /// region by region — see [`Revolving::raise`].
    caps: Option<[FaceId; 2]>,
}

impl Revolving {
    /// Spin `of` into `into`, emptying whatever was there.
    ///
    /// **A body with no faces where it cannot be spun**, which is the answer
    /// an extrusion of no distance gives and means the same thing: there is no
    /// solid, so there is nothing to draw, to pick or to build on. Five things
    /// have no solid — a line with no direction, a region that crosses the
    /// line, an arc that reaches it, which would sweep a surface folded through
    /// itself, a sweep of nothing, and a sweep of more than a whole turn, which
    /// would sweep the same space twice. A region merely *touching* the line
    /// has a solid, and what it touches with is a pole.
    ///
    /// **One region that cannot be spun takes the whole body with it**, rather
    /// than leaving the others standing: what a profile names is one solid, and
    /// half of one is not a smaller answer but a wrong one.
    ///
    /// **And every region has to stand on the same side of the line.** Two
    /// regions of one drawing cannot overlap, and on one side of the line the
    /// map to a radius and a height keeps them apart — but a region mirrored
    /// across the line sweeps the very same space, and two lumps sharing space
    /// is not a solid. It is the same refusal a region *straddling* the line
    /// gets, asked of the profile rather than of one region.
    ///
    /// Four passes per region, and the order is forced for the reason the
    /// extrusion's is: an edge names the two faces that use it, so every face
    /// has to exist before any edge is made, and a face's loops cannot be
    /// written until its edges are.
    pub(super) fn raise(&mut self, of: &Revolution<'_>, strips: &mut Strips, into: &mut Body) {
        into.clear();
        let mut side = None;
        for &at in of.at {
            strips.lay(of.of, at);
            let Some(spinning) = Self::framed(of, strips) else {
                into.clear();
                return;
            };
            // Exactly, both being the same perpendicular times a sign.
            if side.is_some_and(|had| had != spinning.out) {
                into.clear();
                return;
            }
            side = Some(spinning.out);
            if !self.raise_walls(spinning, strips, into) {
                into.clear();
                return;
            }
            self.raise_edges(spinning, strips, into);
            self.write_loops(spinning, strips, into);
            self.gather(strips, into);
        }
    }

    /// The frame the region is spun in, or `None` where it cannot be.
    ///
    /// **Which side of the line the region stands on is read rather than
    /// given**, so a caller need not say — and reading it is the same walk that
    /// refuses a region straddling the line. A place *on* the line says nothing
    /// about which side that is, and is passed over: what it sweeps is a pole.
    ///
    /// **Three places per strip: its two ends and its middle.** The ends alone
    /// answer for a straight run and not for an arc — one drawn from the line
    /// round to the line again has both of them on it and says nothing, where
    /// the bulge it stands off with says everything. The middle alone answers
    /// for neither: a run that *crosses* the line has its two ends either way
    /// of it and its middle on it, which is the case this refuses.
    fn framed(of: &Revolution<'_>, strips: &Strips) -> Option<Spinning> {
        let along = of.along.try_normalize()?;
        let perp = along.perp();
        // The side the region stands on, off the first place clear of the
        // line — and every place after it has to agree.
        let mut side = 0.0_f64;
        for strip in strips.all() {
            let [from, to] = [strip.from, strip.to].map(|corner| strips.corners()[corner]);
            for at in [from, to, laid(strips, *strip, 0.5).at] {
                let off = perp.dot(at - of.axis);
                if predicate::touching(off.abs(), PLACED) {
                    continue;
                }
                if side == 0.0 {
                    side = off.signum();
                } else if side != off.signum() {
                    return None;
                }
            }
        }
        // Every corner on the line, which is a region with no width to spin —
        // and a region with no boundary at all, which stands nowhere.
        if side == 0.0 {
            return None;
        }
        // More than a whole turn sweeps the same space twice, and nothing at
        // all sweeps no space — neither is a solid.
        let Sector { from, sweep } = of.sector;
        if !(sweep.abs() > 0.0 && sweep.abs() <= TAU) {
            return None;
        }
        // Clamped rather than trusted to the arithmetic: a whole turn divided
        // by a third of one is three, and a rounding either side of it would be
        // a fourth face nothing needs or a wall left wrapping.
        let parts = ((sweep.abs() / (TAU / MOST as f64)).ceil() as usize).clamp(1, MOST);
        let out = perp * side;
        Some(Spinning {
            plane: of.plane,
            by: of.by,
            parts,
            closed: sweep.abs() == TAU,
            from,
            sweep,
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
            // line, and a spin the other way round reverses it again — so a
            // negative sweep is the same solid turned the other way rather than
            // one wound inside out.
            forward: (side < 0.0) != (sweep < 0.0),
        })
    }

    /// The two caps of a partial turn and a face per part of the turn per
    /// strip, all empty of loops.
    ///
    /// **More than one face per wall**, because a wall spun the whole way round
    /// covers its own surface and no face may — see `.notes/KERNEL.md` §4.4.
    /// How many is [`Spinning::parts`], and why at most [`MOST`] is argued
    /// there.
    ///
    /// The caps first and in this order, because the order faces are made is
    /// the order their names come back in — which is what an extrusion's two
    /// promise as well, and a caller writing one drawable per name relies on
    /// neither moving between rebuilds.
    ///
    /// `false` where a strip sweeps no surface this can write.
    fn raise_walls(&mut self, spinning: Spinning, strips: &Strips, into: &mut Body) -> bool {
        self.caps = (!spinning.closed).then(|| {
            [(false, Grown::Base), (true, Grown::Far)]
                .map(|(far, grown)| Self::cap(spinning, far, spinning.by.grew(grown), into))
        });
        self.walls.clear();
        for at in 0..strips.all().len() {
            let strip = strips.all()[at];
            // A strip lying *on* the line sweeps a line rather than a surface,
            // which is no wall at all — a profile revolved about one of its own
            // sides has one, and the solid is closed by what the sides either
            // way of it sweep.
            //
            // **A straight run and no other.** An arc bulges off its own chord,
            // so one running from the line round to the line again lies on
            // neither: what it sweeps is a real surface, and a semicircle about
            // its own diameter sweeps the commonest of them.
            if strip.turn.is_none()
                && [strip.from, strip.to]
                    .iter()
                    .all(|&corner| spinning.poles(strips.corners()[corner]))
            {
                self.walls.push(None);
                continue;
            }
            let Some(Walled { surface, outward }) = Self::wall_of(spinning, strips, strip) else {
                return false;
            };
            let name = spinning.by.grew(Grown::Side(strip.bound));
            into.named(name);
            let parts = match surface {
                Surface::Natural(Natural::Plane(_)) => 1,
                _ => spinning.parts,
            };
            let faces = each_part(parts, |_| {
                into.topology_mut().add_face(Face {
                    surface,
                    outward,
                    loops: 0..0,
                    name,
                    tolerance: EXACT,
                })
            });
            self.walls.push(Some(Walls { parts, faces }));
        }
        true
    }

    /// One end of a partial turn: the profile itself, lying in the half-plane
    /// the spin has carried it to.
    ///
    /// The plane *contains* the line rather than standing square to it, which
    /// is what tells this from the annulus a run square across the line sweeps
    /// — its own two are the line's direction and the way out at that angle,
    /// which is the frame the profile was read in.
    fn cap(spinning: Spinning, far: bool, name: Named, into: &mut Body) -> FaceId {
        into.named(name);
        into.topology_mut().add_face(Face {
            surface: Surface::Natural(Natural::Plane(Plane {
                origin: spinning.axis.origin,
                x: spinning.axis.direction,
                y: spinning.axis.radial(spinning.seamed(spinning.ended(far))),
            })),
            // The caps face away from the solid, so the one at the far end
            // faces the way the spin went and the one at the start faces back
            // against it.
            outward: far == (spinning.sweep > 0.0),
            loops: 0..0,
            name,
            tolerance: EXACT,
        })
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
        let ends = [strip.from, strip.to].map(|corner| spinning.profile(strips.corners()[corner]));
        let surface = match strip.turn {
            None => {
                let [from, to] = ends;
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
                    // Read off the end standing further out, which is the whole
                    // of what a strip ending *at* the apex needs: that end
                    // gives a rise of nothing and an angle of nought over
                    // nought.
                    let far = if from.y > to.y { from } else { to };
                    let rise = far.x - apex;
                    Surface::Natural(Natural::Cone(Cone {
                        axis: Axis::new(
                            spinning.axis.origin + spinning.axis.direction * apex,
                            spinning.axis.direction * rise.signum(),
                            spinning.axis.reference,
                        ),
                        half_angle: (far.y / rise.abs()).atan(),
                    }))
                }
            }
            Some(turn) => {
                if !spinning.clears(turn, ends) {
                    return None;
                }
                let middle = spinning.profile(turn.center);
                let axis = spinning.about(middle.x);
                // A centre on the line sweeps a ball rather than a ring —
                // which [`Spinning::profile`] has already snapped, a centre
                // within a place of the line being on it.
                if middle.y == 0.0 {
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
        // Which corners a seam will end at, which has to be known before any
        // vertex is raised — see [`Revolving::corner`], where a pole with no
        // seam at it is left without one.
        self.seamed.clear();
        self.seamed.resize(corners, false);
        for at in 0..strips.all().len() {
            let seamed = match self.walls[at] {
                Some(walls) => walls.parts > 1 || !spinning.closed,
                None => self.caps.is_some(),
            };
            if seamed {
                let strip = strips.all()[at];
                self.seamed[strip.from] = true;
                self.seamed[strip.to] = true;
            }
        }
        for at in 0..strips.all().len() {
            let strip = strips.all()[at];
            self.corner(spinning, strips, strip.from, into);
            self.corner(spinning, strips, strip.to, into);
            let seams = match self.walls[at] {
                // **One face round a closed turn has no seam at all**, which is
                // what makes it worth being one: an edge with that face either
                // way of it is the seam §4.4 forbids. What bounds it is the
                // circles alone — see [`Revolving::wall_loop`].
                Some(walls) if walls.parts == 1 && spinning.closed => None,
                // **At the turn's own seam and not at the wall's.** A wall cut
                // less finely than the turn parts at a subset of the turn's
                // seams — one face parts at its two ends alone — and the
                // vertices and angles a seam is built from are the turn's.
                Some(walls) => Some(each_seam(walls.parts, spinning.closed, |part| {
                    let between = Self::divided(self.caps, walls, part);
                    let seam = part * spinning.parts / walls.parts;
                    self.seam(spinning, strips, strip, seam, between, into)
                })),
                // A strip on the line sweeps no wall, and where the turn is cut
                // it is still a side of both caps — one edge, the line itself,
                // held in every slot so a cap reads it the way it reads any
                // other.
                None => self
                    .caps
                    .map(|caps| [self.seam(spinning, strips, strip, 0, caps, into); MOST + 1]),
            };
            self.seams.push(seams);
        }
        for loop_ in 0..strips.loops() {
            let run = strips.run(loop_);
            for at in run.clone() {
                // The corner between this strip and the next around the loop
                // is where the two walls meeting there share an edge.
                let next = if at + 1 == run.end { run.start } else { at + 1 };
                let corner = strips.all()[at].to;
                // A corner on the line sweeps a point rather than a circle, so
                // there is nothing there for two walls to share. It answers for
                // a strip lying *on* the line too, both of whose corners stand
                // on it — so every strip reached past here raised a wall.
                if spinning.poles(strips.corners()[corner]) {
                    continue;
                }
                let between = [at, next]
                    .map(|wall| self.walls[wall].expect("a strip clear of the line raised a wall"));
                self.circle(spinning, strips, corner, between, into);
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
        // **One vertex where the corner is on the line**, held in every slot:
        // a place there sweeps a point rather than a circle, so every seam of
        // the turn ends at the one vertex. Held per part because every reader
        // wants a slot per part, and the slots being equal is what says which
        // this is.
        let raised = if at.y == 0.0 {
            // **And none at all where no seam ends there.** A pole sweeps no
            // circle, so a seam is the one thing that would reach it — and a
            // vertex raised with nothing on it would still count against the
            // body's own reckoning.
            if !self.seamed[corner] {
                return;
            }
            let pole = into.topology_mut().add_vertex(Vertex {
                at: spinning.spun(at, spinning.from),
                tolerance,
            });
            [pole; MOST + 1]
        } else {
            each_seam(spinning.parts, spinning.closed, |part| {
                into.topology_mut().add_vertex(Vertex {
                    at: spinning.spun(at, spinning.seamed(part)),
                    tolerance,
                })
            })
        };
        self.corners[corner] = Some(raised);
    }

    /// What a wall's seam at `part` divides: the part beginning there and the
    /// one ending there.
    ///
    /// Round a closed turn those are neighbours. At the two ends of a partial
    /// one there is no neighbour, and what stands across the seam is the cap.
    fn divided(caps: Option<[FaceId; 2]>, walls: Walls, part: usize) -> [FaceId; 2] {
        let Walls { parts, faces } = walls;
        let begins = match caps {
            Some(caps) if part == parts => caps[1],
            _ => faces[part],
        };
        let ends = match caps {
            Some(caps) if part == 0 => caps[0],
            _ => faces[(part + parts - 1) % parts],
        };
        [begins, ends]
    }

    /// One copy of a strip's own curve, at the spin the walls part at `part`.
    fn seam(
        &mut self,
        spinning: Spinning,
        strips: &Strips,
        strip: Strip,
        part: usize,
        between: [FaceId; 2],
        into: &mut Body,
    ) -> EdgeId {
        let [from, to] = [strip.from, strip.to]
            .map(|corner| self.corners[corner].expect("every corner of a strip is raised")[part]);
        let Running { curve, bounds } =
            Self::running(spinning, strips, strip, spinning.seamed(part));
        // The parts of one wall lie on one surface, so a seam between two of
        // them is what splitting the turn left behind rather than a crease. The
        // two at the ends of a partial turn divide a wall from a cap, which is
        // a crease unless the two run out into each other there — see
        // `.notes/KERNEL.md` §4.4.
        let topology = into.topology();
        let [one, two] = between.map(|face| topology.face(face));
        let smooth = one.smooth(two, &curve, bounds, topology.carried());
        into.topology_mut().add_edge(Edge {
            curve,
            bounds,
            from,
            to,
            between,
            artificial: smooth,
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

    /// The parts of the circle `corner` sweeps, between the walls that meet
    /// there.
    fn circle(
        &mut self,
        spinning: Spinning,
        strips: &Strips,
        corner: usize,
        between: [Walls; 2],
        into: &mut Body,
    ) {
        debug_assert!(
            self.circling[corner].is_none(),
            "corner {corner} is the end of two strips of one loop",
        );
        let at = spinning.profile(strips.corners()[corner]);
        let axis = spinning.about(at.x);
        let raised = self.corners[corner].expect("every corner of a strip is raised");
        let parts = each_part(spinning.parts, |part| {
            let faces = between.map(|wall| wall.faces[part]);
            // No crease where the two walls run out into each other, which two
            // arcs of one circle the drawing was cut between and two segments
            // drawn straight through a corner both do.
            let curve = Curve::Circle(Circle { axis, radius: at.y });
            let bounds = [spinning.seamed(part), spinning.seamed(part + 1)];
            let topology = into.topology();
            let [one, two] = faces.map(|face| topology.face(face));
            let smooth = one.smooth(two, &curve, bounds, topology.carried());
            into.topology_mut().add_edge(Edge {
                curve,
                bounds,
                from: raised[part],
                to: raised[part + 1],
                between: faces,
                artificial: smooth,
                tolerance: EXACT,
            })
        });
        self.circling[corner] = Some(parts);
    }

    /// Write the loops of every face.
    fn write_loops(&self, spinning: Spinning, strips: &Strips, into: &mut Body) {
        if let Some(caps) = self.caps {
            for (far, face) in [false, true].into_iter().zip(caps) {
                self.cap_loops(spinning, strips, far, face, into);
            }
        }
        for at in 0..self.walls.len() {
            for (part, face) in self.parted(at).enumerate() {
                self.wall_loop(spinning, strips, at, part, face, into);
            }
        }
    }

    /// Every loop of one cap: the region's outline, then each of its holes.
    ///
    /// The two caps face opposite ways, so exactly one of them walks its strips
    /// the way the drawing did — which is the same shape an extrusion's two
    /// caps have, and which one it is comes off [`Spinning::forward`].
    fn cap_loops(
        &self,
        spinning: Spinning,
        strips: &Strips,
        far: bool,
        face: FaceId,
        into: &mut Body,
    ) {
        let seam = spinning.ended(far);
        // **The opposite of what the wall does with the same seam**, which is
        // the whole of it: a wall walks the seam it begins at forwards and the
        // one it ends at back, and every edge is walked once each way.
        let forward = far == spinning.forward;
        let from = into.topology().loops_added();
        for loop_ in 0..strips.loops() {
            let run = strips.run(loop_);
            let seams = &self.seams;
            into.topology_mut().add_loop(|walk| {
                // The buffer a loop is written into holds every other loop of
                // the body as well, so a reversal reaches only what this one
                // just put in it.
                let wrote = walk.len();
                walk.extend(run.filter_map(|at| seams[at]).map(|held| Coedge {
                    edge: held[seam],
                    forward,
                }));
                if !forward {
                    walk[wrote..].reverse();
                }
            });
        }
        let to = into.topology().loops_added();
        into.topology_mut().face_mut(face).loops = from..to;
    }

    /// The one loop of one part of one wall: along the profile at the near
    /// seam, round to the far one, back along the profile, and round again.
    ///
    /// **A side of it goes wherever there is nothing to walk.** A corner on the
    /// line sweeps a point, so the side that would have run round it *is* that
    /// point — the same collapse the hand-built ball has at each of its own
    /// two. And one face round a closed turn has no seams, so what is left is
    /// the circles.
    ///
    /// **One arc of each circle where the wall is cut as finely as the turn,
    /// and all of them where it is one face** — see [`Walls`].
    fn wall_loop(
        &self,
        spinning: Spinning,
        strips: &Strips,
        at: usize,
        part: usize,
        face: FaceId,
        into: &mut Body,
    ) {
        let strip = strips.all()[at];
        let parts = self.walls[at]
            .expect("a wall that was raised has its parts")
            .parts;
        let Some(seams) = self.seams[at] else {
            self.round_loops(spinning, strips, strip, face, into);
            return;
        };
        let circling = [strip.to, strip.from].map(|corner| self.circling[corner]);
        let arcs = part * spinning.parts / parts..(part + 1) * spinning.parts / parts;
        let from = into.topology_mut().add_loop(|walk| {
            let wrote = walk.len();
            walk.push(Coedge {
                edge: seams[part],
                forward: true,
            });
            if let Some(round) = circling[0] {
                walk.extend(arcs.clone().map(|arc| Coedge {
                    edge: round[arc],
                    forward: true,
                }));
            }
            walk.push(Coedge {
                edge: seams[part + 1],
                forward: false,
            });
            if let Some(round) = circling[1] {
                walk.extend(arcs.clone().rev().map(|arc| Coedge {
                    edge: round[arc],
                    forward: false,
                }));
            }
            if !spinning.forward {
                // Read the other way round, which is what keeps the loop
                // counterclockwise about a face whose parameters the frame
                // reversed. Only this loop's own coedges, which is what the
                // buffer is indexed from.
                walk[wrote..].reverse();
                for coedge in &mut walk[wrote..] {
                    *coedge = coedge.turned();
                }
            }
        });
        into.topology_mut().face_mut(face).loops = from..from + 1;
    }

    /// The loops of a wall no seam bounds, which is one face round a closed
    /// turn: a whole circle at each end it stands between.
    ///
    /// **A loop apiece rather than one**, which is what an annulus is: two
    /// circles with the face between them and neither reaching the other. The
    /// wider is the outline and the narrower the hole punched out of it, which
    /// is the order a face's own loops come in. A disc has one of them, the
    /// other end of it being a pole.
    ///
    /// The two are walked the ways the seamed loop walks them — the far circle
    /// with the turn and the near one against it — so the winding is the one a
    /// wall has anywhere.
    fn round_loops(
        &self,
        spinning: Spinning,
        strips: &Strips,
        strip: Strip,
        face: FaceId,
        into: &mut Body,
    ) {
        let round = [strip.to, strip.from].map(|corner| {
            let at = spinning.profile(strips.corners()[corner]);
            (self.circling[corner], at.y)
        });
        let widest = if round[0].1 >= round[1].1 {
            [0, 1]
        } else {
            [1, 0]
        };
        let from = into.topology().loops_added();
        for which in widest {
            let (Some(arcs), _) = round[which] else {
                continue;
            };
            let forward = which == 0;
            into.topology_mut().add_loop(|walk| {
                let wrote = walk.len();
                for step in 0..spinning.parts {
                    let arc = if forward {
                        step
                    } else {
                        spinning.parts - 1 - step
                    };
                    walk.push(Coedge {
                        edge: arcs[arc],
                        forward,
                    });
                }
                if !spinning.forward {
                    walk[wrote..].reverse();
                    for coedge in &mut walk[wrote..] {
                        *coedge = coedge.turned();
                    }
                }
            });
        }
        let to = into.topology().loops_added();
        into.topology_mut().face_mut(face).loops = from..to;
    }

    /// Gather the faces into the shells of the one lump: the outline's round
    /// the outside, and one cavity per hole of the profile.
    ///
    /// **A hole spun the whole way is a cavity and not a hole *through*.** An
    /// extrusion's two caps join the wall of a hole to the wall outside it, so
    /// one shell goes round both. A whole turn raises no cap, so what a hub
    /// inside a region sweeps is a shell of its own with the solid all around
    /// it — and a partial turn, having caps again, is back to the extrusion's
    /// answer.
    fn gather(&self, strips: &Strips, into: &mut Body) {
        if let Some(caps) = self.caps {
            let walls = (0..strips.loops()).flat_map(|loop_| self.walled(strips.run(loop_)));
            let outer = shelled(into, walls.chain(caps));
            into.topology_mut().add_lump(Lump { outer, voids: 0..0 });
            return;
        }
        let outer = shelled(into, self.walled(strips.run(0)));
        let from = into.topology().shells_voided();
        for loop_ in 1..strips.loops() {
            let void = shelled(into, self.walled(strips.run(loop_)));
            into.topology_mut().add_voided(void);
        }
        let to = into.topology().shells_voided();
        into.topology_mut().add_lump(Lump {
            outer,
            voids: from..to,
        });
    }

    /// The faces the strip at `at` was cut into, and none where it raised no
    /// wall.
    ///
    /// **The live parts and no more.** The slots past them repeat the last —
    /// see [`each_part`] — so a walk of the whole array would read one face
    /// several times, which is a loop written twice or a face shelled twice.
    /// One place knows to stop, and both walks go through it.
    fn parted(&self, at: usize) -> impl Iterator<Item = FaceId> {
        self.walls[at]
            .into_iter()
            .flat_map(|walls| walls.faces.into_iter().take(walls.parts))
    }

    /// Every wall the strips at `run` raised.
    fn walled(&self, run: Range<usize>) -> impl Iterator<Item = FaceId> {
        run.flat_map(|at| self.parted(at))
    }
}

/// The faces one strip's wall was cut into, and how many of them there are.
///
/// **A plane is one face however far the turn goes.** §4.4 cuts a wall so that
/// no face wraps its own surface, and a plane's parameters do not wrap — so the
/// annulus a run square across the line sweeps is one face whose loop walks the
/// whole of each circle it stands between. A cylinder, a cone, a sphere and a
/// torus are each cut into [`Spinning::parts`].
///
/// **Which is worth more than the faces it saves.** Three sectors of one disc
/// are held apart by three radial seams, and a cut crossing one of those is
/// broken there by the disc and not by the face across it — see
/// `.notes/KERNEL.md` §9.2, where that is measured.
///
/// The slots past `parts` repeat the last — see [`each_part`] — so a reader
/// asking by part gets the one face wherever it asks.
#[derive(Debug, Clone, Copy)]
struct Walls {
    parts: usize,
    faces: [FaceId; MOST],
}

/// Where a strip stands part way along it, and the way it runs there.
#[derive(Debug, Clone, Copy)]
struct Laid {
    at: DVec2,
    /// Unit, the way the walk runs — which is what says where the material is,
    /// a region lying on the left of its own boundary.
    way: DVec2,
}

/// The most faces one wall is ever cut into, which is what a whole turn takes.
///
/// **Three and not two.** Two is the fewest §4.4's rule allows and it is one
/// too few: the two seams of a half-turn face stand exactly half a turn apart
/// in the surface's own angle, so an inversion asked to carry a reading from
/// one to the other has a *tie* — and beside a pole there is nothing else to
/// break it with, so a face closing at one folds across itself. A third of a
/// turn leaves the two seams nearer than half, which is a strict answer.
///
/// So the rule a shorter sweep follows is the same one: **every part spans at
/// most a third of a turn** — see [`Spinning::parts`], which is where a sector
/// is cut by it. A quarter turn is one face, and this is the most any sweep
/// asks for.
pub(super) const MOST: usize = 3;

/// One entry per part of the turn, the slots past them repeating the last.
///
/// The tail is never read — every walk here stops at [`Spinning::parts`] — and
/// is filled rather than left an [`Option`] because every reader wants a value
/// and none of them wants to ask.
fn each_part<T: Copy>(parts: usize, mut made: impl FnMut(usize) -> T) -> [T; MOST] {
    let mut held: [Option<T>; MOST] = [None; MOST];
    for (at, slot) in held.iter_mut().enumerate().take(parts) {
        *slot = Some(made(at));
    }
    let last = held[parts - 1].expect("a turn is cut into at least one part");
    held.map(|it| it.unwrap_or(last))
}

/// One entry per *seam* of the turn, which is one more than the parts — and the
/// last is the first again where the turn closes on itself.
///
/// The tail past the seams repeats the last, on the terms [`each_part`] states.
fn each_seam<T: Copy>(
    parts: usize,
    closed: bool,
    mut made: impl FnMut(usize) -> T,
) -> [T; MOST + 1] {
    let mut held: [Option<T>; MOST + 1] = [None; MOST + 1];
    for (at, slot) in held.iter_mut().enumerate().take(parts) {
        *slot = Some(made(at));
    }
    let first = held[0].expect("a turn has at least one seam");
    held[parts] = Some(if closed { first } else { made(parts) });
    let last = held[parts].expect("the last seam was just made");
    held.map(|it| it.unwrap_or(last))
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
