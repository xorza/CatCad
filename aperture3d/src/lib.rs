//! A small retained 3D scene renderer for [`palantir`] viewports.
//!
//! Build a [`Scene`] out of [`Object`]s, [`Curve`]s and [`Point`]s, hand it to
//! a [`Renderer`], and give the renderer to a `GpuView` each frame:
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use aperture::{Curve, Mesh, Object, Point, Renderer, Scene};
//! # use glam::Vec3;
//! # use palantir::{Configure, GpuPaint, GpuView, Sizing, Ui};
//! # fn demo(ui: &mut Ui, renderer: &Rc<RefCell<Renderer>>) {
//! let mut scene = Scene::default();
//! scene.objects.push(Object::new(Mesh::cube(1.0)));
//! scene.curves.push(Curve::segment(Vec3::ZERO, Vec3::X));
//! scene.points.push(Point::new(Vec3::X));
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
//! call per pipeline per frame.
//!
//! Input is deliberately absent: palantir owns the pointer, so orbit and zoom
//! are the host's job — drive [`Camera::orbit`] and [`Camera::dolly`] from the
//! `GpuView`'s `Response`, and [`Camera::frame`] from [`Scene::bounds`].
//!
//! # Overlays
//!
//! An [`Object`] is modelled geometry: shaded, back-face culled, and measured
//! in world units. A [`Curve`] and a [`Point`] are overlays — unlit, unculled,
//! and sized in *logical pixels*, so a stroke holds its width and a marker its
//! diameter however far the camera pulls back. That fixed size is what tells
//! drawn geometry from modelled geometry: it is a claim about legibility, not
//! about the model.
//!
//! An overlay drawn on the surface it describes is in a depth fight with that
//! surface. Two fields settle it, and both kinds carry both.
//!
//! `z_offset` is a depth-test bias toward the viewer, counted in steps of
//! depth-buffer resolution. It settles a tie against a surface the overlay
//! shares a plane with — a sketch over the face it was drawn on — without
//! moving where the overlay lands on screen. It is deliberately not a way to
//! draw over everything: a solid in front still hides what is behind it.
//!
//! `plane_normal` names the plane an overlay lies in, as a unit normal, when
//! it lies in one. An overlay is widened in screen space, so its depth is its
//! anchor's held flat across the whole of that width — while the surface
//! beneath it is not flat at all. Seen at an angle the surface rises through
//! the overlay and the depth test eats whichever half it rises into, costing
//! up to half a stroke's width, or half a marker. Naming the plane lets the
//! widened corners take the surface's own depth instead, which is exact and
//! needs no bias. It is `None` for an overlay that is not planar, or is not
//! drawn on anything.
//!
//! # Picking
//!
//! Every primitive carries a `tag`: what a pick that lands on it reports, and
//! nothing else. It is opaque on purpose — whatever the caller models, a body,
//! a sketch edge, a constraint handle, a dimension line, is the caller's own
//! vocabulary, and a renderer that learned it would be a renderer that had to
//! be told about every kind of thing there is. A number it carries and never
//! reads keeps hits answerable without that.
//!
//! `None` is scenery — grids, guides, anything there to be seen and not
//! grabbed. Ask with [`Scene::pick`], which aims in pixels — [`Viewport`] is
//! how those meet the coordinates a projection works in — and answers in
//! [`Hit`]s.

pub(crate) mod aim;
pub(crate) mod bounds;
pub(crate) mod camera;
pub(crate) mod curve;
pub(crate) mod highlight;
pub(crate) mod hit;
pub(crate) mod mesh;
pub(crate) mod object;
pub(crate) mod point;
pub(crate) mod ray;
pub(crate) mod renderer;
pub(crate) mod ring;
pub(crate) mod scene;
pub(crate) mod viewport;

pub use bounds::Bounds;
pub use camera::{Camera, Projection};
pub use curve::Curve;
pub use highlight::Highlight;
pub use hit::{Hit, HitAt};
pub use mesh::{Mesh, Vertex};
pub use object::Object;
pub use point::Point;
pub use ray::Ray;
pub use renderer::Renderer;
pub use ring::Ring;
pub use scene::Scene;
pub use viewport::Viewport;
