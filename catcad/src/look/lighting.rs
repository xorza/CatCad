//! What singling something out looks like.

use aperture::{Highlight, Tint};
use glam::Vec3;

use crate::part::Part;

/// How the thing under the cursor and the things picked out are drawn.
///
/// A colour and a size apiece, rather than the [`Highlight`] values themselves:
/// a highlight is aperture's shape and these are the two decisions that fill it
/// in. Named for the lighting rather than for what is lit, which is aperture's
/// own [`Lit`](aperture::Lit) — one primitive wearing one of these.
#[derive(Debug, Clone)]
pub(crate) struct Lighting {
    /// What the thing under the cursor is lit in.
    pub(crate) hovered: Vec3,
    /// What something picked out is lit in.
    ///
    /// Green, which is the one hue the drawing does not already use: its own
    /// colours run blue through yellow to orange for how much freedom is left,
    /// and red for pinned, and a selection that reused any of them would be
    /// saying two things in one colour.
    pub(crate) selected: Vec3,
    /// How much larger each reads than the thing it is pointing at.
    ///
    /// The hover is the bigger of the two and drawn over the selection, so the
    /// thing under the cursor still reads over the rest of what is picked.
    /// Anything under 1 would hide behind what it is meant to point at.
    pub(crate) hover_scale: f32,
    pub(crate) select_scale: f32,
    /// How much brighter a step reads when it is singled out.
    ///
    /// Brighter rather than recoloured, because a plane's square is *saying
    /// something* in its colour — which of the three the world comes with it is
    /// — and the two looks above exist to override exactly that. Lighting it
    /// yellow would light up a square that had stopped saying which plane it
    /// was.
    pub(crate) step_lift: f32,
}

impl Lighting {
    /// How `part` reads when it has been singled out, and whether it is the one
    /// under the cursor.
    ///
    /// One place rather than two matches at the call, so that a kind whose
    /// highlight is its own cannot be given the general look by whichever of the
    /// two callers was written second.
    pub(crate) fn of(&self, part: Part, hovered: bool) -> Highlight {
        match part {
            // A step is a place to work rather than a thing to gather, so there
            // is no state to tell apart: what a hover means here is only "this
            // is what pressing would take". Unscaled by the constructor, because
            // a shape that keeps its own colour is one already saying what it
            // is, and growing it would move the control out from under the
            // cursor pointing at it.
            Part::Step(_) => Highlight::lifted(self.step_lift),
            _ if hovered => Highlight {
                tint: Tint::Ink(self.hovered),
                scale: self.hover_scale,
            },
            _ => Highlight {
                tint: Tint::Ink(self.selected),
                scale: self.select_scale,
            },
        }
    }

    /// The one preset.
    const DARK: Self = Self {
        hovered: Vec3::new(1.0, 0.85, 0.25),
        selected: Vec3::new(0.30, 0.95, 0.45),
        hover_scale: 1.8,
        select_scale: 1.5,
        step_lift: 1.9,
    };
}

impl Default for Lighting {
    fn default() -> Self {
        Self::DARK
    }
}
