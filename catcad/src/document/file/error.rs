//! What can go wrong putting a document away, and fetching one back.

use std::fmt;
use std::io;

/// Why a document could not be written.
///
/// Two ways, and the split is where the blame is: the encoding is this crate's
/// own doing and the disk is the world's. Nothing a *user* did can reach the
/// first — [`Saved`](crate::document::file::saved::Saved) holds numbers and
/// lists, which RON always has somewhere to put — so a file that fails to
/// encode is a bug here rather than a document that cannot be saved.
#[derive(Debug)]
pub(crate) enum SaveError {
    Write(io::Error),
    Encode(ron::Error),
}

/// Why a document could not be opened.
///
/// Three ways, in the order they are found: the file would not be read, would
/// not parse, or parsed and said something impossible. The last is the one with
/// anything to explain, which is why it is the only one carrying a type of its
/// own — see [`Fault`].
#[derive(Debug)]
pub(crate) enum LoadError {
    Read(io::Error),
    /// It is not RON, or not shaped like a document. Carries RON's own report,
    /// which names the line and column.
    Parse(ron::error::SpannedError),
    Fault(Fault),
}

/// What a file that parses can still get wrong.
///
/// Everything here is checked before a single step reaches the timeline, and
/// that is the point of the type. [`Timeline::add`](crate::timeline::Timeline)
/// and [`Sketch::add_constraint`](silverpoint::Sketch) both assert that what a
/// thing is built on is there — a fair contract between two pieces of this
/// program, and no way at all to meet a file someone has typed into. So the
/// checks that would have been those asserts are these variants instead, and by
/// the time anything is added the answer is already known to be yes.
///
/// Each names *where*: `at` is which step of the file, counting from zero, which
/// is also how a step refers to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    /// Written by another version of the format.
    Version(u32),
    /// It holds no sketch, and a document is opened *in* one.
    NoSketch,
    /// A step names one the file has not got to yet, or has not got at all.
    ///
    /// The two are one answer because the file numbers steps by position: a
    /// reference is only meaningful backwards, so naming a later step and
    /// naming a missing one fail the same check. That is what keeps the recipe
    /// a recipe rather than a graph with cycles in it.
    UnknownStep { at: usize, names: usize },
    /// A sketch is drawn on something that is not a plane, or a plane is
    /// measured off something that is not one.
    NotAPlane { at: usize, names: usize },
    /// An extrude is grown from a step that is not a sketch.
    ///
    /// Its own variant beside [`Fault::NotAPlane`] rather than one that names
    /// the kind it wanted: the two read differently to whoever has to fix the
    /// file, and a message that had to be assembled from a noun would be one
    /// place further from what it is telling them.
    NotASketch { at: usize, names: usize },
    /// Something in a sketch names geometry the sketch does not hold.
    Unknown { at: usize, what: Missing },
    /// The bar says the recipe was built as far as a step the file has not got.
    ///
    /// Its own variant rather than [`Fault::UnknownStep`], which is about a
    /// *step* naming another and carries the position of the one complaining.
    /// The bar is not a step and has no position of its own, so there would be
    /// nothing honest to put in that field.
    UnknownRollback { names: usize },
    /// A rounding picks a face of a step that has no such face.
    ///
    /// Two mistakes in one answer, because they read the same to whoever has to
    /// fix the file: a pick naming a plane or a sketch, neither of which grows
    /// a face at all, and one naming a wall of a step that swept no drawing.
    NoSuchFace { at: usize, names: usize },
    /// A coordinate, a radius or a dimension that is not a number.
    ///
    /// Infinities and NaN parse perfectly well and would poison the first
    /// solve, which has no way to report that it was handed one — so they are
    /// refused here, where there is still someone to tell.
    NotFinite { at: usize },
}

/// Which of a sketch's three kinds a reference missed, and which one it was.
///
/// Its own type rather than three variants of [`Fault`], because the three read
/// identically everywhere but the noun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Missing {
    Point(usize),
    Segment(usize),
    Circle(usize),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Write(error) => write!(f, "could not be written: {error}"),
            SaveError::Encode(error) => write!(f, "could not be encoded: {error}"),
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Read(error) => write!(f, "could not be read: {error}"),
            LoadError::Parse(error) => write!(f, "is not a document: {error}"),
            LoadError::Fault(fault) => write!(f, "{fault}"),
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Fault::Version(version) => {
                write!(f, "was written by version {version} of the format")
            }
            Fault::NoSketch => f.write_str("holds no sketch"),
            Fault::UnknownStep { at, names } => {
                write!(
                    f,
                    "step {at} is built on step {names}, which is not before it"
                )
            }
            Fault::UnknownRollback { names } => {
                write!(f, "was built as far as step {names}, which it has not got")
            }
            Fault::NotAPlane { at, names } => {
                write!(
                    f,
                    "step {at} is built on step {names}, which is not a plane"
                )
            }
            Fault::NotASketch { at, names } => {
                write!(
                    f,
                    "step {at} is grown from step {names}, which is not a sketch"
                )
            }
            Fault::NoSuchFace { at, names } => {
                write!(
                    f,
                    "step {at} picks a face of step {names}, which has none of that kind"
                )
            }
            Fault::Unknown { at, what } => write!(f, "step {at} names {what}"),
            Fault::NotFinite { at } => write!(f, "step {at} states a number that is not one"),
        }
    }
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Missing::Point(at) => write!(f, "point {at}, which it does not hold"),
            Missing::Segment(at) => write!(f, "edge {at}, which it does not hold"),
            Missing::Circle(at) => write!(f, "circle {at}, which it does not hold"),
        }
    }
}
