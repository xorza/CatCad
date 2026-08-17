// Stroked circles, laid out coarsely and resolved exactly.

// Vertex pairs around the rim, inner then outer. Only the width of the band
// depends on it: raise it and the band narrows, lower it and more fragments
// are shaded and thrown away. It never decides how round the circle is, which
// is the whole point of drawing one this way.
//
// Handed over by the pipeline rather than written here, because the index
// buffer walking these same angles is built on the Rust side and the two
// drifting apart is invisible: one way round draws the ring twice over, the
// other draws half of it. There is one of them, and this is not it.
override RING_STEPS: u32;

// Floor under the rate above, in pixels per world unit, so the reciprocal below
// stays finite where the plane turns edge-on and the rim covers no screen area
// at all. A rate rather than a length: it shares neither its meaning nor its
// units with `MIN_RUN_PX`, and only the coincidence of one thousandth ever made
// the two look like one constant.
const MIN_PX_PER_WORLD: f32 = 1e-3;

// Ceiling on how far the band may reach past the rim, as a multiple of the
// radius — see where it is applied. Two rather than something tighter because
// a small circle carrying a wide stroke legitimately wants a band wider than
// itself, and this has only to stop the runaway.
const MAX_COVER: f32 = 2.0;

// How far a clip-space direction `d` moves the screen, in pixels, at a point
// `at` whose own `w` is `w` — the projection's tangent, spent through the `w²`
// and out into pixels.
//
// Free here, because the directions it is asked about are the ring's own axes,
// already in clip space and already paid for. No y flip: what comes back is a
// rate, and every use of it below is a length or a dot product against another
// one, so the axis the screen counts down does not enter.
fn screen_rate(at: vec4<f32>, w: f32, d: vec4<f32>) -> vec2<f32> {
    return px_from_ndc_delta(ndc_tangent(at.xy, w, d) / (w * w));
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
    // Pixels per unit of in-plane radius, square to the rim. Linearly rather
    // than perspective-correctly: it is a rate on the screen, not in the world.
    @location(4) @interpolate(linear) px_per_radius: f32,
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
    @location(3) radius: f32,
    // The tail every overlay record shares — see `Look`.
    @location(4) color: vec3<f32>,
    @location(5) half_width: f32,
) -> RingVsOut {
    let angle = f32(vertex / 2u) / f32(RING_STEPS) * TAU;
    let outward = (vertex & 1u) != 0u;
    let turn = vec2<f32>(cos(angle), sin(angle));

    // The ring's plane as a projective map: where its centre lands, and what
    // its two axes become. Everything below is arithmetic on these three —
    // where the vertex goes, which way the rim runs on screen, and what a pixel
    // is worth there — so the whole instance costs three products and no
    // probing at all.
    let c = u.view_proj * vec4<f32>(center, 1.0);
    let ex = u.view_proj * vec4<f32>(x_axis, 0.0);
    let ey = u.view_proj * vec4<f32>(y_axis, 0.0);

    // Clip-space directions along and around the rim, and the rim itself.
    let radial = turn.x * ex + turn.y * ey;
    let tangential = -turn.y * ex + turn.x * ey;
    let at = c + radius * radial;
    let w = max(at.w, MIN_W);
    let radial_px = screen_rate(at, w, radial);
    let tangential_px = screen_rate(at, w, tangential);

    // What a pixel is worth in world units here — the worst it is worth, in
    // any direction along the plane.
    //
    // Not just the radial one: the stroke runs square to the rim *on screen*,
    // and once the plane leans the circle projects to an ellipse whose normal
    // is nowhere near the projected radius. Sizing the band along the radius
    // alone leaves it too narrow exactly where the view grazes, and the outer
    // edge starts cutting the stroke off. The two directions bracket the worst
    // one to within a factor of root two, which is what pays for the `SQRT_2`
    // below.
    let per_world = min(length(radial_px), length(tangential_px));
    let world_per_px = 1.0 / max(per_world, MIN_PX_PER_WORLD);

    // How far past the rim the stroke reaches, in the plane. One pixel more
    // than half a width covers the edge the fragment stage fades over.
    let half_px = half_width * u.raster_scale;
    // Held to a collar rather than allowed to run away with the reciprocal.
    // As the plane turns edge-on the rim covers no screen area, so covering one
    // pixel of stroke asks for a band hundreds of world units wide — and what
    // the projection leaves of *that* is a wedge fanning out across the screen
    // from a circle a centimetre across. Past this point no in-plane band can
    // cover the stroke anyway: the direction its width is measured across is
    // the one the projection has flattened, and only a screen-space widening
    // could answer, which is the thing a ring deliberately does not do. So the
    // band stops growing and the rim thins out honestly instead.
    let cover = min((half_px + 1.0) * world_per_px * SQRT_2, radius * MAX_COVER);
    // The band's outer edge is a chord, not an arc, so it dips inside the
    // circle it was built on by the time it reaches the midpoint between two
    // steps. Pushing the corners out by exactly that ratio puts the midpoint
    // where the stroke needs it — which is what leaves `RING_STEPS` free to be
    // anything at all, rather than only large enough for the shortfall not to
    // show.
    let reach = (radius + cover) / cos(PI / f32(RING_STEPS)) - radius;
    let offset = select(-reach, reach, outward);
    let out_radius = radius + offset;

    // Pixels per unit of in-plane radius, measured square to the rim as it
    // runs on screen — which is the direction the stroke's width is seen
    // across, and nowhere near the projected radius once the plane leans.
    // Handed to the fragment so it can turn its exact in-plane distance into
    // an exact one in pixels without asking the hardware to difference
    // neighbours for it.
    // Guarded before the divide rather than after it. Edge-on the rim covers
    // no screen area and the tangential rate goes to zero, where `normalize`
    // answers NaN — and a NaN carried into the fragment stage takes the whole
    // band with it. The floor below used to be applied to the *result*, which
    // is one step too late to catch it.
    let square_to = vec2<f32>(-tangential_px.y, tangential_px.x);
    let runs = length(square_to);
    let across = select(vec2<f32>(1.0, 0.0), square_to / runs, runs > MIN_PX_PER_WORLD);
    let px_per_radius = max(abs(dot(radial_px, across)), MIN_PX_PER_WORLD);

    var out: RingVsOut;
    // In the plane, every vertex of it: the depth that comes out is then the
    // plane's own exactly, and none of the gradient probing a stroke or a
    // marker needs applies here.
    out.clip = c + out_radius * radial;
    out.color = color;
    out.plane = turn * out_radius;
    out.radius = radius;
    out.half_px = half_px;
    out.px_per_radius = px_per_radius;
    return out;
}

// The circle itself, measured rather than approximated.
//
// The in-plane radius is exact — every vertex of the band lies in the plane, so
// interpolating a position in it is exact too, and the circle stays a circle at
// any `RING_STEPS`. Turning that into pixels is the vertex stage's answer,
// carried in: the rate is smooth around the rim where the radius is not, so one
// interpolates honestly and the other has to be measured where it is used.
//
// Which is what this stopped asking `fwidth` for. That answers `|dpdx| +
// |dpdy|` where the gradient's own length was wanted — the box's diagonal
// against its hypotenuse, up to `SQRT_2` too long and longest exactly where a
// rim runs diagonally across the grid — and a rate too large understates the
// distance to the rim, which draws the band wider than it was authored.
@fragment
fn ring_fs(in: RingVsOut) -> @location(0) vec4<f32> {
    let from_rim_px = abs(length(in.plane) - in.radius) * in.px_per_radius;
    return vec4<f32>(in.color, coverage_px(in.half_px - from_rim_px));
}
