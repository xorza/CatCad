// Shaded triangles: the modelled geometry.

// How many steps of depth resolution to bring this pass toward the viewer, for
// geometry that is *exactly* coplanar with something else and cannot be settled
// by ordering the passes — a sketch face lies in the very plane the ground
// slab's top does.
//
// Per pipeline rather than per vertex, because what needs it is a whole layer
// of the drawing and not a primitive of one; the overlays ask the same question
// of their own `z_offset`, which travels in the record. Handed over by the
// pipeline for the same reason `RING_STEPS` is — see `OVERRIDES`.
override MESH_LIFT: f32;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn mesh_vs(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
) -> VsOut {
    var out: VsOut;
    out.clip = lift(u.view_proj * vec4<f32>(position, 1.0), MESH_LIFT);
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
fn mesh_fs(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let key = max(dot(n, normalize(KEY_DIR)), 0.0);
    let ambient = mix(GROUND, SKY, n.y * 0.5 + 0.5);
    return vec4<f32>(in.color * (ambient + key), 1.0);
}
