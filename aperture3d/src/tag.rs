//! What a pick reports.

/// The caller's own name for a primitive: what a pick that lands on it
/// reports, and nothing else.
///
/// Opaque on purpose. Whatever the caller models — a body, a sketch edge, a
/// constraint handle, a dimension line — is the caller's own vocabulary, and a
/// renderer that learned it would be a renderer that had to be told about
/// every kind of thing there is. A number carried and never read keeps hits
/// answerable without that.
///
/// A type of its own rather than the `u64` inside it, because a number nothing
/// interprets is a number nothing can catch being wrong: an index, a count and
/// a name are the same eight bytes, and only one of them belongs here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tag(u64);

impl Tag {
    /// Name something `id`. What the number means is the caller's business.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The number this was named with.
    pub const fn get(self) -> u64 {
        self.0
    }
}
