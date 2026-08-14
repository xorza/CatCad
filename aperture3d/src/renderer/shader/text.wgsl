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
    // A glyph's size came from its shaping, so the look's spread is unused.
    @location(6) half_extent: f32,
    @location(7) z_offset: f32,
    @location(8) plane: vec3<f32>,
) -> TextVsOut {
    let corner = vec2<f32>(
        select(0.0, 1.0, (index & 1u) != 0u),
        select(0.0, 1.0, (index & 2u) != 0u),
    );
    let at = u.view_proj * vec4<f32>(anchor, 1.0);
    let px = (offset + corner * size) * u.raster_scale;
    // The y is negated here and nowhere else: `ndc_from_px_delta` leaves the
    // flip out because every other shape widened in this crate is symmetric in
    // ±, so mirroring one only swaps which corner is which. A glyph is not —
    // it hangs down and to the right of its origin — so the difference between
    // a framebuffer counting down and NDC counting up is real, and has to be
    // taken here.
    let offset_ndc = ndc_from_px_delta(vec2<f32>(px.x, -px.y));

    // A label is wide enough for the surface under it to rise through, so it
    // follows the plane's depth exactly as a stroke or a marker does. Without a
    // plane it stays flat and leans on the bias alone.
    let at_ndc = ndc_from_clip(at);
    let plane_shift = plane_depth_shift(anchor, plane, at, at_ndc, offset_ndc);

    var out: TextVsOut;
    out.clip = lift(
        vec4<f32>(
            at.xy + offset_ndc * at.w,
            at.z + plane_shift.shift * at.w,
            at.w,
        ),
        z_offset,
    );
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
