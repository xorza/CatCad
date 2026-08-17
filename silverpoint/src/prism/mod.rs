//! A region of a drawing, carried off its plane.
//!
//! The first three-dimensional thing this crate makes, and it is deliberately
//! the *least* of them: a prism is a region and a distance, and every face,
//! edge and corner it has follows from those two. Nothing here is stored —
//! [`Arrangement`] is already a boundary representation of the plane the region
//! lies in, and a prism over one of its faces adds no topology that has to be
//! kept in step.
//!
//! Beside `sketch` rather than under it, because the two share a crate and not
//! a coordinate space: everything here answers in the world, where a sketch
//! answers in the flat frame a [`Plane`] carries into one.

use crate::math::plane::Plane;
use crate::prism::grown::Grown;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::face::Face;

pub(crate) mod grown;
pub(crate) mod skinner;

/// A region of a drawing and how far it is carried off the plane it was drawn
/// on.
///
/// Borrowed and [`Copy`], like the readings a drawing hands out: what it holds
/// is an arrangement somebody else owns, a position in it, and two values. A
/// caller makes one to ask a question and lets it go.
///
/// Which region by *position*, unlike everything a feature keeps. A prism is
/// read within one frame — cut into triangles, measured, picked against — and a
/// position is what the arrangement it was resolved against answers in. Turning
/// a durable name into one of these is the caller's, and happens once per
/// rebuild.
///
/// The distance is signed, so which way it grows needs no second field: a
/// negative distance is the same prism on the other side of the plane, and the
/// winding below follows it rather than being fixed one way.
#[derive(Debug, Clone, Copy)]
pub struct Prism<'a> {
    of: &'a Arrangement,
    at: usize,
    plane: Plane,
    distance: f64,
}

impl<'a> Prism<'a> {
    /// The region at `at` in `of`, carried `distance` along `plane`'s normal.
    ///
    /// `at` has to be one of `of`'s faces, and `plane` the one the drawing it
    /// came from lies on — a face names its edges by where they fall in the
    /// arrangement that cut them, so neither travels to another.
    pub fn new(of: &'a Arrangement, at: usize, plane: Plane, distance: f64) -> Self {
        Self {
            of,
            at,
            plane,
            distance,
        }
    }

    /// Every face it has, in a fixed order: the base, the far end, then one wall
    /// per curve bounding the region.
    ///
    /// An iterator rather than a list filled into a buffer, which a prism can
    /// offer because it is [`Copy`] and borrows the arrangement for as long as
    /// it lives: the walls are read off the region itself, worked out once when
    /// the drawing was cut. What asks is drawing or picking a whole document,
    /// so not needing room for the answer is worth more here than anywhere — a
    /// caller writing every face of every solid does it in one pass and reaches
    /// the heap for none of it.
    pub fn grown(self) -> impl Iterator<Item = Grown> + 'a {
        let sides = self.face().walls().iter().copied().map(Grown::Side);
        [Grown::Base, Grown::Far].into_iter().chain(sides)
    }

    /// Whether `grown` is one of its faces.
    ///
    /// What anything keeping hold of a face across an edit has to ask. The two
    /// ends are always there — a prism is a region carried, and it has both
    /// however the drawing moves — and a wall is there exactly while the curve
    /// it was swept from still bounds the region on that side.
    ///
    /// Asked of the walk above rather than by a rule of its own, so what a prism
    /// *has* and what it answers for cannot come to differ.
    pub fn holds(self, grown: Grown) -> bool {
        self.grown().any(|had| had == grown)
    }

    /// The region it was grown from.
    pub(crate) fn face(&self) -> &'a Face {
        &self.of.faces()[self.at]
    }

    /// The arrangement the region belongs to.
    pub(crate) fn arrangement(&self) -> &'a Arrangement {
        self.of
    }

    /// Where the drawing lies in the world.
    pub(crate) fn plane(&self) -> Plane {
        self.plane
    }

    /// How far it is carried, and which way.
    ///
    /// Public where the rest of the reading below is not, because it is the one
    /// thing a prism says that a caller did not hand it back: `new` takes a
    /// distance and every other answer is worked out from the arrangement.
    pub fn distance(&self) -> f64 {
        self.distance
    }

    /// Whether it grows along the plane's normal rather than against it.
    ///
    /// Where the two ends and every wall get their winding from, so the answer
    /// is taken in one place: a prism of no depth counts as growing along, which
    /// is a choice about nothing — it has no volume to be inside out.
    pub(crate) fn along(&self) -> bool {
        self.distance >= 0.0
    }
}

#[cfg(test)]
mod tests;
