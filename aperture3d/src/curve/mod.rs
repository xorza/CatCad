//! Stroked polylines in world space.

use crate::aim::{Aim, Inside};
use crate::hit::{Hit, HitAt, Precedence};
use crate::primitive::{DEFAULT_STROKE_WIDTH, Flatten, Primitive};
use crate::renderer::record::CurveInstance;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::Vec3;

/// Screen length below which a projected segment lands on a single pixel and
/// has no direction to project a cursor onto.
///
/// Stated here alone: `common.wgsl` declares it `override` and is handed this at
/// pipeline creation, so the length a stroke is widened from and the length a
/// pick projects onto cannot come apart. Picking works in squared distances to
/// keep a square root out of the walk, which is the only reason
/// [`MIN_RUN_PX2`] exists beside it.
pub(crate) const MIN_RUN_PX: f32 = 1e-3;

/// [`MIN_RUN_PX`] squared, which is what a squared screen length compares to.
const MIN_RUN_PX2: f32 = MIN_RUN_PX * MIN_RUN_PX;

/// Floor under the sum of reciprocal depths that undoes the perspective
/// squeeze. Only a segment with both ends astronomically far off gets near it.
const MIN_RECIP_W: f32 = 1e-6;

/// A polyline through world-space points, stroked at a constant width in
/// logical pixels — an [overlay](crate#overlays), so a sketch edge stays
/// legible however far the camera pulls back.
///
/// Curves are unlit: they carry no normal, and their colour reaches the
/// target unshaded.
///
/// `Default` draws nothing — it has no points.
#[derive(Default, Debug, Clone)]
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
    /// Depth-test bias in steps of depth-buffer resolution, positive toward
    /// the viewer. See [overlays](crate#overlays).
    pub z_offset: i32,
    /// The plane this curve lies in, as a unit normal, when it lies in one.
    /// See [overlays](crate#overlays). `None` keeps the centreline's depth.
    pub plane_normal: Option<Vec3>,
    /// What this is for, which decides what a click meant for two things at
    /// once lands on. See [`Precedence`].
    pub precedence: Precedence,
    /// What a pick that lands on this stroke reports. See
    /// [picking](crate#picking).
    pub tag: Option<Tag>,
}

impl Curve {
    /// An open white curve of default width through `points`.
    ///
    /// The general form, and published as such alongside [`Curve::closed`] even
    /// where an application draws nothing but two-point strokes: a polyline that
    /// can join its ends is what this type says it is, and a shape nothing
    /// outside the crate can construct is not a published one. Both are one
    /// caller away from earning their keep, and neither costs anything until
    /// then.
    pub fn new(points: Vec<Vec3>) -> Self {
        Self {
            points,
            closed: false,
            color: Vec3::ONE,
            width: DEFAULT_STROKE_WIDTH,
            z_offset: 0,
            plane_normal: None,
            precedence: Precedence::default(),
            tag: None,
        }
    }

    /// A single straight stroke.
    pub fn segment(a: Vec3, b: Vec3) -> Self {
        Self::new(vec![a, b])
    }

    /// Make an existing curve the stroke [`Curve::segment`] would have built,
    /// keeping the room its points already have.
    ///
    /// The reason a caller redrawing every frame holds on to the curves it
    /// drew last time. A `Curve` owns its points on the heap, so a list of
    /// them emptied and refilled frees a vector per stroke and asks straight
    /// back for one the same size — every frame a drag lasts.
    ///
    /// Geometry only. Colour, width, bias, plane and tag are left where they
    /// were, because a caller rewriting one of these is redrawing the same
    /// edge somewhere new rather than replacing it with a different one.
    pub fn set_segment(&mut self, a: Vec3, b: Vec3) {
        self.points.clear();
        self.points.push(a);
        self.points.push(b);
        self.closed = false;
    }

    /// Whether the cursor landed on this stroke, and on which segment.
    ///
    /// The nearest segment wins outright rather than every segment answering:
    /// a polyline is one thing to grab, and a corner where two segments meet
    /// would otherwise report itself twice.
    pub(crate) fn pick(&self, aim: &Aim) -> Option<Hit> {
        let tag = self.tag?;
        let reach = aim.reach(self.width);
        let mut best: Option<Hit> = None;
        for (index, (a, b)) in self.segments().enumerate() {
            let Some(near) = nearest_on_segment(a, b, aim) else {
                continue;
            };
            if near.screen > reach || best.is_some_and(|best| best.screen <= near.screen) {
                continue;
            }
            let at = HitAt::Segment { index, t: near.t };
            best = Some(aim.hit(tag, at, self.precedence, a.lerp(b, near.t), near.screen));
        }
        best
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

impl Curve {
    /// Join the last point back to the first.
    pub fn closed(mut self) -> Self {
        self.closed = true;
        self
    }

    /// Say what this is for, which is what decides a click that lands on two
    /// things at once. See [`Precedence`].
    pub fn precede(mut self, precedence: Precedence) -> Self {
        self.precedence = precedence;
        self
    }

    /// Set the stroke width in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Bias the stroke this many steps of depth-buffer resolution toward the
    /// viewer. See [overlays](crate#overlays).
    pub fn z_offset(mut self, z_offset: i32) -> Self {
        self.z_offset = z_offset;
        self
    }

    /// Declare the plane the curve lies in. See [`Curve::plane_normal`].
    pub fn in_plane(mut self, normal: Vec3) -> Self {
        self.plane_normal = Some(normal.normalize());
        self
    }
}

impl Styled for Curve {
    fn color_mut(&mut self) -> &mut Vec3 {
        &mut self.color
    }

    fn tag_mut(&mut self) -> &mut Option<Tag> {
        &mut self.tag
    }
}

/// Where a cursor came closest to one segment.
#[derive(Debug, Clone, Copy)]
struct Nearest {
    /// How far along the segment, in world terms.
    t: f32,
    /// How far the cursor was from it on screen.
    screen: f32,
}

/// The stretch of a segment left after clipping, as fractions of the whole.
#[derive(Debug, Clone, Copy)]
struct Span {
    start: f32,
    end: f32,
}

impl Span {
    fn whole() -> Self {
        Self {
            start: 0.0,
            end: 1.0,
        }
    }

    /// Trim to where a quantity that is affine along the segment — given by
    /// its value at each end — is non-negative. `None` once nothing is left.
    ///
    /// Clip space is affine in the world parameter, so a crossing sits at the
    /// same fraction of the world segment as of the clip one and the surviving
    /// stretch can be picked on as itself.
    fn clip(self, at_start: f32, at_end: f32) -> Option<Self> {
        let Self { start, end } = match (at_start >= 0.0, at_end >= 0.0) {
            (true, true) => self,
            (false, false) => return None,
            (true, false) => Self {
                end: self.end.min(at_start / (at_start - at_end)),
                ..self
            },
            (false, true) => Self {
                start: self.start.max(at_start / (at_start - at_end)),
                ..self
            },
        };
        (start <= end).then_some(Self { start, end })
    }
}

/// The point of segment `a`–`b` nearest `cursor` on screen, or `None` if none
/// of it is drawn.
fn nearest_on_segment(a: Vec3, b: Vec3, aim: &Aim) -> Option<Nearest> {
    let (a_clip, b_clip) = (aim.view_proj * a.extend(1.0), aim.view_proj * b.extend(1.0));
    let (a_in, b_in) = (Inside::of(a_clip), Inside::of(b_clip));
    let span = Span::whole()
        .clip(a_in.near, b_in.near)?
        .clip(a_in.far, b_in.far)?;
    // Inside the near plane `w` is at least `z_near` under perspective and
    // exactly 1 under parallel rays, so what survived can be divided by it.
    let near = a_clip.lerp(b_clip, span.start);
    let far = a_clip.lerp(b_clip, span.end);

    let (from, to) = (
        aim.viewport.pixel_from_clip(near),
        aim.viewport.pixel_from_clip(far),
    );
    let run = to - from;
    let length = run.length_squared();
    // A segment that lands on one pixel has no direction to project onto, and
    // either end answers the same.
    let on_screen = if length > MIN_RUN_PX2 {
        ((aim.cursor - from).dot(run) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let screen = aim.cursor.distance(from + run * on_screen);

    // Screen distance runs evenly along the *projected* segment, and under
    // perspective that is not evenly along the world one — the far half of a
    // receding edge is squeezed into fewer pixels. Undoing that is what makes
    // the returned point land where the cursor actually is rather than short
    // of it, which is the difference between snapping to a midpoint and
    // snapping near one.
    let recip = (1.0 - on_screen) / near.w + on_screen / far.w;
    let in_span = if recip > MIN_RECIP_W {
        (on_screen / far.w) / recip
    } else {
        on_screen
    };
    Some(Nearest {
        t: span.start + in_span * (span.end - span.start),
        screen,
    })
}

impl Primitive for Curve {
    fn tag(&self) -> Option<Tag> {
        self.tag
    }

    fn extend_bounds(&self, mut include: impl FnMut(Vec3)) {
        for point in &self.points {
            include(*point);
        }
    }
}

impl Flatten for Curve {
    type Record = CurveInstance;

    /// One per segment: the shader takes a ribbon's direction from the
    /// difference between its two ends, so both travel.
    fn record_count(&self) -> usize {
        self.segment_count()
    }

    fn records(&self) -> impl Iterator<Item = Self::Record> {
        CurveInstance::of(self)
    }
}

#[cfg(test)]
mod tests;
