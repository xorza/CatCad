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
    @location(4) plane: vec3<f32>,
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
    // the depth wherever the corner actually landed.
    let offset_ndc = offset_px * 2.0 / u.viewport;
    var depth_ndc = here.z / max(here.w, DEGENERATE);
    var placed = false;

    if (dot(plane, plane) > 0.5 && here.w > DEGENERATE) {
        // Any two in-plane directions will do. Sampling a quarter of the view
        // distance away keeps the two probes far enough apart on screen that
        // differencing their depths doesn't cancel down to noise.
        var seed = vec3<f32>(1.0, 0.0, 0.0);
        if (abs(plane.x) > 0.9) {
            seed = vec3<f32>(0.0, 1.0, 0.0);
        }
        let e1 = normalize(cross(plane, seed));
        let e2 = cross(plane, e1);
        let reach = here.w * 0.25;
        let p1 = u.view_proj * vec4<f32>(position + e1 * reach, 1.0);
        let p2 = u.view_proj * vec4<f32>(position + e2 * reach, 1.0);

        if (p1.w > DEGENERATE && p2.w > DEGENERATE) {
            let origin = here.xyz / here.w;
            let a1 = p1.xyz / p1.w - origin;
            let a2 = p2.xyz / p2.w - origin;
            let det = a1.x * a2.y - a1.y * a2.x;
            // Zero determinant is the plane seen exactly edge-on, where it
            // covers no screen area and has no gradient to read.
            if (abs(det) > DEGENERATE) {
                let dzdx = (a1.z * a2.y - a1.y * a2.z) / det;
                let dzdy = (a1.x * a2.z - a1.z * a2.x) / det;
                depth_ndc = origin.z + dzdx * offset_ndc.x + dzdy * offset_ndc.y;
                placed = true;
            }
        }
    }

    // Without a plane only the along-segment half of the error is recoverable:
    // the segment's own depth ramp says what the cap should have, and the
    // sideways half is left to the constant bias. Both ends must be in front
    // of the eye — across the near plane one end's depth is nonsense, and
    // extrapolating from it would throw the quad out of the clip volume
    // instead of merely distorting it.
    if (!placed && length_px > DEGENERATE && here.w > DEGENERATE && there.w > DEGENERATE) {
        let rise = there.z / there.w - here.z / here.w;
        depth_ndc = depth_ndc - rise * half_width / length_px;
    }

    var out: CurveVsOut;
    let widened = vec4<f32>(
        here.xy + offset_ndc * here.w,
        depth_ndc * here.w,
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
