//! A small retained 3D scene renderer for [`palantir`] viewports.
//!
//! Build a [`Scene`] out of [`Object`]s, [`Curve`]s, [`Ring`]s, [`Point`]s
//! and [`Text`], put it in a [`Pane`], hand that to a [`Renderer`], and give
//! the renderer to a `GpuView` each frame:
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use aperture::{Curve, Mesh, Object, Pane, Placement, Point, Renderer, Scene, Text};
//! # use glam::Vec3;
//! # use palantir::{Configure, GpuPaint, GpuView, Sizing, Ui};
//! # fn demo(ui: &mut Ui, renderer: &Rc<RefCell<Renderer>>) {
//! let mut scene = Scene::default();
//! scene.solids.push(Object::new(Mesh::cube(1.0)));
//! scene.curves.push(Curve::segment(Vec3::ZERO, Vec3::X));
//! scene.points.push(Point::new(Vec3::X));
//! scene.texts.push(Text::new(Vec3::X, "1.0", 12.0));
//! let mut view = Renderer::new(Pane::new(scene, Placement::Fill));
//!
//! let paint: Rc<RefCell<dyn GpuPaint>> = renderer.clone();
//! GpuView::new(paint)
//!     .auto_id()
//!     .size((Sizing::FILL, Sizing::FILL))
//!     .show(ui);
//! # }
//! ```
//!
//! The renderer owns its GPU resources and re-uploads geometry only after a
//! scene is mutated, so a still scene costs one uniform write and one draw
//! call per pipeline per frame.
//!
//! # Panes
//!
//! A [`Pane`] is one scene, seen from one camera, landed in one rect of the
//! target — see [`Placement`]. One pane is a viewport. Several are a viewport
//! with furniture over it: an orientation gizmo, an axis triad, a thumbnail,
//! each with a scene and a camera of its own. They are drawn back to front in
//! one pass, through one set of pipelines and one glyph sheet, and each takes
//! its own slice of the depth range — so nothing in one pane can occlude or be
//! occluded by anything in another.
//!
//! Input is deliberately absent: palantir owns the pointer, so orbit and zoom
//! are the host's job — drive [`Camera::orbit`] and [`Camera::dolly`] from the
//! `GpuView`'s `Response`, and [`Camera::frame`] from [`Scene::extent`]. This
//! crate answers questions about the scene and takes no events, which is also
//! why nothing in it is *edited* through the pointer: something a user works in
//! rather than looks at wants palantir's own routing — focus, scopes, who takes
//! a press — and belongs in a widget over the viewport, placed with
//! [`Camera::screen_of`].
//!
//! # Overlays
//!
//! An [`Object`] in [`Scene::solids`] is modelled geometry: shaded, back-face
//! culled, and measured in world units. A [`Curve`], a [`Ring`], a [`Point`] and
//! a [`Text`] are overlays — unlit, unculled, and sized in *logical pixels*, so
//! a stroke holds its width, a marker its diameter and a label its type size
//! however far the camera pulls back. That fixed size is what tells drawn
//! geometry from modelled geometry: it is a claim about legibility, not about
//! the model.
//!
//! An `Object` in [`Scene::faces`] is neither, and is the same type drawn by
//! different rules: a flat sheet lying *in* a drawing rather than standing in
//! the world. It is measured in world units like a model and shaded like one,
//! but it is two-sided — a sheet has no outside to be culled from — and its
//! whole pass is biased toward the camera, because it lies in the very plane
//! whatever it is drawn on does. Which batch it is in is the whole of what
//! decides this; nothing on the object itself says so.
//!
//! An overlay drawn on the surface it describes is in a depth fight with that
//! surface, and two things settle it. Which *layer* reads over which is the
//! renderer's, not the caller's: solids, then faces, then strokes and rims, then
//! markers and type, each pass biased toward the viewer by a fixed step, because
//! that order is this crate's own and an application choosing its own numbers
//! would be restating a layering it does not control. What is left to the
//! primitive is `plane_normal`, which gives its screen-widened corners the
//! surface's own depth rather than the anchor's — a shape question rather than a
//! layering one, and documented where it is declared. A run of text names its
//! surface through [`Facing`] instead, because it can also be *turned* into one,
//! and a normal alone cannot say which way round.
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
//!
//! Where the cursor is over several at once, the answer is settled first by
//! [`Precedence`] — what a primitive is *for*, which only whoever drew it knows
//! — and only then by what shape it is and how near it fell. Saying so is
//! [`Styled::precedence`], the third of that trait's setters, and leaving it
//! unsaid enters a primitive on shape alone.

pub(crate) mod aim;
pub(crate) mod batch;
pub(crate) mod camera;
pub(crate) mod curve;
pub(crate) mod extent;
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

/// What a harness painting whole frames needs, and an application never does.
#[cfg(any(test, feature = "internals"))]
pub mod internals {
    pub use crate::renderer::internals::SceneApp;
}

pub use aim::Aim;
pub use batch::Batch;
pub use camera::{Camera, Projection};
pub use curve::Curve;
pub use extent::Extent;
pub use highlight::{Highlight, Lit, Tint};
pub use hit::{Hit, HitAt, Precedence};
pub use mesh::bounds::Bounds;
pub use mesh::{Mesh, Vertex};
pub use motion::Motion;
pub use object::Object;
pub use point::Point;
pub use ray::Ray;
pub use renderer::Renderer;
pub use renderer::pane::{Pane, Placement};
pub use ring::Ring;
pub use scene::Scene;
pub use styled::Styled;
pub use tag::Tag;
pub use text::Text;
pub use text::turn::{Facing, Turn};
pub use viewport::Viewport;
