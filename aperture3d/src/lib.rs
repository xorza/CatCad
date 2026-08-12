//! A small retained 3D scene renderer for [`palantir`] viewports.
//!
//! Build a [`Scene`] out of [`Object`]s, hand it to a [`Renderer`], and give
//! the renderer to a `GpuView` each frame:
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use aperture::{Mesh, Object, Renderer, Scene};
//! # use palantir::{Configure, GpuPaint, GpuView, Sizing, Ui};
//! # fn demo(ui: &mut Ui, renderer: &Rc<RefCell<Renderer>>) {
//! let mut scene = Scene::default();
//! scene.objects.push(Object::new(Mesh::cube(1.0)));
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
//! call per frame. Input is deliberately absent: palantir owns the pointer, so
//! orbit and zoom are the host's job — drive [`Camera::orbit`] and
//! [`Camera::dolly`] from the `GpuView`'s `Response`.

pub(crate) mod camera;
pub(crate) mod mesh;
pub(crate) mod object;
pub(crate) mod renderer;
pub(crate) mod scene;

pub use camera::Camera;
pub use mesh::{Mesh, Vertex};
pub use object::Object;
pub use renderer::Renderer;
pub use scene::Scene;
