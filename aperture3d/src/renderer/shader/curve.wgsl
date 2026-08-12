// Stroked polylines, widened to ribbons in screen space.

struct CurveVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
};

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
    @location(3) side: f32,
    @location(4) half_width: f32,
    @location(5) z_offset: f32,
    @location(6) plane: vec3<f32>,
) -> CurveVsOut {
    let here = u.view_proj * vec4<f32>(position, 1.0);
    let there = u.view_proj * vec4<f32>(other, 1.0);
    let here_ndc = here.xyz / max(here.w, DEGENERATE);
    let there_ndc = there.xyz / max(there.w, DEGENERATE);

    // NDC spans two units over the whole target, hence the halved viewport.
    let travel = (there_ndc.xy - here_ndc.xy) * u.viewport * 0.5;
    let length_px = length(travel);
    var along = vec2<f32>(1.0, 0.0);
    if (length_px > DEGENERATE) {
        along = travel / length_px;
    }
    let across = vec2<f32>(-along.y, along.x);

    let half_px = half_width * u.raster_scale;
    // Every vertex also steps back from its own end by half a width, which
    // squares off the cap. Two segments meeting at a corner then overlap
    // instead of leaving a notch between them.
    let offset_px = (across * side - along) * half_px;

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
    let offset_ndc = offset_px * 2.0 / u.viewport;
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
    if (!from_plane && length_px > DEGENERATE && here.w > DEGENERATE && there.w > DEGENERATE) {
        depth_shift = -(there_ndc.z - here_ndc.z) * half_px / length_px;
    }

    var out: CurveVsOut;
    let widened = vec4<f32>(
        here.xy + offset_ndc * here.w,
        here.z + depth_shift * here.w,
        here.w,
    );
    out.clip = lift(widened, z_offset);
    out.color = color;
    return out;
}

@fragment
fn curve_fs(in: CurveVsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
