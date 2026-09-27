use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Whether `check` panics, with the panic's own report kept out of the test
/// output only by the harness capturing it.
fn refuses(check: impl FnOnce()) -> bool {
    catch_unwind(AssertUnwindSafe(check)).is_err()
}

fn attribute(shader_location: u32, format: wgpu::VertexFormat) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute {
        format,
        offset: 0,
        shader_location,
    }
}

/// **A record is held to what its entry point declares, both ways round and to
/// the component.**
///
/// The three refusals are the three things wgpu lets through or leaves to a
/// validation layer: an attribute nothing declares (what the glyph record's
/// spread was), a format one component short of the input (wgpu compares base
/// types only), and an input with nothing bound (wgpu refuses that one too, so
/// this row only proves the check is symmetric). A struct input is read member
/// by member, the same as bare arguments.
#[test]
fn a_record_binds_exactly_what_its_entry_point_reads() {
    use wgpu::VertexFormat::{Float32, Float32x2, Float32x3, Uint32};
    let interface = ShaderInterface::parse(
        "
        @vertex fn bare_vs(
            @builtin(vertex_index) index: u32,
            @location(0) at: vec3<f32>,
            @location(1) spread: f32,
            @location(2) kind: u32,
        ) -> @builtin(position) vec4<f32> {
            return vec4<f32>(at, spread + f32(kind + index));
        }

        struct In {
            @location(0) at: vec3<f32>,
            @location(1) spread: f32,
        };
        @vertex fn gathered_vs(input: In) -> @builtin(position) vec4<f32> {
            return vec4<f32>(input.at, input.spread);
        }
        ",
    );
    let exact = [
        attribute(0, Float32x3),
        attribute(1, Float32),
        attribute(2, Uint32),
    ];
    interface.hold_vertex_inputs("bare_vs", &exact);
    // Order in the list is not order in the shader.
    interface.hold_vertex_inputs("bare_vs", &[exact[2], exact[0], exact[1]]);
    interface.hold_vertex_inputs("gathered_vs", &exact[..2]);

    let unread = [exact[0], exact[1], exact[2], attribute(3, Float32)];
    let short = [attribute(0, Float32x2), exact[1], exact[2]];
    let missing = [exact[0], exact[1]];
    let wrong_kind = [exact[0], exact[1], attribute(2, Float32)];
    for (case, attributes) in [
        ("unread", &unread[..]),
        ("short", &short[..]),
        ("missing", &missing[..]),
        ("wrong kind", &wrong_kind[..]),
    ] {
        assert!(
            refuses(|| interface.hold_vertex_inputs("bare_vs", attributes)),
            "{case} was let through"
        );
    }
    assert!(refuses(|| interface.hold_vertex_inputs("absent_vs", &exact)));
}

/// **A shared struct is held to the shader's layout by name, offset and span.**
///
/// The WGSL rules are what make this worth checking: a `vec3` sits on a
/// sixteen-byte boundary and a struct rounds up to one, so `view_proj` takes
/// 0–64, `origin` 64–76, the scalar after it fills 76–80, the `vec2` 80–88, and
/// the whole spans 96. Each refusal is one way a Rust struct drifts: a `vec3`
/// shipped as four floats pushes every later offset by four, a field renamed on
/// one side only, a member missing, and a struct whose tail Rust does not ship.
#[test]
fn a_shared_struct_is_laid_out_the_same_on_both_sides() {
    let interface = ShaderInterface::parse(
        "
        struct Shared {
            view_proj: mat4x4<f32>,
            origin: vec3<f32>,
            scale: f32,
            viewport: vec2<f32>,
        };
        @group(0) @binding(0) var<uniform> block: Shared;
        @vertex fn read_vs() -> @builtin(position) vec4<f32> {
            return block.view_proj * vec4<f32>(block.origin * block.scale, block.viewport.x);
        }
        ",
    );
    let member = |name, offset| StructMember { name, offset };
    let exact = [
        member("view_proj", 0),
        member("origin", 64),
        member("scale", 76),
        member("viewport", 80),
    ];
    interface.hold_struct_layout("Shared", &exact, 96);

    let widened = [
        exact[0],
        exact[1],
        member("scale", 80),
        member("viewport", 88),
    ];
    let renamed = [exact[0], member("eye", 64), exact[2], exact[3]];
    for (case, members, size) in [
        ("widened", &widened[..], 96),
        ("renamed", &renamed[..], 96),
        ("missing", &exact[..3], 96),
        ("short", &exact[..], 88),
    ] {
        assert!(
            refuses(|| interface.hold_struct_layout("Shared", members, size)),
            "{case} was let through"
        );
    }
    assert!(refuses(
        || interface.hold_struct_layout("Absent", &exact, 96)
    ));
}
