//! The wgpu half: flattens a scene into one world-space triangle list and eight
//! instanced overlay buffers — an ordinary and a highlighted one for each of the
//! four overlay kinds — and draws them into the off-screen target palantir hands
//! over each frame.
//!
//! Meshes ship a vertex apiece; a stroke, a rim or a marker ships once and the
//! vertex shader builds its four corners, since the corners differed only in
//! ways the index already says. Text ships one of those per *glyph*, because
//! that is what a run comes to once the shaper has placed it.

use crate::camera::Camera;
use crate::highlight::{Highlights, Lit};
use crate::scene::Scene;
use crate::viewport::Viewport;
use glam::UVec2;
use palantir::{GpuFrameCtx, GpuInitCtx, GpuPaint, TextShaper};

pub(crate) mod atlas;
pub(crate) mod band;
#[cfg(feature = "bench")]
pub(crate) mod bench;
pub(crate) mod cpu;
pub(crate) mod gpu;
pub(crate) mod pass;
pub(crate) mod record;
pub(crate) mod retained;
pub(crate) mod target;
pub(crate) mod uniforms;

use crate::renderer::cpu::{Cpu, Laying, Order};
use crate::renderer::gpu::{Attachments, Gpu};
use crate::renderer::uniforms::{Uniforms, Window};

/// A scene, where it is looked at from, and the two mirrors of it that get
/// drawn.
///
/// Nothing is uploaded when it is edited. An edit marks only what it touched,
/// and the next paint flattens and uploads only that — which is what lets a drag
/// that moves one marker leave every triangle in the model alone.
#[derive(Debug)]
pub struct Renderer {
    scene: Scene,
    /// Where the scene is drawn from.
    ///
    /// Beside the scene rather than in it: a viewpoint is not part of what
    /// there is, and holding one inside the scene let a caller pick through it
    /// without ever naming it.
    camera: Camera,
    /// At most one entry per tag, which is what lets a lookup stop at the
    /// first match rather than having to find the last.
    highlights: Highlights,
    /// Whether *which* primitives are lit has changed since the last refresh.
    ///
    /// The renderer's own flag rather than one of `cpu`'s, because it is the one
    /// edit that leaves every batch in the scene untouched: a pointer crossing a
    /// drawing relights it without moving anything in it.
    relight: bool,
    cpu: Cpu,
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
    /// A renderer for `scene`. No GPU work happens until palantir first paints
    /// the view.
    pub fn new(scene: Scene) -> Self {
        Self {
            scene,
            camera: Camera::default(),
            highlights: Highlights::default(),
            relight: true,
            cpu: Cpu::default(),
            gpu: None,
            shaper: None,
        }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Move the camera. Cheap — no geometry is re-uploaded.
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Edit the scene, re-uploading on the next paint whatever was touched.
    ///
    /// One door rather than one per batch, because each [`Batch`](crate::Batch)
    /// says for itself what was written to it — so a caller handed the whole scene and
    /// moving one marker pays for one marker, and nothing here has to guess from
    /// which accessor was called what the caller was about to do.
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// Light `lit` and nothing else, dropping whatever was lit before.
    ///
    /// What a hover wants, where the answer is one thing and the previous
    /// answer is of no interest. A call that changes nothing dirties nothing,
    /// as with [`Renderer::highlight_all`] below it.
    pub fn highlight_only(&mut self, lit: Lit) {
        self.highlight_all(&[lit]);
    }

    /// Light exactly these, dropping whatever was lit before.
    ///
    /// [`Renderer::highlight_only`] for an answer that is a *set* — a selection
    /// alongside the one thing under the pointer — where the caller knows the
    /// whole of it every frame and would otherwise be adding and removing
    /// entries to arrive back at what it already has.
    ///
    /// Where two entries name one tag the first wins, so a caller that wants
    /// one look to beat another puts it first.
    ///
    /// Compared before it is written, so re-asking for the set already in force
    /// dirties nothing — which is what lets this be called unconditionally
    /// every frame, and is the whole reason it takes the set rather than a
    /// clear followed by a run of single additions.
    pub fn highlight_all(&mut self, lit: &[Lit]) {
        self.relight |= self.highlights.set_all(lit);
    }

    /// Drop every highlight, leaving the scene drawn as nothing but itself.
    ///
    /// The other half of what a hover asks for — a pointer over nothing has no
    /// [`Lit`] to name — and free in the same way when there was nothing lit to
    /// drop.
    pub fn clear_highlights(&mut self) {
        self.relight |= self.highlights.clear();
    }

    /// Bring the CPU mirror up to date with the scene, and the scene's own
    /// measurements up to date with the shaper.
    ///
    /// Answers nothing, and nothing here has to. Each batch says whether it was
    /// written to and each buffer flattened from one says whether it was
    /// rewritten, so what the GPU is owed is asked at the point it is acted on
    /// rather than carried there.
    ///
    /// `relight` is the one thing no buffer can answer for: which primitives are
    /// lit can change with the scene untouched, which is what a pointer crossing
    /// a drawing does. Taken here rather than passed in, on the same terms as
    /// every batch's own mark — a flag read twice is a flag that relights twice.
    ///
    /// The scene is borrowed mutably to be *read*: taking a batch's mark is what
    /// clears it, and a mark left behind would re-flatten the same list every
    /// frame for the rest of the run.
    ///
    /// Text is the one thing written back into the scene rather than mirrored
    /// out of it. How far a run reaches is the shaper's answer and picking needs
    /// it, so it is remembered on the run — and a field remembers where every
    /// caret position along it falls, which is what a click that places the
    /// caret reads. Both go through a memo rather than the batch, which is what
    /// keeps recording them from reading as an edit. See
    /// [`Text::extent`](crate::Text::extent).
    fn refresh(&mut self, raster_scale: f32) {
        let Self {
            scene,
            highlights,
            relight,
            cpu,
            shaper,
            camera,
            ..
        } = self;
        let relight = std::mem::take(relight);
        cpu.solids
            .refresh(&mut scene.solids, highlights, relight, Order::Given);
        // Whatever the faces' own opacity asks for — see [`Gpu::faces_order`].
        // The camera is this frame's rather than the last: a scene sorted
        // against the camera it was drawn through a frame ago would lag a drag
        // by one.
        cpu.faces.refresh(
            &mut scene.faces,
            highlights,
            relight,
            Gpu::faces_order(camera.eye()),
        );
        // Opaque, so the depth test settles what covers what and there is
        // nothing for an order to buy.
        cpu.gizmos
            .refresh(&mut scene.gizmos, highlights, relight, Order::Given);
        cpu.curves
            .refill_from(&mut scene.curves, highlights, relight);
        cpu.rings.refill_from(&mut scene.rings, highlights, relight);
        cpu.points
            .refill_from(&mut scene.points, highlights, relight);
        // Nothing to lay out, and nothing left over from when there was. The
        // first half is what lets a scene with no text in it be flattened
        // without a window having handed a font stack over.
        //
        // The second half is what a batch emptied *after* it was drawn needs.
        // Returning on the batch alone leaves the records — and the buffers
        // behind them — holding glyphs nobody asked for any more, and they go on
        // being drawn for the rest of the run. Records outliving what they were
        // flattened from is the one failure a retained renderer has to answer
        // for, and emptying is the only way to reach it.
        if scene.texts.is_empty() && cpu.texts.is_empty() {
            // Taken on the way out, because a mark is a claim that someone
            // wrote to the batch and this is the one path that answers that
            // claim by doing nothing. Left standing it would outlive the edit
            // it stands for — a batch refilled to empty while no text is drawn
            // would still be reporting that write on whatever later frame first
            // had a run to lay out.
            scene.texts.take_dirty();
            return;
        }
        let shaper = shaper
            .as_ref()
            .expect("laying text out needs the shaper `init` is handed");
        // One lease for the whole scene's text. Measuring and placing are two
        // halves of laying a run out and both go through it — a second would be
        // a second borrow of the same shaper, which panics.
        let mut glyphs = shaper.glyphs();
        let mut laying = Laying {
            atlas: &mut cpu.atlas,
            glyphs: &mut glyphs,
            scale: raster_scale,
        };
        cpu.texts
            .refresh(&mut scene.texts, &mut laying, highlights, relight);
    }
}

impl GpuPaint for Renderer {
    fn init(&mut self, ctx: &GpuInitCtx<'_>) {
        // Re-runs whenever palantir reclaims the view's target. The pipelines
        // and the uploaded buffers both outlive that, so build once.
        if self.gpu.is_none() {
            self.gpu = Some(Gpu::new(ctx.device, ctx.target_format, &self.cpu.atlas));
        }
        // Re-taken rather than guarded, unlike the pipelines above: it is a
        // clone of a handle, and a view whose target was reclaimed should come
        // back holding whatever shaper the window has now.
        self.shaper = Some(ctx.text.clone());
    }

    fn paint(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        // The target's size, and the view it is a part of. The two differ only
        // where something clips the view — palantir allocates for what is on
        // screen — and the scene is framed for the *view*, so that what is drawn
        // agrees with what a pick at the same cursor answers. See [`Window`].
        //
        // Both floored, and the floor is hygiene at a crate boundary rather
        // than a case that arises: the target always has a pixel in it and is
        // never larger than the view it is part of. Neither is this crate's to
        // guarantee, and a zero here would divide by it.
        let size = ctx.size_px.max(UVec2::ONE);
        let full = ctx.full_px.max(size);
        let window = Window {
            min: ctx.offset_px.as_vec2(),
            size: size.as_vec2(),
        };
        let uniforms = Uniforms::of(&self.camera, Viewport::new(full), window, ctx.raster_scale);
        // Refilled before the GPU is borrowed, since both want `self`. Each kind
        // answers for itself, so a hover over a marker no longer rebuilds the
        // highlights of the strokes and rims it passed over.
        self.refresh(ctx.raster_scale);

        // Split so the two mirrors are borrowed apart: the uploads read one
        // while writing the other. Unconditional, all four — each takes what it
        // is owed and does nothing when that is nothing.
        let Self { cpu, gpu, .. } = self;
        let gpu = gpu.as_mut().expect("init runs before paint");
        gpu.solids
            .upload_mesh(ctx.device, ctx.queue, &mut cpu.solids);
        gpu.faces.upload_mesh(ctx.device, ctx.queue, &mut cpu.faces);
        gpu.gizmos
            .upload_mesh(ctx.device, ctx.queue, &mut cpu.gizmos);
        gpu.curves.upload(ctx.device, ctx.queue, &mut cpu.curves);
        gpu.rings.upload(ctx.device, ctx.queue, &mut cpu.rings);
        gpu.points.upload(ctx.device, ctx.queue, &mut cpu.points);
        // The sheet before the records that read it: a restart replaces the
        // texture, and the records name places on the new one.
        gpu.upload_sheet(ctx.device, ctx.queue, &mut cpu.atlas);
        gpu.texts
            .upload(ctx.device, ctx.queue, &mut cpu.texts.records);
        if gpu.attachments.as_ref().map(|used| used.size) != Some(size) {
            gpu.attachments = Some(Attachments::new(ctx.device, size, gpu.target_format));
        }
        // Two statements rather than a get-or-create returning the reference:
        // that would tie what comes back to a `&mut Gpu`, and the bind group is
        // read out of the same `Gpu` a few lines down. The `expect` cannot fire
        // — the line above is what makes sure of it.
        let attachments = gpu.attachments.as_ref().expect("attachments just ensured");
        ctx.queue
            .write_buffer(&gpu.uniforms, 0, bytemuck::bytes_of(&uniforms));

        let mut pass = attachments.begin(ctx.encoder, ctx.target);
        pass.set_viewport(0.0, 0.0, size.x as f32, size.y as f32, 0.0, 1.0);
        pass.set_scissor_rect(0, 0, size.x, size.y);
        pass.set_bind_group(0, &gpu.bind_group, &[]);
        // Everything opaque first, then what is see-through, then what is
        // blended — which is the order transparency has to be drawn in and the
        // whole of why the passes go in this sequence rather than the ladder's.
        //
        // A blend mixes with what is *already* in the target, so whatever should
        // show through a surface has to have been drawn before it. The opaque
        // kinds write depth and can go in any order among themselves, because
        // there the depth test is the whole answer.
        gpu.solids.draw(&mut pass);
        // With the opaque geometry, and before the faces for the reason
        // everything opaque is: a face is blended, so whatever should show
        // through one has to be in the target already — and whatever should
        // *not* has to be in the depth buffer already, which is the half a
        // control depends on. Where it lands among the other opaque kinds is
        // the depth ladder's business and not this sequence's.
        gpu.gizmos.draw(&mut pass);
        // Every ordinary pass before any highlight, rather than each kind's two
        // together: a highlight has to read over anything it doubles whatever
        // kind that is, and not merely over its own kind.
        //
        // Named once and walked twice, so the two halves cannot disagree. A
        // kind listed in one and forgotten in the other is invisible — it
        // flattens, it uploads, and it is simply never drawn.
        let opaque = [&gpu.curves, &gpu.rings, &gpu.points];
        for kind in opaque {
            kind.ordinary.draw(&mut pass);
        }
        for kind in opaque {
            kind.lit.draw(&mut pass);
        }
        // The faces after all of it, because a face is the one see-through
        // thing here: drawn earlier it would mix with a target the drawing had
        // not reached yet, and everything behind it would be missing from the
        // mixture rather than dimmed by it. Drawn now, a stroke it crosses in
        // front of shows through it shaded, and a stroke of its *own* sketch —
        // coplanar, and a ladder rung above — beats it on depth and reads over
        // it untouched.
        gpu.faces.draw(&mut pass);
        // Text last of all. It is the one alpha-blended pass, so what it reads
        // over has to be there already — and it writes no depth, so nothing
        // after it could be sorted against it anyway.
        gpu.texts.ordinary.draw(&mut pass);
        gpu.texts.lit.draw(&mut pass);
    }
}

/// The shaper [`GpuPaint::init`] would hand over, for a test laying text out
/// without a device to hand one over.
#[cfg(test)]
impl Renderer {
    pub(crate) fn shape_with(&mut self, shaper: TextShaper) {
        self.shaper = Some(shaper);
    }
}

/// What a harness painting whole frames needs, and an application never does.
///
/// Gated on `bench` rather than on `internals`, though `bench` carries it: the
/// two callers are this crate's own tests and its allocation bench, and a
/// consumer that turns `internals` on — catcad's test targets do — wants the
/// reach-ins into [`Batch`](crate::Batch) and not a pane it never paints.
#[cfg(any(test, feature = "bench"))]
pub(crate) mod internals {
    use crate::renderer::Renderer;
    use palantir::{App, Configure, GpuPaint, GpuView, Sizing, Ui, WindowToken};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A pane that draws one scene and does nothing else.
    ///
    /// A [`Renderer`] is a [`GpuPaint`] rather than an [`App`], so painting one
    /// at all needs something to show a [`GpuView`] from — this is the least of
    /// that.
    #[derive(Debug)]
    pub(crate) struct ScenePane {
        pub(crate) view: Rc<RefCell<Renderer>>,
    }

    impl App for ScenePane {
        fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
            let paint: Rc<RefCell<dyn GpuPaint>> = self.view.clone();
            GpuView::new(paint)
                .auto_id()
                .size((Sizing::FILL, Sizing::FILL))
                .show(ui);
        }
    }
}

#[cfg(test)]
mod tests;
