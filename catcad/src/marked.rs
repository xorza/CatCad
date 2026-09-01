//! What the overlay's controls are drawn with, and what each one is called.
//!
//! **Artwork on the drawing and a word beside it.** A form stands *on* a model,
//! so a sentence there would be a sentence in the middle of one — which is why
//! these are icons. A tooltip is not on the drawing: it records into palantir's
//! own layer, and only while the pointer rests on the chip. So the rule that
//! keeps words off the model costs a form nothing, and the shape is the one the
//! relations bar already has.
//!
//! **One row per mark, and not a list per reader.** A glyph stated in one place
//! and its word in another is what drifts — a chip drawn with artwork nothing
//! names, or named with a word nothing draws. That is the argument
//! [`wording`](crate::wording) makes about the drawing's own relations, made
//! again here about the controls that build one.
//!
//! At the crate root rather than under `prompt`, because the second table below
//! is read from three places that live apart — a form's caption, the recipe's
//! row and the relation bar's chip — and a table under one of them is a table
//! the other two will not find.
//!
//! Every row is checked by `every_mark_is_drawn_and_named`. That the artwork it
//! names exists is the icon table's own check, which walks all of it rather
//! than the rows a control draws.

use silverpoint::{Bevel, Operation};

use crate::look::icons::Glyph;
use crate::timeline::feature::Feature;

/// One mark on a control: the artwork it carries, and what it is called.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Marked {
    /// What the chip is drawn with, what a caption is headed with, and what a
    /// row of the recipe carries.
    ///
    /// It is also what a chip is recorded under — see
    /// [`Prompt::doing_id`](crate::prompt::Prompt::doing_id), which hashes it —
    /// so two rows sharing a glyph would be two controls sharing an id.
    pub(crate) glyph: Glyph,
    /// What the tooltip says, what a row naming its own setting shows, and what
    /// a form calls itself.
    ///
    /// A caption rather than a noun, on the terms
    /// [`Wording::word`](crate::wording::Wording::word) states: it heads a
    /// control of its own rather than being read inside a sentence.
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

/// What a circle being drawn calls itself, over the field it asks for.
///
/// **What says which form is open**, which is the one thing the old row of
/// controls never carried: two fields and three chips are the same two fields
/// and three chips whichever form they stand on. A sweep's caption comes off
/// the step table below instead, being the same word its row and its chip
/// carry.
///
/// A dimension has none, and that is not an omission — see
/// [`Asking::named`](crate::prompt::Asking::named), which is where the
/// distinction is argued.
pub(crate) const CIRCLE: Marked = Marked::new(Glyph::Circle, "Circle");

/// What each kind of step is called and drawn as, everywhere a person reads it.
///
/// **One table and not one per reader**, which is the argument
/// [`wording`](crate::wording) makes about the drawing's relations, made again
/// about the recipe's steps: a form's caption, a row of the recipe and a chip
/// on the relation bar all show a kind, and three lists are three chances for
/// one of them to draw a kind as its neighbour.
///
/// Which is silent where it is wrong — one artwork reads much like another —
/// so no two rows may share a glyph, and [`EVERY`] is walked to say so. A list
/// nothing walks cannot be held to it.
pub(crate) const PLANE: Marked = Marked::new(Glyph::Plane, "Plane");
pub(crate) const SKETCH: Marked = Marked::new(Glyph::Sketch, "Sketch");
pub(crate) const EXTRUDE: Marked = Marked::new(Glyph::Extrude, "Extrude");
pub(crate) const REVOLVE: Marked = Marked::new(Glyph::Revolve, "Revolve");
pub(crate) const FILLET: Marked = Marked::new(Glyph::Round, "Fillet");
pub(crate) const CHAMFER: Marked = Marked::new(Glyph::Chamfer, "Chamfer");

/// How the step `feature` is drawn and named.
///
/// The pairing in one place rather than beside each reader that lays it out, on
/// the terms [`doing`] below states: three of them show it, and none of them
/// may disagree with the other two.
///
/// **"Fillet" and "Chamfer" where the timeline says one kind of step**, because
/// those are the words a draughtsman uses — the same split
/// [`wording::of`](crate::wording::of) already makes over a segment and
/// an edge. What tells the two apart is the one field they differ in.
pub(crate) fn making(feature: &Feature) -> Marked {
    match feature {
        Feature::Plane(_) => PLANE,
        Feature::Sketch { .. } => SKETCH,
        Feature::Extrude { .. } => EXTRUDE,
        Feature::Revolve { .. } => REVOLVE,
        Feature::Round { bevel, .. } => bevelled(*bevel),
    }
}

/// How a blend of `bevel` is drawn and named.
///
/// Apart from [`making`] above because the bar offers both *before* a step
/// exists: a chip has a kind and no feature to read it off.
pub(crate) fn bevelled(bevel: Bevel) -> Marked {
    match bevel {
        Bevel::Round => FILLET,
        Bevel::Flat => CHAMFER,
    }
}

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

/// Every mark the overlay draws, which only the check that walks it wants.
///
/// Beside the rows rather than down in the check, so a thirteenth is written
/// three lines above the list that has to name it — a row added and left out of
/// here is exactly the fault the check is for.
///
/// Gated on `test` alone rather than on `internals` beside it, on the terms
/// [`CatCad::internals`](crate::cat_cad::internals) argues: the one reader is a unit
/// test, and the wider gate would leave this dead in every build that turned
/// the feature on without turning tests on.
#[cfg(test)]
const EVERY: [Marked; 12] = [
    CONFIRM, CANCEL, JOINS, CUTS, SHARES, CIRCLE, PLANE, SKETCH, EXTRUDE, REVOLVE, FILLET, CHAMFER,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every mark carries a word, and no two of them collide.**
    ///
    /// That the artwork *exists* is the icon table's own check —
    /// `every_source_sits_at_its_own_glyph_and_paints_in_one_colour`, which
    /// walks all of it rather than the rows a control draws. What is left for
    /// this to say is the pairing.
    ///
    /// The collisions are this table's alone, and both are silent. A form
    /// records a chip under its glyph, so two of them sharing one would be two
    /// controls sharing an id — the second would never be pressed. Two rows
    /// under one word would be a tooltip saying the same thing twice, and two
    /// steps under one mark would be two rows of the recipe that look alike.
    #[test]
    fn every_mark_is_drawn_and_named() {
        for mark in EVERY {
            assert!(
                !mark.word.trim().is_empty(),
                "{mark:?} carries no word, so nothing says what it is",
            );
        }

        for (at, one) in EVERY.iter().enumerate() {
            for two in &EVERY[at + 1..] {
                assert_ne!(one.glyph, two.glyph, "{one:?} and {two:?} share a mark");
                assert_ne!(one.word, two.word, "{one:?} and {two:?} share a word");
            }
        }

        // And the three the operation row is laid out from are three, which is
        // what says the pairing carries the operation rather than dropping it.
        let [joins, cuts, shares] =
            [Operation::Join, Operation::Cut, Operation::Intersect].map(doing);
        assert_ne!(joins, cuts, "a join and a cut draw as one chip");
        assert_ne!(cuts, shares, "a cut and an intersect draw as one chip");
        assert_ne!(joins, shares, "a join and an intersect draw as one chip");
    }
}
