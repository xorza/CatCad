// Stroked circles, laid out coarsely and resolved exactly.

// Vertex pairs around the rim, inner then outer. Only the width of the band
// depends on it: raise it and the band narrows, lower it and more fragments
// are shaded and thrown away. It never decides how round the circle is, which
// is the whole point of drawing one this way.
const RING_STEPS: u32 = 32u;

// How far along the radius to step when asking what a pixel is worth there.
// A sixteenth is short enough that the projection barely bends over it and
// long enough that the difference doesn't vanish into rounding.
const RING_PROBE: f32 = 0.0625;

// Pixels between two world positions. Both are divided by their own `w`, so a
// position behind the eye answers with the floor rather than an infinity.
fn pixels_between(start: vec3<f32>, end: vec3<f32>) -> f32 {
    let a = u.view_proj * vec4<f32>(start, 1.0);
    let b = u.view_proj * vec4<f32>(end, 1.0);
    let apart = b.xy / max(b.w, MIN_W) - a.xy / max(a.w, MIN_W);
    return length(px_from_ndc_delta(apart));
}

struct RingVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    // Where this fragment sits in the ring's own plane, measured from the
    // centre. Every vertex of the band lies in that plane, so interpolating
    // it is exact rather than approximate.
    @location(1) plane: vec2<f32>,
    // The instance's, not the fragment's — flat so the hardware carries them
    // rather than interpolating a constant.
    @location(2) @interpolate(flat) radius: f32,
    @location(3) @interpolate(flat) half_px: f32,
};

// A band around the rim, wide enough that the true circle is inside it at any
// zoom, built from a fixed number of steps.
//
// Widened *in the ring's plane* rather than in screen space, which is what
// keeps every vertex on the plane: the depth that comes out is then the
// plane's own, exactly, and none of the gradient probing a stroke or a marker
// needs applies here.
@vertex
fn ring_vs(
    @builtin(vertex_index) vertex: u32,
    @location(0) center: vec3<f32>,
    @location(1) x_axis: vec3<f32>,
    @location(2) y_axis: vec3<f32>,
    @location(3) color: vec3<f32>,
    @location(4) radius: f32,
    @location(5) half_width: f32,
    @location(6) z_offset: f32,
) -> RingVsOut {
    let angle = f32(vertex / 2u) / f32(RING_STEPS) * TAU;
    let outward = (vertex & 1u) != 0u;
    let along = cos(angle) * x_axis + sin(angle) * y_axis;

    // What a pixel is worth in world units here — the worst it is worth, in
    // any direction along the plane.
    //
    // Not just the radial one: the stroke runs square to the rim *on screen*,
    // and once the plane leans the circle projects to an ellipse whose normal
    // is nowhere near the projected radius. Sizing the band along the radius
    // alone leaves it too narrow exactly where the view grazes, and the outer
    // edge starts cutting the stroke off. Two square probes bracket the worst
    // direction to within a factor of root two, which is what pays for the
    // `SQRT_2` below.
    let tangent = -sin(angle) * x_axis + cos(angle) * y_axis;
    let rim = center + along * radius;
    let step = radius * RING_PROBE;
    let spread = min(
        pixels_between(rim, rim + along * step),
        pixels_between(rim, rim + tangent * step),
    );
    let world_per_px = step / max(spread, MIN_PX);

    // The chord between two steps dips this far inside the arc. That is a fact
    // about the circle rather than about the view, so it needs no projecting.
    let sagitta = radius * (1.0 - cos(PI / f32(RING_STEPS)));
    let half_px = half_width * u.raster_scale;
    // One pixel past the stroke covers the edge the fragment stage fades over.
    let reach = (half_px + 1.0) * world_per_px * SQRT_2 + sagitta;
    let offset = select(-reach, reach, outward);
    let out_radius = radius + offset;

    var out: RingVsOut;
    out.clip = lift(
        u.view_proj * vec4<f32>(center + along * out_radius, 1.0),
        z_offset,
    );
    out.color = color;
    out.plane = vec2<f32>(cos(angle), sin(angle)) * out_radius;
    out.radius = radius;
    out.half_px = half_px;
    return out;
}

// The circle itself, measured rather than approximated.
//
// `fwidth` of the in-plane radius is how far that radius moves across one
// fragment, which is exactly the factor turning a distance in the plane into
// a distance in pixels — and it picks up the foreshortening of a circle seen
// at an angle without being told the camera is there at all.
//
// Coverage in alpha for the same reason the markers use it: alpha-to-coverage
// turns it into a sample mask, so nothing blends, depth stays clean and draw
// order stays free.
@fragment
fn ring_fs(in: RingVsOut) -> @location(0) vec4<f32> {
    let radius = length(in.plane);
    let per_fragment = max(fwidth(radius), MIN_FADE);
    let from_rim = abs(radius - in.radius) / per_fragment;
    let coverage = clamp(in.half_px - from_rim + 0.5, 0.0, 1.0);
    return vec4<f32>(in.color, coverage);
}
