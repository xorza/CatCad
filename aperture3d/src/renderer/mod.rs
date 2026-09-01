//! The wgpu half: flattens each pane's scene into one world-space triangle list
//! and eight instanced overlay buffers — an ordinary and a highlighted one for
//! each of the four overlay kinds — and draws them into the off-screen target
//! palantir hands over each frame.
//!
//! Meshes ship a vertex apiece; a stroke, a rim or a marker ships once and the
//! vertex shader builds its four corners, since the corners differed only in
//! ways the index already says. Text ships one of those per *glyph*, because
//! that is what a run comes to once the shaper has placed it.

pub(crate) mod atlas;
pub(crate) mod band;
pub(crate) mod cpu;
pub(crate) mod glyph_quad;
pub(crate) mod gpu;
pub(crate) mod held;
pub(crate) mod highlights;
pub(crate) mod mirror;
pub(crate) mod pane;
pub(crate) mod pass;
pub(crate) mod pipelines;
pub(crate) mod record;
pub(crate) mod retained;
pub(crate) mod target;
pub(crate) mod tile;
pub(crate) mod uniforms;

use crate::highlight::Lit;
use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::gpu::Gpu;
use crate::renderer::mirror::Mirror;
use crate::renderer::pane::Pane;
use crate::renderer::uniforms::{Frame, Uniforms};
use glam::Vec3;
use palantir::{GpuFrameCtx, GpuInitCtx, GpuPaint, TextShaper};

/// What a view is cleared to when its owner says nothing.
///
/// Near black and flat, so a drawing reads against it and a window whose own
/// clear is a different black meets it without a seam. An application with a
/// theme states its own — see [`Renderer::set_ground`] — and nothing here is in
/// a position to know what that is.
const GROUND: Vec3 = Vec3::splat(0.02);

/// A list of panes, and the mirrors of them that get drawn.
///
/// **One pane is a viewport.** Several are a viewport with furniture over it —
/// an orientation gizmo, an axis triad, a thumbnail — each a scene of its own
/// seen from a camera of its own, and all of them drawn in one pass through one
/// set of pipelines. See [`Pane`].
///
/// Nothing is uploaded when a scene is edited. An edit marks only what it
/// touched, and the next paint flattens and uploads only that — which is what
/// lets a drag that moves one marker leave every triangle in the model alone.
#[derive(Debug)]
pub struct Renderer {
    /// What is drawn, back to front — so the last pane is the one a pointer
    /// over both is over.
    panes: Vec<Pane>,
    /// What each pane comes to once it is flattened: one per pane, in the same
    /// order, and never reachable except through that pairing.
    ///
    /// Apart from the panes rather than inside them, on the terms
    /// [`Mirror`] states: one half is written by the caller and the other is
    /// derived from it, and a type that held both would be a type half of which
    /// a caller must not touch.
    mirrors: Vec<Mirror>,
    /// What the view is cleared to behind every pane.
    ///
    /// The renderer's rather than a pane's: panes overlap, so a clear of one
    /// would wipe the pane behind it. A pane that wants a backdrop of its own
    /// draws one, which is a face in its own scene.
    ground: Vec3,
    /// The coverage every glyph drawn is cut from.
    ///
    /// The renderer's rather than a mirror's, because a sheet is keyed by glyph
    /// and size rather than by scene: two panes that write the same word at the
    /// same size read the one entry, and a copy apiece would be a second upload
    /// of the same pixels.
    ///
    /// Beside the [`Gpu`]'s texture rather than in it for the opposite reason —
    /// this side is packed before a device exists, and that side cannot be.
    atlas: GlyphAtlas,
    gpu: Option<Gpu>,
    /// The window's own shaper, taken at [`GpuPaint::init`].
    ///
    /// Held rather than asked for per frame because it is a handle — cloning one
    /// is a refcount, and what it points at is the font stack the rest of the
    /// window is already drawing with, so a label in the scene comes out in the
    /// same faces as the UI around it.
    ///
    /// `None` only before the first paint. Nothing reads it earlier: laying text
    /// out is part of flattening a scene, which is what a paint does.
    ///
    /// Named for what it holds rather than for what it is for, so that it does
    /// not read as the [`text`](crate::text) module it is used beside.
    shaper: Option<TextShaper>,
}

impl Renderer {
    /// A renderer showing `pane`. No GPU work happens until palantir first
    /// paints the view.
    pub fn new(pane: Pane) -> Self {
        Self {
            panes: vec![pane],
            mirrors: vec![Mirror::default()],
            ground: GROUND,
            atlas: GlyphAtlas::default(),
            gpu: None,
            shaper: None,
        }
    }

    /// Show `pane` over everything already shown, and answer where it went.
    ///
    /// In front, because that is what a caller adding furniture to a view
    /// means. A pane that belongs behind is pushed first.
    pub fn push_pane(&mut self, pane: Pane) -> usize {
        self.panes.push(pane);
        self.mirrors.push(Mirror::default());
        self.panes.len() - 1
    }

    /// Clear the view to `ground`, in linear RGB.
    ///
    /// Takes effect on the next paint and invalidates nothing: a clear is one
    /// operation at the head of the pass rather than anything a scene was
    /// flattened into.
    pub fn set_ground(&mut self, ground: Vec3) {
        self.ground = ground;
    }

    /// The `nth` pane, by where [`Renderer::push_pane`] put it.
    pub fn pane(&self, nth: usize) -> &Pane {
        &self.panes[nth]
    }

    /// Edit the `nth` pane: its scene, its camera, or where it lands.
    ///
    /// One door rather than one per thing, because each [`Batch`](crate::Batch)
    /// says for itself what was written to it — so a caller handed the whole
    /// pane and moving one marker pays for one marker, and nothing here has to
    /// guess from which accessor was called what the caller was about to do.
    ///
    /// One pane rather than the list, which is the whole of what it withholds:
    /// a pane and its mirror are added together, and [`Renderer::push_pane`] is
    /// the only place that can be true.
    pub fn pane_mut(&mut self, nth: usize) -> &mut Pane {
        &mut self.panes[nth]
    }

    /// Light `lit` in the `nth` pane and nothing else, dropping whatever was
    /// lit there before.
    ///
    /// [`Renderer::highlight_all`] of a set with one thing in it, and here only
    /// to spare the slice around a single value — an answer that *is* one thing
    /// is the common one. Lighting none of them has no such wrapper: the empty
    /// set says it without hiding that clearing and setting are one door.
    pub fn highlight_only(&mut self, nth: usize, lit: Lit) {
        self.highlight_all(nth, &[lit]);
    }

    /// Light exactly these in the `nth` pane, dropping whatever was lit there
    /// before.
    ///
    /// [`Renderer::highlight_only`] for an answer that is a *set* — a selection
    /// alongside the one thing under the pointer — where the caller knows the
    /// whole of it every frame and would otherwise be adding and removing
    /// entries to arrive back at what it already has.
    ///
    /// Per pane, because a highlight keys on a [`Tag`](crate::Tag) and a tag
    /// names something in *a* scene: two panes lighting one tag would be two
    /// answers to where the pointer is.
    ///
    /// Where two entries name one tag the first wins, so a caller that wants
    /// one look to beat another puts it first. The empty set is how a pointer
    /// over nothing puts everything out.
    ///
    /// Compared before it is written, so re-asking for the set already in force
    /// dirties nothing — which is what lets this be called unconditionally
    /// every frame, and is the whole reason it takes the set rather than a
    /// clear followed by a run of single additions.
    pub fn highlight_all(&mut self, nth: usize, lit: &[Lit]) {
        self.mirrors[nth].highlight_all(lit);
    }

    /// Bring every mirror up to date with the pane it mirrors.
    ///
    /// A door of its own because it is what a paint does first, and what an
    /// allocation gate over flattening reaches with no device in front of it.
    /// What it decides is only which mirror answers for which pane — see
    /// [`Mirror::refresh`], where the rest is.
    fn refresh(&mut self, raster_scale: f32) {
        let Self {
            panes,
            mirrors,
            atlas,
            shaper,
            ..
        } = self;
        for (pane, mirror) in panes.iter_mut().zip(mirrors.iter_mut()) {
            mirror.refresh(
                &mut pane.scene,
                &pane.camera,
                atlas,
                shaper.as_ref(),
                raster_scale,
            );
        }
    }
}

impl GpuPaint for Renderer {
    fn init(&mut self, ctx: &GpuInitCtx<'_>) {
        // Re-runs whenever palantir reclaims the view's target. The pipelines
        // and the uploaded buffers both outlive that, so build once.
        if self.gpu.is_none() {
            self.gpu = Some(Gpu::new(ctx.device, ctx.target_format, &self.atlas));
        }
        // Re-taken rather than guarded, unlike the pipelines above: it is a
        // clone of a handle, and a view whose target was reclaimed should come
        // back holding whatever shaper the window has now.
        self.shaper = Some(ctx.text.clone());
    }

    /// Work out what shape this frame is, bring every mirror up to date with
    /// its pane, hand the device what it is owed, and draw — each of them
    /// somebody else's whole job, and none of them decided here.
    ///
    /// Which kinds are uploaded and which are drawn belongs to the mirror,
    /// where the two lists sit one after the other beside the fields they walk.
    /// They have to agree, and this is not a place that could show whether they
    /// do.
    fn paint(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        let frame = Frame::of(ctx);
        // Refilled before the GPU is borrowed, since both want `self`. Each kind
        // answers for itself, so a hover over a marker leaves the highlights of
        // the strokes and rims it passed over alone.
        self.refresh(frame.raster_scale);
        // Split so the panes, their mirrors and the device are borrowed apart:
        // an upload reads a pane and the shared half while writing the mirror
        // between them.
        let Self {
            panes,
            mirrors,
            atlas,
            gpu,
            ground,
            ..
        } = self;
        let gpu = gpu.as_mut().expect("init runs before paint");
        // The sheet before the records that read it: a restart replaces the
        // texture, and the records name places on the new one.
        let restarted = gpu.upload_sheet(ctx.device, ctx.queue, atlas);
        for (pane, mirror) in panes.iter().zip(mirrors.iter_mut()) {
            let uniforms = Uniforms::of(&pane.camera, frame, frame.tile(pane.placement));
            mirror.upload(ctx.device, ctx.queue, gpu, &uniforms, restarted);
        }
        // The size is spent here and nowhere else: the draw is confined to
        // whatever this built.
        gpu.resize(ctx.device, frame.target.size);
        let size = frame.target.size.as_vec2();
        // **One pass, whatever the panes**, which is what keeps the
        // multisampled buffer discarded rather than stored: a pass apiece would
        // have to load what the one before it left, and loading means storing.
        let mut pass = gpu.begin(ctx.encoder, ctx.target, *ground);
        // How much of the depth range one pane gets, which is all of it while
        // there is one.
        let slice = 1.0 / panes.len() as f32;
        for (nth, (pane, mirror)) in panes.iter().zip(mirrors.iter()).enumerate() {
            // Nothing of this pane is on the target — a scroll can slide the
            // view until that is so. Skipped rather than scissored to nothing,
            // wgpu refusing a rect that leaves the attachment.
            let Some(cut) = frame.tile(pane.placement).within(frame.target) else {
                continue;
            };
            // **A slice of the depth range each, so a pane in front of another
            // is in front of every part of it.** The rect stays the whole
            // target and the scissor does the confining, because a viewport is
            // refused outside the attachment where a scissor merely clips.
            //
            // Depth is reversed — the near plane writes 1 — and the panes are
            // held back to front, so the last of them takes the high end.
            // `Depth32Float` split three ways still leaves more resolution per
            // pane than a 24-bit buffer has in total.
            pass.set_viewport(
                0.0,
                0.0,
                size.x,
                size.y,
                nth as f32 * slice,
                (nth + 1) as f32 * slice,
            );
            cut.scissor(&mut pass);
            mirror.draw(&mut pass);
        }
    }
}

#[cfg(test)]
mod shaping {
    use crate::renderer::Renderer;
    use palantir::TextShaper;

    impl Renderer {
        /// The shaper [`GpuPaint::init`](palantir::GpuPaint::init) would hand
        /// over, for a test laying text out without a device to hand one over.
        pub(crate) fn shape_with(&mut self, shaper: TextShaper) {
            self.shaper = Some(shaper);
        }
    }
}

/// What a harness painting whole frames needs, and an application never does.
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::renderer::Renderer;
    use palantir::{App, Configure, GpuView, Sizing, Ui, WindowToken};
    use std::cell::RefCell;
    use std::rc::Rc;

    impl Renderer {
        /// Re-flatten whatever is marked, with no frame to ask for one.
        ///
        /// What [`Renderer::paint`] does first of all, reached on its own —
        /// which is what an allocation gate over flattening wants, there being
        /// no device in front of it.
        pub fn flatten(&mut self, raster_scale: f32) {
            self.refresh(raster_scale);
        }
    }

    /// An application that shows one renderer and does nothing else.
    ///
    /// A [`Renderer`] is a [`GpuPaint`] rather than an [`App`], so painting one
    /// at all needs something to show a [`GpuView`] from — this is the least of
    /// that.
    #[derive(Debug)]
    pub struct SceneApp {
        pub view: Rc<RefCell<Renderer>>,
    }

    impl App for SceneApp {
        fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
            GpuView::new(&self.view)
                .auto_id()
                .size((Sizing::FILL, Sizing::FILL))
                .show(ui);
        }
    }
}

#[cfg(test)]
mod tests;
