//! Where two solids are put together, and the answer tidied.

use silverpoint::{Body, Boolean, Merging, Operation};

/// The room a solid is put together in, and every buffer that takes.
///
/// **One place where a preview and a commit agree.** A step's own rebuild puts
/// its solid together here, and so does the form still deciding a depth — see
/// [`Growing::body`](crate::paint::growing::Growing). Two copies of the rule
/// would be two chances for the solid on screen while a number is typed to be
/// a solid the timeline goes on to build differently.
///
/// Held across calls, because a document is rebuilt on every frame of a drag
/// and each of these grew its buffers the first time it ran.
#[derive(Debug, Default)]
pub(crate) struct Putting {
    boolean: Boolean,
    merging: Merging,
    /// What the boolean answers, before the pieces of every face are put back
    /// together. Lives no longer than the call.
    split: Body,
}

impl Putting {
    /// Put `raised` together with `standing` per `operation`, into `into`, and
    /// say whether the two came to a body.
    ///
    /// **Nothing standing is not the same as nothing to do.** A join is the
    /// whole of itself, and the two operations that need material to take out
    /// of or to share with come to nothing at all — which is honest rather than
    /// helpful: a first step that says cut has cut nothing, and quietly making
    /// it a boss would hide the mistake. `None` rather than an empty body, so a
    /// caller cannot pass one and mean the other.
    ///
    /// Where the kernel refuses them, `raised` is left holding the step's own
    /// solid for the caller to take: what a refusal costs is that the solid
    /// stands beside the model rather than in it.
    ///
    /// **What comes back is the pieces**, and a caller that will draw them
    /// rather than cut them again wants [`Putting::drawn`] instead.
    pub(crate) fn put(
        &mut self,
        standing: Option<&Body>,
        raised: &mut Body,
        operation: Operation,
        into: &mut Body,
    ) -> bool {
        let Some(standing) = standing else {
            match operation {
                Operation::Join => std::mem::swap(into, raised),
                Operation::Cut | Operation::Intersect => into.clear(),
            }
            return true;
        };
        self.boolean.combine(standing, raised, operation, into)
    }

    /// `from` with the pieces of every face put back together, into `into`.
    ///
    /// **What a cut split, merged**, which is [`Merging`] and
    /// `.notes/KERNEL.md` §9.3: a boolean divides a wall by every surface that
    /// reaches it and hands back the pieces, where the document, the picker and
    /// the mesher all mean the face.
    ///
    /// **A second body rather than an edit**, which the same section measures:
    /// the splits one boolean makes are part of its answer's contract for the
    /// next one, so what a further step is built on is the pieces and what is
    /// drawn is this.
    pub(crate) fn tidy(&mut self, from: &Body, into: &mut Body) {
        self.merging.merge(from, into);
    }

    /// [`Putting::put`] and [`Putting::tidy`] in one, for a body nothing will
    /// cut again.
    ///
    /// What a preview wants, where a step wants the two apart: the solid on
    /// screen while a depth is typed is drawn and thrown away, and no step is
    /// built on it.
    pub(crate) fn drawn(
        &mut self,
        standing: Option<&Body>,
        raised: &mut Body,
        operation: Operation,
        into: &mut Body,
    ) -> bool {
        // Taken out and put back, the boolean wanting the whole of `self`
        // while it writes into a field of it. Neither move touches the heap.
        let mut split = std::mem::take(&mut self.split);
        let stands = self.put(standing, raised, operation, &mut split);
        if stands {
            self.tidy(&split, into);
        }
        self.split = split;
        stands
    }
}
