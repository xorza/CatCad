// Text: glyph quads hung off a world anchor, read from the coverage sheet.

// The sheet every glyph is read from, and how to read it. Only this pass
// samples them, but the bind group layout is shared by every pipeline, so the
// declarations are module-scope like everything else here.
@group(0) @binding(1) var sheet: texture_2d<f32>;
@group(0) @binding(2) var sheet_sampler: sampler;

struct TextVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    // Where on the sheet this fragment reads. Perspective-correct would be
    // wrong and linear would be no better: every corner carries the anchor's
    // own `w`, so the two agree, and the quad is a screen-space rectangle
    // rather than a window onto the world.
    @location(1) @interpolate(linear) uv: vec2<f32>,
};

// Which way a world direction runs on screen where a point of clip position
// `here` is drawn — a bearing and not a rate, which is the whole of what the two
// rules below read.
//
// `Viewport::screen_tangent` is the same question in Rust and `screen_rate` in
// `ring.wgsl` is the same one on this side; all three are the expression
// `ndc_tangent` holds, and differ only in what they spend it through. This one
// spends it through nothing: only the signs are read, and the `w²` and the
// viewport are both positive scales that leave a sign where it was.
//
// The y flip is *not* spare, which is why it survives where those two do not:
// negating one axis reverses the winding, and the winding is the first of the
// two rules.
fn screen_bearing(world: vec3<f32>, here: vec4<f32>) -> vec2<f32> {
    let there = u.view_proj * vec4<f32>(world, 0.0);
    let ndc = ndc_tangent(here.xy, here.w, there);
    return vec2<f32>(ndc.x, -ndc.y);
}

// The plane directions a run laid in one is set along, with both signs settled.
//
// `Turn::axes` in Rust, and the one rule the two must agree on; its reasoning
// lives there and what is here is the arithmetic. No guard, and none wanted: a
// winding and a sign bit are all that is read, and a projection collapsed to
// nothing answers both without dividing by it.
struct RunAxes {
    advance: vec3<f32>,
    down: vec3<f32>,
};

fn run_axes(right: vec3<f32>, plane: vec3<f32>, here: vec4<f32>) -> RunAxes {
    let across = cross(plane, right);
    let along = screen_bearing(right, here);
    let sideways = screen_bearing(across, here);
    // Of the plane's two ways to run down, the one that winds the way the
    // screen does; then the half turn, as a sign on both.
    let winding = along.x * sideways.y - along.y * sideways.x;
    let down = select(-across, across, winding >= 0.0);
    let upright = select(1.0, -1.0, along.x < 0.0);
    var out: RunAxes;
    out.advance = right * upright;
    out.down = down * upright;
    return out;
}

// One instance per glyph: a rectangle in screen space, offset from the run's
// anchor by where the shaper put it. Unlike a marker's quad this is not
// symmetric about anything — a glyph hangs off the run's origin by its bearing
// — so its corners span 0..1 rather than ±1, and the two low bits of the index
// are the whole of what tells them apart.
@vertex
fn text_vs(
    @builtin(vertex_index) index: u32,
    @location(0) anchor: vec3<f32>,
    @location(1) offset: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_size: vec2<f32>,
    @location(5) color: vec3<f32>,
    // Location 6 is the look's `half_extent`, and a glyph has no use for one —
    // its size came from its shaping, not from a width the shader spreads. Left
    // undeclared rather than declared and ignored: what a declaration would buy
    // is wgpu matching its *base type* against the layout and nothing else — the
    // component count is free to differ — so an `f32` among `f32`s is checked by
    // nothing, and `Record::LAYOUT_SPANS_STRUCT` already holds the attribute
    // list to the record's own size.
    @location(7) plane: vec3<f32>,
    // Zero where the run is square to the viewer, which is the whole of what
    // tells the two apart.
    @location(8) right: vec3<f32>,
    // How far the run's box floats off the point it names, per logical pixel of
    // it — already resolved against the plane's authored axes, so there is no
    // second reading of them here. See `Turn::lift_world`.
    @location(9) lift: vec3<f32>,
) -> TextVsOut {
    let corner = vec2<f32>(
        select(0.0, 1.0, (index & 1u) != 0u),
        select(0.0, 1.0, (index & 2u) != 0u),
    );
    let at = u.view_proj * vec4<f32>(anchor, 1.0);
    // Logical pixels, as the run was shaped and as the lift is stated. Which of
    // the two branches below is taken decides what they are turned into: the
    // laid one spends them in the world, the square one in NDC.
    let px = offset + corner * size;

    var out: TextVsOut;
    if (dot(right, right) > 0.5) {
        // **Laid in the plane.** The corner is a world position on it, so the
        // projection does the rest: the run foreshortens, and its depth is the
        // surface's exactly rather than extrapolated back onto it.
        //
        // Sized against the screen all the same. One logical pixel of the
        // shaping is worth `world_per_logical_px` of world per unit of depth, so
        // a run holds the size it would have had square to the viewer — which is
        // also why the sheet is rasterized at one size however far off it is.
        //
        // No y flip: `px.y` runs down the run's own box, and the axes come back
        // with a down that projects downward.
        let axes = run_axes(right, plane, at);
        // One step, and every length below is in the pixels it counts. Nothing
        // here goes through `raster_scale`: a display's scale reaches a laid run
        // through `world_per_logical_px` alone, so a quantity added to this
        // branch cannot be added in the wrong pixel.
        let step = at.w * u.world_per_logical_px;
        // The lift moves the point the box hangs off and nothing else — and it
        // is in the plane's own axes rather than the run's, so the mirror and
        // the half turn above leave it where it is. A run that comes round to
        // stay readable only changes direction.
        let hangs = anchor + lift * step;
        let corner_world =
            hangs + axes.advance * (px.x * step) + axes.down * (px.y * step);
        out.clip = u.view_proj * vec4<f32>(corner_world, 1.0);
    } else {
        // **Square to the viewer.** A rectangle in screen space, hung off the
        // anchor's own projection.
        //
        // The one place a *position* is flipped: `ndc_from_px_delta` leaves the
        // flip out because every other shape widened in this crate is symmetric
        // in ±, so mirroring one only swaps which corner is which. A glyph is
        // not — it hangs down and to the right of its origin — so the difference
        // between a framebuffer counting down and NDC counting up is real, and
        // has to be taken here. `screen_bearing` flips a *direction* above, for
        // a reason of its own.
        //
        // Into the target's own pixels here and nowhere else in this shader:
        // NDC is a fraction of the target, and the target's pixels are physical.
        let offset_ndc = ndc_from_px_delta(vec2<f32>(px.x, -px.y) * u.raster_scale);
        // A label is wide enough for the surface under it to rise through, so it
        // follows the plane's depth as a stroke or a marker does. Without a
        // plane it stays flat and leans on the bias alone. The laid path wants
        // none of this: its corners are already on the surface.
        let at_ndc = ndc_from_clip(at);
        let plane_shift = plane_depth_shift(anchor, plane, at, at_ndc, offset_ndc);
        out.clip = moved_in_ndc(at, offset_ndc, plane_shift.shift);
    }
    out.color = color;
    out.uv = uv_min + corner * uv_size;
    return out;
}

// The sheet holds coverage, so the glyph's own antialiasing is its alpha — and
// this is the one pass that blends rather than asking for alpha-to-coverage.
// Coverage quantized to the sample count is enough for a stroke's edge or a
// disc's rim; on the stem of a glyph a handful of pixels wide it is the
// difference between type and a stipple.
@fragment
fn text_fs(in: TextVsOut) -> @location(0) vec4<f32> {
    let coverage = textureSample(sheet, sheet_sampler, in.uv).r;
    return vec4<f32>(in.color, coverage);
}
