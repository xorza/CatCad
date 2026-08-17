//! How far through a dimension the pointer is, and what the next click states.

use glam::DVec2;
use silverpoint::{Along, Constraint, Dimension, Entity, Sketch};

/// How much a reading has to measure to be on offer, in sketch units.
///
/// Far below anything a hand places and far above the drift a solve leaves,
/// which is what every "is this the same place" in a drawing has to be.
///
/// Not the sketch's own tolerance, and it need not be: what this decides is
/// which reading to *offer*, and being a shade stricter than
/// [`Sketch::fitted`](silverpoint::Sketch) only means falling back to a reading
/// that measures something. Being looser would be the mistake — a reading
/// offered here and refused there is a pointer position that previews nothing.
const MEASURES_NOTHING: f64 = 1e-6;

/// A dimension being placed: what has been picked, and how it is being read.
///
/// Three states because the gesture has three steps, and the type is what keeps
/// them from being three fields that can disagree: what is picked and how far
/// through it is are one value, so putting the tool down abandons the whole of
/// it — the same argument [`Tool`](super::Tool) makes for carrying a line's
/// first click inside the tool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Dimensioning {
    /// Nothing picked. The next click says what is being measured.
    Empty,
    /// One thing picked, waiting for what to measure it against.
    Picked(Entity),
    /// Enough picked. The pointer now says where the number goes and — for a
    /// pair of points — which of the three ways it is read; the next click
    /// commits what it is showing.
    Placing {
        first: Entity,
        /// `None` for a circle, which is a dimension on its own.
        second: Option<Entity>,
        /// The reading the bar named, where one did.
        ///
        /// `None` leaves it to the pointer, which is how the tool works and how
        /// a modeller expects it to: drag out to the side for one span, up for
        /// another. A bar button has no pointer to read and says which it meant,
        /// and then saying it again with the pointer would be taking the answer
        /// back.
        along: Option<Along>,
    },
}

impl Dimensioning {
    /// The tool with `entity` picked, or `None` where picking it means nothing.
    ///
    /// A circle needs no second pick: the only dimension it admits is its own
    /// radius, so the click that names it is the click that finishes naming.
    /// Everything else is two, and a second that admits no dimension is a click
    /// that did nothing rather than one that starts over — the first pick is
    /// still what you meant.
    pub(crate) fn picked(self, sketch: &Sketch, entity: Entity) -> Option<Self> {
        match self {
            Dimensioning::Empty => Some(match entity {
                Entity::Circle(_) => Dimensioning::Placing {
                    first: entity,
                    second: None,
                    along: None,
                },
                _ => Dimensioning::Picked(entity),
            }),
            Dimensioning::Picked(first) => {
                let placing = Dimensioning::Placing {
                    first,
                    second: Some(entity),
                    along: None,
                };
                // Asked by proposing one rather than by a table of which pairs
                // admit what: the table is [`Self::proposed`]'s, and a second
                // here would be a second chance to disagree with it. Where the
                // pointer is makes no difference to *whether* there is a
                // dimension, so the middle of the drawing does.
                placing.proposed(sketch, DVec2::ZERO).map(|_| placing)
            }
            // Already placing. A further click commits rather than picking, so
            // nothing here answers it.
            Dimensioning::Placing { .. } => None,
        }
    }

    /// What the next click would state, with its number at `at`.
    ///
    /// **One answer, read twice.** The preview is drawn from it and the click
    /// commits it, so what is shown and what is stated cannot come apart — which
    /// is the one bug a dimension tool is worth designing against, because it
    /// looks like working software right up until the number lands somewhere
    /// else.
    ///
    /// `None` before there is enough picked, and for a pair that admits no
    /// dimension — two circles, say, which are equal or nothing.
    pub(crate) fn proposed(self, sketch: &Sketch, at: DVec2) -> Option<Constraint> {
        let Dimensioning::Placing {
            first,
            second,
            along,
        } = self
        else {
            return None;
        };
        // Stated with no number and no placement, because both are the
        // drawing's: what it measures and where the frame it is placed in puts
        // that number are worked out by the sketch — see
        // [`Sketch::proposed`](silverpoint::Sketch).
        let bare = Dimension::new(0.0);
        let candidate = match (first, second) {
            (Entity::Circle(circle), None) => Constraint::Radius {
                circle,
                dimension: bare,
            },
            (Entity::Point(a), Some(Entity::Point(b))) => Constraint::Distance {
                a,
                b,
                along: along.or_else(|| {
                    reading([sketch.point(a).position, sketch.point(b).position], at)
                })?,
                dimension: bare,
            },
            (Entity::Point(point), Some(Entity::Segment(segment)))
            | (Entity::Segment(segment), Some(Entity::Point(point))) => Constraint::Standoff {
                point,
                segment,
                dimension: bare,
            },
            // Only where the two already run together, for the reason the bar
            // offers it only there: the gap between two crossing lines depends
            // on where along them it is measured, so there is nothing for one
            // number to hold.
            (Entity::Segment(first), Some(Entity::Segment(second)))
                if sketch.parallel(first, second) =>
            {
                Constraint::Spacing {
                    first,
                    second,
                    dimension: bare,
                }
            }
            _ => return None,
        };
        sketch.proposed(candidate, at)
    }

    /// Everything picked so far, so a prune can ask whether the drawing still
    /// holds it.
    ///
    /// An undo can take away geometry a half-finished gesture is hanging off,
    /// and a handle left over is not merely inert: the sketch is restored arenas
    /// and all, so the next entity created takes the very same handle and the
    /// tool would come back pointing at something nobody picked.
    pub(crate) fn picking(self) -> impl Iterator<Item = Entity> {
        let named = match self {
            Dimensioning::Empty => [None, None],
            Dimensioning::Picked(entity) => [Some(entity), None],
            Dimensioning::Placing { first, second, .. } => [Some(first), second],
        };
        named.into_iter().flatten()
    }
}

/// Which of the three readings the pointer is asking for, or `None` where a pair
/// measures nothing at all.
///
/// **Each reading pushes its dimension line a different way**, so what the
/// pointer has most moved *along* says which was meant: a horizontal one is
/// stood above or below the pair, a vertical one out to one side, and an aligned
/// one square to the span itself. Drag straight up from a leaning pair and the
/// horizontal reading wins; drag out perpendicular to it and the aligned one
/// does. That is what a modeller does and what a hand expects of it.
///
/// **A reading that measures nothing is not on offer.** A pair at one height
/// admits no vertical distance, and without this, dragging sideways from one
/// would pick a reading that states a zero — which is not a dimension and,
/// worse, previews nothing at all.
///
/// **Ties go to the axes.** A pair lying along one of them makes the aligned
/// reading the very same number, and preferring the axis is what stops the
/// answer flickering between two names for one measurement as the pointer
/// crosses.
///
/// A pure function over two places and a third, so the whole of the rule can be
/// asked without a drawing to ask it through.
fn reading(span: [DVec2; 2], at: DVec2) -> Option<Along> {
    let apart = span[1] - span[0];
    let out = at - span[0].midpoint(span[1]);
    // What each reading measures, and the way its line is pushed off the pair.
    // In this order, because a fold that keeps its incumbent through a tie is
    // what puts the axes first.
    [
        (Along::Horizontal, apart.x.abs(), DVec2::Y),
        (Along::Vertical, apart.y.abs(), DVec2::X),
        (Along::Shortest, apart.length(), apart.perp()),
    ]
    .into_iter()
    .filter(|&(_, measures, _)| measures > MEASURES_NOTHING)
    .fold(None, |best: Option<(Along, f64)>, (along, _, offset)| {
        // Unit, so the three scores are comparable: what is being weighed is how
        // far the pointer went *that way*, not how long the offset happens to
        // be. The aligned one is the only one that needs it, and it is only
        // reached where the pair has a length to normalize by.
        let score = out.dot(offset.normalize()).abs();
        match best {
            Some((_, had)) if had >= score => best,
            _ => Some((along, score)),
        }
    })
    .map(|(along, _)| along)
}

#[cfg(test)]
mod tests;
