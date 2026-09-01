//! The counting allocator every allocation test target installs, and the
//! window it counts.

use backtrace::Backtrace;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};

thread_local! {
    /// Whether this thread is inside a measured window.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    /// Whether this thread is already recording one allocation, so the
    /// bookkeeping below neither recurses nor counts itself.
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static BLOCKS: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
    static TRACES: RefCell<Vec<Backtrace>> = const { RefCell::new(Vec::new()) };
}

/// A global allocator that counts what a measured window asks for, and keeps a
/// stack for each of them.
///
/// **Per thread, not per process, and that is correctness before it is speed.**
/// Cargo runs a binary's tests in parallel on one process, so a shared counter
/// would let one test's warmup land inside another's window. A lock does not
/// mend that: the threads that would pollute a window never ask for the lock —
/// the harness's own, and whatever a graphics driver keeps under a GPU gate —
/// so a shared counter would go on counting them.
///
/// **And the re-entry guard has to be per thread whatever the counters are.**
/// `RECORDING` says that *this call stack* is already inside the bookkeeping,
/// which is what stops the trace push below from recording itself forever. A
/// shared flag would answer for the whole process, so one thread recording
/// would silence another thread's counting. Once that one is thread-local the
/// rest cost nothing to keep beside it — and a shared `Vec` of stacks would
/// have to be pushed to under a lock the pushing itself can re-enter.
///
/// **What it gives up**: an allocation the measured work hands to another
/// thread is invisible. Every gate in this workspace drives its work on the
/// thread that opened the window, and one over work that fans out would have to
/// say so.
///
/// Outside a window the whole of the bookkeeping is one thread-local read, so
/// the tests sharing the binary pay no tax for any of it.
///
/// It counts heap *operations* rather than residency, so `dealloc` is passed
/// straight through. What a per-frame budget is about is how often the
/// allocator is reached, not how much is held.
#[derive(Debug)]
pub struct CountingAllocator;

impl CountingAllocator {
    /// Take in one allocation of `size`, with the stack that asked for it.
    ///
    /// The stack is captured unresolved, which is a walk and no symbol lookup —
    /// resolving happens only where a gate trips, and never on the path a
    /// passing one takes.
    #[inline]
    fn record(size: usize) {
        if !COUNTING.with(Cell::get) || RECORDING.with(Cell::get) {
            return;
        }
        BLOCKS.with(|blocks| blocks.set(blocks.get() + 1));
        BYTES.with(|bytes| bytes.set(bytes.get() + size as u64));
        RECORDING.with(|recording| recording.set(true));
        let stack = Backtrace::new_unresolved();
        TRACES.with(|traces| traces.borrow_mut().push(stack));
        RECORDING.with(|recording| recording.set(false));
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::record(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        Self::record(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// What one measured window reached the heap for.
#[derive(Debug)]
pub(super) struct Counted {
    pub(super) blocks: u64,
    pub(super) bytes: u64,
    /// One stack per block, in the order they were asked for.
    pub(super) traces: Vec<Backtrace>,
}

impl Counted {
    /// What `body` allocates on this thread.
    ///
    /// Whatever `body` does, the thread is left outside a window: a panic part
    /// way through would otherwise strand the flag and count everything after
    /// it.
    pub(super) fn of(body: impl FnOnce()) -> Self {
        TRACES.with(|traces| traces.borrow_mut().clear());
        let before = (BLOCKS.with(Cell::get), BYTES.with(Cell::get));
        let counting = Window::open();
        body();
        drop(counting);
        Self {
            blocks: BLOCKS.with(Cell::get) - before.0,
            bytes: BYTES.with(Cell::get) - before.1,
            traces: TRACES.with(|traces| std::mem::take(&mut *traces.borrow_mut())),
        }
    }
}

/// A window held open for as long as it lives, and shut however it ends.
#[derive(Debug)]
struct Window;

impl Window {
    fn open() -> Self {
        COUNTING.with(|counting| counting.set(true));
        Self
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        COUNTING.with(|counting| counting.set(false));
    }
}
