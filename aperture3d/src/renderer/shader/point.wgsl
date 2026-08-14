// Markers: discs facing the viewer, sized in pixels.

struct PointVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    // Where in the disc this fragment sits — the rim is at length 1.
    //
    // Linearly rather than perspective-correctly, because it measures the
    // screen and not the world: every corner carries the anchor's own `w`, so
    // the two agree here anyway, and saying which is meant costs nothing.
    @location(1) @interpolate(linear) corner: vec2<f32>,
    // The instance's, so the fragment can turn the line above into pixels
    // without asking the hardware what a fragment is worth.
    @location(2) @interpolate(flat) half_px: f32,
};

// One instance per marker: a quad facing the screen, sized in pixels and hung
// off a world position. Unlike a stroke it has no direction to widen across,
// so every corner takes the anchor's depth: the glyph is a label on a point,
// not a surface lying over one.
//
// The quad spans ±1 either way and nothing else tells its corners apart, so
// the two low bits of the index are the whole of it — bit 0 picks x, bit 1
// picks y.
@vertex
fn point_vs(
    @builtin(vertex_index) index: u32,
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    // Half the diameter in logical px, then the depth bias.
    @location(2) half_size: f32,
    @location(3) z_offset: f32,
    @location(4) plane: vec3<f32>,
) -> PointVsOut {
    let corner = vec2<f32>(
        select(-1.0, 1.0, (index & 1u) != 0u),
        select(-1.0, 1.0, (index & 2u) != 0u),
    );
    let anchor = u.view_proj * vec4<f32>(position, 1.0);
    let half_px = half_size * u.raster_scale;
    let offset_ndc = ndc_from_px_delta(corner * half_px);

    // A glyph wide enough to see is wide enough for the surface under it to
    // rise through, so the disc follows the plane's depth exactly as a stroke
    // does. Without a plane it stays flat and leans on the bias alone.
    let anchor_ndc = ndc_from_clip(anchor);
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
    out.half_px = half_px;
    return out;
}

// The quad is square and the marker is not, so the corners have to go
// somewhere: how far inside the rim a fragment sits is what decides it, and
// [`coverage_px`] decides the rest.
//
// The distance is exact and costs nothing to know. A corner of ±1 is half a
// width away in screen space by construction, so the radius interpolated across
// the quad is already in units of that half width — multiplying by it is the
// whole conversion to pixels. Asking `fwidth` how far the radius moves across a
// fragment answers the same question by measurement, at the price of shading in
// lockstep quads and of being wrong wherever the quad straddles the rim, which
// on a marker spanning a handful of pixels is most of it.
//
// Multisampling alone would only smooth the quad's own border, which is not
// where the disc is — hence the alpha-to-coverage the pipeline asks for.
@fragment
fn point_fs(in: PointVsOut) -> @location(0) vec4<f32> {
    let inside_px = (1.0 - length(in.corner)) * in.half_px;
    return vec4<f32>(in.color, coverage_px(inside_px));
}
