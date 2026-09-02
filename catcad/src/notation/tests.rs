use super::*;
use crate::notation::quantity::Quantity;

/// A document drawn in `unit`, read out to three places.
fn drawn_in(unit: Unit) -> Notation {
    Notation::drawn_in(unit, 3)
}

/// How near two lengths have to be to count as equal here.
///
/// Loose enough for a division and a multiplication to have been through
/// `f64`, tight enough that a wrong unit is nowhere near it — the two closest
/// lengths named differ by a factor of ten.
const NEAR: f64 = 1e-12;

/// **Every number a field can hold reads as the length it says**, in
/// millimetres, which is the one unit the model is written in.
///
/// Hand-computed throughout. An inch is 25.4 millimetres by definition and a
/// foot is twelve of those, so `1/2in` is 12.7 and `1ft` is 304.8 — exactly,
/// because every conversion here is a product of whole numbers and a tenth.
#[test]
fn a_field_reads_the_arithmetic_and_the_unit_it_was_given() {
    let notation = Notation::default();
    for (text, want) in [
        ("0", 0.0),
        ("5", 5.0),
        ("5.25", 5.25),
        (".5", 0.5),
        ("5.", 5.0),
        ("  7  ", 7.0),
        ("-3", -3.0),
        ("- 3", -3.0),
        ("+3", 3.0),
        ("2+3", 5.0),
        ("2 + 3 * 4", 14.0),
        ("(2 + 3) * 4", 20.0),
        ("10/4", 2.5),
        ("5 - -3", 8.0),
        ("2*3*4", 24.0),
        ("100/5/2", 10.0),
        // The suffix closes the product, which is what makes a fraction of an
        // inch a fraction of an inch.
        ("1in", 25.4),
        ("1/2in", 12.7),
        ("3/4\"", 19.05),
        ("1ft", 304.8),
        ("1'", 304.8),
        ("1cm", 10.0),
        ("1m", 1000.0),
        ("1mm", 1.0),
        ("3*2mm", 6.0),
        ("(1+1)in", 50.8),
        ("1in + 2mm", 27.4),
        ("1 in", 25.4),
        // Case is not read: the shift key says nothing about a length.
        ("1IN", 25.4),
        ("1Mm", 1.0),
    ] {
        let read = notation
            .read(Quantity::Length, text)
            .unwrap_or_else(|| panic!("{text:?}"));
        assert!(
            (read - want).abs() < NEAR,
            "{text:?} read {read}, not {want}"
        );
    }
}

/// **A bare number is said in the document's own unit, and a suffixed one
/// converts into it** — which is the whole of what a notation decides.
///
/// The same four texts read against three documents. A bare `2` is two of
/// whatever the document is drawn in; `2mm` is two millimetres in all three;
/// and `1/2` is a half either way, which is what says the division happens
/// before the unit is applied rather than after — read the other way round a
/// half of an inch over a half of an inch would be one.
#[test]
fn a_bare_number_is_the_documents_own_unit_and_a_suffix_converts() {
    for (unit, bare, half) in [
        (Unit::Millimetre, 2.0, 0.5),
        (Unit::Inch, 50.8, 12.7),
        (Unit::Metre, 2000.0, 500.0),
    ] {
        let notation = drawn_in(unit);
        for (text, want) in [("2", bare), ("1/2", half), ("2mm", 2.0), ("0", 0.0)] {
            let read = notation
                .read(Quantity::Length, text)
                .unwrap_or_else(|| panic!("{text:?}"));
            assert!(
                (read - want).abs() < NEAR,
                "{text:?} in {unit:?} read {read}, not {want}",
            );
        }
    }
}

/// **What is not a number reads as no number**, which is a draft that is not
/// finished rather than a fault to report.
///
/// **A division by nothing is in here** and needs no case of its own: `1/0` is
/// an infinity in `f64` and `0/0` is a `NaN`, and the one reading that the
/// answer has to be finite catches both.
///
/// **And a suffix in the middle of a product**, which is what
/// [`Reading::product`] gives up to make `1/2in` a half inch. A refusal is what
/// it owes there — answering six for `2mm*3` and eighteen for `3*2mm` would be
/// two answers to one question.
#[test]
fn what_does_not_say_a_number_says_none() {
    let notation = Notation::default();
    for text in [
        "", "   ", ".", "-", "abc", "5 apples", "5 5", "(1 + 2", "1 + 2)", "()", "1..2", "1.2.3",
        "*3", "3*", "1/0", "0/0", "2mm*3", "5furlong", "5f", "in",
    ] {
        assert_eq!(
            notation.read(Quantity::Length, text),
            None,
            "{text:?} read as a number"
        );
    }
}

/// **A number written out and read back is the number it started as**, in every
/// unit and at the precision the document names.
///
/// Which is what the pair is for: a field is seeded by writing a value into it
/// and committed by reading the draft back, so a document whose two halves
/// disagreed would move a dimension by opening a form on it.
///
/// Held to the precision rather than exactly — three places of a millimetre is
/// what the notation below promises, and asking for more would be asking it to
/// promise something it says it does not.
#[test]
fn a_number_written_out_reads_back_as_itself() {
    for unit in [
        Unit::Millimetre,
        Unit::Centimetre,
        Unit::Metre,
        Unit::Inch,
        Unit::Foot,
    ] {
        let notation = drawn_in(unit);
        // Half a thousandth of the document's own unit either way, which is
        // what three places rounds to.
        let room = unit.across() / 2000.0;
        for value in [0.0, 1.0, 12.5, -7.25, 1234.5] {
            let mut said = String::new();
            notation.write(Quantity::Length, value, &mut said);
            let read = notation
                .read(Quantity::Length, &said)
                .unwrap_or_else(|| panic!("{unit:?} wrote {said:?}, which reads as nothing"));
            assert!(
                (read - value).abs() <= room,
                "{unit:?} wrote {value} as {said:?} and read back {read}",
            );
        }
    }
}

/// **A document says its numbers in its own unit**, so a length written out is
/// the number somebody drawing in that unit would recognise.
///
/// Hand-computed: 25.4 millimetres is one inch, and a foot of it is a twelfth
/// of that. The decimals are the document's own, so the same length comes out
/// to as many places as it was asked for and no more.
#[test]
fn a_length_is_written_in_the_unit_the_document_is_drawn_in() {
    for (unit, decimals, value, want) in [
        (Unit::Millimetre, 2, 25.4, "25.40"),
        (Unit::Inch, 2, 25.4, "1.00"),
        (Unit::Inch, 3, 25.4, "1.000"),
        (Unit::Foot, 3, 304.8, "1.000"),
        (Unit::Centimetre, 1, 25.4, "2.5"),
        (Unit::Metre, 4, 25.4, "0.0254"),
        (Unit::Millimetre, 0, 25.4, "25"),
    ] {
        let mut said = String::new();
        Notation::drawn_in(unit, decimals).write(Quantity::Length, value, &mut said);
        assert_eq!(said, want, "{value} in {unit:?} to {decimals} places");
    }
}

/// **A notation appends**, which is what keeps a drawing's marks off the heap:
/// a caller holds the string it is filling and hands it over as it stands.
#[test]
fn writing_a_number_leaves_what_was_there_already() {
    let mut said = String::from("R");
    Notation::default().write(Quantity::Length, 3.5, &mut said);
    assert_eq!(said, "R3.50");
}

/// **An angle takes the arithmetic and none of the units**, which is what
/// splits the two kinds of number a form asks for.
///
/// A turn is stated in degrees and in nothing else, so a suffix on one is a
/// refusal rather than a conversion — `90mm` of turn is not an angle, and a
/// reading that scaled it would put a revolve where nobody asked. The document
/// below is drawn in inches, so a length reading of `90` would be `2286`: an
/// angle is not scaled by the document's unit either.
#[test]
fn an_angle_takes_the_arithmetic_and_refuses_a_length() {
    let notation = drawn_in(Unit::Inch);
    for (text, want) in [
        ("90", 90.0),
        ("180/2", 90.0),
        ("45*2", 90.0),
        ("-30", -30.0),
    ] {
        let read = notation
            .read(Quantity::Angle, text)
            .unwrap_or_else(|| panic!("{text:?}"));
        assert!(
            (read - want).abs() < NEAR,
            "{text:?} read {read}, not {want}"
        );
        assert_eq!(
            notation.read(Quantity::Length, text),
            Some(want * Unit::Inch.across()),
            "the same text as a length is not the document's own unit",
        );
    }
    for text in ["90mm", "1in", "90\"", "1'"] {
        assert_eq!(
            notation.read(Quantity::Angle, text),
            None,
            "{text:?} is not an angle"
        );
    }
    // And it is written out unscaled, where a length of the same number would
    // come back in inches.
    let mut said = String::new();
    notation.write(Quantity::Angle, 90.0, &mut said);
    assert_eq!(said, "90.000");
}
