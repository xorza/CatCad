struct Uniforms {
    view_proj: mat4x4<f32>,
    // Target size in physical pixels, and how many of them a logical pixel is
    // worth. Only the curve pass reads them.
    viewport: vec2<f32>,
    raster_scale: f32,
    _pad: f32,
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
// The step is relative rather than a flat amount of NDC because float
// precision is relative: depth values crowd together as they approach the far
// plane, and a bias that ignored that would be thousands of steps up close and
// a fraction of one far away. Scaling moves the same number of representable
// values at any distance — enough to settle which of two coplanar surfaces
// wins, never enough to show through something genuinely in front.
//
// Scaling also keeps the result in the clip volume for free: depth runs 0 at
// the near plane to 1 at the far one, and shrinking a non-negative z toward 0
// cannot push it out the near side.
fn lift(clip: vec4<f32>, z_offset: f32) -> vec4<f32> {
    return vec4<f32>(clip.xy, clip.z * (1.0 - z_offset * DEPTH_STEP), clip.w);
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

// Below this the two ends of a segment land on the same pixel and no
// direction can be recovered from them; any direction will do, since the
// ribbon it widens is sub-pixel anyway.
const DEGENERATE: f32 = 1e-6;

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
    @location(3) params: vec3<f32>,
) -> CurveVsOut {
    let here = u.view_proj * vec4<f32>(position, 1.0);
    let there = u.view_proj * vec4<f32>(other, 1.0);

    // NDC spans two units over the whole target, hence the halved viewport.
    let here_px = here.xy / max(here.w, DEGENERATE) * u.viewport * 0.5;
    let there_px = there.xy / max(there.w, DEGENERATE) * u.viewport * 0.5;
    let travel = there_px - here_px;
    let length_px = length(travel);
    var along = vec2<f32>(1.0, 0.0);
    if (length_px > DEGENERATE) {
        along = travel / length_px;
    }
    let across = vec2<f32>(-along.y, along.x);

    let side = params.x;
    let half_width = params.y * u.raster_scale;
    // Every vertex also steps back from its own end by half a width, which
    // squares off the cap. Two segments meeting at a corner then overlap
    // instead of leaving a notch between them.
    let offset_px = (across * side - along) * half_width;

    // That step back leaves the corner half a width beyond its own end while
    // still carrying that end's depth. On a segment running away from the
    // camera those are not the same depth, and the cap dips below the surface
    // the stroke was drawn on — which the depth test then eats, one gap per
    // segment. NDC depth is linear in screen space, so reading the segment's
    // own ramp half a width further back is the depth the corner should have.
    //
    // Only the along-segment step is corrected. The across step would need the
    // depth of a surface this shader knows nothing about; what covers that is
    // the constant bias below.
    //
    // Both ends have to be in front of the eye for the ramp to mean anything.
    // Across the near plane one end's depth is nonsense, and extrapolating
    // from it would throw the whole quad out of the clip volume instead of
    // merely distorting it — so that case keeps the flat depth it had before.
    var ramp = 0.0;
    if (length_px > DEGENERATE && here.w > DEGENERATE && there.w > DEGENERATE) {
        let rise = there.z / there.w - here.z / here.w;
        ramp = -rise * half_width / length_px;
    }

    var out: CurveVsOut;
    let widened = vec4<f32>(
        here.xy + offset_px * 2.0 / u.viewport * here.w,
        here.z + ramp * here.w,
        here.w,
    );
    out.clip = lift(widened, params.z);
    out.color = color;
    return out;
}

@fragment
fn curve_fs(in: CurveVsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
