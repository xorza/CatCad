//! The box a mesh fills, and what a ray makes of it.

use glam::Vec3;

/// The axis-aligned box a mesh's vertices fill, in the mesh's own space.
///
/// **Carried rather than measured, which is the whole of why it is a type.** A
/// box is what a pick asks first, to find out whether the triangles are worth
/// walking — and working it out costs a walk of every vertex, which is the same
/// order as the walk it was meant to save. Answered from a mesh that already
/// knows, it costs six floats.
///
/// Object space, so it survives the transform moving: a mesh that is spun or
/// slid is the same mesh, and a pick puts its ray into that space to ask — see
/// [`Object::crosses`](crate::Object).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub low: Vec3,
    pub high: Vec3,
}

impl Default for Bounds {
    /// The identity [`Bounds::of`] folds from: low above high, so the first
    /// point it meets replaces both ends.
    ///
    /// Inverted rather than zero-sized, which would be a box at the origin and
    /// so a claim to be there. What it is *not* is a box every ray misses —
    /// against a reciprocal direction the infinities cancel and every slab comes
    /// out spanning everything, so a mesh with no vertices admits the ray and is
    /// then refused for having no triangles. Which is where it was refused
    /// before any of this was kept, the walk having started from these same two
    /// values.
    fn default() -> Self {
        Self {
            low: Vec3::INFINITY,
            high: Vec3::NEG_INFINITY,
        }
    }
}

impl Bounds {
    /// The smallest box holding every one of `positions`.
    pub fn of(positions: impl IntoIterator<Item = Vec3>) -> Self {
        positions
            .into_iter()
            .fold(Self::default(), |grown, at| Self {
                low: grown.low.min(at),
                high: grown.high.max(at),
            })
    }

    /// Whether the ray from `origin` along `direction` passes through.
    ///
    /// The slab test: how far along the ray each pair of planes is crossed,
    /// keeping the last entry and the first exit. Behind the eye is a miss,
    /// which is the `0` in the comparison.
    ///
    /// A direction with a zero component divides to an infinity, which orders
    /// correctly — and to a `NaN` on an axis the mesh is *flat* on and the
    /// origin lies exactly in, which is a ray running along a sketch face's own
    /// plane. That axis then knows nothing and the other two have to decide, so
    /// the two folds are written out with [`f32::max`] and [`f32::min`], which
    /// drop a `NaN` in favour of the other operand. `Vec3::max_element` and its
    /// twin do not: they carry it out, and what comes back then refuses every
    /// comparison it is put to — a miss on a ray that crosses the box.
    pub fn crossed(self, origin: Vec3, direction: Vec3) -> bool {
        let recip = direction.recip();
        let (entry, exit) = ((self.low - origin) * recip, (self.high - origin) * recip);
        let (near, far) = (entry.min(exit), entry.max(exit));
        let enter = near.x.max(near.y).max(near.z);
        let leave = far.x.min(far.y).min(far.z);
        leave >= enter.max(0.0)
    }
}
