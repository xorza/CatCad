//! The colour and spread every widened overlay record ends with.

use glam::Vec3;

/// What every overlay record the shader widens ends with, whatever shape
/// carries it: the colour it is drawn in, and how far the shader spreads it.
///
/// The two fields that mean the same thing for a stroke, a rim and a marker,
/// laid out once so they cannot drift.
///
/// A *look* in this crate is a [`Highlight`](crate::Highlight) — reached as
/// `Lit::look`, answered by `Highlights::look_of`, and laid over this by
/// [`Instance::highlighted`](super::Instance::highlighted). So this is named
/// for what it is rather than for what overwrites it: one word for both would
/// be one word for both sides of that call.
///
/// The plane a primitive lies in is *not* here, though three of the four carry
/// one. A stroke, a marker and a label are widened in screen space, so their
/// corners leave the plane and the shader has to put their depth back on it; a
/// ring's band is widened in its own plane and never leaves it. Sharing the
/// field would ship a ring twelve bytes it has no use for and name something
/// about it that is not true.
///
/// A label carries none of this, for the same reason: a glyph's size came from
/// its shaping, so it has no spread, and it ships its colour alone. Every
/// record ships only what its shader reads, and `ShaderInterface` holds each
/// one to that when its pipeline is built.
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
}
