struct Uniforms {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
) -> VsOut {
    var out: VsOut;
    out.clip = u.view_proj * vec4<f32>(position, 1.0);
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
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let key = max(dot(n, normalize(KEY_DIR)), 0.0);
    let ambient = mix(GROUND, SKY, n.y * 0.5 + 0.5);
    return vec4<f32>(in.color * (ambient + key), 1.0);
}
