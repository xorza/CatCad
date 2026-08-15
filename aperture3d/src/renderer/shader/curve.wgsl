// Stroked polylines, widened to ribbons in screen space.

struct CurveVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    // How far across the ribbon this fragment sits, in pixels from the
    // centreline. Linearly rather than perspective-correctly: it measures the
    // screen, and a ribbon running away from the viewer has ends of very
    // different `w` — dividing by it would bend a straight ramp.
    @location(1) @interpolate(linear) across_px: f32,
    @location(2) @interpolate(flat) half_px: f32,
};

// One instance per segment: it arrives knowing both ends, how wide the stroke
// is. The widening happens here rather than
// on the CPU so it can be measured in pixels after the projection divide —
// that is what keeps a stroke the same width near and far.
//
// The four corners differ only in which end they sit at and which side they
// lean to, so both come from the index rather than the buffer. Corners 0 and 1
// sit at `start` facing `end`; 2 and 3 sit at `end` facing back, and their
// sides invert because the direction they measure across runs the other way —
// which is what keeps each pair on one edge of the ribbon.
//
// A segment crossing the near plane has an end with no meaningful screen
// position. Its ribbon distorts, but every fragment of it is clipped away, so
// only the visible remainder is drawn.
@vertex
fn curve_vs(
    @builtin(vertex_index) corner: u32,
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) half_width: f32,
    @location(4) plane: vec3<f32>,
) -> CurveVsOut {
    let at_end = corner >= 2u;
    let position = select(start, end, at_end);
    let other = select(end, start, at_end);
    let side = select(1.0, -1.0, corner == 1u || corner == 2u);

    let here = u.view_proj * vec4<f32>(position, 1.0);
    let there = u.view_proj * vec4<f32>(other, 1.0);
    let here_ndc = ndc_from_clip(here);
    let there_ndc = ndc_from_clip(there);

    let travel = px_from_ndc_delta(there_ndc.xy - here_ndc.xy);
    let length_px = length(travel);
    var along = vec2<f32>(1.0, 0.0);
    if (length_px > MIN_RUN_PX) {
        along = travel / length_px;
    }
    let across = vec2<f32>(-along.y, along.x);

    let half_px = half_width * u.raster_scale;
    // A pixel of room past the edge for the fade to happen in. Across only:
    // the cap stays square, because it is what two segments meeting at a corner
    // overlap through, and a cap that faded would leave a seam down the join.
    let edge_px = half_px + 1.0;
    // Every vertex also steps back from its own end by half a width, which
    // squares off that cap.
    let offset_px = across * side * edge_px - along * half_px;

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
    // how far the depth moves over the offset the corner actually took.
    //
    // Carried as a *shift* from the centreline rather than a depth in its own
    // right, so that leaving it at zero passes `here.z` through untouched.
    // Recomputing the absolute value instead would divide by `w` and multiply
    // it straight back, which is a rounding step on every vertex and outright
    // nonsense on one behind the eye, where `w` is negative and the clamp below
    // is all that stands between the divide and infinity. Those vertices exist
    // to be clipped against, and the clip reads the `z` they carry.
    let offset_ndc = ndc_from_px_delta(offset_px);
    var depth_shift = 0.0;
    let plane_shift = plane_depth_shift(position, plane, here, here_ndc, offset_ndc);
    depth_shift = plane_shift.shift;
    let from_plane = plane_shift.found;

    // Without a plane only the along-segment half of the error is recoverable:
    // the segment's own depth ramp says what the cap should have, and the
    // sideways half is left to the constant bias. Both ends must be in front
    // of the eye — across the near plane one end's depth is nonsense, and
    // extrapolating from it would throw the quad out of the clip volume
    // instead of merely distorting it.
    if (!from_plane && length_px > MIN_RUN_PX && here.w > MIN_W && there.w > MIN_W) {
        depth_shift = -(there_ndc.z - here_ndc.z) * half_px / length_px;
    }

    var out: CurveVsOut;
    let widened = vec4<f32>(
        here.xy + offset_ndc * here.w,
        here.z + depth_shift * here.w,
        here.w,
    );
    out.clip = widened;
    out.color = color;
    // Which edge of the ribbon this corner is on, in a frame the whole quad
    // agrees about. `across` is measured from each end toward the other, so it
    // points opposite ways at the two ends and `side` alone names one edge at
    // the start and the other at the far end — the ramp would then run *along*
    // the ribbon rather than across it, and the stroke would come out as wide
    // as its own padding.
    out.across_px = side * select(1.0, -1.0, at_end) * edge_px;
    out.half_px = half_px;
    return out;
}

// Coverage from the distance across, rather than from how many samples the
// quad's own edges happen to catch.
//
// The quad is a pixel wider than the stroke on each side and the ramp inside it
// is what draws the edge, so what lands is a stroke of exactly the authored
// width wherever the hardware can express one — which is the same bargain the
// rim and the marker already make, and the reason all three now say it the same
// way.
@fragment
fn curve_fs(in: CurveVsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, coverage_px(in.half_px - abs(in.across_px)));
}
