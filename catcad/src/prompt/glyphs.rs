//! The marks a form's buttons are drawn with.
//!
//! **A glyph and never a word**, because these sit *on* the drawing beside the
//! thing they are about, and a sentence there would be a sentence in the middle
//! of a model.
//!
//! Every one is checked against the font stack by
//! `every_button_on_the_form_has_a_glyph_to_draw_it`, on the same terms the
//! constraint marks are: a character nothing offers rasterizes to nothing, and a
//! button that draws blank is a button that is not there.

/// What confirm and cancel are drawn as.
///
/// The pair every interface agrees on, so nothing has to be read to be
/// understood.
pub(crate) const CONFIRM: &str = "\u{2713}";
pub(crate) const CANCEL: &str = "\u{2717}";

/// What an extrude does with the solid standing before it.
///
/// The same three every modeller draws these with: `+` adds what is grown to
/// what stands, `−` takes it out, and `∩` keeps only what both hold.
pub(crate) const JOINS: &str = "+";
pub(crate) const CUTS: &str = "\u{2212}";
pub(crate) const SHARES: &str = "\u{2229}";
