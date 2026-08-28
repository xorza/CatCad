//! The alphabet the gizmo writes its face names in.
//!
//! **Strokes rather than glyphs, because the words are not on the screen.** A
//! name on this cube lies *in* the face it names — it leans and foreshortens
//! with it, which is what says the cube is a solid being turned rather than a
//! picture of one. A shaped run of text is a rectangle of pixels the compositor
//! puts down square to the screen, and there is no asking it to lean. A letter
//! made of line segments has corners, and a corner is a point that can be put
//! wherever the face puts it.
//!
//! Capitals only, and only the sixteen that spell the six names. A letter
//! nothing writes is a letter nobody drew, so the set is closed and
//! `every_letter_the_cube_writes_is_drawn` says so.

use glam::Vec2;

/// One letter, as the strokes a pen would draw it in.
///
/// Nested, and it costs nothing: these are `'static` slices in the binary
/// rather than collections, so the run of strokes and the points in each are
/// one read apiece and no allocation at all.
pub(super) type Strokes = &'static [&'static [Vec2]];

/// How wide a letter is drawn, against the height of one.
///
/// **Condensed, because six letters have to fit across one face.** `BOTTOM` is
/// the longest name the cube carries, and a square alphabet would have to set
/// it small enough to be a smudge. Narrowed, the same run fits at half again
/// the cap height — and a condensed capital is what a drawing office letters
/// with anyway.
pub(super) const NARROW: f32 = 0.62;

/// How much air is left between two letters, in the same units.
pub(super) const TRACKING: f32 = 0.22;

/// How wide the word `name` is set, as a multiple of its cap height.
pub(super) fn width(name: &str) -> f32 {
    let letters = name.len() as f32;
    letters * NARROW + (letters - 1.0) * TRACKING
}

/// How `letter` is drawn, on a box one wide and one tall with the baseline at
/// zero.
///
/// Authored square and set narrow, which is a decision about the *type* rather
/// than about the letter: a diagonal drawn on a square box and then squeezed is
/// the same diagonal a condensed face would have drawn, and it keeps every
/// letter in one coordinate system to read.
pub(super) fn strokes(letter: u8) -> Strokes {
    match letter {
        b'A' => A,
        b'B' => B,
        b'C' => C,
        b'E' => E,
        b'F' => F,
        b'G' => G,
        b'H' => H,
        b'I' => I,
        b'K' => K,
        b'L' => L,
        b'M' => M,
        b'N' => N,
        b'O' => O,
        b'P' => P,
        b'R' => R,
        b'T' => T,
        _ => &[],
    }
}

const fn at(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

const A: Strokes = &[
    &[at(0.0, 0.0), at(0.5, 1.0), at(1.0, 0.0)],
    &[at(0.18, 0.36), at(0.82, 0.36)],
];

const B: Strokes = &[
    &[
        at(0.0, 0.0),
        at(0.0, 1.0),
        at(0.7, 1.0),
        at(1.0, 0.78),
        at(0.7, 0.55),
        at(0.0, 0.55),
    ],
    &[at(0.7, 0.55), at(1.0, 0.3), at(0.7, 0.0), at(0.0, 0.0)],
];

const C: Strokes = &[&[
    at(1.0, 0.8),
    at(0.72, 1.0),
    at(0.28, 1.0),
    at(0.0, 0.72),
    at(0.0, 0.28),
    at(0.28, 0.0),
    at(0.72, 0.0),
    at(1.0, 0.2),
]];

const E: Strokes = &[
    &[at(1.0, 1.0), at(0.0, 1.0), at(0.0, 0.0), at(1.0, 0.0)],
    &[at(0.0, 0.5), at(0.78, 0.5)],
];

const F: Strokes = &[
    &[at(1.0, 1.0), at(0.0, 1.0), at(0.0, 0.0)],
    &[at(0.0, 0.52), at(0.75, 0.52)],
];

const G: Strokes = &[&[
    at(1.0, 0.8),
    at(0.72, 1.0),
    at(0.28, 1.0),
    at(0.0, 0.72),
    at(0.0, 0.28),
    at(0.28, 0.0),
    at(0.72, 0.0),
    at(1.0, 0.2),
    at(1.0, 0.45),
    at(0.55, 0.45),
]];

const H: Strokes = &[
    &[at(0.0, 1.0), at(0.0, 0.0)],
    &[at(1.0, 1.0), at(1.0, 0.0)],
    &[at(0.0, 0.5), at(1.0, 0.5)],
];

const I: Strokes = &[
    &[at(0.1, 1.0), at(0.9, 1.0)],
    &[at(0.5, 1.0), at(0.5, 0.0)],
    &[at(0.1, 0.0), at(0.9, 0.0)],
];

const K: Strokes = &[
    &[at(0.0, 1.0), at(0.0, 0.0)],
    &[at(1.0, 1.0), at(0.05, 0.45), at(1.0, 0.0)],
];

const L: Strokes = &[&[at(0.0, 1.0), at(0.0, 0.0), at(1.0, 0.0)]];

const M: Strokes = &[&[
    at(0.0, 0.0),
    at(0.0, 1.0),
    at(0.5, 0.42),
    at(1.0, 1.0),
    at(1.0, 0.0),
]];

const N: Strokes = &[&[at(0.0, 0.0), at(0.0, 1.0), at(1.0, 0.0), at(1.0, 1.0)]];

const O: Strokes = &[&[
    at(0.28, 1.0),
    at(0.72, 1.0),
    at(1.0, 0.72),
    at(1.0, 0.28),
    at(0.72, 0.0),
    at(0.28, 0.0),
    at(0.0, 0.28),
    at(0.0, 0.72),
    at(0.28, 1.0),
]];

const P: Strokes = &[&[
    at(0.0, 0.0),
    at(0.0, 1.0),
    at(0.72, 1.0),
    at(1.0, 0.77),
    at(0.72, 0.54),
    at(0.0, 0.54),
]];

const R: Strokes = &[
    &[
        at(0.0, 0.0),
        at(0.0, 1.0),
        at(0.72, 1.0),
        at(1.0, 0.77),
        at(0.72, 0.54),
        at(0.0, 0.54),
    ],
    &[at(0.45, 0.54), at(1.0, 0.0)],
];

const T: Strokes = &[&[at(0.0, 1.0), at(1.0, 1.0)], &[at(0.5, 1.0), at(0.5, 0.0)]];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::cube::facet::SIDES;

    /// **Every letter the cube writes is drawn, and every stroke of it is a
    /// stroke.**
    ///
    /// The failure this guards is silent and total, and it is the one a set of
    /// icons is guarded against for the same reason: a letter the table lacks
    /// draws nothing at all, so the word comes out with a hole in it and
    /// nothing else notices. Renaming a face to one with a new letter in it is
    /// exactly how that happens.
    ///
    /// A stroke of one point is checked beside it, because a polyline of one
    /// point is a segment of no length: it draws nothing, on the same terms.
    #[test]
    fn every_letter_the_cube_writes_is_drawn() {
        for side in SIDES {
            for letter in side.name.bytes() {
                let strokes = strokes(letter);
                assert!(
                    !strokes.is_empty(),
                    "{} is spelled with {}, which nothing draws",
                    side.name,
                    letter as char,
                );
                for stroke in strokes {
                    assert!(
                        stroke.len() >= 2,
                        "{} has a stroke of {} points",
                        letter as char,
                        stroke.len(),
                    );
                    for at in *stroke {
                        let inside = (0.0..=1.0).contains(&at.x) && (0.0..=1.0).contains(&at.y);
                        assert!(inside, "{} reaches {at:?}, outside its box", letter as char);
                    }
                }
            }
        }
    }

    /// A word is as wide as its letters and the air between them, which is what
    /// the face has to make room for.
    ///
    /// Worked by hand rather than read back off the same call: three letters
    /// are three bodies and two gaps — `3 × 0.62 + 2 × 0.22`, which is `2.30`.
    #[test]
    fn a_word_is_as_wide_as_its_letters_and_the_air_between_them() {
        assert!((width("TOP") - 2.30).abs() < 1e-5, "{}", width("TOP"));
        // And the longest name the cube carries is the one a face is sized
        // against, so it had better be the widest.
        let widest = SIDES
            .into_iter()
            .map(|side| width(side.name))
            .fold(0.0, f32::max);
        assert!((width("BOTTOM") - widest).abs() < 1e-5);
    }
}
