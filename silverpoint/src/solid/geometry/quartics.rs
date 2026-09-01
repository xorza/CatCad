//! The store a body keeps its quartic curves in, and the handle an edge names
//! one by.
//!
//! **Apart from the curve itself**, which is an algebraic construction and
//! answers about itself — see [`Quartic`](super::quartic::Quartic). What is
//! here is what a *body* has to hold: one component per edge, filed so a
//! reader answers without evaluating it.
//!
//! The handle sits beside the store on the terms
//! [`marchings`](super::marchings) keeps: one file per subject, holding the
//! arena, the handle and whatever small thing either wants.

use crate::math::arc;
use crate::solid::geometry::quartic::{Closing, Quartic, Stretch, Walk};
use glam::DVec3;
use std::f64::consts::TAU;

/// A curve of the exact tier, written down rather than laid out.
///
/// **The handle, on the terms [`Marched`](super::marchings::Marched) states.**
/// What holds the construction is [`Quartics`], which a body keeps beside its
/// topology; what is here is which component of it, and the two numbers a
/// reader answers without reaching for the store.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Quartered {
    /// Which component of the body's own store it is.
    pub(crate) run: u32,
    /// What it is filed under — see
    /// [`Curve::key`](super::curve::Curve::key).
    pub(crate) key: u64,
    /// How large the numbers reading it work in.
    pub(crate) reach: f64,
}

/// One component of one quartic, and what a reader answers without evaluating
/// it.
#[derive(Debug, Clone)]
pub(super) struct Filed {
    curve: Quartic,
    arc: Stretch,
    closing: Closing,
    /// Which parts of the arc read, as [`Filed::trimmed`] found them.
    ///
    /// **Held beside the arc rather than written into it.** What the trim
    /// finds is which parts of the arc read, and that is a fact about the
    /// reading rather than about where the arc is — an arc rewritten to its
    /// own trim would have to name the hair in the affine chart, where the
    /// place at infinity has no name.
    holds: [f64; 2],
    /// How hard the loop can turn, which is what a chord count is taken
    /// against — see [`arc::chords`], and
    /// [`Saddle::bending`](super::saddle::Saddle), which works its own out
    /// where this measures.
    bending: f64,
    /// How large the numbers reading it work in.
    reach: f64,
}

/// Every quartic one body's curves are cut from.
///
/// **Not flat, where [`Marchings`](super::marchings::Marchings) is**, and that
/// is the one thing about it worth arguing. A marched run is places, which pack
/// into one buffer; a quartic is a ruled member and a quadric over exact
/// rationals, which are bignums and own blocks of their own. Clearing keeps the
/// room for the constructions and hands back the room inside them.
///
/// **What that costs is an allocation per quartic edge per rebuild**, on a body
/// the drawing under it rebuilds every frame of a drag. A handful of blocks for
/// a handful of edges, against a marched run's none — worth naming, and not
/// worth a second representation of an exact curve to avoid.
#[derive(Debug, Default)]
pub(crate) struct Quartics {
    held: Vec<Filed>,
}

/// How many places a component is measured at when it is filed.
///
/// **Enough to bound a bend rather than to draw one.** What the two numbers
/// below are for is a chord count and a size, both of which want the worst of
/// the loop and neither of which wants it to a digit. Thirty-two is a
/// hundredth of a turn's error on a circle, and a quartic bends no more sharply
/// than the tightest circle through three of its places.
const MEASURED: usize = 32;

/// The angle the `at`th of `of` steps round a loop stands at.
///
/// Stated once because every walk below wants it and a slip in one would be a
/// measurement quietly taken somewhere else. `stepped(1, of)` is the interval
/// itself, a step being what one of them is.
fn stepped(at: usize, of: usize) -> f64 {
    TAU * at as f64 / of as f64
}

/// How many places the coarsest of [`Filed::resolves`]'s three walks takes.
///
/// **Coarse on purpose**, the test being about what a *sample* misses: a walk
/// fine enough to resolve any chart would tell nothing apart. Sixteen is under
/// a percent of a circle's own length, so the three walks together cost about
/// as much as one measurement of a bend.
const RESOLVING: usize = 16;

/// How many times a search over the parameter halves what it is looking in.
///
/// **What an `f64` holds between two ends**, which is the same standing
/// [`bisect`](crate::math::bisect) takes: halved until the middle is one of the
/// two, and no tolerance anywhere in it. Both readers below are bisections —
/// one for the last part of an arc that reads, one for the parameter nearest a
/// place — and neither wants a bound of its own.
const HALVINGS: usize = 60;

impl Quartics {
    /// Forget every curve, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.held.clear();
    }

    /// Take a copy of what `of` holds, over the room these took — see
    /// [`Carried::take_from`](super::carried::Carried), where the reason for a
    /// copy rather than a trade is.
    pub(crate) fn take_from(&mut self, of: &Self) {
        self.held.clear();
        self.held.extend_from_slice(&of.held);
    }

    /// How many are filed, which is the number the next one takes.
    pub(crate) fn len(&self) -> u32 {
        self.held.len() as u32
    }

    /// File one component of `curve` — the `arc` it runs over, closed as
    /// `closing` says — and say which it is.
    ///
    /// **Measured on the way in**, which is the arrangement
    /// [`Marchings::add`](super::marchings::Marchings) keeps and for the same
    /// reason: how hard the loop turns and how large its numbers work are
    /// asked on every walk over it, and working either out from the
    /// construction each time would be a walk inside a walk.
    pub(crate) fn add(&mut self, curve: Quartic, arc: Stretch, closing: Closing) -> u32 {
        let mut filed = Filed::of(curve, arc, closing);
        filed.bending = filed.bending();
        filed.reach = filed.reaching();
        let at = self.len();
        self.held.push(filed);
        at
    }

    /// Where the parameter `t` of the component at `run` lands.
    pub(crate) fn at(&self, run: u32, t: f64) -> DVec3 {
        self.held[run as usize]
            .place(t)
            .expect("a trimmed arc reads everywhere")
    }

    /// How large the numbers reading the component at `run` work in.
    pub(crate) fn reach(&self, run: u32) -> f64 {
        self.held[run as usize].reach
    }

    /// How many chords of the component at `run` a `span` of it wants.
    pub(crate) fn steps(&self, run: u32, span: f64, sagitta: f64) -> usize {
        arc::chords(self.held[run as usize].bending, span, sagitta)
    }

    /// Which angle round the component at `run` the place `at` stands at.
    ///
    /// **Solved rather than searched** — see [`Ruled::through`], which is the
    /// whole of it: a place of the ruled member is written in the member's own
    /// corner basis by one four by four solve, and the ruling falls out of the
    /// weights. What is left is the two walks the store lays over that ruling,
    /// each read the other way round.
    ///
    /// Exact for a place *of* the curve, which is what every caller asks about:
    /// a corner of a region's boundary was laid down off this curve, and a
    /// crossing was solved onto it. A place beside the curve is answered as far
    /// beside its true parameter as it stands off, the solve being linear. A
    /// place nowhere near it is answered with some parameter of the component
    /// and no promise which — and that is still sound for the one caller that
    /// asks a component about a place that may belong to another, because what
    /// that reads off the answer is a distance from a place the component
    /// really holds, which can only be longer than the nearest.
    pub(crate) fn along(&self, run: u32, at: DVec3) -> f64 {
        self.held[run as usize].along(at)
    }
}

impl Filed {
    /// One component, trimmed to the part of its arc that reads.
    ///
    /// **Trimmed here and measured by the caller**, which is the order the two
    /// have to come in: a bend or a reach taken outside what reads would be a
    /// measurement of nothing. [`Quartics::add`] takes both after this, and
    /// [`Quartic::resolved`] takes neither — a chart being tried wants the trim
    /// and nothing after it.
    pub(super) fn of(curve: Quartic, arc: Stretch, closing: Closing) -> Self {
        let mut filed = Self {
            curve,
            arc,
            closing,
            holds: [0.0, 1.0],
            bending: 0.0,
            reach: 0.0,
        };
        filed.holds = filed.trimmed();
        filed
    }

    /// Whether walking the parameter says what this component is.
    ///
    /// **A projective chart is a bijection and a walk of it need not be.** The
    /// parameter is the ruling of whichever member of the pencil the search
    /// landed on, and nothing about that member says the curve is spread
    /// evenly over it. Two rods meeting at a lean put nine tenths of each loop
    /// inside a thousandth of the parameter: a walk steps clean over the nine
    /// tenths, [`Filed::bending`] measures a bend the curve does not have, and
    /// what comes of it is an edge chorded once across half a loop. The face
    /// built on that folds over itself.
    ///
    /// **So the walk is refined and what it gains is watched.** Chording a
    /// smooth loop at `n` places falls short of its length by `O(1/n²)`, so
    /// doubling `n` leaves a quarter of what was missing — a walk that resolves
    /// its curve gains less each time it is refined. One that does not gains
    /// *more*, each refinement reaching parts of the curve the last stepped
    /// over. Half the last gain is the bound: twice the quarter a smooth loop
    /// owes, and nowhere near the growth a chart that misses one shows.
    ///
    /// Sampled, and it has to be: what is asked is whether a *sample* of this
    /// chart stands for the curve, which no closed form about the curve
    /// answers.
    pub(super) fn resolves(&self) -> bool {
        let walked = |count: usize| {
            let mut walk = 0.0;
            let mut last = self.place(0.0);
            for at in 1..=count {
                let here = self.place(stepped(at, count));
                if let (Some(here), Some(was)) = (here, last) {
                    walk += here.distance(was);
                }
                last = here;
            }
            walk
        };
        let [coarse, fine, finer] = [RESOLVING, RESOLVING * 2, RESOLVING * 4].map(walked);
        finer - fine <= (fine - coarse) / 2.0
    }

    /// Where the parameter `t` lands, or `None` where it names no real place.
    ///
    /// **The one walk of a component**, which is what the two measurements
    /// below and every reader above go through. Two spellings of it would be a
    /// curve measured on one path and read on another.
    fn place(&self, t: f64) -> Option<DVec3> {
        let walk = self.closing.walk(t);
        self.walked(walk.part)[usize::from(walk.far)]
    }

    /// Both branches `part` of the way along the arc that reads.
    fn walked(&self, part: f64) -> [Option<DVec3>; 2] {
        let [low, high] = self.holds;
        self.curve.both(&self.arc.at(low + (high - low) * part))
    }

    /// Which angle round it the place `at` stands at — see
    /// [`Quartics::along`], which is what this answers.
    fn along(&self, at: DVec3) -> f64 {
        let [low, high] = self.holds;
        // Held to the arc that reads. A place off the component is answered
        // with the end of it nearest, which is a place the component holds —
        // and a component that reads at one place only holds nothing else.
        let part = match high == low {
            true => 0.0,
            false => ((self.arc.along(self.curve.along(at)) - low) / (high - low)).clamp(0.0, 1.0),
        };
        let far = match self.closing {
            Closing::Alone { far } => far,
            // Both branches of one ruling stand at the one `u`, so which of the
            // two the place is is settled by reading, not by the solve.
            Closing::Round => {
                let apart = self
                    .walked(part)
                    .map(|had| had.map_or(f64::INFINITY, |had| had.distance(at)));
                apart[1] < apart[0]
            }
        };
        self.closing.along(Walk { part, far })
    }

    /// The parts of the arc that read, each end halved inward until one does.
    ///
    /// An end is a root of `Δ` found in floats, so a reading exactly at one
    /// falls a rounding either side of it — and a hair beyond, the branch is
    /// not real and there is no place to give. Halved rather than stepped in by
    /// a tolerance: what is wanted is the last place the curve *is*.
    fn trimmed(&self) -> [f64; 2] {
        let reads = |part: f64| self.curve.at(&self.arc.at(part), false).is_some();
        let inward = |from: f64, toward: f64| {
            let (mut out, mut held) = (from, toward);
            if reads(out) {
                return out;
            }
            for _ in 0..HALVINGS {
                let middle = (out + held) / 2.0;
                match reads(middle) {
                    true => held = middle,
                    false => out = middle,
                }
            }
            held
        };
        [inward(0.0, 0.5), inward(1.0, 0.5)]
    }

    /// How hard the component turns, as a bound on the second derivative of the
    /// place with the parameter.
    ///
    /// Measured rather than worked out: a quartic has no closed form for it the
    /// way a saddle does, and the construction is evaluable everywhere.
    fn bending(&self) -> f64 {
        let step = stepped(1, MEASURED);
        (0..MEASURED).fold(0.0f64, |most, at| {
            let t = stepped(at, MEASURED);
            let (Some(back), Some(here), Some(on)) =
                (self.place(t - step), self.place(t), self.place(t + step))
            else {
                return most;
            };
            most.max((back - here * 2.0 + on).length() / (step * step))
        })
    }

    /// How far from the origin the component reaches.
    fn reaching(&self) -> f64 {
        (0..MEASURED).fold(0.0f64, |most, at| match self.place(stepped(at, MEASURED)) {
            Some(place) => most.max(place.length()),
            None => most,
        })
    }
}
