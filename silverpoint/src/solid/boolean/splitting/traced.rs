//! The cut a curve makes in a face's own parameters, traced from its own
//! places.
//!
//! **The general shape, where every other here is a closed form.** A cut knows
//! how far a place stands off it by asking the *other surface*, and lays its
//! corners down by walking the curve — neither of which asks how the curve was
//! made. So one cut serves the fitted tier's marched runs and the exact tier's
//! quartics alike, and the quartic arm took this cut rather than growing a
//! second copy of it.

use crate::loops::Loops;
use crate::math::bisect;
use crate::math::bounds::Bounds;
use crate::math::branch;
use crate::math::intersect::{self, Span};
use crate::math::winding;
use crate::number::tolerance::PLACED;
use crate::solid::boolean::splitting::corner::{Came, Corner};
use crate::solid::boolean::splitting::dipped::Dipped;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::curve::{Curve, Sampled};
use crate::solid::geometry::surface::Surface;
use glam::DVec2;
use std::f64::consts::TAU;

/// The longest stretch of a run that stands clear of the face, as it is walked.
///
/// **What a marched cut measures its own parameter from.** A run is closed in
/// space and its parameter is a whole turn round it, so where that turn reads
/// nought is wherever the walk happened to be seeded — and a piece that merely
/// crosses a face is then a stretch of parameter with the wrap somewhere in the
/// middle of it, which is an ordering the reassembly cannot use. Measured from
/// a place the face does not hold, the wrap falls outside the face and every
/// crossing inside it orders along the cut.
///
/// The middle of the longest such stretch rather than any place in one, so the
/// nought stands as far from the face as the run allows.
#[derive(Debug, Clone, Copy, Default)]
struct Clear {
    /// Where the stretch being walked began and where it has reached.
    from: f64,
    upto: f64,
    /// Whether one is being walked at all.
    walking: bool,
    /// How long the longest one shut so far is, and the middle of it.
    widest: f64,
    middle: f64,
}

impl Clear {
    /// Carry the stretch being walked out to `at`, beginning one if none is.
    fn reach(&mut self, at: f64) {
        if self.walking {
            self.upto = at;
        } else {
            (self.from, self.upto, self.walking) = (at, at, true);
        }
    }

    /// Shut the stretch being walked, if any, and keep it if it is the longest.
    ///
    /// **Measured round the circle rather than along the line**, which the
    /// widest stretch always needs: the walk is begun in the middle of it — see
    /// [`Piece::from`] — so it is the one stretch that runs off the end of the
    /// run's own parameter and back to the start of it. Read as a difference,
    /// a stretch from `5.5` round to `0.8` comes back as four fifths of a turn
    /// wide with its middle at `3.15`, which is the far side of the run and a
    /// place the face holds. The nought of [`Traced::downed`] then stands
    /// *inside* the face, every crossing in it wraps, and the reassembly asks
    /// for the stretch it is not walking.
    ///
    /// The walk runs the way the run's own parameter grows and wraps once, so
    /// how far it carried is that difference taken round.
    fn shut(&mut self) {
        if !self.walking {
            return;
        }
        self.walking = false;
        let span = (self.upto - self.from).rem_euclid(TAU);
        if span > self.widest {
            (self.widest, self.middle) = (span, branch::halfway(self.from, self.upto));
        }
    }
}

/// One piece of a meeting, as it falls in one face's parameters.
///
/// **A meeting comes in pieces and a cut is the whole of it**, which is what
/// separates a traced cut from every closed form here. How far a place stands
/// off one is read off the *other surface* — see [`Traced::side`] — and that
/// reading comes to nought on every piece at once, so a cut that carried one
/// piece would call a place on another piece its own. So the pieces are carried
/// together, and what is per piece is here.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Piece {
    /// The curve it is a piece of, which is what a place on it is read
    /// through — see [`Curve::along`].
    curve: Curve,
    /// Which of the caller's runs its corners are marked with — see
    /// [`Came::Arc`]. One per piece, two pieces of one meeting being two
    /// curves and two edges.
    run: u32,
    /// Where its own parameter reads nought — see [`Clear`].
    phase: f64,
    /// How many whole turns of each wrapping parameter the run is carried by
    /// before it is read as this face's own.
    ///
    /// **Which turn a run lands in is the walk's business, not the face's.**
    /// Carrying a run on keeps it continuous but leaves it in whichever turn it
    /// was seeded in — and a run right round a torus's tube, walked the way the
    /// angle shrinks, comes out a whole turn *below* the face that holds it.
    /// So the whole run is moved to the turn its middle stands nearest the
    /// face's in, which is what the face's own box in these parameters says.
    shift: DVec2,
    /// The stretch of parameters it fills, which is what turns a graze into two
    /// comparisons for the boundary runs that reach nowhere near it.
    fills: Bounds<DVec2>,
    /// Whether the run's parameter grows the way the cut runs, for the cut that
    /// keeps the inside of the other surface.
    ///
    /// **Measured rather than reasoned.** Which way a marched run was walked is
    /// the marcher's business and the surfaces' own orientation is neither
    /// cut's, so the one honest answer is to step along the run once and look.
    ///
    /// Read against [`Traced::inward`] rather than turned over with the cut,
    /// which is what lets the pieces be shared by both ways round.
    forward: bool,
    /// Which of the cut's own sampled places are its, as the first and the one
    /// past the last — see [`Traced::sampled`], which is one buffer for every
    /// piece.
    ///
    /// Two numbers rather than a `Range`, which is not [`Copy`] and this is:
    /// every corner of every region asks a piece about itself by value.
    taken: [usize; 2],
    /// Which of its own samples the walk over it begins at.
    ///
    /// **Where the run stands clear of the face** — see [`Piece::of`], which is
    /// where that is found and why it has to be so. A run the face wholly holds
    /// begins where it was sampled, there being nothing to rotate.
    from: usize,
    /// Whether it closes inside the face rather than running across it.
    ///
    /// A curve that is closed in space is not closed in a face's parameters
    /// where those parameters wrap: a loop right round a ring comes back to
    /// where it began in the world and a whole turn further along in `v`. Both
    /// of a torus's wrap, so both regimes reach here.
    closed: bool,
}

impl Piece {
    /// The piece the run at `marched` makes on `on` where it meets `other`, or
    /// `None` where it stands clear of the face `laid` was laid out in.
    ///
    /// `run` is the number its corners carry.
    pub(crate) fn of(
        on: &Surface,
        other: &Surface,
        sampled: &[Sampled],
        taken: [usize; 2],
        laid: Bounds<DVec2>,
        curve: Curve,
        run: u32,
    ) -> Option<Self> {
        let sampled = &sampled[taken[0]..taken[1]];
        let about = laid.middle();
        // **Begun where the run stands clear of the face.** A run longer than
        // the face's own range in a parameter that wraps — a curve right round
        // a cylinder, laid into one of the two faces a cylinder comes in —
        // covers that face whichever turn it is carried onto. Begun inside, it
        // leaves at one edge and comes back at the other, and the face reads
        // one arc of it as two; begun outside, it enters once and leaves once.
        //
        // Sounded a sample at a time, each carried onto the turn nearest the
        // face: a face holds less than a whole turn, so a place inside it
        // stands nearer the middle of it than any other turn of that place
        // does. Twice round, so a clear stretch that wraps the list is found
        // whole rather than as its two ends — and capped at one turn, so a run
        // wholly clear of the face is begun at a place of it rather than a lap
        // along. Nothing clear at all leaves the walk where it began, which is
        // a run the face wholly holds and nothing to rotate.
        let count = sampled.len();
        let outside = |at: usize| !laid.holds(on.carried(on.uv(sampled[at % count].at), about));
        let (mut from, mut best, mut held) = (0usize, 0usize, 0usize);
        for step in 0..count * 2 {
            held = if outside(step) { held + 1 } else { 0 };
            if held > best && held <= count {
                (best, from) = (held, (step + 1 - held + held / 2) % count);
            }
        }
        // **Whether it closes is the run's business, not the walk's.** Read off
        // the walk as the run was sampled rather than as it is rotated above: a
        // closed run walked from anywhere is closed, and one rotated to begin
        // clear of the face ends a chord short of where it began.
        //
        // **The same place and not a place nearby**, which is why no tolerance
        // is read here. A run that shuts hands back its own first place again
        // as its last — a march pushes the one it began at, and a quartic's
        // parameter wraps onto nought — so a run closed in the *face's*
        // parameters is the very same pair of `f64`s twice over. One closed in
        // space alone is carried onto another turn and stands a whole one off.
        // There is nothing in between for a margin to decide.
        let closed = {
            let mut walked = flattened(on, sampled, 0, about, DVec2::ZERO);
            let first = walked.next()?.1;
            walked.last().is_some_and(|last| last.1 == first)
        };
        let mut walked = flattened(on, sampled, from, about, DVec2::ZERO);
        let (first, second) = (walked.next()?, walked.next()?);
        // A step to the left of the way it runs, which is where the side kept
        // has to be. One chord long: shorter than the curve's own bending, so
        // it cannot cross to the far side, and far enough off that the reading
        // there is a distance rather than a rounding.
        let ahead = second.1 - first.1;
        let left = DVec2::new(-ahead.y, ahead.x);
        let forward = within(
            on,
            other,
            first.1 + left.normalize_or_zero() * ahead.length(),
        ) > 0.0;

        let fills: Bounds<DVec2> = flattened(on, sampled, from, about, DVec2::ZERO)
            .map(|(_, at)| at)
            .collect();
        // Onto the face's own turn, and only where the parameter has turns to
        // be moved by.
        let turns = ((about - fills.middle()) / TAU).round() * TAU;
        let wraps = on.round();
        let shift = DVec2::new(
            if wraps.x { turns.x } else { 0.0 },
            if wraps.y { turns.y } else { 0.0 },
        );
        let fills = fills.moved(shift);
        if !laid.meets(fills, 0.0) {
            return None;
        }
        // **Measured the way the run runs, and turned over once at the end.**
        // [`Clear`] reads how far a stretch carried by the difference of its
        // two ends taken round, which wants them in the order the walk visits
        // them — so which way the cut goes is applied to the answer rather than
        // to every reading that makes it.
        let mut clear = Clear::default();
        for (along, at) in flattened(on, sampled, from, about, shift) {
            if laid.holds(at) {
                clear.shut();
            } else {
                clear.reach(along);
            }
        }
        clear.shut();
        let phase = if forward {
            clear.middle
        } else {
            (TAU - clear.middle).rem_euclid(TAU)
        };
        Some(Self {
            curve,
            taken,
            from,
            run,
            phase,
            shift,
            fills,
            forward,
            closed,
        })
    }
}

/// Which piece of a cut a place stands on, and where along it.
#[derive(Debug, Clone, Copy)]
struct Found {
    piece: usize,
    along: f64,
}

/// A cut along a curve that was walked rather than written down.
///
/// **The one cut here that carries its own geometry.** Every other shape is a
/// formula in the face's two parameters — a line, an ellipse, a cosine, a root
/// of a sine — and a marched curve is none of those. What it carries instead is
/// the two surfaces that made it, and that turns out to be *better* than the
/// runs of places it was walked as: five of the questions a cut answers are
/// about a single place, and how far a place stands from the other surface
/// answers all five in closed form where a walk of the runs would answer them
/// in their own length.
///
/// **So the runs are wanted only where corners are laid down.** [`Traced::down`],
/// [`Traced::between`], [`Traced::walk`], [`Traced::came`] and
/// [`Traced::grazes`] read them; the rest do not.
///
/// **Borrowed rather than carried**, which is the opposite of what
/// [`Curve`] does one shelf down and is
/// the right way round here: a curve is *stored* — in an edge, in the imprints
/// — so a lifetime on one would reach the whole topology, where a cut lives for
/// the one call that splits by it. Carried, two surfaces and a list of pieces
/// would make a [`Cut`](super::cut::Cut) some two hundred bytes, which every
/// corner of every region would then be asked about by value.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Traced<'a> {
    /// The surface this is written on, which is what a parameter lifts through.
    on: &'a Surface,
    /// The surface it meets there, which is what says which side of it a place
    /// falls on.
    other: &'a Surface,
    /// What a place on one of the pieces is read through — see
    /// [`Curve::along`], which every arm answers and which is the only thing
    /// here that knows how a curve was made.
    carried: &'a Carried,
    /// The places every piece was sampled at, one buffer for the lot — see
    /// [`Piece::taken`], which is how each names its own.
    sampled: &'a [Sampled],
    /// The stretch of its own parameters the face was laid out in, which both
    /// the carrying and the laying of corners read.
    laid: Bounds<DVec2>,
    /// The pieces of the meeting that reach this face, in the order they were
    /// walked — see [`Piece`], and [`Traced::down`], which orders along the cut
    /// by this order and then along each piece.
    pieces: &'a [Piece],
    /// Whether the side kept is the one inside [`Traced::other`].
    inward: bool,
}

impl<'a> Traced<'a> {
    /// The cut `pieces` make on `on` where it meets `other`.
    ///
    /// **Inside `other` is what is kept**, exactly as every other row of
    /// `imprinted` keeps the inside first: the splitter cuts both ways round
    /// and each side is read by where it stands, so which is asked first says
    /// nothing about the answer.
    pub(crate) fn of(
        on: &'a Surface,
        other: &'a Surface,
        carried: &'a Carried,
        sampled: &'a [Sampled],
        laid: Bounds<DVec2>,
        pieces: &'a [Piece],
    ) -> Self {
        Self {
            on,
            other,
            carried,
            sampled,
            laid,
            pieces,
            inward: true,
        }
    }

    /// The same cut with the other side kept.
    ///
    /// **The pieces are left alone**, which is what reading their direction
    /// against [`Traced::inward`] buys: turning the cut round turns every piece
    /// round with it, and neither the runs nor this list is written to.
    pub(super) fn turned(self) -> Self {
        Self {
            inward: !self.inward,
            ..self
        }
    }

    /// Whether every piece closes inside the face rather than running across
    /// it.
    pub(super) fn closed(self) -> bool {
        self.pieces.iter().all(|piece| piece.closed)
    }

    /// How far off the cut `at` stands, positive on the side kept.
    ///
    /// **Off the other surface rather than off the pieces**, which is what
    /// makes this a reading and not a walk: a place of one surface is on one
    /// side of their meeting exactly as it stands inside or outside the other,
    /// and how far it stands from a surface is how far it stands from its own
    /// nearest place on it, read along the normal there.
    pub(super) fn side(self, at: DVec2) -> f64 {
        let off = within(self.on, self.other, at);
        if self.inward { off } else { -off }
    }

    /// How far along the cut `at` stands — see `Cut::down`.
    ///
    /// **A whole turn to a piece**, so the number says which piece as well as
    /// where along it: the pieces are disjoint, so ordering by this orders
    /// along each of them and never runs one into the next.
    pub(super) fn down(self, at: DVec2) -> f64 {
        let found = self.found(at);
        found.piece as f64 * TAU + self.downed(self.pieces[found.piece], found.along)
    }

    /// Which piece the parameter `at` runs along.
    ///
    /// **A whole turn of the parameter apiece** — see [`Traced::down`], which is
    /// where that is laid out. A cut of several pieces is several disjoint
    /// curves, so its parameter is not one circle but one circle each, and
    /// anything that wraps has to wrap inside a piece.
    pub(super) fn piece(self, at: f64) -> usize {
        at.div_euclid(TAU) as usize
    }

    /// What the corners the piece at `at` puts down are marked with.
    pub(super) fn came(self, at: DVec2) -> Came {
        Came::Arc(self.pieces[self.found(at).piece].run)
    }

    /// Which piece of the cut `at` stands on, and where along that piece's own
    /// run.
    ///
    /// The nearest one, which is the one it stands on for every place the
    /// callers here ask about: a crossing of the cut, or a corner the cut laid
    /// down itself.
    fn found(self, at: DVec2) -> Found {
        let place = self.on.at(at);
        let mut found = Found {
            piece: 0,
            along: 0.0,
        };
        let mut off = f64::INFINITY;
        for (at, piece) in self.pieces.iter().enumerate() {
            let along = piece.curve.along(place, self.carried);
            let near = piece.curve.at(along, self.carried).distance(place);
            if near < off {
                (off, found) = (near, Found { piece: at, along });
            }
        }
        found
    }

    /// How far along `piece` a parameter of its run stands.
    fn downed(self, piece: Piece, along: f64) -> f64 {
        let along = if piece.forward == self.inward {
            along
        } else {
            TAU - along
        };
        let phase = if self.inward {
            piece.phase
        } else {
            TAU - piece.phase
        };
        (along - phase).rem_euclid(TAU)
    }

    /// Where the straight run from `from` to `to` crosses it.
    ///
    /// **Bisected on the side rather than solved**, there being nothing to
    /// solve: the two ends stand either side of the cut, which every caller has
    /// just established, and how far off a place stands is a reading that
    /// changes sign exactly where the cut is.
    pub(super) fn crossing(self, from: DVec2, to: DVec2) -> DVec2 {
        let at = |along: f64| self.side(from.lerp(to, along));
        let along = bisect::root(0.0, 1.0, at).expect("the run crosses the cut");
        from.lerp(to, along)
    }

    /// Whether any piece of the run gets into the box `fills`.
    ///
    /// **A marched meeting comes in pieces**, each a stretch of curve with a
    /// box of its own — see [`Piece::fills`], which the walk holds for exactly
    /// this kind of question. A run whose pieces all lie clear of a region is a
    /// cut that divides nothing there, and the pieces are where a marched cut
    /// is *local* in a way its two surfaces are not.
    pub(super) fn reaches(self, fills: Bounds<DVec2>) -> bool {
        self.pieces
            .iter()
            .any(|piece| piece.fills.meets(fills, 0.0))
    }

    /// Where the straight run from `from` to `to` crosses it *twice*, both ends
    /// standing on the same side.
    ///
    /// **Against the chords it lays down rather than against the reading**,
    /// which is the one question here a bisection cannot be given a bracket
    /// for: what says there is a dip at all is finding it. The chords are what
    /// the cut puts into a region's boundary — see [`Traced::lay`] — so a dip
    /// found against them is a dip in the loops that come out.
    ///
    /// Two, or the run went across rather than dipping, for the reason
    /// [`Cut::grazes`](super::cut::Cut::grazes) gives — see [`Dipped`], which
    /// is that rule and is shared with the flare's own walk.
    pub(super) fn grazes(self, from: DVec2, to: DVec2) -> Option<[DVec2; 2]> {
        let along = to - from;
        let reach = along.length_squared();
        if reach == 0.0 {
            return None;
        }
        let span = Span { from, to };
        let run: Bounds<DVec2> = [from, to].into_iter().collect();
        let mut dipped = Dipped::default();
        for piece in self.pieces {
            if !piece.fills.meets(run, 0.0) {
                continue;
            }
            let mut walked = self.flattened(*piece);
            let Some((_, mut here)) = walked.next() else {
                continue;
            };
            for (_, next) in walked {
                let chord = Span {
                    from: here,
                    to: next,
                };
                for crossing in intersect::spans(span, chord) {
                    let share = (crossing.at - from).dot(along) / reach;
                    if (PLACED..=1.0 - PLACED).contains(&share) {
                        dipped.hold(crossing.at);
                    }
                }
                here = next;
            }
        }
        dipped.both().map(|met| {
            let shares = met.map(|at| (at - from).dot(along) / reach);
            let (first, second) = (shares[0].min(shares[1]), shares[0].max(shares[1]));
            // **Found against the chords and then read off the surface.** The
            // chords are what says there *is* a dip — a bisection has no
            // bracket until one is found — but a chord stands a sagitta off the
            // curve it was cut from, and the crossing is a corner of three
            // surfaces that the other two faces meeting there work out exactly.
            // Two vertices a sagitta apart is a body the sewing refuses.
            //
            // The dip itself gives the brackets: the run is on the far side at
            // either end and on the near side between, so a crossing is fenced
            // by an end and the middle of the two the chords found. Where the
            // reading does not change sign over that fence — a graze so shallow
            // the chords found what the surface does not — the chord's own
            // answer stands.
            let middle = (first + second) / 2.0;
            let read = |lo: f64, hi: f64, had: f64| {
                bisect::root(lo, hi, |along| self.side(from.lerp(to, along))).unwrap_or(had)
            };
            [
                from.lerp(to, read(0.0, middle, first)),
                from.lerp(to, read(middle, 1.0, second)),
            ]
        })
    }

    /// Whether what the loop through `at` shuts in is the side being kept.
    ///
    /// **The one thing a closed marched cut has in place of a middle.** A
    /// region every corner of which lies on the cut *is* what the cut bounds —
    /// see `kept` — so the question is whether that is the side kept, and a
    /// loop run with the side kept on its left shuts it in exactly when it
    /// winds counterclockwise.
    pub(super) fn holds(self, at: DVec2) -> bool {
        let piece = self.pieces[self.found(at).piece];
        let counterclockwise = winding::doubled_over(self.flattened(piece).map(|(_, at)| at)) > 0.0;
        counterclockwise == (piece.forward == self.inward)
    }

    /// The corners of the cut between two places along it, in the direction it
    /// runs, exclusive of both.
    ///
    /// **`false` where the two stand on different pieces**, which is a join
    /// nothing can make: the pieces are disjoint, so there is no stretch of cut
    /// running from one to the other and no honest set of corners to answer
    /// with.
    ///
    /// A backstop rather than a case, the reassembly looking for the next chain
    /// on the piece this one left off on — see
    /// [`Splitting::close`](super::Splitting), which turns away a piece with no
    /// chain of its own before it asks for corners.
    pub(super) fn between(self, from: f64, to: f64, into: &mut Vec<Corner>) -> bool {
        let which = self.piece(from);
        if which != self.piece(to) {
            return false;
        }
        let piece = self.pieces[which];
        let (from, to) = (from - which as f64 * TAU, to - which as f64 * TAU);
        if piece.closed {
            let sweep = (to - from).rem_euclid(TAU);
            self.lay(piece, from, into, |down| {
                let along = (down - from).rem_euclid(TAU);
                along > 0.0 && along < sweep
            });
            return true;
        }
        // Not wrapped, an open piece running from one edge of the face to the
        // other — and taken backwards where the reassembly asks for it
        // backwards, as an open cut of any other shape is.
        let held = into.len();
        let (low, high) = (from.min(to), from.max(to));
        self.lay(piece, low, into, |down| down > low && down < high);
        if to < from {
            into[held..].reverse();
        }
        true
    }

    /// The cut as loops of corners, each wound so the side kept is on its left.
    ///
    /// **One loop per piece that closes**, and nothing for a piece that does
    /// not: an open one is no loop and bounds nothing on its own.
    pub(super) fn walk(self, into: &mut Loops<Corner>) {
        for &piece in self.pieces.iter().filter(|piece| piece.closed) {
            into.add(|write| self.lay(piece, 0.0, write, |_| true));
        }
    }

    /// Append the places of `piece` that `wanted` keeps, in the order the cut
    /// runs, beginning at the first of them past `origin`.
    ///
    /// **Turned and rotated where they were pushed rather than gathered and
    /// sorted**, which is what keeps laying corners down off the allocator: a
    /// cut is walked once per region per rebuild, and a run copied into a
    /// buffer of its own would be a heap block every time.
    ///
    /// The place it began at is left off the end, a loop's first corner being
    /// its last as well.
    fn lay(self, piece: Piece, origin: f64, into: &mut Vec<Corner>, wanted: impl Fn(f64) -> bool) {
        let held = into.len();
        let mut least = (f64::INFINITY, 0);
        let mut walked = self.flattened(piece).peekable();
        while let Some((along, place)) = walked.next() {
            if walked.peek().is_none() {
                break;
            }
            // **A corner of the face's own, and not merely of the run.** A run
            // is closed in space and its two ends are one place, so the end of
            // a stretch and the run's own beginning can be the same corner read
            // a whole turn apart — and a parameter comparison decides that by a
            // rounding where the face's own stretch decides it outright.
            if !self.laid.holds(place) {
                continue;
            }
            let down = self.downed(piece, along);
            if !wanted(down) {
                continue;
            }
            let past = (down - origin).rem_euclid(TAU);
            if past < least.0 {
                least = (past, into.len() - held);
            }
            into.push(Corner {
                at: place,
                came: Came::Arc(piece.run),
            });
        }
        let laid = into.len() - held;
        if laid == 0 {
            return;
        }
        if piece.forward != self.inward {
            into[held..].reverse();
        }
        // The run is in order and so is the cut, so the corners kept climb from
        // `origin` with one wrap in them — and the wrap is at the least of
        // them, wherever the reversal above has left it.
        let first = if piece.forward == self.inward {
            least.1
        } else {
            laid - 1 - least.1
        };
        into[held..].rotate_left(first);
    }

    /// The places of `piece` in this face's parameters, in the order it was
    /// walked.
    fn flattened(self, piece: Piece) -> impl Iterator<Item = (f64, DVec2)> + 'a {
        flattened(
            self.on,
            &self.sampled[piece.taken[0]..piece.taken[1]],
            piece.from,
            self.laid.middle(),
            piece.shift,
        )
    }
}

/// How far inside `other` the place of `on` at `at` stands.
fn within(on: &Surface, other: &Surface, at: DVec2) -> f64 {
    let place = on.at(at);
    let there = other.uv(place);
    (other.at(there) - place).dot(other.normal(there))
}

/// The places `walked` stands at in `on`'s own parameters, in the order the
/// curve runs.
///
/// **Carried on rather than read afresh**, in whichever parameters the surface
/// runs round: an inversion answers in a half turn either side of the
/// reference, so a run crossing the far side of a cylinder would come back in
/// two pieces a whole turn apart — the rule
/// [`Face::flatten`](crate::solid::topology::face::Face) keeps, kept here for
/// the same reason and started off the turn the face itself was laid out in.
///
/// **Begun at `from` and walked round to it**, which is what lets a closed run
/// be carried on from a place of the caller's choosing rather than from
/// wherever it was sampled — see [`Piece::from`].
fn flattened<'a>(
    on: &'a Surface,
    walked: &'a [Sampled],
    from: usize,
    about: DVec2,
    shift: DVec2,
) -> impl Iterator<Item = (f64, DVec2)> + 'a {
    let mut last = about;
    let count = walked.len();
    (0..count).map(move |step| {
        let sampled = &walked[(from + step) % count];
        last = on.carried(on.uv(sampled.at), last);
        (sampled.along, last + shift)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::geometry::axis::Axis;
    use crate::solid::geometry::fitted::Fitted;
    use crate::solid::geometry::marchings::Marched;
    use crate::solid::geometry::natural::Natural;
    use crate::solid::geometry::torus::Torus;
    use crate::solid::meeting::marching::Marching;
    use crate::solid::meeting::seeding;
    use glam::DVec3;

    /// Every piece of one meeting, laid down and sampled.
    #[derive(Debug)]
    struct Walked {
        carried: Carried,
        curves: Vec<Curve>,
        /// Every piece's places in one buffer, each naming its own.
        sampled: Vec<Sampled>,
        taken: Vec<[usize; 2]>,
    }

    /// The ring every cut below is made on: three out to the tube's own centre,
    /// one thick, about the world's `+Y` through the origin.
    fn ring() -> Surface {
        Surface::Fitted(Fitted::Torus(Torus {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            major: 3.0,
            minor: 1.0,
        }))
    }

    /// The plane through `origin` facing `normal`, framed however.
    fn facing(origin: DVec3, normal: DVec3) -> Surface {
        Surface::Natural(Natural::Plane(
            Axis::about(origin, normal.normalize()).plane(),
        ))
    }

    /// A plane through the ring's middle at forty-five degrees, which cuts two
    /// pieces and neither of them closes: each goes right round the tube.
    fn leaning() -> Surface {
        facing(DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0))
    }

    /// A plane a twentieth inside the ring's outer equator, which cuts one
    /// small piece that closes.
    fn grazing() -> Surface {
        facing(DVec3::X * (3.0 + 1.0 - 0.05), DVec3::X)
    }

    /// A whole turn of each parameter, which holds every piece below and asks
    /// nothing of where the face was laid out.
    fn laid() -> Bounds<DVec2> {
        let mut laid = Bounds::default();
        laid.hold(DVec2::ZERO);
        laid.hold(DVec2::splat(TAU));
        laid
    }

    /// Every piece of what the ring and `other` meet in, walked and filed.
    fn walked(other: &Surface) -> Walked {
        let round = ring();
        let Surface::Fitted(Fitted::Torus(torus)) = round else {
            panic!("the ring is a torus");
        };
        let mut seeds = Vec::new();
        assert!(
            seeding::seeded(other, &torus, &mut seeds),
            "the pair has a reading"
        );
        let mut marching = Marching::default();
        let mut walked = Walked {
            carried: Carried::default(),
            curves: Vec::new(),
            sampled: Vec::new(),
            taken: Vec::new(),
        };
        for &seed in &seeds {
            let strayed = marching
                .walk(&round, other, seed, 1e-4)
                .expect("the walk did not close");
            let run = walked.carried.marched.add(marching.walked(), strayed);
            let filed = walked.carried.marched.strayed(run);
            let curve = Curve::Marched(Marched {
                run,
                key: u64::from(run),
                reach: filed.reach,
                shut: filed.shut,
            });
            let from = walked.sampled.len();
            let mut into = Vec::new();
            curve.sample(TAU, 1e-4, &walked.carried, &mut into);
            walked.sampled.append(&mut into);
            walked.taken.push([from, walked.sampled.len()]);
            walked.curves.push(curve);
        }
        walked
    }

    /// The pieces of that meeting, as one face laid out over a whole turn of
    /// each parameter sees them.
    fn pieces(other: &Surface, walked: &Walked) -> Vec<Piece> {
        walked
            .curves
            .iter()
            .zip(&walked.taken)
            .enumerate()
            .map(|(at, (&curve, &taken))| {
                Piece::of(
                    &ring(),
                    other,
                    &walked.sampled,
                    taken,
                    laid(),
                    curve,
                    at as u32,
                )
                .expect("the piece reaches the face")
            })
            .collect()
    }

    /// **A traced cut answers where it stands without walking what it is made
    /// of**, which is the whole of why it carries two surfaces.
    ///
    /// A ring cut by a plane through its middle at forty-five degrees. A place
    /// of the ring is on the kept side exactly as it stands on the kept side of
    /// that plane, so the two answers can be held against each other outright:
    /// the plane's own normal says which side a place is on, and the cut has to
    /// agree at every one of them.
    ///
    /// **And turning the cut round turns every one of those readings over**,
    /// which is what makes cutting both ways one operation asked twice.
    #[test]
    fn a_traced_cut_reads_its_side_off_the_surface_it_meets() {
        let round = ring();
        let plane = leaning();
        let Surface::Natural(Natural::Plane(flat)) = plane else {
            panic!("the cut is against a plane");
        };
        let walked = walked(&plane);
        let pieces = pieces(&plane, &walked);
        let traced = Traced::of(
            &round,
            &plane,
            &walked.carried,
            &walked.sampled,
            laid(),
            &pieces,
        );
        for step in 0..64 {
            let uv = DVec2::new(TAU * (step % 8) as f64 / 8.0, TAU * (step / 8) as f64 / 8.0);
            let place = round.at(uv);
            let off = (place - flat.origin).dot(flat.normal());
            if off.abs() < 1e-9 {
                continue;
            }
            let side = traced.side(uv);
            assert_eq!(side > 0.0, off < 0.0, "{uv:?} stands {off} off the plane");
            // And the reading is the distance itself, up to its sign.
            assert!(
                (side.abs() - off.abs()).abs() < 1e-9,
                "{side} against {off}"
            );
            assert_eq!(traced.turned().side(uv), -side, "turning kept the side");
        }
    }

    /// **And it lays its corners down on both surfaces**, wound so the side it
    /// keeps is on its left.
    ///
    /// The small loop a plane just inside the outer equator cuts, walked whole.
    /// Every corner is a place of the ring that stands on the plane as well —
    /// they are the run's own places, read into the ring's parameters — and a
    /// step to the left of the way the loop runs lands on the side the cut
    /// keeps. Turned round, the loop keeps the other side and shuts in the
    /// other one.
    #[test]
    fn a_traced_cut_walks_a_loop_with_what_it_keeps_on_the_left() {
        // The leaning plane's own pieces run right round the tube, so neither
        // of them is a loop of the face's parameters however closed it is in
        // the world — which is the other regime and the reason for the flag.
        let round = ring();
        let across = leaning();
        let over = walked(&across);
        let leaning = pieces(&across, &over);
        assert_eq!(leaning.len(), 2, "a leaning plane cuts two pieces");
        assert!(
            !Traced::of(
                &round,
                &across,
                &over.carried,
                &over.sampled,
                laid(),
                &leaning
            )
            .closed(),
            "a piece that wraps closed",
        );

        let plane = grazing();
        let walked = walked(&plane);
        let pieces = pieces(&plane, &walked);
        assert_eq!(pieces.len(), 1, "a grazing plane cuts one piece");
        let traced = Traced::of(
            &round,
            &plane,
            &walked.carried,
            &walked.sampled,
            laid(),
            &pieces,
        );
        assert!(traced.closed(), "a small piece is a loop of the parameters");
        let mut loops = Loops::default();
        traced.walk(&mut loops);
        assert_eq!(loops.len(), 1, "one piece is one loop");
        let laid = loops.get(0);
        assert!(laid.len() > 16, "{} corners is no loop", laid.len());

        for corner in laid {
            assert_eq!(corner.came, Came::Arc(0), "the run was not carried");
            let place = round.at(corner.at);
            for (named, surface) in [("the ring", &round), ("the plane", &plane)] {
                let off = surface.off(place);
                assert!(off < 1e-9, "{place:?} stands {off} off {named}");
            }
        }

        let (here, there) = (laid[0].at, laid[1].at);
        let ahead = there - here;
        let left = DVec2::new(-ahead.y, ahead.x).normalize() * 1e-4;
        assert!(
            traced.side(here + left) > 0.0,
            "the loop runs with what it keeps on its right",
        );
        // What the loop shuts in is the cap beyond the plane, which is the
        // side *dropped* while the inside is kept — and turned it is the one
        // kept, the disc being on one side of the cut and not on both.
        assert!(!traced.holds(here), "the loop shut in the side it keeps");
        assert!(traced.turned().holds(here), "turning kept the same side");
    }

    /// **A run across the cut is crossed where the reading changes sign**, and
    /// the place that comes back stands on both surfaces.
    ///
    /// Bisected rather than solved, a marched curve having nothing to solve
    /// against. What says the answer is right is that it is *on the curve* —
    /// which is two surfaces agreeing, neither of them the run.
    ///
    /// **And how far along it stands says which piece it is on**, a whole turn
    /// to a piece: the two pieces of this meeting are disjoint, and a crossing
    /// on one of them never orders into the other.
    #[test]
    fn a_run_across_a_traced_cut_is_crossed_on_the_curve() {
        let round = ring();
        let plane = leaning();
        let walked = walked(&plane);
        let pieces = pieces(&plane, &walked);
        let traced = Traced::of(
            &round,
            &plane,
            &walked.carried,
            &walked.sampled,
            laid(),
            &pieces,
        );
        let mut met = [0, 0];
        // Both halves of the turn, the two pieces standing one in each.
        for step in 0..16 {
            let v = TAU * (step % 8) as f64 / 8.0;
            let half = TAU / 2.0 * (step / 8) as f64;
            let (from, to) = (DVec2::new(half, v), DVec2::new(half + TAU / 2.0, v));
            if (traced.side(from) > 0.0) == (traced.side(to) > 0.0) {
                continue;
            }
            let at = traced.crossing(from, to);
            let place = round.at(at);
            for (named, surface) in [("the ring", &round), ("the plane", &plane)] {
                let off = surface.off(place);
                assert!(off < 1e-9, "{place:?} stands {off} off {named}");
            }
            let down = traced.down(at);
            met[down.div_euclid(TAU) as usize] += 1;
        }
        assert!(met[0] > 0 && met[1] > 0, "{met:?} is not both pieces");
    }

    /// **A run that dips across the cut and back is met twice**, which is the
    /// one question here that is asked of the chords rather than of the
    /// reading.
    ///
    /// A line straight through the middle of the small closed loop, from well
    /// outside it to well outside it the other way. It crosses twice, both
    /// crossings stand on the loop, and they come back in the order the run
    /// meets them. A line that stands clear of the loop altogether meets it
    /// nowhere, which is the two comparisons the box is there for.
    #[test]
    fn a_run_dipping_across_a_traced_cut_is_met_twice() {
        let round = ring();
        let plane = grazing();
        let walked = walked(&plane);
        let pieces = pieces(&plane, &walked);
        let traced = Traced::of(
            &round,
            &plane,
            &walked.carried,
            &walked.sampled,
            laid(),
            &pieces,
        );
        let mut loops = Loops::default();
        traced.walk(&mut loops);
        let laid = loops.get(0);
        let middle = laid.iter().map(|corner| corner.at).sum::<DVec2>() / laid.len() as f64;

        let (from, to) = (middle - DVec2::X * 10.0, middle + DVec2::X * 10.0);
        let met = traced
            .grazes(from, to)
            .expect("the run dips across the loop");
        let along = (to - from).normalize();
        assert!(
            along.dot(met[1] - met[0]) > 0.0,
            "{met:?} came back the wrong way round",
        );
        for at in met {
            let off = traced.side(at).abs();
            assert!(off < 1e-3, "{at:?} stands {off} off the cut");
            assert!(
                (at - middle).y.abs() < 1e-12,
                "{at:?} is not on the run it was asked about",
            );
        }

        // A run a whole turn away in `v` reaches nothing the loop covers.
        let away = DVec2::Y * TAU;
        assert!(traced.grazes(from + away, to + away).is_none());
    }
}
