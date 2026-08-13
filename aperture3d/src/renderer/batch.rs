//! One overlay kind's CPU-side batch, and what a refresh of it rewrote.

use crate::highlight::{Highlight, Lit};
use crate::overlay::Overlay;
use crate::renderer::record::Instance;
use crate::tag::Tag;

/// One overlay kind's whole CPU-side state: what it flattens to, what a
/// highlight over it flattens to, and whether either needs rebuilding.
///
/// Held between frames for the same reason the GPU buffers they feed are: an
/// edit that moves one vertex should not discard and rebuild a whole batch.
/// Both vectors are emptied and refilled in place, so once one has grown to fit
/// the scene it stops allocating — which is what keeps a hover, whose only work
/// is rebuilding `lit`, off the heap entirely.
///
/// One per kind rather than one triple per stage: the flag, the instances and
/// the highlights of a kind are only ever touched together, and keeping them
/// apart is what used to mean five separate triples.
#[derive(Debug)]
pub(super) struct Batch<O: Overlay> {
    pub(super) instances: Vec<O::Record>,
    pub(super) lit: Vec<O::Record>,
    /// Whether the scene's own list has been edited since this was flattened.
    pub(super) dirty: bool,
}

/// What every overlay batch needs uploaded after a refresh.
#[derive(Debug, Clone, Copy)]
pub(super) struct Refreshed {
    pub(super) curves: Rebuilt,
    pub(super) rings: Rebuilt,
    pub(super) points: Rebuilt,
}

/// Which of a batch's two buffers a refresh rewrote, and so which need
/// uploading.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rebuilt {
    pub(super) instances: bool,
    pub(super) lit: bool,
}

impl<O: Overlay> Default for Batch<O> {
    /// Dirty from the start: nothing has been flattened yet, so everything is
    /// outstanding.
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            lit: Vec::new(),
            dirty: true,
        }
    }
}

impl<O: Overlay> Batch<O> {
    /// Bring both buffers up to date with `items`, and say which moved.
    ///
    /// `relight` is the caller's own flag: what is lit can change without the
    /// scene changing at all, which is what a pointer moving across a drawing
    /// does. The scene changing forces both, since an edit can add or remove
    /// whatever a tag named.
    pub(super) fn refresh(&mut self, items: &[O], highlights: &[Lit], relight: bool) -> Rebuilt {
        let moved = std::mem::take(&mut self.dirty);
        if moved {
            self.instances.clear();
            self.instances
                .reserve_exact(items.iter().map(O::record_count).sum());
            for item in items {
                self.instances.extend(item.records());
            }
        }
        if moved || relight {
            // How many there will be is not known before the walk — it depends
            // on what the caller lit — so this is the one buffer that grows
            // rather than reserving exactly. It settles after a few frames of
            // hovering and stops allocating there.
            self.lit.clear();
            for item in items {
                if let Some(look) = look_of(highlights, item.tag()) {
                    self.lit
                        .extend(item.records().map(|record| record.highlighted(look)));
                }
            }
        }
        Rebuilt {
            instances: moved,
            lit: moved || relight,
        }
    }
}

/// The look a tag was given, if any.
fn look_of(highlights: &[Lit], tag: Option<Tag>) -> Option<Highlight> {
    let tag = tag?;
    highlights
        .iter()
        .find_map(|lit| (lit.tag == tag).then_some(lit.look))
}
