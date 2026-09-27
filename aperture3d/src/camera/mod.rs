//! Where the scene is viewed from, and the matrix that follows from it.

pub(crate) mod view_proj;

use crate::camera::view_proj::ViewProj;
use crate::extent::Extent;
use crate::ray::Ray;
use crate::viewport::Viewport;
use glam::camera::rh::proj::directx;
use glam::{Mat3, Mat4, Vec2, Vec3};

/// Pitch never reaches the pole, where yaw stops naming a direction: the
/// screen's right is level by construction, and straight up has no level
/// square to it.
const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 1e-3;

/// Distance floor. All it has to do is keep the near plane a positive, normal
/// `f32` — the plane rides with the distance, and nothing else depends on how
/// close the eye may come. Not even which way the eye looks: that is read off
/// the angles, so an eye that rounds onto a far-off target still looks along
/// them.
const MIN_DISTANCE: f32 = 1e-3;

/// Distance ceiling, 2²⁹ — about 5.4 × 10⁸ world units.
///
/// There has to be one because an infinite distance is a fixed point of
/// scaling: once the eye is there, no factor a gesture hands over brings it
/// back. Where it sits is where the arithmetic gives out, not where the view
/// stops being useful, and the orthographic view gives out first. Its
/// determinant is `1 / (128 · aspect · tan²(fov_y / 2) · distance³)`, picking
/// inverts that matrix, and once the determinant falls below the smallest
/// normal `f32` the inverse — and every ray read out of it — is infinite. At
/// the widest field [`Camera::sane`] allows (179°) across a 32768-by-1
/// viewport, the widest texture desktop GPUs allow, that happens just past 2³⁰.
/// One power of two inside it leaves room for a target twice as wide.
/// Perspective holds to 2⁴².
const MAX_DISTANCE: f32 = (1u32 << 29) as f32;

/// How near the eye the near plane may be put, and how near the target, as a
/// fraction of the orbit distance.
///
/// Open at both ends, because both are real failures rather than tight spots: at
/// zero the plane lands on the eye and the projection has nothing to divide by,
/// at one it lands on what is being looked at and the whole scene is clipped
/// away. A millionth off each end is far outside anything a camera is given.
const MIN_NEAR_RATIO: f32 = 1e-6;
const MAX_NEAR_RATIO: f32 = 1.0 - 1e-6;

/// How far, in orbit distances, the orthographic depth slab reaches either side
/// of the eye.
///
/// Parallel rays have no perspective divide to play float precision against, so
/// there is no infinite far plane to be had here: the slab ends somewhere, and
/// every unit of it is paid for in resolution. Sizing it by the orbit distance
/// makes that a fixed *relative* cost — the same bargain the near ratio strikes
/// — so zoom tightens the slab rather than leaving the range spent on space no
/// longer being looked at. Framing puts a whole scene within two orbit
/// distances of the eye, so this leaves room to orbit and dolly around one
/// before anything clips, at about `distance × 2⁻¹⁷` of resolution.
///
/// It reaches as far behind the eye as in front, which is what makes
/// orthographic zoom a pure rescale: with no vanishing point there is nothing
/// to justify clipping what the eye has passed, and clipping it would slice the
/// model open as you dolly in.
const ORTHO_SLAB: f32 = 64.0;

/// How the view volume flattens onto the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Projection {
    /// Foreshortening with depth, the way an eye or a lens sees.
    #[default]
    Perspective,
    /// Parallel rays: equal lengths measure equal on screen wherever they
    /// sit, and parallel edges stay parallel. What makes a view scalable.
    Orthographic,
}

impl Projection {
    /// The other one.
    pub fn toggled(self) -> Self {
        match self {
            Self::Perspective => Self::Orthographic,
            Self::Orthographic => Self::Perspective,
        }
    }
}

/// A right-handed, Y-up orbit camera: the eye is derived from a target point,
/// a distance, and two angles, so every gesture is a change to one scalar.
///
/// Yaw turns around the world Y axis and is zero when the eye sits on +Z;
/// pitch lifts the eye toward +Y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Whether the view foreshortens.
    pub projection: Projection,
    /// The point the eye looks at and orbits around.
    pub target: Vec3,
    /// Eye-to-target distance in world units.
    pub distance: f32,
    /// Rotation around the world Y axis, in radians.
    pub yaw: f32,
    /// Elevation above the XZ plane, in radians.
    pub pitch: f32,
    /// Vertical field of view, in radians.
    ///
    /// Parallel rays subtend nothing, so the orthographic view has no field of
    /// view of its own and reads this one as the extent it spans at the orbit
    /// distance instead. Switching projections then leaves whatever is being
    /// looked at exactly the size it was, and changes only the foreshortening
    /// around it.
    pub fov_y: f32,
    /// Near clip distance, as a fraction of the orbit distance. Perspective
    /// only.
    ///
    /// A ratio rather than a distance because an absolute near plane is a
    /// second number that has to stay in step with how close the eye may come,
    /// and the two drift apart the moment either is touched. There is nothing
    /// here to drift: the near plane is always this far along the way to what
    /// you are looking at, so dollying in can never run the target through it
    /// and zoom has no floor.
    ///
    /// Strictly between 0 and 1, and brought there where the near plane is
    /// worked out rather than refused here. At zero the plane lands on the eye,
    /// at one on what is being looked at.
    pub near_ratio: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            projection: Projection::Perspective,
            target: Vec3::ZERO,
            distance: 6.0,
            yaw: 0.6,
            pitch: 0.4,
            fov_y: 45f32.to_radians(),
            // At the distance a mid-sized scene frames to this lands the near
            // plane around a tenth of a unit, which is where a fixed one would
            // have been put by hand.
            near_ratio: 1.0 / 128.0,
        }
    }
}

impl Camera {
    /// The eye position implied by the target, distance, and angles.
    ///
    /// A position, and rounded like one: far enough from the origin, an eye
    /// close in lands a step of the world off where the angles put it. Nothing
    /// that projects reads it for that reason — see [`ViewProj`].
    pub fn eye(&self) -> Vec3 {
        self.target + self.basis().back * self.distance
    }

    /// The unit direction the camera looks along.
    ///
    /// What a caller turning flat geometry to face the viewer needs: anything
    /// square to this lies in the screen's own plane, so a shape laid out on
    /// two such directions reads at full size from wherever the camera is
    /// rather than collapsing to a line as it comes round.
    ///
    /// Read off the angles rather than differenced out of the eye, so it is a
    /// unit direction however far off the target is and however close in the
    /// eye has come.
    pub fn facing(&self) -> Vec3 {
        -self.basis().back
    }

    /// The screen's axes in the world, straight from the angles.
    ///
    /// Right is level whatever the pitch, because yaw turns about the world up
    /// axis and nothing else does.
    fn basis(&self) -> ViewBasis {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        ViewBasis {
            right: Vec3::new(cos_yaw, 0.0, -sin_yaw),
            up: Vec3::new(-sin_yaw * sin_pitch, cos_pitch, -cos_yaw * sin_pitch),
            back: Vec3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch),
        }
    }

    /// Where the near plane currently sits, in world units. Perspective only:
    /// the orthographic slab is centred on the eye and clips nothing in front
    /// of it.
    ///
    /// Brought into range here rather than refused, which is the one place the
    /// ratio is spent and so the one place it has to be a number. [`Camera::sane`]
    /// applies the same bounds to a camera arriving from outside, and says why a
    /// viewpoint is replaced where a drawing would be reported — but the fields
    /// are public and nothing routes a caller through `sane`, so a ratio that
    /// never came through it still has to draw something.
    pub(crate) fn z_near(&self) -> f32 {
        self.distance * self.near_ratio.clamp(MIN_NEAR_RATIO, MAX_NEAR_RATIO)
    }

    /// Half the world height the viewport covers at the orbit target, which is
    /// what the orthographic view is built on. See [`Camera::fov_y`].
    fn half_height(&self) -> f32 {
        self.distance * (self.fov_y * 0.5).tan()
    }

    /// How many world units one logical pixel covers at `at`.
    ///
    /// What a caller sizing geometry *in pixels* needs. Overlays get this for
    /// free — a stroke's width and a marker's diameter are measured after the
    /// projection divide, in the shader — but a caller that wants a whole
    /// *shape* to hold its size has to build it in the world, and this is the
    /// number that says how big to build it.
    ///
    /// Depends on where, and only under perspective: there a pixel covers more
    /// world the further off it is, so a shape sized against the orbit target
    /// would come out wrong anywhere else.
    ///
    /// Geometry built with this is geometry the camera moving invalidates, and
    /// a caller taking it on is taking that on: [`Pane::camera`] stays
    /// cheap because nothing in a scene depends on the camera, and a batch that
    /// does has to be rewritten when it moves.
    ///
    /// Split in two inside the crate, so that a vertex shader can supply the
    /// depth half out of the `w` it already carries and hold something to the
    /// screen without the camera reaching the scene at all. From outside, this
    /// is the whole number.
    ///
    /// [`Pane::camera`]: crate::Pane::camera
    pub fn world_per_pixel(&self, at: Vec3, viewport: Viewport) -> f32 {
        self.view_depth(at) * self.world_per_clip_w(viewport)
    }

    /// The same scale, less the depth it is taken at: world units per logical
    /// pixel, per unit of clip `w`.
    ///
    /// What lets a *shader* size something against the screen. Every vertex
    /// already carries the `w` its own projection wrote — see
    /// [`Camera::view_depth`] — so handing this over is handing over the whole
    /// of what the camera knows and the vertex does not, and the multiplication
    /// finishes where the vertex is.
    ///
    /// Factored out of [`Camera::world_per_pixel`] rather than stated beside
    /// it, and that is the point of it existing: a renderer that worked the
    /// same number out again from `fov_y` and the viewport would agree with
    /// this one until the day one of the two was changed, and what it sizes is
    /// type that has to land where picking says it is.
    pub(crate) fn world_per_clip_w(&self, viewport: Viewport) -> f32 {
        let half = match self.projection {
            // The slab is the same width all the way through, so the whole of
            // the scale is here and the `w` it is multiplied by is a flat one.
            Projection::Orthographic => self.half_height(),
            // Per unit of depth, which is exactly what `w` then supplies.
            Projection::Perspective => (self.fov_y * 0.5).tan(),
        };
        2.0 * half / viewport.size().y
    }

    /// The `w` this camera's projection writes for a point at `at`.
    ///
    /// The view depth under perspective, measured *along the view* rather than
    /// from the eye — because that is what a projection measures against, so a
    /// point off to one side is no further away for being off to one side. A
    /// flat 1 under parallel rays, which have no depth to divide by.
    ///
    /// Floored at the near plane, which decides nothing about what is drawn:
    /// anything nearer is clipped. All the floor buys is that a scale taken
    /// from it is a positive number rather than a negative or an enormous one.
    /// A shader has no such worry and needs no such floor, since a vertex that
    /// would have wanted it is one the hardware has already thrown away.
    fn view_depth(&self, at: Vec3) -> f32 {
        match self.projection {
            Projection::Orthographic => 1.0,
            Projection::Perspective => {
                ((at - self.target).dot(self.facing()) + self.distance).max(self.z_near())
            }
        }
    }

    /// Combined view-projection for a viewport of the given width/height
    /// ratio.
    ///
    /// Depth is **reversed** either way: the near plane maps to 1 and distance
    /// falls away toward 0, so the depth test runs `Greater` against a buffer
    /// cleared to 0. Under perspective that is what buys the resolution. Float
    /// precision crowds near zero and the perspective divide crowds its own
    /// near the eye; aiming those at opposite ends is what makes them cancel,
    /// leaving roughly constant *relative* resolution — about `distance × 2⁻²⁴`
    /// — in place of resolution that decays with the square of distance. Linear
    /// orthographic depth has nothing to cancel and gains nothing; it is
    /// reversed because the pipeline reads one way round.
    ///
    /// The perspective far plane is at infinity. Nothing is clipped for being
    /// too far off, and since depth resolution is no longer bought at the far
    /// plane's expense, giving it up costs nothing. Orthographic has no such
    /// option and takes a slab instead, reaching sixty-four orbit distances
    /// either side of the target — as far behind the eye as in front, because
    /// with no vanishing point there is nothing to justify clipping what the
    /// eye has passed, and clipping it would slice the model open on the way
    /// in.
    ///
    /// Measured from the orbit target rather than from the world's origin — see
    /// [`ViewProj`] for why, and for what that costs a caller: nothing, as long
    /// as every projection goes through it.
    pub fn view_proj(&self, aspect: f32) -> ViewProj {
        let proj = match self.projection {
            Projection::Perspective => {
                directx::perspective_infinite_reverse(self.fov_y, aspect, self.z_near())
            }
            Projection::Orthographic => {
                let half_height = self.half_height();
                let half_width = half_height * aspect;
                let reach = ORTHO_SLAB * self.distance;
                // Near and far handed over swapped, which is what reverses a
                // depth glam would otherwise run 0 at the near plane to 1.
                directx::orthographic(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    reach,
                    -reach,
                )
            }
        };
        // The world turned into the screen's axes, then pushed the orbit
        // distance down the view — which is where the target sits, and why the
        // target is the origin this is measured from.
        let basis = self.basis();
        let turn = Mat3::from_cols(basis.right, basis.up, basis.back).transpose();
        let view =
            Mat4::from_translation(Vec3::new(0.0, 0.0, -self.distance)) * Mat4::from_mat3(turn);
        ViewProj {
            relative: proj * view,
            origin: self.target,
        }
    }

    /// Where `world` lands on the viewport, in logical pixels down from its
    /// top-left corner, or `None` where the projection does not draw it.
    ///
    /// The way back from [`ray_through`](Self::ray_through), and what an
    /// application asks to put something of its *own* over a place in the
    /// scene — a label in another layer, a field to type into, a menu about
    /// what is drawn there. Read out of the same matrix the vertex shaders are
    /// handed, for the reason the ray gives: a projection derived independently
    /// agrees with the picture only until someone changes it.
    ///
    /// One position at a time, and it builds a whole view-projection to answer:
    /// a caller placing a handful of things per frame pays one apiece, which is
    /// nothing against the arithmetic of drawing them. A caller with a *run* of
    /// positions — a region's boundary, a rim's corners — should build the
    /// matrix once with [`Camera::view_proj`] and read each through
    /// [`Viewport::pixel_of`], which is what this does and all it does.
    pub fn screen_of(&self, world: Vec3, viewport: Viewport) -> Option<Vec2> {
        viewport.pixel_of(self.view_proj(viewport.aspect()).point(world))
    }

    /// The world-space ray through a point on the viewport. `cursor` counts
    /// down from the top-left corner, in the units the [`Viewport`] was built
    /// in.
    ///
    /// The origin sits on the near plane and the direction runs into the scene,
    /// so everything drawn under that point lies at a non-negative distance
    /// along it. Under a parallel projection the direction is the same for every
    /// cursor position and the origin is what moves.
    ///
    /// Read out of the same matrix the vertex shaders are handed rather than
    /// rebuilt from the camera's own parameters. A ray derived independently
    /// agrees with the picture only until someone changes the projection, and
    /// picking that disagrees with what is on screen is worse than none.
    pub fn ray_through(&self, cursor: Vec2, viewport: Viewport) -> Ray {
        self.ray_from(cursor, viewport, self.view_proj(viewport.aspect()))
    }

    /// The same ray, read out of a view-projection the caller already built.
    ///
    /// For [`Aim`](crate::Aim), which needs the matrix itself as well and would
    /// otherwise build it twice. `view_proj` has to be this camera's own for
    /// this viewport — the assert below is what catches one that is not, since a
    /// foreign matrix aims the ray somewhere the picture is not.
    pub(crate) fn ray_from(&self, cursor: Vec2, viewport: Viewport, view_proj: ViewProj) -> Ray {
        let ray = view_proj.ray_through_ndc(viewport.ndc_from_pixel(cursor));
        debug_assert!(
            ray.direction.dot(self.facing()) > 0.0,
            "ray points away from the scene, so depth no longer runs 1 at the \
             near plane — the two ends of this projection have swapped"
        );
        ray
    }

    /// The same camera with everything it insists on brought back into range.
    ///
    /// What a camera arriving from outside goes through — read out of a file,
    /// or typed in. Every other way one moves keeps its own limits as it goes:
    /// [`Camera::orbit`] clamps the pitch it lands on, [`Camera::dolly`] the
    /// distance it lands at, and the fields are public so that a gesture can
    /// name one without a setter apiece. This is the one arrival that has no
    /// such call to come through, so it is where a whole camera is brought into
    /// range at once — where the near plane brings only its own ratio, and only
    /// as it is spent.
    ///
    /// A number that is not one is replaced rather than refused, which is the
    /// difference between a viewpoint and the drawing it looks at: the drawing
    /// is what someone authored and a wrong number in it has to be reported,
    /// where a camera is only where you happen to be standing. Losing that and
    /// opening the document beats keeping it and refusing to.
    pub fn sane(self) -> Self {
        let default = Self::default();
        // Field by field rather than a check over the whole, so one bad number
        // costs its own field and not the rest of the viewpoint.
        let finite = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
        let target = if self.target.is_finite() {
            self.target
        } else {
            default.target
        };
        Self {
            projection: self.projection,
            target,
            distance: bounded_distance(finite(self.distance, default.distance)),
            yaw: finite(self.yaw, default.yaw),
            pitch: finite(self.pitch, default.pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT),
            // Zero would flatten the view to a line and π or more would turn it
            // inside out, so the range is open at both ends and the bounds are
            // a degree off each.
            fov_y: finite(self.fov_y, default.fov_y).clamp(1f32.to_radians(), 179f32.to_radians()),
            // The same bounds [`Camera::z_near`] spends it through, so a camera
            // that came this way and one that did not are drawn alike.
            near_ratio: finite(self.near_ratio, default.near_ratio)
                .clamp(MIN_NEAR_RATIO, MAX_NEAR_RATIO),
        }
    }

    /// Turn the eye around the target by the given angles, in radians.
    pub fn orbit(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.yaw += yaw_delta;
        self.pitch = (self.pitch + pitch_delta).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Point the eye down a stated direction, in radians.
    ///
    /// The absolute twin of [`Camera::orbit`], for a caller that knows where it
    /// wants to end up rather than how far to turn: a gizmo naming a view, or a
    /// key bound to one. Pitch is clamped exactly as an orbit clamps it —
    /// straight up is where the look-at degenerates, and it degenerates however
    /// the camera got there.
    pub fn aim(&mut self, yaw: f32, pitch: f32) {
        self.yaw = yaw;
        self.pitch = pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Scale the orbit distance — `factor` below 1 moves the eye in.
    ///
    /// Zero and infinity land on the nearest and the furthest the eye may be.
    /// A factor that is not a scale at all — negative, or not a number — leaves
    /// the eye where it is: whatever reported one said nothing about which way
    /// to go, and reading it as "all the way in" would jump the view.
    pub fn dolly(&mut self, factor: f32) {
        if factor.is_nan() || factor < 0.0 {
            return;
        }
        self.distance = bounded_distance(self.distance * factor);
    }

    /// Slide what is being looked at, taking the eye with it. Neither angle
    /// nor distance moves, so the scene keeps the pose it was turned to.
    pub fn pan(&mut self, by: Vec3) {
        self.target += by;
    }

    /// How far the target has to move for the picture to travel `screen`
    /// pixels — what [`Camera::pan`] is handed to answer a gesture.
    ///
    /// `screen` counts the way a cursor does, x right and y down, and names
    /// where the *viewport* goes rather than where the scene does: a page
    /// scrolled down moves its content up, and a viewport handed a scroll
    /// delta straight through moves a model the same way.
    ///
    /// Measured at the orbit target. Under perspective that is the one depth
    /// where a pixel has a settled world size, and it is the depth the sketch
    /// being panned is nearest to; the orthographic view has that size
    /// everywhere and reads the same number.
    pub fn pan_step(&self, screen: Vec2, viewport: Viewport) -> Vec3 {
        let ViewBasis { right, up, .. } = self.basis();
        // The scale at the target, asked for rather than worked out: "at the
        // orbit target" is what the paragraph above promises, and a second
        // spelling of it here would be free to stop meaning that.
        let step = self.world_per_pixel(self.target, viewport);
        (right * screen.x - up * screen.y) * step
    }

    /// Look at `extent` from the current angles, far enough back that all of
    /// it is in frame.
    ///
    /// The fit is against the *vertical* field of view, so a viewport wider
    /// than it is tall has room to spare and a taller one crops. What is
    /// fitted is the bounding sphere rather than the box, which is why
    /// orbiting afterwards never swings a corner out of view.
    pub fn frame(&mut self, extent: Extent) {
        self.target = extent.centre();
        let radius = extent.radius();
        self.distance = bounded_distance(radius / (self.fov_y * 0.5).sin());
    }
}

/// The screen's three axes in the world: right and up across it, and back out
/// of it toward the eye.
#[derive(Debug, Clone, Copy)]
struct ViewBasis {
    right: Vec3,
    up: Vec3,
    back: Vec3,
}

/// `distance` brought between the floor and the ceiling.
///
/// `max` then `min` rather than `clamp`, because those two pass over a NaN
/// operand where `clamp` hands it back: an infinite distance written straight
/// into the field and then scaled by zero lands on the floor instead of
/// reaching the renderer.
#[allow(clippy::manual_clamp)]
fn bounded_distance(distance: f32) -> f32 {
    distance.max(MIN_DISTANCE).min(MAX_DISTANCE)
}

#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::camera::{Camera, Projection};
    use glam::Vec3;

    impl Camera {
        /// Straight down −Z from five away with a 90° fov, so every number read
        /// off it can be worked out by hand.
        ///
        /// The projection is half of what that buys: `1/tan(45°)` is one, and a
        /// fifth of the five-unit orbit puts the near plane on one, so reversed
        /// depth reads straight off as `1 / distance`. The framing is the other
        /// half — over a hundred-pixel square it puts the origin dead centre
        /// with the world spanning ±5 across it at the target's depth, which is
        /// ten pixels to the world unit.
        ///
        /// Every field is stated because none of [`Camera::default`] survives
        /// here: that one is angled, further off, narrower and clipped far
        /// nearer, and a fixture wanting arithmetic can borrow none of it.
        pub fn head_on() -> Self {
            Self {
                projection: Projection::Perspective,
                target: Vec3::ZERO,
                distance: 5.0,
                yaw: 0.0,
                pitch: 0.0,
                fov_y: std::f32::consts::FRAC_PI_2,
                near_ratio: 1.0 / 5.0,
            }
        }
    }
}

#[cfg(test)]
mod turning {
    use crate::camera::Camera;

    impl Camera {
        /// [`Camera::head_on`] from the other side of the z = 0 plane.
        ///
        /// The pair a rule about which way round something reads is asked of:
        /// half a turn is the one move that mirrors what is drawn on that plane
        /// without foreshortening it, so what comes back has to be settled by
        /// the rule rather than by the projection.
        ///
        /// Its own mod rather than sitting beside the camera it turns: the
        /// allocation bench shares that one and has nothing to view from behind,
        /// and a `pub(crate)` fixture compiled for a feature nothing under it
        /// calls is dead code.
        pub(crate) fn from_behind() -> Self {
            Self {
                yaw: std::f32::consts::PI,
                ..Self::head_on()
            }
        }
    }
}

#[cfg(test)]
mod tests;
