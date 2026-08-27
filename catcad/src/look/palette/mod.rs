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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::palette::swatch::internals::hex;

    /// The shipped file parses, and says what the mapping says it says.
    ///
    /// One value out of each roster, chosen where a mistake would be quiet: a
    /// ground that came back a shade off is a seam, and a hue off the freedom
    /// ladder is a drawing making a claim it does not mean.
    ///
    /// **Written out rather than read back off the table**, which is the only
    /// way it says anything: a check that fetched its own expectation would
    /// pass whatever the generator emitted. So these six move by hand whenever
    /// the palette upstream is regenerated, and that is the moment somebody
    /// looks at what moved.
    #[test]
    fn the_shipped_palette_parses_and_holds_the_mapped_colours() {
        let palette = Palette::default();
        assert_eq!(palette.ground, hex("#1e1e1e"));
        assert_eq!(palette.chip, hex("#373737"));
        assert_eq!(palette.determined, hex("#87e2fe"));
        assert_eq!(palette.pinned, hex("#fd8974"));
        assert_eq!(palette.selected, hex("#b9dd0d"));
        assert_eq!(palette.goes, hex("#687e04"));
    }

    /// Every grey in the table is grey.
    ///
    /// The whole reason the palette is generated through a neutralisation
    /// rather than copied: ayu's own ramp runs cool at `gray_600` and warm at
    /// `gray_200`, and a step that arrived here still tinted would be the
    /// tint nobody asked for, in the one place it is most visible — a large
    /// flat surface.
    #[test]
    fn every_neutral_role_has_no_tint_left() {
        let palette = Palette::default();
        for (role, swatch) in [
            ("ground", palette.ground),
            ("pill", palette.pill),
            ("pill_edge", palette.pill_edge),
            ("rule", palette.rule),
            ("chip", palette.chip),
            ("chip_lit", palette.chip_lit),
            ("chip_active", palette.chip_active),
            ("chip_held", palette.chip_held),
            ("on_held", palette.on_held),
            ("ink", palette.ink),
            ("ink_lit", palette.ink_lit),
            ("ink_dim", palette.ink_dim),
            ("cube_low", palette.cube_low),
            ("cube_high", palette.cube_high),
            ("focus", palette.focus),
            ("solid", palette.solid),
            ("dormant", palette.dormant),
            ("ghost", palette.ghost),
            ("sheet_datum", palette.sheet_datum),
            ("depth_arrow", palette.depth_arrow),
        ] {
            let color = swatch.color();
            assert!(
                color.r == color.g && color.g == color.b,
                "{role} is {} and still carries a tint",
                ron::to_string(&swatch).unwrap()
            );
        }
    }

    /// The table survives being written back out, which is what says the file
    /// and this struct agree on all thirty-six roles rather than on the six the
    /// test above names.
    #[test]
    fn the_palette_round_trips_through_its_own_format() {
        let palette = Palette::default();
        let text = ron::to_string(&palette).unwrap();
        assert_eq!(ron::from_str::<Palette>(&text).unwrap(), palette);
        assert_eq!(text.matches("\"#").count(), 36);
    }

    /// The table is the generator's output and not a hand-written copy of it.
    ///
    /// The one property this whole arrangement rests on: the palette lives in
    /// `ayu-graphite.toml` and reaches here through `ayu-graphite/catcad/build.py`.
    /// A table somebody edited in place would be a second definition of every
    /// colour, and two definitions of a colour are two colours as soon as either
    /// is touched.
    #[test]
    fn the_table_is_generated_rather_than_written_here() {
        let banner = TABLE.lines().next().expect("the table is not empty");
        assert!(banner.contains("generated"), "{banner}");
        assert!(
            TABLE.contains("ayu-graphite/catcad/build.py"),
            "the table does not say what wrote it"
        );
    }

    /// A role the generator renamed is a hard error rather than a colour
    /// quietly left at whatever the struct happened to say.
    #[test]
    fn a_table_with_a_role_this_crate_does_not_know_is_refused() {
        let stale = TABLE.replacen("ground:", "backdrop:", 1);
        assert!(ron::from_str::<Palette>(&stale).is_err());
    }
}
