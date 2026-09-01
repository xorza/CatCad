//! What one overlay kind flattens to.

use crate::batch::Batch;
use crate::primitive::Flatten;
use crate::renderer::cpu::marked::Marked;
use crate::renderer::highlights::Highlights;
use crate::renderer::record::Instance;

/// Two buffers: what one kind flattens to, and what a highlight over it
/// flattens to.
///
/// Held apart from what fills either, because the filling is the one thing the
/// kinds disagree about — three of them flatten themselves from a batch and text
/// needs a shaper and a sheet — while this half is the same for all four: two
/// [`Marked`] buffers refilled in place, each answering for itself on the terms
/// [`Batch::take_dirty`] sets.
#[derive(Debug)]
pub(crate) struct Records<R> {
    /// The kind drawn as itself.
    pub(crate) ordinary: Marked<R>,
    /// The same records again in a highlight's look, for whatever a caller has
    /// singled out — and empty whenever nothing is lit.
    pub(crate) lit: Marked<R>,
}

impl<R> Default for Records<R> {
    /// Hand-written because deriving would demand `R: Default`, which is a
    /// claim about records that nothing here needs.
    fn default() -> Self {
        Self {
            ordinary: Marked::default(),
            lit: Marked::default(),
        }
    }
}

impl<R> Records<R> {
    /// Bring both buffers up to date with `batch`, for a kind that can flatten
    /// itself.
    ///
    /// Takes the batch's mark as it goes, which is the whole of how this knows
    /// the scene changed, and leaves its own for whoever uploads. `relight` is
    /// the caller's own flag alongside it: what is lit can change without the
    /// scene changing at all, which is what a pointer moving across a drawing
    /// does. The scene changing forces both, since an edit can add or remove
    /// whatever a tag named.
    ///
    /// Every kind but text, which fills its buffers itself — see
    /// [`TextRecords::refresh`]. It needs the shaper and the sheet to know what a
    /// run comes to, it has two more things that can move it, and its highlight
    /// is a second *shaping* rather than the same records in another colour.
    /// None of that fits a seam whose whole input is a batch.
    pub(crate) fn refresh<O: Flatten<Record = R>>(
        &mut self,
        batch: &mut Batch<O>,
        highlights: &Highlights,
        relight: bool,
    ) where
        R: Instance,
    {
        let moved = batch.take_dirty();
        let items: &[O] = batch;
        if moved {
            let ordinary = self.ordinary.emptied();
            // Counted here rather than up front, so a still frame does not walk
            // the batch to reserve room it is not about to fill.
            ordinary.reserve_exact(items.iter().map(O::record_count).sum());
            for item in items {
                ordinary.extend(item.records());
            }
        }
        if moved || relight {
            // How many there will be is not known before the walk — it depends
            // on what the caller lit — so this is the one buffer that grows
            // rather than reserving exactly. It settles after a few frames of
            // hovering and stops allocating there.
            let lit = self.lit.emptied();
            for item in items {
                let Some(look) = highlights.look_of(item.tag()) else {
                    continue;
                };
                lit.extend(item.records().map(|record| record.highlighted(look)));
            }
        }
    }
}
