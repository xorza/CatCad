//! Combining two bodies, stage by stage.

use crate::loops::Loops;
use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::math::chorded::Chorded;
use crate::math::inside::Inside;
use crate::math::plane::Plane;
use crate::math::winding;
use crate::number::predicate;
use crate::number::tolerance::ALIGNED;
use crate::number::tolerance::CHORDED;
use crate::number::tolerance::PLACED;
use crate::solid::boolean::imprints::Imprints;
use crate::solid::boolean::operation::Operation;
use crate::solid::boolean::sounding::Sounding;
use crate::solid::boolean::splitting::Splitting;
use crate::solid::boolean::splitting::bough::Bough;
use crate::solid::boolean::splitting::bow::Bow;
use crate::solid::boolean::splitting::cells::Cells;
use crate::solid::boolean::splitting::corner::{self, Came, Corner};
use crate::solid::boolean::splitting::cut::Cut;
use crate::solid::boolean::splitting::flare::Flare;
use crate::solid::boolean::splitting::oval::Oval;
use crate::solid::boolean::splitting::reading::Reading;
use crate::solid::boolean::splitting::ripple::Ripple;
use crate::solid::boolean::splitting::traced::{Piece, Traced};
use crate::solid::buckets::Buckets;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::curve::{Curve, Sampled};
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::marchings::Marched;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::quartic::Quartic;
use crate::solid::geometry::quartics::Quartered;
use crate::solid::geometry::surface::Surface;
use crate::solid::meeting::marching::Marching;
use crate::solid::meeting::seeding;
use crate::solid::meeting::{Curves, Meeting};
use crate::solid::named::Named;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::{Face, FaceId};
use glam::{DVec2, DVec3};
use std::f64::consts::FRAC_PI_2;
use std::f64::consts::TAU;
use std::ops::Range;

/// What the sewing reads of an operation that has finished cutting — see
/// [`Combining::sewn`], which is the only thing that makes one.
#[derive(Debug)]
pub(super) struct Sewn<'a> {
    pub(super) kept: &'a [Kept],
    pub(super) loops: &'a Loops<Corner>,
    pub(super) imprints: &'a Imprints,
    pub(super) carried: &'a mut Carried,
}

/// What the `nth` curve of the meeting between `on` and `other` is filed under.
///
/// **Over the two surfaces and which piece rather than over the places**, which
/// is what makes a crossing met from either side key alike — see
/// [`Marched::key`], and [`pairing`], where the pair's own half is worked out.
fn named(on: &Surface, other: &Surface, nth: u32) -> u64 {
    on.paired(other).word(u64::from(nth)).done()
}

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
    /// Everything every curve of this operation is made of — see
    /// [`Combining::sewn`], and [`Carried`], which is why it is one value.
    carried: Carried,
    /// Which surface pairs have been marched already, and the runs each was
    /// laid down as — see [`Combining::march`].
    ///
    /// `paired` is which of them key alike, so a pair met again is told from a
    /// handful rather than from every pair marched so far.
    pairs: Vec<Paired>,
    paired: Buckets,
    /// Every curve either store was asked for, in the stretches [`Paired`]
    /// names.
    ///
    /// **Cached with the pair rather than built per face**, because a handle is
    /// a name and not a reading: which store a curve is in, what it is filed
    /// under and how large its numbers work are settled the once the meeting is
    /// worked out, and two faces meeting one pair would otherwise key it twice.
    curves: Vec<Curve>,
    kept: Vec<Kept>,
    scratch: Scratch,
}

/// One surface pair that had to be walked, and the runs its pieces are.
///
/// **Walked once for the two bodies rather than once per face.** A cylinder is
/// two faces of one surface and a ring is four, so a pair reaches
/// [`Combining::against`] once for each face standing on either of them — and a
/// march is thousands of corrections where every other meeting here is a
/// formula. The pieces are the same pieces whichever face asks, so the first
/// face to ask walks them and the rest are handed the runs.
#[derive(Debug, Clone, Copy)]
struct Paired {
    on: Surface,
    other: Surface,
    /// The stretch of [`Combining::curves`] the meeting's own handles were
    /// filed in.
    from: u32,
    upto: u32,
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
    /// The regions one face has been cut into, plane after plane — cut in
    /// place, for the reason [`Cells`] gives.
    cells: Cells,
    inside: Inside,
    /// A face's boundary in the world, walked as chords: one loop of it on its
    /// way into that face's own parameters, or the whole of it on its way into
    /// the box the face fills. `marks` is in step with `traced` for the first
    /// of those, saying which edge put each place there, and `spread` is the
    /// same marks in step with the flattening — see [`Face::doubled`], a place
    /// the surface has no angle for being written twice.
    traced: Vec<DVec3>,
    marks: Vec<Came>,
    spread: Vec<Came>,
    /// The places one meeting's walks are started from, which are at least one
    /// on every piece of it and for a leaning drill are more — see
    /// [`seeding::seeded`].
    seeds: Vec<DVec3>,
    /// Every place every piece of one meeting was sampled at, each piece naming
    /// the stretch that is its — see [`Combining::trace`].
    sampled: Vec<Sampled>,
    walk: Vec<DVec2>,
    corners: Vec<Corner>,
    /// The stretch of its own parameters the face being cut was laid out in —
    /// see [`imprinted`], which is the one thing that reads it.
    ///
    /// A face may not wrap, so that stretch is less than a whole turn wide in
    /// each parameter and at most one turn of a wrapping cut falls inside it.
    /// Both parameters, a torus running round twice over. Meaningless for a
    /// plane, whose parameters do not wrap, and read by nothing for one.
    laid: Bounds<DVec2>,
    /// The one walk a marched pair is laid down by — see [`Combining::march`],
    /// which keeps the room it works in the same way everything here does.
    marching: Marching,
    /// The pieces of one marched meeting that reach the face being cut — see
    /// [`Combining::trace`], which is the only thing that fills it.
    pieces: Vec<Piece>,
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
    fills: Bounds<DVec3>,
}

impl Combining {
    /// Cut both bodies against each other and keep what `doing` asks for.
    ///
    /// `false` where a crossing turns up that nothing here can write down in a
    /// face's own parameters, which is [`imprinted`], or where the sounder
    /// cannot place a region — see [`Combining::sift`]. See `.notes/KERNEL.md`
    /// §8's `Built::Refused`.
    pub(super) fn combine(&mut self, one: &Body, two: &Body, doing: Operation) -> bool {
        self.loops.clear();
        self.kept.clear();
        self.imprints.clear();
        self.carried.clear();
        self.pairs.clear();
        self.paired.clear();
        self.curves.clear();
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

    /// Everything the sewing reads of an operation that has finished cutting.
    ///
    /// **One borrow rather than four**, which is the compiler asking rather
    /// than a preference: the runs change hands where the sewing ends, so that
    /// one is a `&mut` and the other three cannot be taken beside it a call at
    /// a time.
    pub(super) fn sewn(&mut self) -> Sewn<'_> {
        Sewn {
            kept: &self.kept,
            loops: &self.loops,
            imprints: &self.imprints,
            carried: &mut self.carried,
        }
    }

    /// Cut every face of `mine` against `theirs` and keep what survives.
    ///
    /// `false` where a cut met a shape the splitter does not handle, which is a
    /// refusal like any other here: what is kept would be a region quietly
    /// missing a bite of itself.
    fn against(&mut self, mine: &Body, theirs: &Body, doing: Operation, first: bool) -> bool {
        // Every region of every face of `mine` is sounded against this one
        // body, and the layout is the whole of what does not depend on which
        // region — see [`Sounding::about`].
        self.scratch.sounding.about(theirs);
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
        // the two stand.
        //
        // **What decides is the surface's own reach and never the face's**,
        // asked against the whole of this body rather than the face being cut.
        // Not conservatism: a cut that divides one face and not the face beside
        // it leaves a vertex on one side of the edge they share and none on the
        // other, and the sewing then finds three edges where it wanted two. Cutting further than necessary is not merely tolerated —
        // see [`splitting`] — it has to be uniform.
        //
        // **And a body is already split along its own surfaces**, which is what
        // makes the face's reach the wrong question rather than merely a
        // coarser one. Cut a pocket into a block and the block's top is divided
        // along every wall plane of the tool; cut a second pocket beside it and
        // those planes divide the rim of the new one. The planes reach the new
        // tool's faces and would divide them too — but the faces *standing on*
        // those planes are the first pocket's walls, far away, so a cull that
        // asks about them leaves the new tool whole and the rim divided on one
        // side only. Measured: sixteen of twenty-five pairs of pockets were
        // refused, and none is.
        //
        // Which run of the boxes is whose follows from `first`, the two calls
        // being the two ways round — see [`Scratch::boxed`].
        let split = self.scratch.between..self.scratch.boxed.len();
        let (here, there) = if first {
            (0..self.scratch.between, split)
        } else {
            (split, 0..self.scratch.between)
        };
        // A body with no faces is divided by nothing, and has no box to ask a
        // surface about either.
        if here.is_empty() {
            return true;
        }
        self.scratch.met.clear();
        self.scratch.reached.clear();
        let mut reach = Bounds::default();
        for at in here.clone() {
            reach.swallow(self.scratch.boxed[at].fills);
        }
        for at in there {
            let other = theirs.topology().face(self.scratch.boxed[at].face);
            if !other.surface.reaches(reach, CHORDED) {
                continue;
            }
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
        for at in here {
            let Boxed { face: id, fills } = self.scratch.boxed[at];
            let face = mine.topology().face(id);
            self.lay(mine, face);
            // Copied out of the two lists rather than borrowed from them, so
            // that a cut standing on the pair may borrow the surfaces while the
            // splitter beside it is taken mutably.
            let on = face.surface;
            for which in 0..self.scratch.met.len() {
                let other = self.scratch.met[which];
                // **And only the surfaces that reach this face.** A surface
                // whose faces reach the body was kept above; one that comes
                // nowhere near *this* face of it divides nothing here, and
                // taking the cut anyway leaves a region the next cut walks
                // again — see [`Surface::reaches`], which is also why refusing
                // one keeps the cut uniform.
                if !other.reaches(fills, CHORDED) {
                    continue;
                }
                let along = match Meeting::of(&on, &other) {
                    // Nothing that divides anything. Apart, the same surface —
                    // two faces on one are told apart by where each region
                    // *stands* — or grazing at a point, which is a place rather
                    // than a line and divides no face.
                    Meeting::Apart | Meeting::Same | Meeting::Touching(_) => continue,
                    // They meet along a curve no row of the reducible table
                    // writes down, and the two tiers differ only in what
                    // writes one: a quartic the exact route parameterizes, or
                    // a run walked where nothing writes one at all. Both are
                    // produced *here* rather than by [`Meeting::of`], on one
                    // division — what produces a curve is the caller that
                    // knows which faces it has to be long enough for. See
                    // `.notes/KERNEL.md` §7.3, where that division is argued.
                    laid @ (Meeting::Algebraic | Meeting::Marched) => {
                        let along = match laid {
                            Meeting::Algebraic => self.quartics(&on, &other),
                            _ => self.march(&on, &other),
                        };
                        let Some(along) = along else {
                            return false;
                        };
                        if !self.trace(&on, &other, along) {
                            return false;
                        }
                        continue;
                    }
                    Meeting::Along(along) => along,
                };
                // **A closed form where the face's own parameters hold one,
                // and the curve walked where they do not.** `imprinted` writes
                // a meeting down in the parameters of the surface being cut,
                // and there are pairs it has no line for — a circle leaning
                // across a sphere runs `v = ψ(u) ± acos(…)` there, a graph over
                // the angle with two branches. What answers those is the shape
                // that asks nothing of the parameters at all: a traced cut
                // reads how far a place stands off it from the other *surface*
                // and lays its corners down by walking the curve. So a gap in
                // the table costs sampling rather than the whole boolean.
                //
                // **All of the meeting or none of it**, which the traced cut
                // requires rather than prefers: its reading comes to nought on
                // every piece of the meeting at once, so a cut carrying one
                // piece would call a place on another piece its own — see
                // [`Traced`]. One curve with no closed form sends the pair of
                // them down the walked route.
                if along.all().iter().any(|it| self.written(on, *it).is_none()) {
                    if !self.walked(&on, &other, along) {
                        return false;
                    }
                    continue;
                }
                // Each curve of the meeting in turn: a plane cutting a chord
                // off a cylinder meets it in two, and both divide the face.
                for curve in along.all() {
                    let cut = self
                        .written(on, *curve)
                        .expect("every curve of the meeting was written down");
                    let reading = Reading {
                        on,
                        imprints: &self.imprints,
                        carried: &self.carried,
                    };
                    let Scratch {
                        splitting, cells, ..
                    } = &mut self.scratch;
                    if !splitting.split(cells, cut, reading) {
                        return false;
                    }
                }
            }
            if !self.sift(face, theirs, doing, first) {
                return false;
            }
        }
        true
    }

    /// Walk every piece of the curve `on` and `other` meet in and file it, or
    /// hand back the handles a face already asked for.
    ///
    /// `None` refuses the boolean, and it means one of two things: no seeding
    /// is written for the pair, or a walk was seeded and did not come back. A
    /// pair that genuinely *misses* is neither — it comes back with no runs at
    /// all, and divides nothing.
    fn march(&mut self, on: &Surface, other: &Surface) -> Option<Range<u32>> {
        if let Some(had) = self.filed(on, other) {
            return Some(had);
        }
        let from = self.curves.len() as u32;
        // Which of the two is the ring. A pair of them has no reading written
        // for it, and falls out of the seeding rather than being turned away
        // here — see [`seeding::seeded`].
        //
        // Neither being one is a pair the exact table should have answered:
        // every meeting that arrives here has a fitted half, and one that did
        // not would be a hole in that table rather than a walk to attempt.
        let (surface, torus) = match (on, other) {
            (Surface::Fitted(Fitted::Torus(torus)), surface)
            | (surface, Surface::Fitted(Fitted::Torus(torus))) => (surface, *torus),
            _ => return None,
        };
        if !seeding::seeded(surface, &torus, &mut self.scratch.seeds) {
            return None;
        }
        for step in 0..self.scratch.seeds.len() {
            let seed = self.scratch.seeds[step];
            // **A piece already walked is not walked again.** A leaning drill
            // is offered a place for every turn of every stretch of the tube
            // its curve holds one over, and one piece of it spans several — see
            // [`seeding::seeded`]. A run stands within its own sagitta of the
            // curve it was walked on, so a seed on that curve stands within the
            // same of the run, and two pieces that came that close would be a
            // tangency this walk could not tell apart either.
            let laid = self.curves[from as usize..].iter().any(|curve| {
                let Curve::Marched(had) = curve else {
                    return false;
                };
                let runs = &self.carried.marched;
                let near = runs.at(had.run, runs.along(had.run, seed));
                near.distance(seed) <= CHORDED
            });
            if laid {
                continue;
            }
            // **Walked at the classification tolerance**, which is as fine as a
            // marched edge can be drawn: nothing downstream can lay a run down
            // again, so the sagitta here is the one the edge carries — see
            // [`Marchings::steps`].
            let strayed = self.scratch.marching.walk(on, other, seed, CHORDED)?;
            let run = self
                .carried
                .marched
                .add(self.scratch.marching.walked(), strayed);
            self.curves.push(Curve::Marched(Marched {
                run,
                // Over the two surfaces and which piece rather than over the
                // places — see [`Marched::key`], and [`pairing`], which is what
                // makes a crossing met from either side key alike.
                key: named(on, other, self.curves.len() as u32 - from),
                reach: self.carried.marched.strayed(run).reach,
            }));
        }
        Some(self.file(on, other, from))
    }

    /// The handles already filed for the meeting of `on` and `other`, where it
    /// has been worked out before.
    ///
    /// **One cache for both tiers**, because what it holds is neither's: a
    /// stretch of [`Combining::curves`], which says nothing about how the
    /// curves in it were made. A pair is met once per face of each body, and
    /// working it out twice would be a walk or an algebraic route run twice.
    fn filed(&self, on: &Surface, other: &Surface) -> Option<Range<u32>> {
        let key = on.paired(other).done();
        let at = self.paired.under(key).find(|&at| {
            let it = &self.pairs[at as usize];
            (it.on == *on && it.other == *other) || (it.on == *other && it.other == *on)
        })?;
        let it = &self.pairs[at as usize];
        Some(it.from..it.upto)
    }

    /// File the handles pushed since `from` as what `on` and `other` meet in.
    fn file(&mut self, on: &Surface, other: &Surface, from: u32) -> Range<u32> {
        let upto = self.curves.len() as u32;
        let slot = self.paired.file(on.paired(other).done());
        debug_assert_eq!(slot as usize, self.pairs.len(), "the index lost step");
        self.pairs.push(Paired {
            on: *on,
            other: *other,
            from,
            upto,
        });
        from..upto
    }

    /// The components of the quartic `on` and `other` meet in, filed and named.
    ///
    /// **The pair to [`Combining::march`] one tier down**, and cached the same
    /// way: a pair met from both its faces is worked out once, and what a face
    /// asks for afterwards is the stretch of handles it left behind.
    ///
    /// `None` for a pair the algebraic route cannot write down, which is a
    /// meeting the boolean has been told exists and cannot build on — the same
    /// refusal a seeding nobody has written gives one tier up.
    fn quartics(&mut self, on: &Surface, other: &Surface) -> Option<Range<u32>> {
        if let Some(had) = self.filed(on, other) {
            return Some(had);
        }
        let from = self.curves.len() as u32;
        let curve = Quartic::of(on.quadric()?, other.quadric()?)?;
        for component in curve.components().all() {
            let run = self
                .carried
                .quartics
                .add(curve.clone(), component.arc, component.closing);
            self.curves.push(Curve::Quartic(Quartered {
                run,
                key: named(on, other, self.curves.len() as u32 - from),
                reach: self.carried.quartics.reach(run),
            }));
        }
        Some(self.file(on, other, from))
    }

    /// How `curve` divides the face being laid out, in that face's own
    /// parameters — `None` where nothing here writes that shape down.
    ///
    /// **Numbered before the cut is built rather than after**, which is what a
    /// round cut needs: it carries its own number — see [`Cut::Round`] — so
    /// there is nothing to hand back. A straight imprint carries none and
    /// spends none.
    ///
    /// **Asked twice of every curve that has one**, once to find out whether the
    /// whole meeting is written down and once to cut by it. That costs a second
    /// reading of the table and no second run: the number a curve takes is the
    /// number it already has — see [`Imprints::crossing`].
    fn written(&mut self, on: Surface, curve: Curve) -> Option<Cut<'static>> {
        let run = match curve {
            Curve::Line(_) => None,
            _ => Some(self.imprints.crossing(curve)),
        };
        imprinted(on, curve, run, self.scratch.laid)
    }

    /// Cut the face being worked on by a meeting its own parameters have no
    /// line for, walking the curves instead.
    ///
    /// **The floor under [`Combining::written`]**, and the route the quartic
    /// and the marched tiers already take: the curves are filed for the pair
    /// exactly as those file theirs, so a meeting worked out for one face is
    /// walked once and read by every face after it.
    ///
    /// **An open curve is refused rather than walked.** A traced cut samples a
    /// whole turn of its curve's own parameter and orders places by how far
    /// round they stand, and a line, a parabola and a hyperbola's branch have
    /// neither — see [`Curve::closed`]. A line lies on a plane, a cylinder or a
    /// cone, and the first two hold it outright; the two open conics lie on a
    /// plane and a cone, and only the plane holds them. So what reaches here is
    /// an open conic on the *cone*, which is where `.notes/KERNEL.md` §9.2
    /// still owes a cut.
    fn walked(&mut self, on: &Surface, other: &Surface, along: Curves) -> bool {
        if along.all().iter().any(|it| !it.closed()) {
            return false;
        }
        let curves = match self.filed(on, other) {
            Some(had) => had,
            None => {
                let from = self.curves.len() as u32;
                for curve in along.all() {
                    self.curves.push(*curve);
                }
                self.file(on, other, from)
            }
        };
        self.trace(on, other, curves)
    }

    /// Cut the face being worked on by what `on` and `other` meet in, which is
    /// the curves filed at `curves`.
    ///
    /// **One cut for the whole meeting rather than one per piece**, which is
    /// what its reading asks for: how far a place stands off a traced cut is
    /// read off the other *surface*, so it comes to nought on every piece at
    /// once and a cut carrying one piece would call a place on another piece
    /// its own. See [`Traced`].
    ///
    /// Nothing at all where no piece reaches the face, which divides nothing.
    fn trace(&mut self, on: &Surface, other: &Surface, curves: Range<u32>) -> bool {
        self.scratch.pieces.clear();
        self.scratch.sampled.clear();
        for at in curves {
            let curve = self.curves[at as usize];
            // **Every piece's places in one buffer, each naming its own.** A
            // cut is asked about a place by every corner of every region, so
            // the walk it reads is laid down once for the whole meeting rather
            // than kept per piece — the arrangement [`Loops`] keeps a face's
            // own loops in.
            let from = self.scratch.sampled.len();
            curve.sample(TAU, CHORDED, &self.carried, &mut self.scratch.sampled);
            // Numbered whether or not it reaches this face, so that a piece
            // carries the one number wherever it is imprinted.
            let numbered = self.imprints.crossing(curve);
            let piece = Piece::of(
                on,
                other,
                &self.scratch.sampled,
                [from, self.scratch.sampled.len()],
                self.scratch.laid,
                curve,
                numbered,
            );
            if let Some(piece) = piece {
                self.scratch.pieces.push(piece);
            }
        }
        if self.scratch.pieces.is_empty() {
            return true;
        }
        // Taken apart before the cut is made, the cut standing on two of these
        // while the splitter takes two more.
        let Scratch {
            splitting,
            cells,
            sampled,
            laid,
            pieces,
            ..
        } = &mut self.scratch;
        let cut = Cut::Traced(Traced::of(on, other, &self.carried, sampled, *laid, pieces));
        let reading = Reading {
            on: *on,
            imprints: &self.imprints,
            carried: &self.carried,
        };
        splitting.split(cells, cut, reading)
    }

    /// The box `face` fills, walked as chords at [`CHORDED`].
    ///
    /// Off the boundary, which is enough for every surface but a sphere — see
    /// [`Surface::fills`], where that argument is written down and where the
    /// one it is not enough for is widened.
    fn reach(&mut self, body: &Body, face: &Face) -> Bounds<DVec3> {
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
        let mut laid = Bounds::default();
        let Self {
            scratch, imprints, ..
        } = self;
        let Scratch {
            cells,
            walk,
            corners,
            traced,
            marks,
            spread,
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
                // The turn the outline was laid out in, which every hole of
                // the face is read into as well — see [`Face::flatten`]. The
                // outline comes first, so what `laid` already holds is it.
                let about = (at > 0).then(|| laid.middle());
                face.flatten(traced, about, walk);
                for at in walk.iter() {
                    laid.hold(*at);
                }
                spread.clear();
                face.doubled(traced, marks, spread);
                corners.clear();
                corners.extend(
                    walk.iter()
                        .zip(spread.iter())
                        .map(|(&at, &came)| Corner { at, came }),
                );
                if at == 0 {
                    turned = winding::doubled(corners) < 0.0;
                }
                if turned {
                    corner::turned(corners);
                }
                loops.push(corners);
            }
        });
        self.scratch.laid = laid;
    }

    /// Ask every region where it stands and keep the ones `doing` wants.
    ///
    /// `false` where the sounder could not place a region at all, which is a
    /// refusal like any other here: keeping the region or dropping it would
    /// both be a guess, and one of the two leaves material where there is none.
    fn sift(&mut self, face: &Face, theirs: &Body, doing: Operation, first: bool) -> bool {
        for at in 0..self.scratch.cells.len() {
            let Some(within) = self.within(at) else {
                continue;
            };
            let Some(standing) = self
                .scratch
                .sounding
                .standing(face.surface.at(within), theirs)
            else {
                return false;
            };
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
        true
    }

    /// A place well within the region at `at`, or `None` where it covers
    /// nothing to be within — see [`Inside::of`], which is the whole of the
    /// rule.
    fn within(&mut self, at: usize) -> Option<DVec2> {
        let Scratch { cells, inside, .. } = &mut self.scratch;
        inside.of(cells.cell(at))
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
/// **A marched meeting is not here**, and the reason is that it is not one
/// curve: it comes in pieces and one cut carries all of them, so what makes it
/// is a pair of surfaces rather than a curve — see [`Combining::trace`].
///
/// `run` is the run the curve was given, and `None` where it was given none
/// because it is a straight line — see [`Imprints`]. The round arms want one
/// and the straight arms do not, which is exactly the two states of that
/// argument.
fn imprinted(
    on: Surface,
    along: Curve,
    run: Option<u32>,
    laid: Bounds<DVec2>,
) -> Option<Cut<'static>> {
    let about = laid.middle();
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
        ) if predicate::parallel(circle.axis.direction, axis.direction) => Some(Cut::Straight {
            at: DVec2::new(0.0, axis.along(circle.axis.origin)),
            along: DVec2::X,
            run,
        }),
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
        // [`Combining::walked`].
        (Surface::Natural(Natural::Sphere(sphere)), Curve::Circle(circle))
            if predicate::parallel(circle.axis.direction, sphere.axis.direction) =>
        {
            Some(Cut::Straight {
                at: DVec2::new(0.0, sphere.uv(circle.at(0.0)).y),
                along: DVec2::X,
                run,
            })
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
            Some(Cut::Straight {
                at: DVec2::new(branch::nearest(angle, about.x), 0.0),
                along: DVec2::Y,
                run: None,
            })
        }
        // **A ruling on a cone is one straight cut across both nappes**, which
        // its parameters make of a line through the apex: `u = that`, the same
        // number either side. A place at a negative `v` is measured from the
        // apex *back* along the ray — see [`Cone::uv`] — so the angle the ray
        // going one way stands at is the angle the ray going the other way
        // stands at, and the ruling is one line of the chart rather than two.
        //
        // What a plane through the apex leaves — see [`Meeting::apexed`] — and
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
            Some(Cut::Straight {
                at: DVec2::new(branch::nearest(angle, about.x), 0.0),
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
                Some(Cut::Straight {
                    at: DVec2::new(0.0, branch::nearest(uv.y, about.y)),
                    along: DVec2::X,
                    run,
                })
            } else if predicate::square(circle.axis.direction, axis.direction) {
                Some(Cut::Straight {
                    at: DVec2::new(branch::nearest(uv.x, about.x), 0.0),
                    along: DVec2::Y,
                    run,
                })
            } else {
                None
            }
        }
        // Everything else.
        _ => None,
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

/// What the cutting left, read a piece at a time.
///
/// Nothing production reads it that way: [`Combining::sewn`] hands the whole of
/// it over in one borrow, which is what the runs changing hands asks for. Taking
/// one piece is a test holding the cutting to what it should have produced.
#[cfg(test)]
pub(crate) mod internals {
    use super::*;

    impl Combining {
        pub(crate) fn kept(&self) -> &[Kept] {
            &self.kept
        }

        pub(crate) fn loops(&self) -> &Loops<Corner> {
            &self.loops
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::geometry::circle::Circle;
    use crate::solid::geometry::sphere::Sphere;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_6};

    /// **A circle on a sphere square to its axis is the line `v = that`**,
    /// where a circle on a cylinder is the line at a height.
    ///
    /// A ball of radius two about the origin, spun about the world's `+y`. The
    /// circle at `y = 1` on it stands where `sin v = 1/2`, so the cut is the
    /// line `v = π/6` — the whole of the hand computation, and the reason the
    /// two cannot share the cylinder's arm: that one carries a *distance* along
    /// the axis where this carries an angle up from the equator.
    ///
    /// **And a circle that leans is not written down at all**, there being no
    /// straight line in these parameters holding one: it runs at an angle that
    /// moves with the angle round. What cuts by one is
    /// [`Combining::walked`], and the boolean over it is held by
    /// `a_ball_halved_by_a_leaning_plane_keeps_the_circle_it_was_cut_by`.
    #[test]
    fn a_circle_square_to_a_sphere_is_a_straight_cut_at_its_own_angle() {
        let sphere = Sphere {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            radius: 2.0,
        };
        let on = Surface::Natural(Natural::Sphere(sphere));
        let laid = Bounds {
            low: DVec2::new(0.0, -FRAC_PI_2),
            high: DVec2::new(TAU, FRAC_PI_2),
        };
        let square = Curve::Circle(Circle {
            axis: Axis::new(DVec3::Y, DVec3::Y, DVec3::X),
            radius: 3.0f64.sqrt(),
        });
        let Some(Cut::Straight { at, along, .. }) = imprinted(on, square, Some(0), laid) else {
            panic!("a circle square to the axis is no straight cut");
        };
        assert!((at.y - FRAC_PI_6).abs() < 1e-12, "{at:?}");
        assert_eq!(along, DVec2::X, "the cut runs the wrong way");

        let leaning = Curve::Circle(Circle {
            axis: Axis::new(DVec3::ZERO, DVec3::new(0.0, 1.0, 1.0).normalize(), DVec3::X),
            radius: 2.0,
        });
        assert!(
            imprinted(on, leaning, Some(0), laid).is_none(),
            "a leaning circle was written down as a straight cut",
        );
    }

    /// **A meeting one of whose curves has no closed form is walked whole.**
    ///
    /// A traced cut reads how far a place stands off it from the other
    /// *surface*, and that reading comes to nought on every piece of the
    /// meeting at once — so a cut carrying one piece would call a place on
    /// another piece its own. A sphere on a cylinder's axis meets it in two
    /// circles, and those are square to the sphere's axis and written down; the
    /// same pair on a *leaning* sphere gives two circles neither of which is,
    /// and both go down the walked route together.
    #[test]
    fn a_meeting_is_written_down_whole_or_walked_whole() {
        let laid = Bounds {
            low: DVec2::new(0.0, -FRAC_PI_2),
            high: DVec2::new(TAU, FRAC_PI_2),
        };
        let sphere = |direction| {
            Surface::Natural(Natural::Sphere(Sphere {
                axis: Axis::new(DVec3::ZERO, direction, DVec3::X),
                radius: 2.0,
            }))
        };
        let tube = Surface::Natural(Natural::Cylinder(Cylinder {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            radius: 1.0,
        }));
        let Meeting::Along(along) = Meeting::of(&sphere(DVec3::Y), &tube) else {
            panic!("a sphere on a cylinder's axis meets it in circles");
        };
        assert_eq!(along.all().len(), 2, "one circle either side of the axis");
        let upright = sphere(DVec3::Y);
        let leaning = sphere(DVec3::new(0.0, 1.0, 1.0).normalize());
        for curve in along.all() {
            assert!(
                imprinted(upright, *curve, Some(0), laid).is_some(),
                "a circle square to the sphere's own axis is the line `v = that`",
            );
            assert!(
                imprinted(leaning, *curve, Some(0), laid).is_none(),
                "a circle leaning across a sphere is no straight cut",
            );
        }
    }

    /// **The two branches of one hyperbola are two cuts, and a face holds one
    /// of them.**
    ///
    /// A plane past a cone's rulings cuts an arc on each nappe, and the two
    /// carry the identical reading — the same plane, read the same way, so
    /// [`Flare`] cannot tell them apart by its own numbers. What tells them
    /// apart is the nappe, and a face lies on one: the branch on the other
    /// divides it nowhere, so the cut is culled and the region is put aside
    /// whole.
    ///
    /// **Numbered by the branch it is and not by the face it divides**, which
    /// is what the two sides of one arc have to agree on. Read the other way a
    /// face is cut twice by one shape under two numbers, and the face across
    /// that arc then breaks its edge along a run the first has never heard of.
    ///
    /// Hand-computed. The cone is one across for every two along, apex at
    /// `(0, 4, 0)` and opening down, so the wall runs `v` from nought at the
    /// apex to four at the base. The plane `x = 1` runs parallel to the axis,
    /// so it cuts a hyperbola whose vertices stand at `(1, 2, 0)` on the near
    /// nappe and `(1, 6, 0)` on the far.
    #[test]
    fn the_two_branches_of_one_hyperbola_cut_the_nappe_each_stands_on() {
        let cone = Cone {
            axis: Axis::new(DVec3::new(0.0, 4.0, 0.0), DVec3::NEG_Y, DVec3::X),
            half_angle: 0.5_f64.atan(),
        };
        let on = Surface::Natural(Natural::Cone(cone));
        let alongside = Surface::Natural(Natural::Plane(Axis::about(DVec3::X, DVec3::X).plane()));
        let Meeting::Along(along) = Meeting::of(&on, &alongside) else {
            panic!("a plane parallel to the axis cuts a hyperbola");
        };
        assert_eq!(
            along.all().len(),
            2,
            "{:?} is not two branches",
            along.all()
        );

        // The lateral wall, which stands on the nappe `v` reads positive on.
        let laid = Bounds {
            low: DVec2::new(-FRAC_PI_2, 0.0),
            high: DVec2::new(FRAC_PI_2, 4.0),
        };
        let mut nappes = [false; 2];
        for (at, curve) in along.all().iter().enumerate() {
            let Some(Cut::Flare(flare)) = imprinted(on, *curve, Some(0), laid) else {
                panic!("{curve:?} on a cone is no flare");
            };
            assert_eq!(flare.reaches(laid), flare.upward, "{flare:?}");
            nappes[at] = flare.upward;
        }
        assert_ne!(
            nappes[0], nappes[1],
            "the two branches were read onto one nappe",
        );
    }
}
