//! The lengths a number can be said in.

use serde::{Deserialize, Serialize};

/// One of the lengths a document reads and writes its numbers in.
///
/// **Five, and every one of them is a length.** An angle is not here: a sweep
/// is stated in degrees wherever it is stated at all, and a unit that could be
/// either would make every field ask which. What this settles is what a *bare*
/// number means and what a suffix converts from — see [`Notation`](super).
///
/// **The order is smallest first**, so the two imperial ones sit together at the
/// end rather than either side of the metric ones. Nothing walks the set yet —
/// the order is here so that whatever offers a choice of them needs no opinion
/// of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Unit {
    Millimetre,
    Centimetre,
    Metre,
    Inch,
    Foot,
}

impl Unit {
    /// How many millimetres one of these is.
    ///
    /// **Exact for all five**, which is the whole reason the store is a
    /// millimetre rather than a metre: an inch is 25.4 mm exactly by
    /// definition, and a foot is twelve of those. Written down as products
    /// rather than as decimals so that the definition is visible and cannot be
    /// mistyped in the last place.
    pub(crate) fn across(self) -> f64 {
        match self {
            Unit::Millimetre => 1.0,
            Unit::Centimetre => 10.0,
            Unit::Metre => 1000.0,
            Unit::Inch => 25.4,
            Unit::Foot => 12.0 * 25.4,
        }
    }

    /// Which unit `word` names, or `None` where it names none.
    ///
    /// **The marks as well as the words.** A drawing says `1/2"` far more often
    /// than it says `1/2in`, and a field that took only the letters would be
    /// refusing what its own readout is written in. Both are one unit rather
    /// than two, so nothing downstream can tell which was typed.
    ///
    /// Case is not read. `MM` and `mm` are the same length, and a field that
    /// argued about which would be arguing about the shift key.
    pub(crate) fn named(word: &str) -> Option<Self> {
        NAMED
            .iter()
            .find(|(said, _)| said.eq_ignore_ascii_case(word))
            .map(|&(_, unit)| unit)
    }
}

/// Every word and mark that names a unit, and which one it names.
///
/// A table rather than a `match`, because the reading is case-blind and a match
/// arm is not: folding the case of what somebody typed would want a buffer to
/// fold it into, where comparing against each of these wants nothing at all.
const NAMED: [(&str, Unit); 7] = [
    ("mm", Unit::Millimetre),
    ("cm", Unit::Centimetre),
    ("m", Unit::Metre),
    ("in", Unit::Inch),
    ("\"", Unit::Inch),
    ("ft", Unit::Foot),
    ("'", Unit::Foot),
];
