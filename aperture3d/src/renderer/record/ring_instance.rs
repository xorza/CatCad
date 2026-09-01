//! One stroked circle, as the vertex buffer takes it.

use crate::renderer::record::paint::Paint;
use crate::renderer::record::{Attributed, Instance};
use crate::ring::Ring;

/// One stroked circle, shipped once however large it is drawn.
///
/// Both in-plane axes travel so the shader can walk the rim without picking a
/// basis of its own — the only place a basis is chosen is [`Ring::new`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RingInstance {
    pub(super) center: [f32; 3],
    pub(super) x_axis: [f32; 3],
    pub(super) y_axis: [f32; 3],
    pub(super) radius: f32,
    pub(crate) paint: Paint,
}

impl RingInstance {
    pub(crate) fn of(ring: &Ring) -> Self {
        Self {
            center: ring.center.to_array(),
            x_axis: ring.x_axis.to_array(),
            y_axis: ring.y_axis.to_array(),
            radius: ring.radius,
            paint: Paint::of(ring.color, ring.width),
        }
    }
}

impl Instance for RingInstance {
    fn paint_mut(&mut self) -> &mut Paint {
        &mut self.paint
    }
}

impl Attributed for RingInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32,
        4 => Float32x3, 5 => Float32
    ];
}
