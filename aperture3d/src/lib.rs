//! A small retained 3D scene renderer for [`palantir`] viewports.
//!
//! Build a [`Scene`] out of [`Object`]s and [`Curve`]s, hand it to a
//! [`Renderer`], and give the renderer to a `GpuView` each frame:
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use aperture::{Curve, Mesh, Object, Renderer, Scene};
//! # use glam::Vec3;
//! # use palantir::{Configure, GpuPaint, GpuView, Sizing, Ui};
//! # fn demo(ui: &mut Ui, renderer: &Rc<RefCell<Renderer>>) {
//! let mut scene = Scene::default();
//! scene.objects.push(Object::new(Mesh::cube(1.0)));
//! scene.curves.push(Curve::segment(Vec3::ZERO, Vec3::X));
//!
//! let paint: Rc<RefCell<dyn GpuPaint>> = renderer.clone();
//! GpuView::new(paint)
//!     .auto_id()
//!     .size((Sizing::FILL, Sizing::FILL))
//!     .show(ui);
//! # }
//! ```
//!
//! The renderer owns its GPU resources and re-uploads geometry only after the
//! scene is mutated, so a still scene costs one uniform write and one draw
//! call per pipeline per frame. Meshes are shaded; curves are unlit ribbons
//! widened in screen space, so a stroke keeps its pixel width at any depth.
//!
//! Either kind carries a `z_offset`, a depth-test bias toward the viewer
//! counted in steps of depth-buffer resolution. It settles which of two
//! coplanar surfaces is drawn — a sketch over the face it was drawn on — and
//! moves nothing on screen. It is deliberately not a way to draw over
//! everything: a solid in front still hides what is behind it.
//!
//! Input is deliberately absent: palantir owns the pointer, so orbit and zoom
//! are the host's job — drive [`Camera::orbit`] and [`Camera::dolly`] from the
//! `GpuView`'s `Response`, and [`Camera::frame`] from [`Scene::bounds`].

pub(crate) mod bounds;
pub(crate) mod camera;
pub(crate) mod curve;
pub(crate) mod mesh;
pub(crate) mod object;
pub(crate) mod ray;
pub(crate) mod renderer;
pub(crate) mod scene;

pub use bounds::Bounds;
pub use camera::{Camera, Projection};
pub use curve::Curve;
pub use mesh::{Mesh, Vertex};
pub use object::Object;
pub use ray::Ray;
pub use renderer::Renderer;
pub use scene::Scene;
