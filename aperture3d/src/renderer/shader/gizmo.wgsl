// Flat controls: world geometry drawn in the colour it was handed.

struct GizmoVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
};

// The whole of it. A gizmo's shape is cut on the CPU in the plane it lies in —
// see `Scene::gizmos` — so by the time it reaches here there is nothing left to
// work out but where the camera puts it.
//
// The normal at location 1 is not read. It is in the vertex all the same,
// because a gizmo is a [`GpuVertex`] like a solid and a face, and one triangle
// list serving three passes is what lets a control be the same type drawn by
// different rules. Twelve bytes on a shape of seven vertices is not worth a
// second vertex layout and the second `Triangles` that would come with it.
@vertex
fn gizmo_vs(
    @location(0) position: vec3<f32>,
    @location(2) color: vec3<f32>,
) -> GizmoVsOut {
    var out: GizmoVsOut;
    out.clip = u.view_proj * vec4<f32>(position, 1.0);
    out.color = color;
    return out;
}

// Unlit, which is the point of the pass. A control says what it is by its
// colour, and one shaded by `mesh_fs` would say it in a different colour on
// every plane — the key light's own, times whichever way the plane happens to
// face. It is furniture on the drawing rather than something in the world for
// the light to find.
@fragment
fn gizmo_fs(in: GizmoVsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
