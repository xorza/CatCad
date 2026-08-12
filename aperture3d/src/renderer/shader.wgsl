struct Uniforms {
    view_proj: mat4x4<f32>,
    // Target size in physical pixels, and how many of them a logical pixel is
    // worth. Only the curve pass reads them.
    viewport: vec2<f32>,
    raster_scale: f32,
    // World distance per unit of clip w to step when probing the plane a curve
    // lies on. The projection sets it — see `Camera::probe_reach`.
    probe_reach: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

// The relative gap between neighbouring f32 values, which is what one step of
// depth resolution costs. The mantissa is 24 bits, so the gap runs between
// 2⁻²⁴ and 2⁻²³ depending on where in the binade the value sits; the larger is
// the one that always clears it.
const DEPTH_STEP: f32 = 1.0 / 8388608.0;

// Pull a clip position toward the viewer by `z_offset` steps of depth
// resolution. Only z moves, so the geometry lands on exactly the same pixels.
//
// Depth is reversed, so toward the viewer is *up*. The step is relative rather
// than a flat amount of NDC because float precision is relative, and scaling
// therefore moves the same number of representable values at whatever distance
// the geometry sits — which under reversed depth with an infinite far plane is
// exactly sliding the vertex along its view ray toward the eye by that
// fraction of its distance.
//
// A bias is needed at all, however good the depth format gets, because
// bit-exact agreement between two vertex shaders is not guaranteed: WGSL
// permits `curve_vs` and `vs` to compute different values for the same
// position, so a coplanar tie between a stroke and a face cannot be left to
// arithmetic alone.
//
// Nothing here guards the near plane, and nothing should. Reversed, the near
// plane is `z == w`, so a lift can push a vertex out through it — but the
// hardware *clips* a primitive to the volume rather than dropping it, so the
// part still in front is drawn either way and the lift simply saturates.
// Clamping instead is actively wrong: this projection writes a constant
// `clip.z`, so `min(z, w)` fires on every vertex nearer than the near plane,
// behind-the-eye ones included, and rewriting their `z` moves where the clip
// lands. A face large enough to reach past the camera then loses the part that
// should have survived.
fn lift(clip: vec4<f32>, z_offset: f32) -> vec4<f32> {
    return vec4<f32>(clip.xy, clip.z * (1.0 + z_offset * DEPTH_STEP), clip.w);
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) z_offset: f32,
) -> VsOut {
    var out: VsOut;
    out.clip = lift(u.view_proj * vec4<f32>(position, 1.0), z_offset);
    out.normal = normal;
    out.color = color;
    return out;
}

// Fixed key light plus a hemisphere ambient, both in world space: the camera
// orbits, so a view-space light would make the shading swim as you drag.
const KEY_DIR: vec3<f32> = vec3<f32>(0.4, 0.8, 0.45);
const SKY: vec3<f32> = vec3<f32>(0.22, 0.24, 0.30);
const GROUND: vec3<f32> = vec3<f32>(0.06, 0.05, 0.05);

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let key = max(dot(n, normalize(KEY_DIR)), 0.0);
    let ambient = mix(GROUND, SKY, n.y * 0.5 + 0.5);
    return vec4<f32>(in.color * (ambient + key), 1.0);
}

struct CurveVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
};

// The floor under every quantity this shader divides by: a segment's screen
// length, a clip `w`, the determinant of a two-by-two. Each means something
// different, and they share a constant only because the answer to all three is
// the same — below this there is no information left to recover, so take the
// fallback rather than the noise.
const DEGENERATE: f32 = 1e-6;

/// How far the depth of a plane moves over a screen-space step away from a
/// point on it, and whether that could be answered at all.
struct PlaneShift {
    shift: f32,
    found: bool,
}

// Under a projective transform a world plane stays a plane, so NDC depth over
// it is an exact affine function of screen position. Sample it at two more
// points of the plane, solve for its gradient, and read off how far the depth
// moves over `offset_ndc`.
fn plane_depth_shift(
    position: vec3<f32>,
    plane: vec3<f32>,
    here: vec4<f32>,
    here_ndc: vec3<f32>,
    offset_ndc: vec2<f32>,
) -> PlaneShift {
    var out: PlaneShift;
    out.shift = 0.0;
    out.found = false;
    if (dot(plane, plane) <= 0.5 || here.w <= DEGENERATE) {
        return out;
    }

    // Two in-plane directions, neither unit nor orthogonal on purpose: the
    // gradient that falls out belongs to the plane, not to the basis it was
    // read against, so normalizing would cost an inverse square root to
    // arrive at the same answer. They need only be independent, and far
    // enough apart on screen that differencing their depths doesn't cancel
    // down to noise — hence a reach scaled to the viewing distance, which
    // under a parallel projection has to come from the uniform because `w` is
    // then a constant 1 that knows nothing about it.
    var seed = vec3<f32>(1.0, 0.0, 0.0);
    if (abs(plane.x) > 0.9) {
        seed = vec3<f32>(0.0, 1.0, 0.0);
    }
    let e1 = cross(plane, seed) * (here.w * u.probe_reach);
    let e2 = cross(plane, e1);
    let p1 = u.view_proj * vec4<f32>(position + e1, 1.0);
    let p2 = u.view_proj * vec4<f32>(position + e2, 1.0);
    if (p1.w <= DEGENERATE || p2.w <= DEGENERATE) {
        return out;
    }

    let a1 = p1.xyz / p1.w - here_ndc;
    let a2 = p2.xyz / p2.w - here_ndc;
    let det = a1.x * a2.y - a1.y * a2.x;
    // Zero determinant is the plane seen exactly edge-on, where it covers no
    // screen area and has no gradient to read.
    if (abs(det) <= DEGENERATE) {
        return out;
    }

    let dzdx = (a1.z * a2.y - a1.y * a2.z) / det;
    let dzdy = (a1.x * a2.z - a1.z * a2.x) / det;
    out.shift = dzdx * offset_ndc.x + dzdy * offset_ndc.y;
    out.found = true;
    return out;
}

// A vertex arrives knowing both ends of its segment, which side of it to sit
// on, how wide the stroke is, and how far to lift it in depth. The widening
// happens here rather than on the CPU so it can be measured in pixels after
// the projection divide — that is what keeps a stroke the same width near and
// far.
//
// A segment crossing the near plane has an end with no meaningful screen
// position. Its ribbon distorts, but every fragment of it is clipped away, so
// only the visible remainder is drawn.
@vertex
fn curve_vs(
    @location(0) position: vec3<f32>,
    @location(1) other: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) side: f32,
    @location(4) half_width: f32,
    @location(5) z_offset: f32,
    @location(6) plane: vec3<f32>,
) -> CurveVsOut {
    let here = u.view_proj * vec4<f32>(position, 1.0);
    let there = u.view_proj * vec4<f32>(other, 1.0);
    let here_ndc = here.xyz / max(here.w, DEGENERATE);
    let there_ndc = there.xyz / max(there.w, DEGENERATE);

    // NDC spans two units over the whole target, hence the halved viewport.
    let travel = (there_ndc.xy - here_ndc.xy) * u.viewport * 0.5;
    let length_px = length(travel);
    var along = vec2<f32>(1.0, 0.0);
    if (length_px > DEGENERATE) {
        along = travel / length_px;
    }
    let across = vec2<f32>(-along.y, along.x);

    let half_px = half_width * u.raster_scale;
    // Every vertex also steps back from its own end by half a width, which
    // squares off the cap. Two segments meeting at a corner then overlap
    // instead of leaving a notch between them.
    let offset_px = (across * side - along) * half_px;

    // Widening moves the corner off its own centreline — half a width sideways
    // and, from the cap, half a width past its own end — while the depth it
    // arrived with belongs to the centreline. On anything but a head-on
    // surface those are different depths, so the corner dips below the surface
    // the stroke lies on and the depth test eats it: gaps at every join where
    // the segment recedes, and up to half the stroke's width where the surface
    // rises across it.
    //
    // A curve that named its plane can be given the surface's own depth
    // instead. Under a projective transform a world plane stays a plane, so
    // NDC depth over it is an exact affine function of screen position: sample
    // it at two more points of the plane, solve for its gradient, and read off
    // how far the depth moves over the offset the corner actually took.
    //
    // Carried as a *shift* from the centreline rather than a depth in its own
    // right, so that leaving it at zero passes `here.z` through untouched.
    // Recomputing the absolute value instead would divide by `w` and multiply
    // it straight back, which is a rounding step on every vertex and outright
    // nonsense on one behind the eye, where `w` is negative and the clamp below
    // is all that stands between the divide and infinity. Those vertices exist
    // to be clipped against, and the clip reads the `z` they carry.
    let offset_ndc = offset_px * 2.0 / u.viewport;
    var depth_shift = 0.0;
    let plane_shift = plane_depth_shift(position, plane, here, here_ndc, offset_ndc);
    depth_shift = plane_shift.shift;
    let from_plane = plane_shift.found;

    // Without a plane only the along-segment half of the error is recoverable:
    // the segment's own depth ramp says what the cap should have, and the
    // sideways half is left to the constant bias. Both ends must be in front
    // of the eye — across the near plane one end's depth is nonsense, and
    // extrapolating from it would throw the quad out of the clip volume
    // instead of merely distorting it.
    if (!from_plane && length_px > DEGENERATE && here.w > DEGENERATE && there.w > DEGENERATE) {
        depth_shift = -(there_ndc.z - here_ndc.z) * half_px / length_px;
    }

    var out: CurveVsOut;
    let widened = vec4<f32>(
        here.xy + offset_ndc * here.w,
        here.z + depth_shift * here.w,
        here.w,
    );
    out.clip = lift(widened, z_offset);
    out.color = color;
    return out;
}

@fragment
fn curve_fs(in: CurveVsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}

struct PointVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    // Where in the disc this fragment sits — the rim is at length 1.
    @location(1) corner: vec2<f32>,
};

// A marker is a quad facing the screen, sized in pixels and hung off a world
// position. Unlike a stroke it has no direction to widen across, so every
// corner takes the anchor's depth: the glyph is a label on a point, not a
// surface lying over one.
@vertex
fn point_vs(
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) corner: vec2<f32>,
    // Half the diameter in logical px, then the depth bias.
    @location(3) half_size: f32,
    @location(4) z_offset: f32,
    @location(5) plane: vec3<f32>,
) -> PointVsOut {
    let anchor = u.view_proj * vec4<f32>(position, 1.0);
    let half_px = half_size * u.raster_scale;
    let offset_ndc = corner * half_px * 2.0 / u.viewport;

    // A glyph wide enough to see is wide enough for the surface under it to
    // rise through, so the disc follows the plane's depth exactly as a stroke
    // does. Without a plane it stays flat and leans on the bias alone.
    let anchor_ndc = anchor.xyz / max(anchor.w, DEGENERATE);
    let plane_shift = plane_depth_shift(position, plane, anchor, anchor_ndc, offset_ndc);

    var out: PointVsOut;
    out.clip = lift(
        vec4<f32>(
            anchor.xy + offset_ndc * anchor.w,
            anchor.z + plane_shift.shift * anchor.w,
            anchor.w,
        ),
        z_offset,
    );
    out.color = color;
    out.corner = corner;
    return out;
}

// The quad is square and the marker is not, so the corners have to go
// somewhere. Coverage falls off over one fragment's worth of radius, which is
// what `fwidth` measures.
//
// Multisampling alone would only smooth the quad's own border, which is not
// where the disc is. Alpha-to-coverage turns this into a sample mask instead,
// which is why the pipeline asks for it and why nothing here blends: samples
// outside the disc are never written, so depth stays clean and draw order
// stays free.
@fragment
fn point_fs(in: PointVsOut) -> @location(0) vec4<f32> {
    let radius = length(in.corner);
    // How much the radius moves across one fragment, which is the width the
    // rim has to fade over. Smoothstepping the whole of it instead would blur
    // across two, and at the handful of pixels a marker spans that is most of
    // the disc.
    let per_fragment = fwidth(radius);
    let coverage = clamp((1.0 - radius) / max(per_fragment, DEGENERATE) + 0.5, 0.0, 1.0);
    return vec4<f32>(in.color, coverage);
}
