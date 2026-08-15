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
    // lies on. The projection sets it — see `Uniforms::probe_reach`.
    probe_reach: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

// WGSL has no built-in for either, and a shader that spells one out inline is
// a shader that can spell it out differently.
const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const SQRT_2: f32 = 1.41421356237;

// Screen length below which a segment lands on one pixel and has no direction
// to widen across.
//
// Handed over by the pipeline rather than written here, because picking floors
// the same length on the Rust side and the two answering differently would
// widen a segment one way and project a cursor onto it the other. There is one
// of them — `curve::MIN_RUN_PX` — and this is not it.
override MIN_RUN_PX: f32;

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

// The near end of the reversed-depth volume a plane's extrapolated gradient
// has to land inside.
//
// One limit rather than two: the shift is one-sided — it only ever moves a
// corner *toward* the viewer — so it cannot leave the far end, and a floor
// there would be a guard against something that cannot happen. Stepped off the
// edge rather than sitting on it, because a corner has still to survive the
// clip and the depth test after the rasteriser has added the pass's own bias,
// and that leaves the largest of those decades below the 1% held back here.
const MAX_NDC_Z: f32 = 0.99;


// How much of a fragment a shape covers, given how far inside its own edge the
// fragment sits — negative outside, in pixels.
//
// The one coverage rule the overlays have. What differs between a stroke, a rim
// and a marker is how each measures that distance, which is irreducible: a
// ribbon knows it across its width, a band from the radius in its own plane, a
// disc from its centre. What must not differ is what the distance then *means*,
// because that is what decides whether a width asked for is a width drawn —
// and three statements of it are three chances to answer differently.
//
// The half pixel is the whole of the antialiasing: a fragment is shaded from
// its centre, so an edge exactly on that centre covers half of it, and the ramp
// either side spans the one pixel the edge is uncertain over. Integrating it
// across a shape returns the shape's own width exactly.
//
// What comes back is alpha, and every pass that uses it asks for
// alpha-to-coverage rather than blending, so it lands as a sample mask: nothing
// blends, depth stays clean, and draw order stays free.
fn coverage_px(inside_px: f32) -> f32 {
    return clamp(inside_px + 0.5, 0.0, 1.0);
}

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

// The perspective divide, floored so a position at or behind the eye answers
// with something finite rather than an infinity to widen a quad from. What
// comes back is meaningless there, and is meant to be: those vertices exist to
// be clipped against, and the clip reads the `z` they carry.
fn ndc_from_clip(clip: vec4<f32>) -> vec3<f32> {
    return clip.xyz / max(clip.w, MIN_W);
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
    let raw = dzdx * offset_ndc.x + dzdy * offset_ndc.y;

    // Dropped where it would carry the corner out of the depth volume, because
    // the affine answer above is exact over the plane and the plane only reaches
    // so far. Its horizon is where the surface runs to infinity, and screen
    // positions past that line name points *behind* the eye: reversed depth
    // carries them through zero and out the far side, taking the fragments with
    // them. That is a label cut across its waist by nothing at all — no surface
    // is involved, and no amount of depth bias moves it, because what is thrown
    // away was never depth-tested.
    //
    // Dropped rather than pinned to the edge, which is the same mistake wearing
    // the other face: a corner held at the far limit is a corner at infinity,
    // and a label with half of it out there reads *behind* everything standing
    // on the very surface it annotates. The anchor's own depth is the honest
    // stand-in — it is a real point of the plane, the one the label is placed
    // by, and exactly what a label naming no plane gets.
    //
    // It also settles the anchor sitting outside the volume itself, which is the
    // camera having moved through it: every corner falls back, the quad keeps
    // the depth it would have had, and the clip takes it away as before.
    let toward = max(raw, 0.0);
    let depth = here_ndc.z + toward;
    out.shift = select(0.0, toward, depth < MAX_NDC_Z);
    out.found = true;
    return out;
}
