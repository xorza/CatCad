//! Stroked polylines in world space.

use glam::Vec3;

/// Default stroke width, in logical pixels.
const DEFAULT_WIDTH: f32 = 1.5;

/// A polyline through world-space points, stroked at a constant width in
/// *logical pixels* rather than world units — a sketch edge stays legible
/// however far the camera pulls back, which is what tells drawn geometry
/// apart from modelled geometry.
///
/// Curves are unlit: they carry no normal, and their colour reaches the
/// target unshaded.
#[derive(Debug, Clone)]
pub struct Curve {
    /// Each neighbouring pair is one stroked segment. Fewer than two points
    /// draws nothing.
    pub points: Vec<Vec3>,
    /// Whether the last point joins back to the first. Ignored below three
    /// points, where the closing segment would double the only one there is.
    pub closed: bool,
    /// Linear-RGB.
    pub color: Vec3,
    /// Stroke width in logical pixels.
    pub width: f32,
}

impl Curve {
    /// An open white curve of default width through `points`.
    pub fn new(points: Vec<Vec3>) -> Self {
        Self {
            points,
            closed: false,
            color: Vec3::ONE,
            width: DEFAULT_WIDTH,
        }
    }

    /// A single straight stroke.
    pub fn segment(a: Vec3, b: Vec3) -> Self {
        Self::new(vec![a, b])
    }

    /// Join the last point back to the first.
    pub fn closed(mut self) -> Self {
        self.closed = true;
        self
    }

    /// Set the linear-RGB colour.
    pub fn colored(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    /// Set the stroke width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// How many segments this curve strokes.
    pub(crate) fn segment_count(&self) -> usize {
        let open = self.points.len().saturating_sub(1);
        if self.wraps() { open + 1 } else { open }
    }

    /// Each stroked segment as its two endpoints, the closing one last.
    pub(crate) fn segments(&self) -> impl Iterator<Item = (Vec3, Vec3)> {
        let wrap = self
            .wraps()
            .then(|| (self.points[self.points.len() - 1], self.points[0]));
        self.points
            .windows(2)
            .map(|pair| (pair[0], pair[1]))
            .chain(wrap)
    }

    fn wraps(&self) -> bool {
        self.closed && self.points.len() > 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_pair_up_neighbours_and_close_on_request() {
        let points = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
        let open = Curve::new(points.clone());
        assert_eq!(open.segment_count(), 2);
        assert_eq!(
            open.segments().collect::<Vec<_>>(),
            [(Vec3::ZERO, Vec3::X), (Vec3::X, Vec3::Y)]
        );

        // Closing adds the last-to-first segment, and nothing else moves.
        let closed = Curve::new(points).closed();
        assert_eq!(closed.segment_count(), 3);
        assert_eq!(
            closed.segments().last(),
            Some((Vec3::Y, Vec3::ZERO)),
            "the closing segment runs last so a stroke reads in order"
        );

        // Two points have only one segment between them either way: closing
        // would stroke it a second time, backwards.
        let pair = Curve::segment(Vec3::ZERO, Vec3::X).closed();
        assert_eq!(pair.segment_count(), 1);
        assert_eq!(pair.segments().count(), 1);

        // A lone point, and nothing at all, draw nothing.
        assert_eq!(Curve::new(vec![Vec3::ZERO]).segment_count(), 0);
        assert_eq!(Curve::new(Vec::new()).closed().segment_count(), 0);
        assert_eq!(Curve::new(Vec::new()).segments().count(), 0);
    }
}
