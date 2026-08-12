// Markers: discs facing the viewer, sized in pixels.

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
    let offset_ndc = ndc_from_px_delta(corner * half_px);

    // A glyph wide enough to see is wide enough for the surface under it to
    // rise through, so the disc follows the plane's depth exactly as a stroke
    // does. Without a plane it stays flat and leans on the bias alone.
    let anchor_ndc = anchor.xyz / max(anchor.w, MIN_W);
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
    let coverage = clamp((1.0 - radius) / max(per_fragment, MIN_FADE) + 0.5, 0.0, 1.0);
    return vec4<f32>(in.color, coverage);
}
