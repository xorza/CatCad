//! The colour and spread every overlay record ends with.

use crate::highlight::Highlight;
use glam::Vec3;

/// What every overlay record ends with, whatever shape carries it: the colour
/// it is drawn in, and how far the shader spreads it.
///
/// The two fields that mean the same thing for a stroke, a rim and a marker,
/// laid out once so they cannot drift.
///
/// A *look* in this crate is a [`Highlight`] — reached as `Lit::look`, answered
/// by `Highlights::look_of`, and laid over this by [`Paint::take_on`]. So this
/// is named for what it is rather than for what overwrites it: one word for
/// both would be one word for both sides of that call.
///
/// The plane a primitive lies in is *not* here, though three of the four carry
/// one. A stroke, a marker and a label are widened in screen space, so their
/// corners leave the plane and the shader has to put their depth back on it; a
/// ring's band is widened in its own plane and never leaves it. Sharing the
/// field would ship a ring twelve bytes it has no use for and name something
/// about it that is not true.
///
/// [`Paint::spread`] is here on the opposite reasoning, and the two are worth
/// telling apart. A label has no use for it either — a glyph's size came from
/// its shaping — so it ships four dead bytes in a ninety-six byte record, and
/// `text_vs` does not even declare the attribute. What buys them is that
/// [`Instance::highlighted`] applies a highlight's `scale` to this field for
/// every kind alike; pulling it out would put a per-kind hook in the one
/// operation that is currently written once, to save four per cent of one
/// record. The ring's twelve bytes were not worth that trade and the label's
/// four are.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Paint {
    pub(crate) color: [f32; 3],
    /// Half the stroke width, or half a marker's diameter: the distance the
    /// shader spreads either side of the shape's own centre.
    pub(crate) spread: f32,
}

impl Paint {
    /// The paint a primitive drawn `across` wide is given.
    pub(super) fn of(color: Vec3, across: f32) -> Self {
        Self {
            color: color.to_array(),
            spread: across * 0.5,
        }
    }

    /// Take on a highlight's look, in place of the paint that was here.
    ///
    /// Named for what it does to a `Paint` rather than sharing
    /// [`Instance::highlighted`]'s name: that one answers with a whole record,
    /// this one edits the tail of one, and two things called `highlighted` on
    /// either side of a `paint_mut()` read as the same operation twice.
    pub(super) fn take_on(&mut self, look: Highlight) {
        self.color = look.tint.over(Vec3::from_array(self.color)).to_array();
        self.spread *= look.scale;
    }
}
