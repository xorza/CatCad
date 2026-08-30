//! The constructions the exact tier's own curves are made of.
//!
//! **Where a quartic keeps what it is** — the pair to
//! [`Marchings`](super::marchings::Marchings) one tier up, and the same
//! arrangement for the same reason: a [`Curve`](super::curve::Curve) is a
//! `Copy` value and a quartic is ninety heap blocks, so the construction lives
//! here and the curve names it.
//!
//! What fills one is the boolean, meeting a pair no row of the reducible table
//! answers; what reads one is every walk over an edge on it.
//!
//! **No production reader yet**, on the same terms
//! [`quartic`](super::quartic) states: the store is threaded to where a curve
//! is asked what it is made of, and the arm that names one arrives with the
//! routine that walks it. See `.notes/KERNEL.md` §9.1.
#![allow(dead_code)]

use crate::solid::geometry::quartic::Quartic;

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
    held: Vec<Quartic>,
}

impl Quartics {
    /// Forget every curve, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.held.clear();
    }

    /// How many are filed, which is the number the next one takes.
    pub(crate) fn len(&self) -> u32 {
        self.held.len() as u32
    }

    /// File `curve` as one of its own, and say which it is.
    pub(crate) fn add(&mut self, curve: Quartic) -> u32 {
        let at = self.len();
        self.held.push(curve);
        at
    }

    /// The curve filed at `at`.
    ///
    /// Panics on a handle this store never minted, which is a mistake in
    /// whatever named it rather than a state a reader has to handle — the same
    /// standing every arena here takes.
    pub(crate) fn at(&self, at: u32) -> &Quartic {
        &self.held[at as usize]
    }
}
