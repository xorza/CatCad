// Shared by every pipeline. Concatenated ahead of the others — WGSL has no
// include, so this is what keeps one statement of the depth contract
// instead of one per primitive.

struct Uniforms {
    view_proj: mat4x4<f32>,
    // Target size in physical pixels, and how many of them a logical pixel is
    // worth. Both overlay passes read them; the mesh pass needs neither.
    viewport: vec2<f32>,
    raster_scale: f32,
    // World distance per unit of clip w to step when probing the plane a curve
    // lies on. The projection sets it — see `Camera::probe_reach`.
    probe_reach: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

// WGSL has no built-in for either, and a shader that spells one out inline is
// a shader that can spell it out differently.
const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const SQRT_2: f32 = 1.41421356237;

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

// Screen length below which a segment lands on one pixel and has no direction
// to widen across. A thousandth of a pixel — the floor `MIN_RUN_PX2` holds,
// squared, on the picking side.
const MIN_PX: f32 = 1e-3;

// Clip `w` floor for the perspective divide.
//
// It decides nothing about visibility — the hardware's near-plane clip does
// that, and everything this catches is on its way to being clipped anyway.
// All it buys is a finite NDC to compute a widening from, so the quad handed
// to the clipper is a quad rather than a page of infinities.
const MIN_W: f32 = 1e-6;

// Determinant floor for the two-by-two solved to read a plane's depth
// gradient. Zero determinant is the plane seen exactly edge-on, where it
// covers no screen area and has no gradient to read.
const MIN_DET: f32 = 1e-6;

// Floor under the width a marker's rim fades over. Only a disc covering most
// of the target approaches it, and the fade is then a hard edge either way.
const MIN_FADE: f32 = 1e-6;

// NDC spans two units across the whole target, so one NDC unit is half the
// viewport in pixels, and back the other way.
//
// These carry *differences* — never positions. The y-flip that separates a
// framebuffer counting down from NDC counting up is deliberately not in them:
// every shape widened here is symmetric in ±, so mirroring it only swaps which
// corner is which, and paying for the flip would buy nothing. Handing either
// of them a position would put it in the wrong half of the screen. Positions
// are converted on the CPU, by `Viewport`.
fn px_from_ndc_delta(delta: vec2<f32>) -> vec2<f32> {
    return delta * u.viewport * 0.5;
}

fn ndc_from_px_delta(delta: vec2<f32>) -> vec2<f32> {
    return delta * 2.0 / u.viewport;
}

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
    if (dot(plane, plane) <= 0.5 || here.w <= MIN_W) {
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
    if (p1.w <= MIN_W || p2.w <= MIN_W) {
        return out;
    }

    let a1 = p1.xyz / p1.w - here_ndc;
    let a2 = p2.xyz / p2.w - here_ndc;
    let det = a1.x * a2.y - a1.y * a2.x;
    if (abs(det) <= MIN_DET) {
        return out;
    }

    let dzdx = (a1.z * a2.y - a1.y * a2.z) / det;
    let dzdy = (a1.x * a2.z - a1.z * a2.x) / det;
    out.shift = dzdx * offset_ndc.x + dzdy * offset_ndc.y;
    out.found = true;
    return out;
}
