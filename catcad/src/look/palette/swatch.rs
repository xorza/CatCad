//! A colour as a palette file writes one down.

use std::fmt;

use glam::Vec3;
use palantir::Color;
use serde::de::{Error, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// One colour of a palette, packed as `0xRRGGBB` in sRGB.
///
/// **Bytes rather than floats**, and that is the whole of why the type exists.
/// A palette is authored, read and diffed as hex — `#7adcf3` is what the source
/// says and what a person checks against it — and a float triple does not come
/// back as the hex it was written as. [`Color`](palantir::Color) quantises to
/// sRGB through a cubic inverse that is good to one count per channel, which is
/// one count too many for a value that has to survive a round trip unchanged.
///
/// Alpha is absent on purpose. How much of the drawing shows through a pill is
/// a decision about the overlay rather than about the palette, so the roster
/// that states the size of a chip states the opacity of the surface it stands
/// on too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Swatch(u32);

impl Swatch {
    /// This colour as the overlay takes it.
    ///
    /// Linearised on the way, because the file states sRGB — what a person
    /// authors and what every other target of this palette receives — and
    /// [`Color`] holds linear. A value written into the fields unconverted
    /// renders bright.
    pub(crate) const fn color(self) -> Color {
        Color::hex(self.0)
    }

    /// This colour as aperture takes it.
    ///
    /// The same linearisation and then a reinterpretation, since a shade the
    /// renderer strokes with is the three channels [`Color`] already holds.
    pub(crate) const fn ink(self) -> Vec3 {
        let color = self.color();
        Vec3::new(color.r, color.g, color.b)
    }

    /// This colour at `alpha`, where `255` is opaque.
    ///
    /// Alpha is straight and is not gamma-encoded, so it is the one channel
    /// that passes through the conversion untouched.
    pub(crate) const fn fade(self, alpha: u8) -> Color {
        self.color().with_alpha(alpha as f32 / 255.0)
    }

    /// `#rrggbb`, or nothing.
    ///
    /// Strict about the whole shape rather than lenient about parts of it: a
    /// palette is generated, so a value this refuses is a broken generator and
    /// not a person to be forgiving towards.
    fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 7 || bytes[0] != b'#' {
            return None;
        }
        let mut packed = 0;
        for &byte in &bytes[1..] {
            let digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => return None,
            };
            packed = packed << 4 | u32::from(digit);
        }
        Some(Self(packed))
    }
}

impl Serialize for Swatch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("#{:06x}", self.0))
    }
}

impl<'de> Deserialize<'de> for Swatch {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Hex)
    }
}

struct Hex;

impl Visitor<'_> for Hex {
    type Value = Swatch;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a colour written as \"#rrggbb\"")
    }

    fn visit_str<E: Error>(self, text: &str) -> Result<Swatch, E> {
        Swatch::parse(text).ok_or_else(|| E::invalid_value(Unexpected::Str(text), &self))
    }
}

#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::look::palette::swatch::Swatch;

    impl Swatch {
        /// The three channels as the file wrote them, still sRGB.
        ///
        /// What a check reading pixels off a rendered frame holds them
        /// against: the target is encoded sRGB, and everything the overlay
        /// paints flat lands on it as the very bytes here — so a comparison in
        /// this currency is exact where one in
        /// [`Color`](palantir::Color)'s would be a comparison of two
        /// approximations.
        ///
        /// Nothing the application itself draws needs this. The theme takes
        /// the linear form, so a reading in the file's own currency exists for
        /// whatever reads pixels back.
        pub(crate) const fn srgb(self) -> [u8; 3] {
            [(self.0 >> 16) as u8, (self.0 >> 8) as u8, self.0 as u8]
        }
    }

    /// The swatch `text` names, which is the only way to build one: a palette
    /// is parsed, so a test that stated a colour any other way would be stating
    /// it in a currency the file does not use.
    #[cfg(test)]
    pub(crate) fn hex(text: &str) -> Swatch {
        ron::from_str(&format!("{text:?}")).expect(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text a palette holds is the text it comes back as, for every digit
    /// and both cases — which is the property the whole type is for.
    #[test]
    fn a_swatch_comes_back_as_the_hex_it_was_written_as() {
        for text in ["#000000", "#ffffff", "#1e1e1e", "#7adcf3", "#0f3e58"] {
            let swatch: Swatch = ron::from_str(&format!("{text:?}")).expect(text);
            assert_eq!(ron::to_string(&swatch).unwrap(), format!("{text:?}"));
        }
        // Written in either case, answered in one: a generator that shouted
        // would otherwise produce a file that failed its own round trip.
        let upper: Swatch = ron::from_str("\"#7ADCF3\"").unwrap();
        let lower: Swatch = ron::from_str("\"#7adcf3\"").unwrap();
        assert_eq!(upper, lower);
        assert_eq!(ron::to_string(&upper).unwrap(), "\"#7adcf3\"");
    }

    /// Every way of getting it wrong, refused.
    #[test]
    fn nothing_but_a_full_six_digit_hex_parses() {
        for text in [
            "\"\"",
            "\"#\"",
            "\"1e1e1e\"",   // no hash
            "\"#1e1e1\"",   // five digits
            "\"#1e1e1e0\"", // seven
            "\"#1e1e1eff\"",
            "\"#gggggg\"",
            "\"#1e 1e1\"",
            "\"rgb(30,30,30)\"",
        ] {
            assert!(
                ron::from_str::<Swatch>(text).is_err(),
                "{text} was accepted"
            );
        }
    }

    /// The packing is big-endian by channel, so the digits land where the hex
    /// says and not reversed — and both currencies read it the same way round.
    #[test]
    fn the_channels_pack_red_high_and_blue_low() {
        let swatch: Swatch = ron::from_str("\"#123456\"").unwrap();
        assert_eq!(swatch, Swatch(0x123456));
        assert_eq!(swatch.srgb(), [0x12, 0x34, 0x56]);
        let color = swatch.color();
        assert!(color.r < color.g && color.g < color.b, "{color:?}");
        assert_eq!(swatch.ink(), Vec3::new(color.r, color.g, color.b));
    }

    /// The file states sRGB and both currencies hold linear, so the conversion
    /// has to happen exactly once.
    ///
    /// Mid grey is the value that catches it: `#808080` is 0.502 encoded and
    /// 0.2159 linear. A swatch that skipped the conversion would answer 0.502,
    /// and one that ran it twice would answer 0.0356.
    ///
    /// Loose to a part in five hundred, because palantir linearises in a `const
    /// fn` and so approximates the transfer function rather than evaluating it.
    /// Either way of getting it wrong is two orders of magnitude further off.
    #[test]
    fn a_swatch_linearises_once_on_the_way_out() {
        let swatch: Swatch = ron::from_str("\"#808080\"").unwrap();
        assert!(
            (swatch.color().r - 0.2159).abs() < 2e-3,
            "{:?}",
            swatch.color()
        );
        assert_eq!(swatch.color().a, 1.0);
        // The two ends of the range are fixed points of the transfer function,
        // so they say the same thing whichever way it was applied.
        let black: Swatch = ron::from_str("\"#000000\"").unwrap();
        let white: Swatch = ron::from_str("\"#ffffff\"").unwrap();
        assert_eq!(black.ink(), Vec3::ZERO);
        assert_eq!(white.ink(), Vec3::ONE);
    }

    /// Fading keeps the colour and changes only what covers.
    #[test]
    fn a_faded_swatch_keeps_its_colour_and_takes_the_alpha_straight() {
        let swatch: Swatch = ron::from_str("\"#7adcf3\"").unwrap();
        let faded = swatch.fade(0x80);
        let Color { r, g, b, a } = faded;
        assert_eq!(Vec3::new(r, g, b), swatch.ink());
        // Straight, not gamma-encoded: 0x80 is 128/255 and stays there.
        assert!((a - 128.0 / 255.0).abs() < 1e-6, "{a}");
        assert_eq!(swatch.fade(0xff), swatch.color());
    }
}
