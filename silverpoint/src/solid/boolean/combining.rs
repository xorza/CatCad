//! Combining two bodies, stage by stage.

use crate::loops::Loops;
use crate::math::bounds::Bounds;
use crate::math::chorded::Chorded;
use crate::math::triangulate::{Cutter, Fill};
use crate::math::winding;
use crate::number::predicate;
use crate::number::tolerance::ALIGNED;
use crate::number::tolerance::CHORDED;
use crate::solid::boolean::imprints::Imprints;
use crate::solid::boolean::operation::Operation;
use crate::solid::boolean::sounding::Sounding;
use crate::solid::boolean::splitting::Splitting;
use crate::solid::boolean::splitting::bow::Bow;
use crate::solid::boolean::splitting::cells::Cells;
use crate::solid::boolean::splitting::corner::{self, Came, Corner};
use crate::solid::boolean::splitting::cut::Cut;
use crate::solid::boolean::splitting::oval::Oval;
use crate::solid::boolean::splitting::ripple::Ripple;
use crate::solid::buckets::Buckets;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::meeting::Meeting;
use crate::solid::named::Named;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::{Face, FaceId};
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, TAU};
use std::ops::Range;

/// One region of one face that a boolean kept, and what it inherited.
///
/// In the surface's own parameters rather than in the world, because that is
/// where it was cut and where it is still exact — lifting it back out is the
/// sewing's, and it does that once.
#[derive(Debug)]
pub(super) struct Kept {
    pub(super) surface: Surface,
    /// Whether material lies on the side the surface's normal points at, after
    /// whatever the operation did to it.
    pub(super) outward: bool,
    pub(super) name: Named,
    /// Which of the boolean's loops are its: the outline first, then holes.
    pub(super) loops: Range<usize>,
}

/// Combines bodies, keeping the room it works in.
#[derive(Debug, Default)]
pub(super) struct Combining {
    /// Every loop of every region kept, laid end to end.
    loops: Loops<Corner>,
    /// Every curve a stretch of boundary runs along, and the run each stretch
    /// was marked with — see [`Imprints`], which says why those are two things.
    ///
    /// Held for the whole combine rather than per face, because the loops above
    /// are too: a region of one face and a region of another both point in
    /// here, and a list emptied between faces would have them pointing at each
    /// other's curves.
    imprints: Imprints,
    kept: Vec<Kept>,
    scratch: Scratch,
}

/// Every list a combine works in, kept so that the next one need not ask for
/// them again.
///
/// Apart from the answer above rather than mixed in with it: what a combine
/// leaves is the regions it kept, their loops and the imprints those point
/// into, and none of the below outlives the call that filled it.
#[derive(Debug, Default)]
struct Scratch {
    splitting: Splitting,
    sounding: Sounding,
    cutter: Cutter,
    fill: Fill,
    /// The regions one face has been cut into, and the ones it is being cut
    /// into next: swapped rather than replaced, plane after plane.
    cells: Cells,
    spare: Cells,
    /// One region taken apart for the cutter, which wants an outline and its
    /// holes separately where a region holds them together — and wants bare
    /// places, having nothing to do with where a stretch came from.
    outline: Vec<DVec2>,
    holes: Loops<DVec2>,
    /// A face's boundary in the world, walked as chords: one loop of it on its
    /// way into that face's own parameters, or the whole of it on its way into
    /// the box the face fills. `marks` is in step with `traced` for the first
    /// of those, saying which edge put each place there.
    traced: Vec<DVec3>,
    marks: Vec<Came>,
    walk: Vec<DVec2>,
    laid: Vec<Corner>,
    /// Which turn of a wrapping parameter the face being cut was laid out in —
    /// see [`imprinted`], the one thing that needs it.
    ///
    /// The middle of the range its loops cover. A face may not wrap, so that
    /// range is less than a whole turn wide and at most one turn of a wrapping
    /// cut falls inside it: the one nearest this. Meaningless for a plane,
    /// whose parameters do not wrap, and read by nothing for one.
    about: f64,
    /// The distinct surfaces of the body being cut against that reach it at all
    /// — see [`Combining::against`], which says why they are surfaces rather
    /// than faces, and why "reach it" is asked of the whole body.
    ///
    /// `reached` is which of them key alike, so a face's surface is told from
    /// a handful rather than compared against every surface already collected.
    met: Vec<Surface>,
    reached: Buckets,
    /// Each face of the two bodies with the box it fills, one body's run
    /// after the other's, and where the second run begins.
    ///
    /// **Both bodies at once, because each is wanted twice.** Cutting one
    /// against the other asks how far the *first* reaches, to know which of the
    /// second's surfaces are worth cutting by; and cutting the other way round
    /// asks the same of each face of the first. A body's own reach is the union
    /// of its faces' — so measured a call at a time, every boundary of both
    /// bodies is traced twice over, on the path a document is rebuilt down
    /// sixty times a second.
    boxed: Vec<Boxed>,
    between: usize,
}

/// One face of a body and the box it fills.
///
/// The face travels with its box rather than being counted to afterwards: what
/// reads a stretch of [`Scratch::boxed`] is not the walk that filled it, and
/// two walks agreeing about which face is which is an agreement that would
/// break without a word.
#[derive(Debug, Clone, Copy)]
struct Boxed {
    face: FaceId,
    fills: Bounds,
}

impl Combining {
    /// Cut both bodies against each other and keep what `doing` asks for.
    ///
    /// `false` where a crossing turns up that nothing here can write down in a
    /// face's own parameters, which is [`imprinted`]. See `.notes/KERNEL.md`
    /// §8's `Built::Refused`.
    pub(super) fn combine(&mut self, one: &Body, two: &Body, doing: Operation) -> bool {
        self.loops.clear();
        self.kept.clear();
        self.imprints.clear();
        // Every curved edge of either body takes a curve in the imprint list
        // and a run per face that walks it, before one crossing has been found
        // — see [`Imprints::reserve`].
        let curved = one.topology().curved_edges() + two.topology().curved_edges();
        self.imprints.reserve(curved);
        self.scratch.boxed.clear();
        self.box_up(one);
        self.scratch.between = self.scratch.boxed.len();
        self.box_up(two);
        self.against(one, two, doing, true) && self.against(two, one, doing, false)
    }

    /// Take in the box every face of `body` fills — see [`Scratch::boxed`].
    fn box_up(&mut self, body: &Body) {
        for (id, face) in body.topology().faces() {
            let fills = self.reach(body, face);
            self.scratch.boxed.push(Boxed { face: id, fills });
        }
    }

    /// What the last combine kept.
    pub(super) fn kept(&self) -> &[Kept] {
        &self.kept
    }

    /// The loops of the regions kept, laid end to end.
    pub(super) fn loops(&self) -> &Loops<Corner> {
        &self.loops
    }

    /// The curves those loops' arcs run along, and which run is which.
    pub(super) fn imprints(&self) -> &Imprints {
        &self.imprints
    }

    /// Cut every face of `mine` against `theirs` and keep what survives.
    ///
    /// `false` where a cut met a shape the splitter does not handle, which is a
    /// refusal like any other here: what is kept would be a region quietly
    /// missing a bite of itself.
    fn against(&mut self, mine: &Body, theirs: &Body, doing: Operation, first: bool) -> bool {
        // **The other body's surfaces, not its faces.** A face may not wrap, so
        // a whole cylinder is *two* faces of one surface — see
        // `.notes/KERNEL.md` §4.4 — and cutting once per face would imprint the
        // same circle twice and leave the same hole punched through a face
        // twice over. What divides a face is a surface; which faces of the
        // other body lie on it is settled later, by where each region stands.
        //
        // Told apart by exact equality and not by a tolerance, which is sound
        // because two faces of one surface were *given* the same value rather
        // than each working one out: an extrusion raises one cylinder and hands
        // it to both halves — see [`Builder`](crate::Builder). Two faces that
        // computed the same surface separately would fall a rounding apart and
        // be imprinted twice again, which is a reason to go on handing them the
        // one value rather than a reason to compare loosely here.
        //
        // **And only the ones that reach this body at all.** A surface is
        // unbounded where the faces standing on it are not, so a wall at the
        // far end of a model meets a surface here whether or not anything of
        // that body is anywhere near — which costs pieces where the crossing
        // can be carried and costs the whole boolean where it cannot, a plane
        // parallel to a cylinder's axis meeting it in two ruling lines wherever
        // the two stand. What decides is the *face's* reach, and what it is
        // asked against is the whole of this body rather than the face being
        // cut. That is not conservatism: a cut that divides one face and not
        // the face beside it leaves a vertex on one side of the edge they share
        // and none on the other, and the sewing then finds three edges where it
        // wanted two. Cutting further than necessary is not merely tolerated —
        // see [`splitting`] — it has to be uniform.
        //
        // Which run of the boxes is whose follows from `first`, the two calls
        // being the two ways round — see [`Scratch::boxed`].
        let split = self.scratch.between..self.scratch.boxed.len();
        let (here, there) = if first {
            (0..self.scratch.between, split)
        } else {
            (split, 0..self.scratch.between)
        };
        let mut reach = Bounds::default();
        for at in here {
            reach.swallow(self.scratch.boxed[at].fills);
        }
        self.scratch.met.clear();
        self.scratch.reached.clear();
        for at in there {
            let Boxed { face, fills } = self.scratch.boxed[at];
            if !fills.meets(reach, CHORDED) {
                continue;
            }
            let other = theirs.topology().face(face);
            // Through the index rather than by walking what is already here:
            // every face of one body asks about every surface of the other,
            // and the cost of a walk grows as the square of the body. Equality
            // still decides, exactly as above — see [`Surface::key`].
            let key = other.surface.key();
            if self
                .scratch
                .reached
                .under(key)
                .any(|at| self.scratch.met[at as usize] == other.surface)
            {
                continue;
            }
            let slot = self.scratch.reached.file(key);
            debug_assert_eq!(slot as usize, self.scratch.met.len(), "the index lost step");
            self.scratch.met.push(other.surface);
        }
        for (_, face) in mine.topology().faces() {
            self.lay(mine, face);
            for at in 0..self.scratch.met.len() {
                let along = match Meeting::of(&face.surface, &self.scratch.met[at]) {
                    // Nothing that divides anything. Apart, the same surface —
                    // two faces on one are told apart by where each region
                    // *stands* — or grazing at a point, which is a place rather
                    // than a line and divides no face.
                    Meeting::Apart | Meeting::Same | Meeting::Touching(_) => continue,
                    // They meet, along a curve nothing here can cut with: a
                    // quartic no `Cut` has a shape for, or one that is marched
                    // rather than written down at all. Not nothing, and saying
                    // so is the whole of what those two arms are for — see
                    // [`Meeting::Algebraic`].
                    Meeting::Algebraic | Meeting::Marched => return false,
                    Meeting::Along(along) => along,
                };
                // Each curve of the meeting in turn: a plane cutting a chord
                // off a cylinder meets it in two, and both divide the face.
                for curve in along.all() {
                    // Numbered before the cut is built rather than after, which
                    // is what a round cut needs: it carries its own number —
                    // see [`Cut::Round`] — so there is nothing to hand back.
                    // A straight imprint carries none and spends none.
                    let next = match curve {
                        Curve::Line(_) => None,
                        _ => Some(self.imprints.crossing(*curve)),
                    };
                    let Some(cut) = imprinted(face.surface, *curve, next, self.scratch.about)
                    else {
                        return false;
                    };
                    if !self.scratch.splitting.split(
                        &self.scratch.cells,
                        cut,
                        &mut self.scratch.spare,
                    ) {
                        return false;
                    }
                    std::mem::swap(&mut self.scratch.cells, &mut self.scratch.spare);
                }
            }
            self.sift(face, theirs, doing, first);
        }
        true
    }

    /// The box `face` fills, walked as chords at [`CHORDED`].
    ///
    /// Off the boundary, which is enough for every surface but a sphere — see
    /// [`Surface::fills`], where that argument is written down and where the
    /// one it is not enough for is widened.
    fn reach(&mut self, body: &Body, face: &Face) -> Bounds {
        let topology = body.topology();
        self.scratch.traced.clear();
        for coedge in topology.loops_of(face).flatten() {
            topology
                .walked(*coedge)
                .walk(CHORDED, &mut self.scratch.traced);
        }
        let mut boundary = Bounds::default();
        for &at in &self.scratch.traced {
            boundary.hold(at);
        }
        face.surface.fills(boundary)
    }

    /// Lay one face out in its own parameters as the one region to cut.
    ///
    /// **Turned counterclockwise where it was not**, which is the one thing
    /// that has to be arranged here. A face keeps its material on whichever
    /// side of its surface [`Face::outward`] says, and its loops are wound to
    /// suit — so a face facing the other way comes round its own parameters
    /// clockwise, and a splitter reading a clockwise outline reads a hole. The
    /// winding is made canonical and the side goes on being carried by
    /// `outward`, which is where it belongs.
    fn lay(&mut self, body: &Body, face: &Face) {
        let topology = body.topology();
        let mut across = [f64::INFINITY, f64::NEG_INFINITY];
        let Self {
            scratch, imprints, ..
        } = self;
        let Scratch {
            cells,
            walk,
            laid,
            traced,
            marks,
            ..
        } = scratch;
        cells.clear();
        cells.add(|loops| {
            let mut turned = false;
            for (at, round) in topology.loops_of(face).enumerate() {
                // **The whole loop traced before any of it is flattened.**
                // Flattening unwraps the angle as it goes so the loop comes out
                // continuous — see [`Face::flatten`] — and a call per edge
                // restarts that, which leaves a face on a cylinder in as many
                // branches as it has edges and no polygon at all.
                traced.clear();
                marks.clear();
                for coedge in round {
                    // **An edge of the face is an imprint like any other.** A
                    // curved one is chorded to be classified and has to be put
                    // back together afterwards, exactly as a cut's arc does —
                    // without this the rim of a bore would be sewn as a hundred
                    // straight edges rather than as the circle it is.
                    // A run of its own, even along a curve the face has
                    // already run along — see [`Imprints::edge`].
                    let came = match topology.edge(coedge.edge).curve {
                        Curve::Line(_) => Came::Edge,
                        curve => Came::Arc(imprints.edge(curve)),
                    };
                    topology.walked(*coedge).walk(CHORDED, traced);
                    marks.resize(traced.len(), came);
                }
                walk.clear();
                face.flatten(traced, walk);
                for at in walk.iter() {
                    across = [across[0].min(at.x), across[1].max(at.x)];
                }
                laid.clear();
                laid.extend(
                    walk.iter()
                        .zip(marks.iter())
                        .map(|(&at, &came)| Corner { at, came }),
                );
                if at == 0 {
                    turned = winding::swept(laid) < 0.0;
                }
                if turned {
                    corner::turned(laid);
                }
                loops.push(laid);
            }
        });
        self.scratch.about = (across[0] + across[1]) / 2.0;
    }

    /// Ask every region where it stands and keep the ones `doing` wants.
    fn sift(&mut self, face: &Face, theirs: &Body, doing: Operation, first: bool) {
        for at in 0..self.scratch.cells.len() {
            let Some(within) = self.within(at) else {
                continue;
            };
            let standing = self
                .scratch
                .sounding
                .standing(face.surface.at(within), theirs);
            if !doing.keeps(standing, face.normal(within), first) {
                continue;
            }
            let from = self.loops.len();
            for walk in self.scratch.cells.cell(at) {
                self.loops.push(walk);
            }
            self.kept.push(Kept {
                surface: face.surface,
                outward: face.outward != doing.turns(first),
                name: face.name,
                loops: from..self.loops.len(),
            });
        }
    }

    /// A place well within the region at `at`, or `None` where it covers
    /// nothing to be within.
    ///
    /// The middle of the widest triangle it cuts into, which is inside it
    /// however the region bends — where the average of its corners is only
    /// inside a region that happens to be convex, and a boolean makes plenty
    /// that are not.
    fn within(&mut self, at: usize) -> Option<DVec2> {
        let Scratch {
            cells,
            cutter,
            fill,
            outline,
            holes,
            ..
        } = &mut self.scratch;
        outline.clear();
        holes.clear();
        let mut walks = cells.cell(at);
        outline.extend(walks.next()?.iter().map(|corner| corner.at));
        for walk in walks {
            holes.add(|into| into.extend(walk.iter().map(|corner| corner.at)));
        }
        cutter.polygon(outline, holes, fill);
        let widest = fill.triangles.iter().copied().max_by(|&a, &b| {
            let area = |[x, y, z]: [u32; 3]| {
                let corner = |at: u32| fill.corners[at as usize];
                (corner(y) - corner(x))
                    .perp_dot(corner(z) - corner(x))
                    .abs()
            };
            area(a).partial_cmp(&area(b)).expect("a fill is finite")
        })?;
        let corner = |at: u32| fill.corners[at as usize];
        Some((corner(widest[0]) + corner(widest[1]) + corner(widest[2])) / 3.0)
    }
}

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
/// would really have been divided — see [`Combining::against`], which turns
/// that into a refusal of the whole boolean.
///
/// `run` is the run the curve was given, and `None` where it was given none
/// because it is a straight line — see [`Imprints`]. The round arms want one
/// and the straight arms do not, which is exactly the two states of that
/// argument. `about` is the turn of a wrapping parameter the face was laid out in,
/// which is the whole of what a ruling line needs and what nothing else here
/// reads.
fn imprinted(on: Surface, along: Curve, run: Option<u32>, about: f64) -> Option<Cut> {
    match (on, along) {
        // A line on a plane is a line in its parameters.
        (Surface::Natural(Natural::Plane(plane)), Curve::Line(line)) => {
            let at = plane.flatten(line.origin);
            Some(Cut::Straight {
                at,
                along: (plane.flatten(line.origin + line.direction) - at).normalize(),
                run: None,
            })
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
        // A circle on a cylinder square to its axis is a *straight* cut in the
        // cylinder's own parameters: every place on it stands the same distance
        // along the axis, so it is the line `v = that`. Which is what the end of
        // a block does to a bore through it, and the reason a bore needs no cut
        // shape a plane did not already need.
        //
        // Square to the axis or not at all: a circle on a cylinder that is not
        // is no circle at all, and one whose plane is tilted meets it in an
        // ellipse, which arrives here as [`Curve::Ellipse`] and falls through.
        (Surface::Natural(Natural::Cylinder(tube)), Curve::Circle(circle))
            if predicate::parallel(circle.axis.direction, tube.axis.direction) =>
        {
            Some(Cut::Straight {
                at: DVec2::new(0.0, tube.axis.along(circle.axis.origin)),
                along: DVec2::X,
                run,
            })
        }
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
            let turns = ((about - angle) / TAU).round();
            Some(Cut::Straight {
                at: DVec2::new(angle + turns * TAU, 0.0),
                along: DVec2::Y,
                run: None,
            })
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
            let run = run.expect("a saddle is numbered");
            let axis = tube.axis;
            // Where the two axes come nearest, read along whichever of them
            // this face stands on — which is the same reading either way, the
            // saddle's own origin being that place.
            let level = (saddle.axis.origin - axis.origin).dot(axis.direction);
            // The wider cylinder by exact equality, and sound for the reason
            // [`Combining::against`] tells two faces of one surface apart that
            // way: the saddle was *given* this direction rather than working
            // one out.
            if axis.direction == saddle.axis.direction {
                let phase = axis.bearing(saddle.axis.reference);
                let turns = ((about - phase) / TAU).round();
                return Some(Cut::Bow(Bow {
                    across: saddle.across,
                    reach: saddle.reach,
                    phase: phase + turns * TAU,
                    off: saddle.off,
                    level,
                    // The loop is both branches, so there is no branch to pick.
                    upper: true,
                    inward: true,
                    run,
                }));
            }
            // The narrower cylinder reads the same numbers the other way round:
            // the offset is the same length square to both axes, and which
            // branch this loop is is which way the narrower axis was taken —
            // see [`Saddle`], where the two loops are that one flip.
            let upper = saddle.axis.reference.dot(axis.direction) > 0.0;
            Some(Cut::Bow(Bow {
                across: saddle.reach,
                reach: saddle.across,
                phase: axis.bearing(saddle.axis.direction),
                off: if upper { saddle.off } else { -saddle.off },
                level,
                upper,
                inward: true,
                run,
            }))
        }
        // Everything else.
        _ => None,
    }
}
