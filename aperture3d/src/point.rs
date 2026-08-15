//! A marker at a world position, drawn at a size the zoom cannot change.

use crate::aim::Aim;
use crate::hit::{Hit, HitAt, Precedence};
use crate::primitive::{Flatten, Primitive};
use crate::renderer::record::PointInstance;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::Vec3;

/// Default marker diameter, in logical pixels.
const DEFAULT_SIZE: f32 = 8.0;

/// A position worth marking — a sketch vertex, a handle, a snap.
///
/// A disc facing the viewer, sized in logical pixels — an
/// [overlay](crate#overlays). A marker says "something is here"; how big it is
/// says nothing about the model, so zooming must not inflate it. Anything
/// whose extent *is* the point belongs in a [`Mesh`](crate::Mesh) or a
/// [`Curve`](crate::Curve).
///
/// Round because a disc is the one glyph with no orientation to get wrong: it
/// reads the same however the camera is turned, and its silhouette is its own
/// pick radius.
///
/// `Default` draws nothing — its diameter is zero.
#[derive(Default, Debug, Clone)]
pub struct Point {
    pub position: Vec3,
    /// Linear-RGB.
    pub color: Vec3,
    /// Diameter in logical pixels.
    pub size: f32,
    /// What this is for, which decides what a click meant for two things at
    /// once lands on. See [`Precedence`].
    pub precedence: Precedence,
    /// What a pick that lands here reports. See [picking](crate#picking).
    pub tag: Option<Tag>,
    /// The plane this marker sits on, as a unit normal, when it sits on one.
    /// See [overlays](crate#overlays). `None` keeps the anchor's depth, flat
    /// across the disc.
    pub plane_normal: Option<Vec3>,
}

impl Point {
    /// A white marker of default size at `position`.
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            color: Vec3::ONE,
            size: DEFAULT_SIZE,
            precedence: Precedence::default(),
            tag: None,
            plane_normal: None,
        }
    }

    /// Whether the cursor landed on this marker, and where.
    ///
    /// The glyph takes its depth from the anchor, so the anchor clipping is
    /// the whole glyph clipping — there is no part of it drawn once that is
    /// gone.
    pub(crate) fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let screen = aim.reach_to(self.position)?;
        (screen <= aim.reach(self.size))
            .then(|| aim.hit(tag, HitAt::Point, self.precedence, self.position, screen))
    }
}

impl Point {
    /// Set the diameter in logical pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Declare the plane the marker sits on. See [`Point::plane_normal`].
    pub fn in_plane(mut self, normal: Vec3) -> Self {
        self.plane_normal = Some(normal.normalize());
        self
    }
}

impl Styled for Point {
    fn color_mut(&mut self) -> &mut Vec3 {
        &mut self.color
    }

    fn tag_mut(&mut self) -> &mut Option<Tag> {
        &mut self.tag
    }

    fn precedence_mut(&mut self) -> &mut Precedence {
        &mut self.precedence
    }
}

impl Flatten for Point {
    type Record = PointInstance;

    fn record_count(&self) -> usize {
        1
    }

    fn records(&self) -> impl Iterator<Item = Self::Record> {
        std::iter::once(PointInstance::of(self))
    }
}

impl Primitive for Point {
    fn tag(&self) -> Option<Tag> {
        self.tag
    }

    fn extend_bounds(&self, mut include: impl FnMut(Vec3)) {
        include(self.position);
    }
}
