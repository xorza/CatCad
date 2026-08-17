//! What a pick found.

use crate::tag::Tag;
use glam::Vec3;
use std::cmp::Ordering;

/// Where on a primitive a pick landed.
///
/// A marker has no interior, so there is nothing to say beyond that it was
/// hit, and a label is the same — what a caller does with one is about the
/// whole run. A stroke does have an interior: which of its segments, and how
/// far along.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitAt {
    /// A flat control, anywhere on it. Like a surface, it has no part of itself
    /// nearer the aim than the rest — but unlike one it is a target rather than
    /// a backdrop, which is what puts it at the far end of this ladder from
    /// [`HitAt::Surface`].
    Gizmo,
    Point,
    /// A run of text, anywhere within the box it is drawn in.
    Text,
    Segment {
        /// Index into the curve's segments, counting the closing one last.
        index: usize,
        /// Where along that segment, from 0 at its start to 1 at its end.
        t: f32,
    },
    Ring {
        /// Radians round from the ring's own `x_axis`.
        angle: f32,
    },
    /// Anywhere on a mesh — the whole of it is one target, because a surface
    /// has no part of itself that is nearer the aim than the rest.
    Surface,
}

impl HitAt {
    /// How specific this kind of hit is, lowest first.
    ///
    /// A vertex under the cursor beats an edge through it even when the edge
    /// is nearer the eye, because the smaller thing is the harder one to aim
    /// at and so the one the aim was meant for. Sorting on depth alone would
    /// make the corner of a rectangle unselectable — every corner has two
    /// edges running through it.
    ///
    /// A label sits between the two. It is a larger target than a marker, so a
    /// marker drawn over one still wins; but it is an opaque thing deliberately
    /// placed, and the edge it labels usually runs right under it — a dimension
    /// sits on its own dimension line — so an edge crossing a label must not
    /// take the click meant for the label.
    ///
    /// A gizmo beats a marker, and everything else with it. It is the one thing
    /// in a scene put there to be *taken hold of*: it is drawn over the geometry
    /// it controls, deliberately, so a click inside it can have been meant for
    /// nothing under it.
    pub(crate) fn rank(&self) -> u8 {
        match self {
            Self::Gizmo => 0,
            Self::Point => 1,
            Self::Text => 2,
            // An edge is an edge however it curves, so a stroke and a rim rank
            // together and the cursor's distance decides between them.
            Self::Segment { .. } | Self::Ring { .. } => 3,
            // Last, and never actually reached: a surface is ranked against
            // other surfaces and nothing else — see [`Hit::aim_order`] — so
            // every comparison this arm could enter is one the arms above have
            // already tied. It is here to keep the ladder total.
            Self::Surface => 4,
        }
    }
}

/// How a primitive stands in the competition for a click, foremost first.
///
/// How specific a [`HitAt`] is answers what *kind of shape* was hit, which is
/// true of any drawing: a marker is a harder thing to aim at than an edge,
/// wherever either was drawn. This answers what the thing is *for*, which only whoever drew it
/// knows — a frame around a drawing and an edge of one are the same shape, and
/// no amount of care about shape tells them apart.
///
/// A category rather than a number of steps. There is no continuum here to
/// measure along, the way there is for the depth bias an overlay carries: what
/// a primitive is for is one of a few things, and a number would only be
/// readable against the ranking above — which is this crate's own business and
/// no caller's.
///
/// The order below is the order they compete in, and the derive is what makes
/// that true: nothing adds, subtracts or compares against a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Precedence {
    /// Ranked by shape alone, which is the usual thing and what everything is
    /// unless it says otherwise.
    #[default]
    Shaped,
    /// Behind whatever is being worked on. Drawn to be seen and read rather
    /// than aimed at, so it yields a click to anything ordinary under the
    /// cursor.
    Aside,
    /// Behind everything: furniture around a drawing rather than part of one.
    ///
    /// Behind every drawn thing, that is — not behind the surfaces they are
    /// drawn on. A surface is never ranked against a drawn thing at all, so
    /// this says nothing about one: see [`Hit::aim_order`].
    Frame,
}

/// One primitive the cursor was near enough to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// The [`Tag`] the primitive was named with. Untagged primitives are
    /// scenery and never appear here.
    pub tag: Tag,
    pub at: HitAt,
    /// What the primitive said it was for, which outranks what shape it is.
    pub precedence: Precedence,
    /// **Where on the primitive the cursor is**, in world space — the point of
    /// the stroke nearest it, the triangle the ray went through, the place on a
    /// run's own surface it fell.
    ///
    /// The cursor's place and not the primitive's, which for anything with area
    /// are different points and for a marker are the same one. Two things read
    /// this and both want the former: the depth an overlay is ordered and
    /// occluded by, and whatever a caller does with the place it found. A run
    /// answering from the middle of its box passed every test the crate had and
    /// broke both at once — a label lies flat, so the face it is drawn on is
    /// nearer than its middle over half its area, and the lower half of every
    /// number read as being behind the sheet under it.
    ///
    /// Stated as a bound against [`Hit::screen`], which is what makes it one
    /// rule over five kinds rather than five readings:
    ///
    /// ```text
    /// |screen_of(hit.world) − cursor| ≤ hit.screen
    /// ```
    ///
    /// Four kinds meet it with equality. A run of text is the one that can sit
    /// strictly inside it: with the cursor outside its box but within reach,
    /// `screen` is the gap to the box's edge while this is still the point of
    /// the run's surface under the cursor.
    pub world: Vec3,
    /// How far that landed from the cursor on screen, in the units the pick
    /// was asked in.
    pub screen: f32,
    /// How far along the cursor's ray, in world units. Comparable across
    /// primitives and across projections, which is what makes it a usable
    /// tiebreak.
    pub distance: f32,
}

impl Hit {
    /// Which of two hits the aim was more likely meant for, lowest first.
    ///
    /// What the primitive is for, then how specific the hit is, then how near
    /// the cursor it fell, then how near the eye. The first two are the ones
    /// that are not measurements.
    ///
    /// The first is the caller's and the second this crate's, which is why it
    /// sits where it does. What a thing is *for* is known only to whoever drew
    /// it, so it decides before what shape it is: a frame the caller set aside
    /// loses to geometry however sharp a target it makes.
    ///
    /// **Never asked across the two kinds of thing a scene holds.**
    /// [`Scene::nearest`](crate::Scene::nearest) ranks the overlays among
    /// themselves and the surfaces among themselves, and settles the two against
    /// each other by structure rather than by ordering — a surviving overlay
    /// always wins, because a surface is the one target with no size of its own
    /// and one allowed to beat what stands in front of it would swallow every
    /// click meant for its own boundary. So nothing here has to say which of a
    /// face and an edge is wanted, and nothing here does.
    pub(crate) fn aim_order(&self, other: &Self) -> Ordering {
        self.precedence
            .cmp(&other.precedence)
            .then(self.at.rank().cmp(&other.at.rank()))
            .then(self.screen.total_cmp(&other.screen))
            .then(self.distance.total_cmp(&other.distance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hit at `screen` pixels from the cursor, of a kind and a standing.
    fn hit(precedence: Precedence, at: HitAt, screen: f32) -> Hit {
        Hit {
            tag: Tag::new(0),
            at,
            precedence,
            world: Vec3::ZERO,
            screen,
            distance: 0.0,
        }
    }

    /// What a hit is *for* decides before what shape it is, and shape decides
    /// before how near it fell.
    ///
    /// The order of the ones that are not measurements is the whole of what this
    /// pins. A frame set aside has to lose to geometry however sharp a target it
    /// makes and however exactly the cursor is on it — which is the one thing
    /// ranking by shape alone cannot say, because a frame and an edge are the
    /// same shape.
    #[test]
    fn what_a_hit_is_for_decides_before_what_shape_it_is() {
        let edge = hit(Precedence::Shaped, HitAt::Segment { index: 0, t: 0.5 }, 5.0);
        // Dead under the cursor, and the sharpest kind of target there is.
        let frame = hit(Precedence::Frame, HitAt::Point, 0.0);
        assert_eq!(edge.aim_order(&frame), Ordering::Less);

        // Within one standing, shape decides as it always did — a marker beats
        // an edge running through it even from further off.
        let marker = hit(Precedence::Shaped, HitAt::Point, 5.0);
        let nearer = hit(Precedence::Shaped, HitAt::Segment { index: 0, t: 0.5 }, 1.0);
        assert_eq!(marker.aim_order(&nearer), Ordering::Less);

        // And within one shape, distance decides.
        let far = hit(Precedence::Aside, HitAt::Point, 4.0);
        let near = hit(Precedence::Aside, HitAt::Point, 1.0);
        assert_eq!(near.aim_order(&far), Ordering::Less);

        // A sketch set aside loses to the one being worked in, whatever the two
        // are made of.
        assert_eq!(edge.aim_order(&near), Ordering::Less);
    }

    /// A gizmo beats a marker beats a label beats an edge, whatever their depths.
    ///
    /// The whole ladder in one place, because what it says is a chain of
    /// *comparisons* and a rung is only ever wrong relative to its neighbours. The
    /// middle one is what a dimension needs — a dimension sits on its own line, so
    /// an edge running under a label must not take the click meant for the label —
    /// and the top one is what a control needs, which is the opposite claim: a
    /// gizmo is drawn over the geometry it controls deliberately, so a click inside
    /// it cannot have been meant for what it covers.
    #[test]
    fn a_gizmo_outranks_a_marker_outranks_a_label_outranks_an_edge() {
        let gizmo = HitAt::Gizmo.rank();
        let marker = HitAt::Point.rank();
        let label = HitAt::Text.rank();
        let edge = HitAt::Segment { index: 0, t: 0.5 }.rank();
        let rim = HitAt::Ring { angle: 0.0 }.rank();
        let surface = HitAt::Surface.rank();

        assert!(gizmo < marker, "a gizmo should beat a marker");
        assert!(marker < label, "a marker should beat a label");
        assert!(label < edge, "a label should beat an edge");
        assert_eq!(edge, rim, "an edge is an edge however it curves");
        // The two ends are the same shape asked about — a mesh with no part of
        // itself nearer the aim than the rest — and they sit at opposite ends,
        // which is the whole of what makes a control a control and a face a
        // backdrop.
        assert!(edge < surface, "a backdrop should lose to everything drawn");
    }

    /// Two surfaces rank by what they are for, like anything else.
    ///
    /// The whole of what this ordering has to say about surfaces, because a
    /// surface is only ever ranked against another one — see
    /// [`Hit::aim_order`]. That a face loses to what is drawn *on* it is not
    /// said here at all: it is the shape of
    /// [`Scene::nearest`](crate::Scene::nearest), which takes the least of the
    /// overlays and falls through to the ground only when none survived. The
    /// assertion that used to stand here asked this ordering to answer for it,
    /// and answered for a comparison the pick never makes.
    #[test]
    fn two_surfaces_rank_by_what_they_are_for() {
        let face = hit(Precedence::Shaped, HitAt::Surface, 0.0);
        let dormant = hit(Precedence::Aside, HitAt::Surface, 0.0);
        assert_eq!(face.aim_order(&dormant), Ordering::Less);
    }
}
