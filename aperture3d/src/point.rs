//! A marker at a world position, drawn at a size the zoom cannot change.

use glam::Vec3;

/// Default marker diameter, in logical pixels.
const DEFAULT_SIZE: f32 = 8.0;

/// A position worth marking — a sketch vertex, a handle, a snap.
///
/// A disc facing the viewer, sized in *logical pixels* rather than world
/// units, which is what separates a marker from geometry. A marker says
/// "something is here"; how big it is says nothing about the model, so zooming
/// must not inflate it. Modelled geometry is the other way round and belongs
/// in a [`Mesh`](crate::Mesh) or a [`Curve`](crate::Curve).
///
/// Round because a disc is the one glyph with no orientation to get wrong: it
/// reads the same however the camera is turned, and its silhouette is its own
/// pick radius.
#[derive(Debug, Clone)]
pub struct Point {
    pub position: Vec3,
    /// Linear-RGB.
    pub color: Vec3,
    /// Diameter in logical pixels.
    pub size: f32,
    /// Depth-test bias in steps of depth-buffer resolution, positive toward
    /// the viewer. See [`Curve::z_offset`](crate::Curve::z_offset).
    pub z_offset: i32,
    /// What a pick that lands here reports. See
    /// [`Curve::tag`](crate::Curve::tag).
    pub tag: Option<u64>,
    /// The plane this marker sits on, as a unit normal, when it sits on one.
    ///
    /// A disc is flat in depth while the surface under it is not, so on a
    /// plane seen at an angle the surface rises through the glyph and the
    /// depth test eats whichever half it rises into — a marker sliced in two.
    /// Naming the plane lets the disc take the surface's own depth across its
    /// width, which is the same answer [`Curve::plane_normal`] gives a stroke.
    ///
    /// `None` for a marker floating free of any surface.
    pub plane_normal: Option<Vec3>,
}

impl Point {
    /// A white marker of default size at `position`.
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            color: Vec3::ONE,
            size: DEFAULT_SIZE,
            z_offset: 0,
            tag: None,
            plane_normal: None,
        }
    }

    /// Set the linear-RGB colour.
    pub fn colored(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    /// Set the diameter in logical pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Set the depth-test bias. See [`Point::z_offset`].
    pub fn z_offset(mut self, z_offset: i32) -> Self {
        self.z_offset = z_offset;
        self
    }

    /// Name this marker to whatever a pick will be reported to. See
    /// [`Point::tag`].
    pub fn tagged(mut self, tag: u64) -> Self {
        self.tag = Some(tag);
        self
    }

    /// Declare the plane the marker sits on. See [`Point::plane_normal`].
    pub fn in_plane(mut self, normal: Vec3) -> Self {
        self.plane_normal = Some(normal.normalize());
        self
    }
}
