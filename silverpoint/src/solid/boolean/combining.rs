//! Combining two bodies, stage by stage.

use crate::loops::Loops;
use crate::math::bounds::Bounds;
use crate::math::chorded::Chorded;
use crate::math::inside::Inside;
use crate::math::winding;
use crate::number::tolerance::CHORDED;
use crate::solid::boolean::imprints::Imprints;
use crate::solid::boolean::operation::Operation;
use crate::solid::boolean::sounding::Sounding;
use crate::solid::boolean::splitting::Splitting;
use crate::solid::boolean::splitting::cells::Cells;
use crate::solid::boolean::splitting::corner::{self, Came, Corner};
use crate::solid::boolean::splitting::cut::Cut;
use crate::solid::boolean::splitting::reading::Reading;
use crate::solid::boolean::splitting::traced::{Piece, Traced};
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::curve::{Curve, Sampled};
use crate::solid::geometry::fitted::Fitted;
use crate::solid::geometry::marchings::Marched;
use crate::solid::geometry::quartic::Quartic;
use crate::solid::geometry::quartics::Quartered;
use crate::solid::geometry::surface::Surface;
use crate::solid::keyed::Keyed;
use crate::solid::meeting::marching::Marching;
use crate::solid::meeting::seeding;
use crate::solid::meeting::{Curves, Meeting};
use crate::solid::named::Named;
use crate::solid::topology::body::Body;
use crate::solid::topology::face::{Face, FaceId};
use glam::{DVec2, DVec3};
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
    /// Keyed, so a pair met again is told from a handful rather than from every
    /// pair marched so far.
    pairs: Keyed<Paired>,
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
    /// Keyed, so a face's surface is told from a handful rather than compared
    /// against every surface already collected.
    met: Keyed<Surface>,
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
                .met
                .under(key)
                .any(|(_, held)| *held == other.surface)
            {
                continue;
            }
            self.scratch.met.file(key, other.surface);
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
                let other = self.scratch.met.all()[which];
                if !self.cut(on, other, fills) {
                    return false;
                }
            }
            if !self.sift(face, theirs, doing, first) {
                return false;
            }
        }
        true
    }

    /// Cut the face being laid out by everything it shares with `other`.
    ///
    /// **Nothing to cut is an answer and not a refusal.** A surface that comes
    /// nowhere near this face, one the face already stands on, and one meeting
    /// it at a point all divide nothing — so `true` covers both the cut taken
    /// and the cut there was no call for.
    ///
    /// `false` refuses the boolean, on the terms
    /// [`Boolean::combine`](crate::solid::boolean::Boolean) lists.
    fn cut(&mut self, on: Surface, other: Surface, fills: Bounds<DVec3>) -> bool {
        // **Only the surfaces that reach this face.** A surface whose faces
        // reach the body was kept by the caller; one that comes nowhere near
        // *this* face of it divides nothing here, and taking the cut anyway
        // leaves a region the next cut walks again — see [`Surface::reaches`],
        // which is also why refusing one keeps the cut uniform.
        if !other.reaches(fills, CHORDED) {
            return true;
        }
        let along = match Meeting::of(&on, &other) {
            // Nothing that divides anything. Apart, the same surface — two
            // faces on one are told apart by where each region *stands* — or
            // grazing at a point, which is a place rather than a line and
            // divides no face.
            Meeting::Apart | Meeting::Same | Meeting::Touching(_) => return true,
            // They meet along a curve no row of the reducible table writes
            // down, and the two tiers differ only in what writes one: a quartic
            // the exact route parameterizes, or a run walked where nothing
            // writes one at all. Both are produced *here* rather than by
            // [`Meeting::of`], on one division — what produces a curve is the
            // caller that knows which faces it has to be long enough for. See
            // `.notes/KERNEL.md` §7.3, where that division is argued.
            laid @ (Meeting::Algebraic | Meeting::Marched) => {
                let along = match laid {
                    Meeting::Algebraic => self.quartics(&on, &other),
                    _ => self.march(&on, &other),
                };
                let Some(along) = along else {
                    return false;
                };
                return self.trace(&on, &other, along);
            }
            Meeting::Along(along) => along,
        };
        // **A closed form where the face's own parameters hold one, and the
        // curve walked where they do not.** [`Cut::of`] writes a meeting down
        // in the parameters of the surface being cut, and there are pairs it
        // has no line for — a circle leaning across a sphere runs
        // `v = ψ(u) ± acos(…)` there, a graph over the angle with two branches.
        // What answers those is the shape that asks nothing of the parameters
        // at all: a traced cut reads how far a place stands off it from the
        // other *surface* and lays its corners down by walking the curve. So a
        // gap in the table costs sampling rather than the whole boolean.
        //
        // **All of the meeting or none of it**, which the traced cut requires
        // rather than prefers: its reading comes to nought on every piece of
        // the meeting at once, so a cut carrying one piece would call a place
        // on another piece its own — see [`Traced`]. One curve with no closed
        // form sends the pair of them down the walked route.
        if along.all().iter().any(|it| self.written(on, *it).is_none()) {
            return self.walked(&on, &other, along);
        }
        // Each curve of the meeting in turn: a plane cutting a chord off a
        // cylinder meets it in two, and both divide the face.
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
            let filed = self.carried.marched.strayed(run);
            self.curves.push(Curve::Marched(Marched {
                run,
                // Over the two surfaces and which piece rather than over the
                // places — see [`Marched::key`], and [`pairing`], which is what
                // makes a crossing met from either side key alike.
                key: named(on, other, self.curves.len() as u32 - from),
                reach: filed.reach,
                shut: filed.shut,
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
        let (_, it) = self.pairs.under(key).find(|(_, it)| {
            (it.on == *on && it.other == *other) || (it.on == *other && it.other == *on)
        })?;
        Some(it.from..it.upto)
    }

    /// File the handles pushed since `from` as what `on` and `other` meet in.
    fn file(&mut self, on: &Surface, other: &Surface, from: u32) -> Range<u32> {
        let upto = self.curves.len() as u32;
        self.pairs.file(
            on.paired(other).done(),
            Paired {
                on: *on,
                other: *other,
                from,
                upto,
            },
        );
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
        Cut::of(on, curve, run, self.scratch.laid)
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
        for walk in topology.loops_of(face) {
            topology.trace(walk, CHORDED, &mut self.scratch.traced);
        }
        face.surface
            .fills(self.scratch.traced.iter().copied().collect())
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
            let mut about = None;
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
                face.flatten(traced, &mut about, walk);
                laid.extend(walk.iter().copied());
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
