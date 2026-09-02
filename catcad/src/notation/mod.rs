//! What a number in this document means, read and written.

pub(crate) mod quantity;
pub(crate) mod unit;

mod reading;

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::notation::quantity::Quantity;
use crate::notation::reading::Reading;
use crate::notation::unit::Unit;

/// The most decimal places a number can be read out to.
///
/// Seventeen, which is what a `f64` carries: past that a reading writes digits
/// the machine does not have. A file naming more is refused — see
/// [`Fault::Precision`](crate::document::file::error::Fault).
pub(crate) const MOST_DECIMALS: u8 = 17;

/// How a document spells a number: what a bare one means, and how many places
/// it is read out to.
///
/// **The store is a millimetre and this is how it is said.** Every length the
/// model holds — a dimension, a plane's offset, how far a solid is carried — is
/// a millimetre, whatever this says. That is what makes the unit a *notation*
/// rather than a scale: choosing another one converts no geometry, moves
/// nothing, and cannot make a drawing come back a thousand times its own size.
/// What it changes is what somebody typing `10` means by it and what the
/// drawing reads back.
///
/// A millimetre because it is the one unit every other here is an exact number
/// of — see [`Unit::across`] — so a length typed in inches and read back in
/// inches is the number that was typed.
///
/// **Content, and so saved.** Two documents may be drawn in different units and
/// the file has to say which, or reopening one is reading its numbers as
/// somebody else's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Notation {
    unit: Unit,
    /// How many decimal places a number is read out to.
    ///
    /// Beside the unit rather than off it, because the two are one question
    /// asked twice: a document drawn in metres wants more places than one drawn
    /// in millimetres, and a reader deriving one from the other would be
    /// deciding for whoever set them.
    decimals: u8,
}

impl Default for Notation {
    /// Millimetres to two places, which is what every document drawn before
    /// there was a choice was drawn in.
    ///
    /// Two places is a hundredth of a millimetre — fine enough to draw with and
    /// coarse enough that a solve's own drift never shows.
    fn default() -> Self {
        Self {
            unit: Unit::Millimetre,
            decimals: 2,
        }
    }
}

impl Notation {
    /// What `text` says as a `quantity`, or `None` where it says no number.
    ///
    /// A length comes back in millimetres and an angle in degrees, which is
    /// what each of them is stored as.
    ///
    /// **A whole expression rather than a literal.** `10`, `1/2in`, `3*4` and
    /// `(1 + 2) * 5` all read; so does a suffix in any of the five lengths, and
    /// the marks a drawing writes them as. A bare length is said in this
    /// document's own unit and a suffixed one converts into it — see
    /// [`Reading`], which is the whole of the grammar.
    ///
    /// **`None` is "not a number yet" rather than a failure**, which is what
    /// every caller here wants: a field is read on the keystroke that changed
    /// it, and half a sum is a draft rather than a fault to report.
    pub(crate) fn read(&self, quantity: Quantity, text: &str) -> Option<f64> {
        Reading::of(text, self.said(quantity)).whole()
    }

    /// `value` appended to `into`, in whatever a `quantity` is said in.
    ///
    /// **Appended rather than handed back**, because both callers already hold
    /// the string they are filling: a drawing lays its marks out every frame
    /// and a form rewrites a draft as a drag moves, and a `String` per number
    /// per frame is the heap sixty times a second.
    ///
    /// **No suffix.** A document has one unit and every number in it is said in
    /// that unit, so writing it beside each of them says the same thing as many
    /// times as there are numbers. Where the unit is shown is the frame's
    /// business rather than the number's.
    pub(crate) fn write(&self, quantity: Quantity, value: f64, into: &mut String) {
        let said = value / self.across(quantity);
        write!(into, "{said:.*}", usize::from(self.decimals))
            .expect("writing to a string cannot fail");
    }

    /// How many of the stored number one of what is *shown* is worth.
    ///
    /// What a control scrubbing a raw number reads and writes through: a
    /// dimension is held in millimetres and shown in the document's own unit,
    /// so a widget handed the stored number would show millimetres and move by
    /// them. One for an angle, which is stored in what it is shown in.
    pub(crate) fn across(&self, quantity: Quantity) -> f64 {
        self.said(quantity).map_or(1.0, Unit::across)
    }

    /// The unit a bare `quantity` is said in, and `None` for one that is not a
    /// length — see [`Reading::said`].
    fn said(&self, quantity: Quantity) -> Option<Unit> {
        match quantity {
            Quantity::Length => Some(self.unit),
            Quantity::Angle => None,
        }
    }

    /// How many decimal places a number is read out to.
    pub(crate) fn decimals(&self) -> usize {
        usize::from(self.decimals)
    }

    /// Whether it asks for no more places than a number has — see
    /// [`MOST_DECIMALS`].
    pub(crate) fn readable(&self) -> Result<(), u8> {
        match self.decimals <= MOST_DECIMALS {
            true => Ok(()),
            false => Err(self.decimals),
        }
    }
}

#[cfg(test)]
mod internals {
    use crate::notation::Notation;
    use crate::notation::unit::Unit;

    impl Notation {
        /// A document drawn in `unit` and read out to `decimals` places.
        ///
        /// **Nothing in the application makes one yet**, which is what gates
        /// it: every document is drawn in millimetres to two places until
        /// there is a chooser to say otherwise. What asks is a test holding the
        /// pair against a document that is not the default, so that a reader
        /// writing a constant fails rather than agreeing by coincidence.
        pub(crate) fn drawn_in(unit: Unit, decimals: u8) -> Self {
            Self { unit, decimals }
        }
    }
}

#[cfg(test)]
mod tests;
