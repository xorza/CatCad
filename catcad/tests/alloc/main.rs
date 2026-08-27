//! Per-frame allocation gates for the application's record pass.
//!
//! Five of the six are a strict zero, and between them that is the whole of a
//! frame: recording is all this crate does per frame, and none of it reaches
//! the heap. The status line is formatted into the record pass's own text arena
//! rather than a `String`; `Scene::nearest` answers a hover without building a
//! list; the drawing is laid out over the primitives the renderer already holds
//! rather than into fresh ones; what a dragged frame takes down — the solver's
//! parameter vector, so a drag with nowhere to go can be handed back untouched,
//! and the history's two ends of what it is recording — all refill buffers that
//! have the room; and what the curves enclose is worked out in an
//! [`Arrangement`](silverpoint::Arrangement) kept across frames, which refills
//! the list per corner of what leaves it, the list per loop, the list per curve
//! of where it is cut, and the fill per face rather than building each afresh.
//!
//! The sixth is the frame a depth is decided over, and it is the dearest of
//! them: a form open on a region raises a solid and puts it together with the
//! model on every frame the number moves, which is a whole kernel boolean where
//! the rest are a redraw. It is a strict two rather than a strict zero, and
//! neither block is the drawing's — a form draws a `TextEdit` whose placeholder
//! wants a `Cow<'static, str>`, so the string is cloned once a frame. Every
//! body it builds is refilled in place under that.
//!
//! Six gates rather than one, because what separates them is what each thing
//! the app can be doing costs — and a regression in one and not the others
//! says immediately which part moved.
//!
//! **Every gate asserts the frame it reached is the frame it names**, which is
//! the one thing a gate on a number cannot check for itself: a gate that stops
//! reaching its own frame goes on reporting zero, and reports it about a frame
//! nobody wanted measured. Three of the four had drifted that way at once — a
//! click on chrome the constant no longer pointed at, a press on a point in a
//! sketch the session was not editing, and a sweep that never left the region it
//! started on. All three read as passing.
//!
//! No GPU: `Ui` records and lays out without one, which is the half of a frame
//! this crate owns. What the renderer does with the result is gated in
//! `aperture`'s own suite, and what palantir does beneath both is gated in
//! palantir's.
//!
//! Harness wiring only: the allocator has to be *the* one of the binary that
//! runs, and that is the whole reason this is a target of its own — see
//! `common` for what it and the tester do.

#[global_allocator]
static ALLOC: common::CountingAllocator = common::CountingAllocator;

mod pointer;
mod raised;
