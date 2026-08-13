//! What the off-screen target is, which two files have to agree about.
//!
//! Not settings so much as a contract. `gpu` builds the textures a frame is
//! drawn into and `pass` builds the pipelines that draw into them, and wgpu
//! rejects a pipeline whose sample count or depth format disagrees with the
//! attachment it is used against. Stating them here rather than in either file
//! is what stops one of the two reading as the owner and the other as a
//! borrower, when neither is.

/// The depth buffer's format. Reversed depth is the convention throughout —
/// cleared to zero at the far end — which a float format is what makes worth
/// having.
pub(super) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Samples per pixel. Ribbons a pixel and a half wide are most of what this
/// draws, and their edges are what multisampling is for. WebGPU guarantees 4×
/// on every renderable format, so there is no fallback path to carry.
pub(super) const SAMPLES: u32 = 4;
