//! One marker, as the vertex buffer takes it.

use crate::point::Point;
use crate::renderer::record::paint::Paint;
use crate::renderer::record::{Attributed, Instance, direction_of};

/// One marker, shipped once. Its quad spans `±1` either way, and the two low
/// bits of `@builtin(vertex_index)` pick a corner, so none travels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PointInstance {
    pub(super) position: [f32; 3],
    pub(super) paint: Paint,
    /// The plane the marker sits on, as [`direction_of`] encodes it.
    pub(super) plane: [f32; 3],
}

impl PointInstance {
    pub(crate) fn of(point: &Point) -> Self {
        Self {
            position: point.position.to_array(),
            paint: Paint::of(point.color, point.size),
            plane: direction_of(point.plane_normal),
        }
    }
}

impl Instance for PointInstance {
    fn paint_mut(&mut self) -> &mut Paint {
        &mut self.paint
    }
}

impl Attributed for PointInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32, 3 => Float32x3
    ];
}
