//! One stroked segment, as the vertex buffer takes it.

use crate::curve::Curve;
use crate::renderer::record::paint::Paint;
use crate::renderer::record::{Attributed, Instance, direction_of};

/// One stroked segment, shipped once rather than four times.
///
/// The ribbon's corners are built in the vertex shader out of
/// `@builtin(vertex_index)`: which end a corner sits at and which side of the
/// line it leans to are the only things that differed between them, and both
/// follow from the index. Everything below was identical across all four.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CurveInstance {
    pub(crate) start: [f32; 3],
    pub(crate) end: [f32; 3],
    pub(crate) paint: Paint,
    /// The plane the curve lies in, as [`direction_of`] encodes it.
    pub(crate) plane: [f32; 3],
}

impl CurveInstance {
    /// The instances one stroke ships, one per segment.
    pub(crate) fn of(curve: &Curve) -> impl Iterator<Item = Self> + '_ {
        let paint = Paint::of(curve.color, curve.width);
        let plane = direction_of(curve.plane_normal);
        curve.segments().map(move |(a, b)| Self {
            start: a.to_array(),
            end: b.to_array(),
            paint,
            plane,
        })
    }
}

impl Instance for CurveInstance {
    fn paint_mut(&mut self) -> &mut Paint {
        &mut self.paint
    }
}

impl Attributed for CurveInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32x3, 2 => Float32x3,
        3 => Float32, 4 => Float32x3
    ];
}
