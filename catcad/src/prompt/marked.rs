//! What a form's controls are drawn with, and what each one is called.
//!
//! **Artwork on the drawing and a word beside it.** A form stands *on* a model,
//! so a sentence there would be a sentence in the middle of one — which is why
//! these are icons. A tooltip is not on the drawing: it records into palantir's
//! own layer, and only while the pointer rests on the chip. So the rule that
//! keeps words off the model costs a form nothing, and the shape is the one the
//! relations bar already has.
//!
//! **One row per mark, and not two lists.** A glyph stated in one place and its
//! word in another is what drifts — a chip drawn with artwork nothing names, or
//! named with a word nothing draws. That is the argument
//! [`wording`](crate::wording) makes about the drawing's own relations, made
//! again here about a form's controls.
//!
//! Every row is checked by `every_mark_on_the_form_is_drawn_and_named`. That
//! the artwork it names exists is the icon table's own check, which walks all
//! of it rather than the rows a form draws.

use silverpoint::Operation;

use crate::look::icons::Glyph;

/// One mark on a form: the artwork it carries, and what it is called.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Marked {
    /// What the chip is drawn with, or what a caption is headed with.
    ///
    /// It is also what a chip is recorded under — see
    /// [`Prompt::doing_id`](crate::prompt::Prompt::doing_id), which hashes it —
    /// so two rows sharing a glyph would be two controls sharing an id.
    pub(crate) glyph: Glyph,
    /// What the tooltip says, what a row naming its own setting shows, and what
    /// a form calls itself.
    ///
    /// A caption rather than a noun, on the terms
    /// [`Named::word`](crate::wording::Named::word) states: it heads a control
    /// of its own rather than being read inside a sentence.
    pub(crate) word: &'static str,
}

impl Marked {
    const fn new(glyph: Glyph, word: &'static str) -> Self {
        Self { glyph, word }
    }
}

/// What confirm and cancel are drawn as.
///
/// The pair every interface agrees on, so nothing has to be read to be
/// understood.
pub(crate) const CONFIRM: Marked = Marked::new(Glyph::Confirm, "Confirm");
pub(crate) const CANCEL: Marked = Marked::new(Glyph::Cancel, "Cancel");

/// What a sweep does with the solid standing before it.
///
/// The same three every modeller offers: a join adds what is grown to what
/// stands, a cut takes it out, and an intersect keeps only what both hold.
///
/// **Drawn as the answer rather than as the operator.** A `+` and a `−` below a
/// field are a stepper to anybody who has not been told otherwise, and `∩` is a
/// symbol somebody has to already know. Each of these is instead a picture of
/// what comes out: one pair of squares, three times over, with the part that is
/// kept stroked whole and the part that is consumed dashed. The word each
/// carries is still shown for the one that is set, because a picture of a
/// result is read faster once you know it is a result.
pub(crate) const JOINS: Marked = Marked::new(Glyph::Join, "Join");
pub(crate) const CUTS: Marked = Marked::new(Glyph::Cut, "Cut");
pub(crate) const SHARES: Marked = Marked::new(Glyph::Intersect, "Intersect");

/// What each kind of form calls itself, over the fields it asks for.
///
/// **What says which form is open**, and the one thing the old row of controls
/// never carried: two fields and three chips are the same two fields and three
/// chips whether the sweep is a revolve or an extrude.
///
/// A dimension has none, and that is not an omission — see
/// [`Asking::named`](crate::prompt::Asking::named), which is where the
/// distinction is argued.
pub(crate) const CIRCLE: Marked = Marked::new(Glyph::Circle, "Circle");
pub(crate) const EXTRUDE: Marked = Marked::new(Glyph::Extrude, "Extrude");
pub(crate) const REVOLVE: Marked = Marked::new(Glyph::Revolve, "Revolve");

/// The chip that sets `operation`.
///
/// The pairing in one place rather than beside the row that lays it out: the
/// form reads it to draw the three chips, and again to name the one that is
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
/// Beside the rows rather than in the test file, so an eighth mark is written
/// three lines above the list that has to name it — a row added and left out of
/// here is exactly the fault the check is for.
///
/// Gated on `test` alone rather than on `internals` beside it, on the terms
/// [`CatCad::internals`](crate::internals) argues: the one caller is a unit
/// test, and the wider gate would leave this dead in every build that turned
/// the feature on without turning tests on.
#[cfg(test)]
pub(crate) mod internals {
    use crate::prompt::marked::{
        CANCEL, CIRCLE, CONFIRM, CUTS, EXTRUDE, JOINS, Marked, REVOLVE, SHARES,
    };

    /// Every mark a form draws.
    pub(crate) const EVERY: [Marked; 8] = [
        CONFIRM, CANCEL, JOINS, CUTS, SHARES, CIRCLE, EXTRUDE, REVOLVE,
    ];
}
