//! What the drawing's relations are called where a person will read them.

use silverpoint::{Along, Constraint};

/// What one relation is called, in both places a person reads it.
///
/// **One table where there were two**, and they were two tables over the same
/// fourteen variants that grouped them differently — the drawing marked a point
/// on an edge and a point on a circle with one glyph, and the bar captioned them
/// "On edge" and "On circle". Neither knew about the other, so a relation added
/// to one was a button with no mark or a mark with no button, and the two were
/// four files apart.
///
/// Stated at the finer of the two groupings, which is what lets one row answer
/// both: a row per thing a reader can be shown, each naming its word and its
/// mark together.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Named {
    /// What a control offering it is captioned with — a word, because a button
    /// is read once and deliberately.
    ///
    /// A caption rather than a noun, which is what tells it from
    /// [`noun`](crate::status): that one's answers are lowercase because they
    /// are read inside a sentence in the status line, where these head a button
    /// of their own.
    pub(crate) word: &'static str,
    /// The draughtsman's mark the drawing shows it as, or `None` for a
    /// dimension, which is drawn as its number.
    ///
    /// `None` rather than a blank, because the two are different answers: a
    /// dimension has no mark *because* it has a figure, and a caller that
    /// reached for one without asking [`Constraint::value`] first has made a
    /// mistake rather than found an empty string.
    pub(crate) glyph: Option<&'static str>,
}

impl Named {
    /// A relation, which has both.
    const fn relation(word: &'static str, glyph: &'static str) -> Self {
        Self {
            word,
            glyph: Some(glyph),
        }
    }

    /// A dimension, which is drawn as its number and so has no mark.
    const fn dimension(word: &'static str) -> Self {
        Self { word, glyph: None }
    }
}

/// What to call `constraint` where a person will read it.
///
/// The marks are the draughtsman's where there is one, because a drawing is read
/// at a glance and a word is not: ⊥ and ∥ say what they mean to anyone who has
/// seen a technical drawing, and are what every modeller uses. Every glyph here
/// was checked to have one in the faces the shaper falls back through — see
/// `every_relation_is_named_both_ways_and_every_mark_has_a_glyph`.
///
/// The words are the drawing's rather than the solver's: a segment reads as an
/// *edge*, because what the drawing shows is the boundary of something and
/// "segment" is the word for what solves it.
pub(crate) fn named(constraint: Constraint) -> Named {
    match constraint {
        // A coincidence makes two points one, so it is drawn as the one.
        Constraint::Coincident { .. } => Named::relation("Coincident", "\u{2022}"),
        // The three readings of one pair are three different things to offer,
        // so each is captioned for the span it measures — see
        // [`Along`](silverpoint::Along).
        Constraint::Distance {
            along: Along::Shortest,
            ..
        } => Named::dimension("Distance"),
        Constraint::Distance {
            along: Along::Horizontal,
            ..
        } => Named::dimension("Horizontal distance"),
        Constraint::Distance {
            along: Along::Vertical,
            ..
        } => Named::dimension("Vertical distance"),
        // A standoff and a spacing are a distance to a reader: what differs is
        // what they are measured between, which is plain from what was picked.
        Constraint::Standoff { .. } | Constraint::Spacing { .. } => Named::dimension("Distance"),
        Constraint::Radius { .. } => Named::dimension("Radius"),
        Constraint::Horizontal { .. } => Named::relation("Horizontal", "\u{2015}"),
        Constraint::Vertical { .. } => Named::relation("Vertical", "\u{2502}"),
        Constraint::Parallel { .. } => Named::relation("Parallel", "\u{2225}"),
        Constraint::Perpendicular { .. } => Named::relation("Perpendicular", "\u{22A5}"),
        // One mark and two words. "Is on" is the same relation whether what it
        // is on is straight or curved, and the drawing says so at a glance —
        // where a button has room to say which, and a reader choosing between
        // two of them wants it.
        Constraint::PointOnSegment { .. } => Named::relation("On edge", "\u{2208}"),
        Constraint::PointOnCircle { .. } => Named::relation("On circle", "\u{2208}"),
        // Likewise: what the drawing has to say is that two things match, and
        // which two is plain from what the mark sits between.
        Constraint::EqualLength { .. } | Constraint::EqualRadius { .. } => {
            Named::relation("Equal", "=")
        }
        // A letter, like the `R` a radius is prefixed with. Tangency has no
        // draughtsman's mark that carries into a font — the drawings that need
        // one letter it too.
        Constraint::Tangent { .. } => Named::relation("Tangent", "T"),
    }
}
