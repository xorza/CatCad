//! What every drawable primitive carries, and the setters that follow from it.

use crate::precedence::Precedence;
use crate::tag::Tag;
use glam::Vec3;

/// The three attributes every primitive has: the colour it is drawn in, the
/// [`Tag`] a pick that lands on it reports, and the [`Precedence`] that decides
/// what a click meant for two of them at once lands on.
///
/// Three, and there is no fourth waiting. Depth bias was the obvious candidate
/// and is now nobody's to set: how far forward a kind reads is a property of
/// the order this crate draws in, so it is pipeline state and lives beside that
/// order rather than on the primitives.
///
/// The setters live here rather than being restated on each primitive, so that
/// "colour it", "name it" and "say what it is for" mean one thing across the
/// crate and cannot drift apart. A primitive supplies only the accessors they
/// reach through; its fields stay its own, and stay public.
pub trait Styled: Sized {
    /// The linear-RGB colour the primitive is drawn in.
    fn color_mut(&mut self) -> &mut Vec3;

    /// What a pick that lands on the primitive reports. `None` is scenery.
    fn tag_mut(&mut self) -> &mut Option<Tag>;

    /// What the primitive is for, as picking weighs it.
    fn precedence_mut(&mut self) -> &mut Precedence;

    /// Set the linear-RGB colour.
    fn colored(mut self, color: Vec3) -> Self {
        *self.color_mut() = color;
        self
    }

    /// Name this primitive to whatever a pick will be reported to.
    fn tagged(mut self, tag: Tag) -> Self {
        *self.tag_mut() = Some(tag);
        self
    }

    /// Say what this is for, which is what decides a click that lands on two
    /// things at once. See [`Precedence`].
    fn precedence(mut self, precedence: Precedence) -> Self {
        *self.precedence_mut() = precedence;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::mesh::Mesh;
    use crate::object::Object;
    use crate::point::Point;
    use crate::ring::Ring;
    use crate::text::Text;

    /// Colouring a primitive reaches its colour, and nothing else.
    ///
    /// The three accessors this trait is reached through are one line apiece on
    /// five types, and two of the three cannot be got wrong: each primitive
    /// carries exactly one `Option<Tag>` and one [`Precedence`], so a `tag_mut`
    /// or a `precedence_mut` that named the wrong field would not compile.
    ///
    /// `color_mut` is the one that would. A [`Ring`] carries four `Vec3`s — a
    /// centre, two axes and a colour — and a [`Point`] and a [`Text`] two
    /// apiece, so an accessor answering with the wrong one type-checks, and
    /// colouring a rim would move it or turn it instead. That is the whole of
    /// what five hand-written impls can get wrong, so it is what is asked here.
    #[test]
    fn colouring_a_primitive_reaches_its_colour_and_nothing_else() {
        let ink = Vec3::new(0.25, 0.5, 0.75);

        // One `Vec3` apiece, so these two are here to say the trait is wired up
        // rather than to catch a field it could have named instead.
        assert_eq!(Object::new(Mesh::cube(2.0)).colored(ink).color, ink);
        let curve = Curve::segment(Vec3::ZERO, Vec3::Y).colored(ink);
        assert_eq!(curve.color, ink);
        assert_eq!(curve.points, [Vec3::ZERO, Vec3::Y]);

        let point = Point::new(Vec3::X).colored(ink);
        assert_eq!(point.color, ink);
        assert_eq!(point.position, Vec3::X, "colouring a marker moved it");

        let text = Text::new(Vec3::X, "125.4", 12.0).colored(ink);
        assert_eq!(text.color, ink);
        assert_eq!(text.position, Vec3::X, "colouring a label moved it");

        // The one with room to go wrong three ways over.
        let rim = Ring::new(Vec3::Z, 2.0, Vec3::Y);
        let (centre, axes) = (rim.center, (rim.x_axis, rim.y_axis));
        assert_ne!(
            axes.0,
            Vec3::ZERO,
            "the fixture has no plane to be turned in"
        );
        let rim = rim.colored(ink);
        assert_eq!(rim.color, ink);
        assert_eq!(rim.center, centre, "colouring a rim moved it");
        assert_eq!((rim.x_axis, rim.y_axis), axes, "colouring a rim turned it");
    }
}
