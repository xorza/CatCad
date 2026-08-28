//! What a form's buttons are drawn with, and what each one is called.
//!
//! **A mark on the drawing and a word on hover.** A form stands *on* a model,
//! so a sentence beside it would be a sentence in the middle of one — which is
//! why these are glyphs. A tooltip is not on the drawing: it records into
//! palantir's own layer, and only while the pointer rests on the square. So the
//! rule that keeps words off the model costs a form nothing, and the shape is
//! the one the relations bar already has.
//!
//! **One row per button, and not two lists.** A mark stated in one place and
//! its word in another is what drifts — a square drawn with a glyph nothing
//! names, or named with a word nothing draws. That is the argument
//! [`wording`](crate::wording) makes about the drawing's own relations, made
//! again here about a form's controls.
//!
//! Every row is checked by `every_button_on_the_form_is_drawn_and_named`, on
//! the same terms the constraint marks are: a character nothing offers
//! rasterizes to nothing, and a button that draws blank is a button that is not
//! there. The word is checked beside it, a button nothing names being what this
//! table exists to stop.

use silverpoint::Operation;

/// One button of a form: the mark it carries, and what it is called.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Marked {
    /// What the square is drawn with.
    ///
    /// It is also what the button is recorded under — see
    /// [`Prompt::doing_id`](crate::prompt::Prompt::doing_id), which hashes it —
    /// so two rows sharing a glyph would be two controls sharing an id.
    pub(crate) glyph: &'static str,
    /// What the tooltip says, and what a row naming its own setting shows.
    ///
    /// A caption rather than a noun, on the terms
    /// [`Named::word`](crate::wording::Named::word) states: it heads a control
    /// of its own rather than being read inside a sentence.
    pub(crate) word: &'static str,
}

impl Marked {
    const fn new(glyph: &'static str, word: &'static str) -> Self {
        Self { glyph, word }
    }
}

/// What confirm and cancel are drawn as.
///
/// The pair every interface agrees on, so nothing has to be read to be
/// understood.
pub(crate) const CONFIRM: Marked = Marked::new("\u{2713}", "Confirm");
pub(crate) const CANCEL: Marked = Marked::new("\u{2717}", "Cancel");

/// What a sweep does with the solid standing before it.
///
/// The same three every modeller draws these with: `+` adds what is grown to
/// what stands, `−` takes it out, and `∩` keeps only what both hold.
///
/// **And not one of the three reads as itself under a number.** A plus and a
/// minus below a field are a stepper to anybody who has not been told
/// otherwise, and an intersection is a symbol somebody has to already know.
/// That is what the word each carries is for, and why the row shows the word of
/// the one that is set rather than waiting to be hovered.
pub(crate) const JOINS: Marked = Marked::new("+", "Join");
pub(crate) const CUTS: Marked = Marked::new("\u{2212}", "Cut");
pub(crate) const SHARES: Marked = Marked::new("\u{2229}", "Intersect");

/// The button that sets `operation`.
///
/// The pairing in one place rather than beside the row that lays it out: the
/// form reads it to draw the three squares, and again to name the one that is
/// set, and those two must not be able to disagree.
pub(crate) fn doing(operation: Operation) -> Marked {
    match operation {
        Operation::Join => JOINS,
        Operation::Cut => CUTS,
        Operation::Intersect => SHARES,
    }
}

/// The whole table at once, which only the check that walks it wants.
///
/// Beside the rows rather than in the test file, so a sixth button is written
/// three lines above the list that has to name it — a row added and left out of
/// here is exactly the fault the check is for.
///
/// Gated on `test` alone rather than on `internals` beside it, on the terms
/// [`CatCad::internals`](crate::internals) argues: the one caller is a unit
/// test, and the wider gate would leave this dead in every build that turned
/// the feature on without turning tests on.
#[cfg(test)]
pub(crate) mod internals {
    use crate::prompt::marked::{CANCEL, CONFIRM, CUTS, JOINS, Marked, SHARES};

    /// Every button a form draws.
    pub(crate) const EVERY: [Marked; 5] = [CONFIRM, CANCEL, JOINS, CUTS, SHARES];
}
