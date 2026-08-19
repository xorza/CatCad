//! A closed sheet of faces.

use crate::arena::Id;
use std::ops::Range;

pub(crate) type ShellId = Id<Shell>;

/// A closed, connected, oriented set of faces.
///
/// What separates inside from outside. A [`Lump`](super::lump::Lump) has one
/// around it and one around each cavity in it, and the only difference between
/// the two is which way the faces face.
#[derive(Debug, Default)]
pub(crate) struct Shell {
    /// Which of the body's faces are its, as a stretch of one buffer — see
    /// [`Face::loops`](super::face::Face) for why nothing here holds a vector
    /// of its own.
    pub(crate) faces: Range<usize>,
}
