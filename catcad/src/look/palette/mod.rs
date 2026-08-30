//! The colours a theme is built out of, and where they come from.
//!
//! **Generated rather than decided here.** The table is a target of the
//! `ayu-graphite` palette repository, whose own rule is that its TOML is the
//! only palette definition — so a hand-written copy of these values in this crate
//! would be a second one, and two definitions of a colour are two colours as
//! soon as either is touched.
//!
//! Embedded rather than read at run time, because a theme is not something the
//! application can start without: a binary that had to find a file to know what
//! grey a chip is would be a binary with a way to have no answer.
//!
//! One table below is stated here and not generated —
//! [`Palette::probe`], which the visual suite paints every frame it
//! measures in. It is a second definition of nothing: no colour in it dresses
//! the application, and the rule it answers to is the opposite of the one
//! above, that a generator must never reach it.

pub(crate) mod swatch;

use serde::{Deserialize, Serialize};

use crate::look::palette::swatch::Swatch;

/// Ayu Graphite, as it is written down.
const TABLE: &str = include_str!("ayu-graphite.ron");

/// Every colour the application draws with, once each.
///
/// **One field per role and never per colour**, which is why several of these
/// hold the same grey. What a pill's rim is and what a rule between two groups
/// is are two decisions that happen to agree today, and a table that shared one
/// entry between them would be a table where they could not stop agreeing.
///
/// Colours only. A stroke width and the side of a chip are facts about the
/// interface rather than about the palette, and stay where the roster that uses
/// them states them.
///
/// [`Serialize`] is here for the round trip rather than for a writer: nothing in
/// this crate emits a palette, and a table that can be written back out is one a
/// test can prove the file agrees with, field for field.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Palette {
    pub(crate) ground: Swatch,
    pub(crate) pill: Swatch,
    pub(crate) pill_edge: Swatch,
    pub(crate) rule: Swatch,
    pub(crate) chip: Swatch,
    pub(crate) chip_lit: Swatch,
    pub(crate) chip_active: Swatch,
    pub(crate) chip_held: Swatch,
    pub(crate) on_held: Swatch,
    pub(crate) ink: Swatch,
    pub(crate) ink_lit: Swatch,
    pub(crate) ink_dim: Swatch,
    pub(crate) cube_low: Swatch,
    pub(crate) cube_high: Swatch,
    /// What palantir rings a widget the keyboard has reached with.
    ///
    /// A role of its own rather than the held colour it currently equals,
    /// because the two answer different questions — one is what a control wears
    /// while what it stands for is held, the other is where typing would go —
    /// and a palette that wanted to tell them apart could.
    pub(crate) focus: Swatch,

    pub(crate) solid: Swatch,
    pub(crate) determined: Swatch,
    pub(crate) partly: Swatch,
    pub(crate) free: Swatch,
    pub(crate) pinned: Swatch,
    pub(crate) dormant: Swatch,
    pub(crate) dormant_face: Swatch,
    pub(crate) face: Swatch,
    pub(crate) ghost: Swatch,
    pub(crate) sheet_ground: Swatch,
    pub(crate) sheet_front: Swatch,
    pub(crate) sheet_side: Swatch,
    pub(crate) sheet_datum: Swatch,
    pub(crate) mark: Swatch,
    pub(crate) redundant: Swatch,
    pub(crate) depth_arrow: Swatch,

    pub(crate) hovered: Swatch,
    pub(crate) selected: Swatch,

    pub(crate) goes: Swatch,
    pub(crate) stops: Swatch,
    /// The blue a form's operations were once told apart by.
    ///
    /// **Nothing draws with it.** A form's controls are the overlay's chips
    /// now, and a chip says *this one is set* by inverting rather than by a hue
    /// — see [`Chrome::chip_held`](crate::look::chrome::Chrome::chip_held),
    /// which is where that rule is argued. The role stays because this table is
    /// the generator's and not this crate's: a field dropped here is a file
    /// that will not parse the next time the palette repository emits one.
    pub(crate) doing: Swatch,
}

impl Default for Palette {
    /// The one shipped palette.
    ///
    /// Panics on a file this crate ships itself, which is the point: a table
    /// that will not parse is a generator that broke, and the test below turns
    /// that from something a user meets into something a build does.
    fn default() -> Self {
        ron::from_str(TABLE).expect("the shipped palette is malformed")
    }
}

#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::look::palette::Palette;
    use crate::look::palette::swatch::Swatch;

    impl Palette {
        /// The visual suite's own table, and the one palette this crate writes
        /// down rather than parses.
        ///
        /// **Why the suite does not measure the shipped one.** That table is a
        /// target of the palette repository, so it moves whenever the generator
        /// runs — and a golden is a claim about bytes. Held against the shipped
        /// palette, three goldens fail every regeneration and say only that a
        /// colour moved, which is the one thing a golden was never needed for.
        /// Held against this, they fail when the *rendering* moves and at no
        /// other time.
        ///
        /// **Stated here rather than in a file beside the shipped one**, which
        /// is what the argument at the top of this module asks for. A second
        /// file would be a second thing that can be regenerated, edited or
        /// forgotten, and the property this table exists for is that nothing
        /// moves it. Written as a literal it is also the compiler that keeps it
        /// whole: a role added to [`Palette`] is a build error here rather than
        /// a field silently left at a default.
        ///
        /// **The neutrals run one ramp at wide steps and the hues take a side
        /// of the wheel each**, on three rules the suite depends on:
        ///
        /// * The slab is neutral. A stroke is told from the surface under it by
        ///   saturation, so a tinted slab reads as ink.
        /// * The roles a sweep reads stand well clear of every other colour the
        ///   drawing paints, so a pixel wearing the wrong one is a failure
        ///   rather than a near miss.
        /// * Nothing is magenta. The suite stages `#ff00ff` as a colour a scene
        ///   cannot already wear, and counts it to find a highlight.
        ///
        /// The last two are checked below. What all of this costs is that a
        /// golden no longer shows the application in the colours a user sees it
        /// in — a deliberate price, and the reason this is a palette rather
        /// than a set of markers: the frames stay readable, so a rendering
        /// fault is still something an eye can find in one.
        pub(crate) const fn probe() -> Self {
            Self {
                ground: Swatch::of(0x101010),
                pill: Swatch::of(0x282828),
                pill_edge: Swatch::of(0xf0f0f0),
                rule: Swatch::of(0xf0f0f0),
                chip: Swatch::of(0x404040),
                chip_lit: Swatch::of(0x585858),
                chip_active: Swatch::of(0x707070),
                chip_held: Swatch::of(0xf0f0f0),
                on_held: Swatch::of(0x101010),
                ink: Swatch::of(0xb8b8b8),
                ink_lit: Swatch::of(0xf0f0f0),
                ink_dim: Swatch::of(0x909090),
                cube_low: Swatch::of(0x282828),
                cube_high: Swatch::of(0x585858),
                focus: Swatch::of(0xf0f0f0),

                solid: Swatch::of(0xd0d0d0),
                determined: Swatch::of(0x00b0ff),
                partly: Swatch::of(0xffd000),
                free: Swatch::of(0xff7000),
                pinned: Swatch::of(0xff0040),
                dormant: Swatch::of(0xa8a8a8),
                dormant_face: Swatch::of(0x004060),
                face: Swatch::of(0x0090c0),
                ghost: Swatch::of(0xc8c8c8),
                sheet_ground: Swatch::of(0x00c040),
                sheet_front: Swatch::of(0x0060ff),
                sheet_side: Swatch::of(0xc03030),
                sheet_datum: Swatch::of(0x909090),
                mark: Swatch::of(0x8060ff),
                redundant: Swatch::of(0xff2000),
                depth_arrow: Swatch::of(0xe0e0e0),

                hovered: Swatch::of(0xffffff),
                selected: Swatch::of(0x40ff40),

                goes: Swatch::of(0x00c000),
                stops: Swatch::of(0xe02020),
                doing: Swatch::of(0x0080ff),
            }
        }
    }
}

#[cfg(test)]
mod tests;
