//! A small retained 3D scene renderer for [`palantir`] viewports.
//!
//! Build a [`Scene`] out of [`Object`]s, [`Curve`]s, [`Ring`]s, [`Point`]s and
//! [`Text`], hand it to a [`Renderer`], and give the renderer to a `GpuView`
//! each frame:
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use aperture::{Curve, Mesh, Object, Point, Renderer, Scene, Text};
//! # use glam::Vec3;
//! # use palantir::{Configure, GpuPaint, GpuView, Sizing, Ui};
//! # fn demo(ui: &mut Ui, renderer: &Rc<RefCell<Renderer>>) {
//! let mut scene = Scene::default();
//! scene.objects.push(Object::new(Mesh::cube(1.0)));
//! scene.curves.push(Curve::segment(Vec3::ZERO, Vec3::X));
//! scene.points.push(Point::new(Vec3::X));
//! scene.texts.push(Text::new(Vec3::X, "1.0", 12.0));
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
//! in world units. A [`Curve`], a [`Ring`], a [`Point`] and a [`Text`] are
//! overlays — unlit, unculled, and sized in *logical pixels*, so a stroke holds
//! its width, a marker its diameter, and a label its type size however far the
//! camera pulls back. That fixed size is what tells drawn geometry from modelled
//! geometry: it is a claim about legibility, not about the model.
//!
//! An overlay drawn on the surface it describes is in a depth fight with that
//! surface, and two fields on the overlay settle it. `z_offset` biases the
//! depth test toward the viewer without moving where the overlay lands on
//! screen; `plane_normal` gives its screen-widened corners the surface's own
//! depth rather than the anchor's, which is exact and needs no bias. Each is
//! documented where it is declared.
//!
//! # Picking
//!
//! Every primitive carries an optional [`Tag`]: what a pick that lands on it
//! reports, and nothing else. `None` is scenery — grids, guides, anything
//! there to be seen and not grabbed. Ask with [`Scene::nearest`], which aims in
//! pixels — [`Viewport`] is how those meet the coordinates a projection works
//! in — and answers in [`Hit`]s.
//!
//! Naming one is [`Styled::tagged`], which every primitive gets from
//! [`Styled`] along with [`colored`](Styled::colored) — so the trait has to be
//! in scope to reach either. What a tag names can then be drawn differently
//! with [`Renderer::highlight_all`], which takes [`Lit`]s.

pub(crate) mod aim;
pub(crate) mod batch;
pub(crate) mod bounds;
pub(crate) mod camera;
pub(crate) mod curve;
pub(crate) mod highlight;
pub(crate) mod hit;
pub(crate) mod mesh;
pub(crate) mod motion;
pub(crate) mod object;
pub(crate) mod point;
pub(crate) mod primitive;
pub(crate) mod ray;
pub(crate) mod renderer;
pub(crate) mod ring;
pub(crate) mod scene;
pub(crate) mod styled;
pub(crate) mod tag;
pub(crate) mod text;
pub(crate) mod viewport;

/// The one call `tests/alloc.rs` makes. The driver itself stays in `src/`,
/// where it can reach what it measures.
#[cfg(feature = "bench")]
pub use renderer::bench::alloc_bench;

pub use aim::Aim;
pub use batch::Batch;
pub use bounds::Bounds;
pub use camera::{Camera, Projection};
pub use curve::Curve;
pub use highlight::{Highlight, Lit};
pub use hit::{Hit, HitAt};
pub use mesh::{Mesh, Vertex};
pub use motion::Motion;
pub use object::Object;
pub use point::Point;
pub use ray::Ray;
pub use renderer::Renderer;
pub use ring::Ring;
pub use scene::Scene;
pub use styled::Styled;
pub use tag::Tag;
pub use text::Text;
pub use viewport::Viewport;
