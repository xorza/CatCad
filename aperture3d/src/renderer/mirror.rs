//! One scene, flattened and handed to the device.

use crate::camera::Camera;
use crate::highlight::{Highlights, Lit};
use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::cpu::Cpu;
use crate::renderer::cpu::records::Laying;
use crate::renderer::cpu::triangles::Order;
use crate::renderer::gpu::Gpu;
use crate::renderer::held::Held;
use crate::renderer::uniforms::Uniforms;
use crate::scene::Scene;
use palantir::TextShaper;

/// What one scene comes to once it has been flattened: the records on this side
/// of the queue, the buffers they were written into on the other, and what is
/// lit in it.
///
/// **Derived, where a [`Scene`] is authored.** Nothing here is written by a
/// caller and nothing here survives being rebuilt from the scene it mirrors. The
/// split is the whole of what lets an edit that moves one marker leave every
/// triangle in the model alone: each batch says for itself what was written to
/// it, and only that is flattened and uploaded again.
///
/// **One per scene drawn, and that is why it is a type.** Two scenes drawn into
/// one frame are two of these and one [`Gpu`] — the pipelines and the glyph
/// sheet are shared, and only the records differ.
#[derive(Debug, Default)]
pub(super) struct Mirror {
    pub(super) cpu: Cpu,
    /// At most one entry per tag, which is what lets a lookup stop at the
    /// first match rather than having to find the last.
    ///
    /// The mirror's rather than the renderer's, because a highlight keys on a
    /// [`Tag`](crate::Tag) and a tag names something in *a* scene. Two mirrors
    /// lighting one tag would be two answers to where the pointer is.
    highlights: Highlights,
    /// Whether *which* primitives are lit has changed since the last refresh.
    ///
    /// Its own flag rather than one of `cpu`'s, because it is the one edit that
    /// leaves every batch in the scene untouched: a pointer crossing a drawing
    /// relights it without moving anything in it.
    ///
    /// A mirror starts with it down, and owes nothing by starting there: a
    /// batch with anything in it is already marked, and a highlight over an
    /// empty one has nothing to light.
    pub(super) relight: bool,
    /// `None` until the first paint, on the terms
    /// [`Renderer::gpu`](super::Renderer) is.
    pub(super) held: Option<Held>,
}

impl Mirror {
    /// Light exactly these, dropping whatever was lit before, and say whether
    /// that changed anything.
    ///
    /// Compared before it is written, so re-asking for the set already in force
    /// dirties nothing — which is what lets a caller call it unconditionally
    /// every frame.
    pub(super) fn highlight_all(&mut self, lit: &[Lit]) {
        self.relight |= self.highlights.set_all(lit);
    }

    /// Bring the mirror up to date with `scene`, and the scene's own
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
    /// it, so it is remembered on the run — through a memo rather than the batch,
    /// which is what keeps recording it from reading as an edit. See
    /// [`Text::extent`](crate::Text::extent).
    pub(super) fn refresh(
        &mut self,
        scene: &mut Scene,
        camera: &Camera,
        atlas: &mut GlyphAtlas,
        shaper: Option<&TextShaper>,
        raster_scale: f32,
    ) {
        let Self {
            cpu,
            highlights,
            relight,
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
        cpu.ghosts.refresh(
            &mut scene.ghosts,
            highlights,
            relight,
            Gpu::ghosts_order(camera.eye()),
        );
        cpu.gizmos.refresh(&mut scene.gizmos, highlights, relight);
        cpu.curves.refresh(&mut scene.curves, highlights, relight);
        cpu.rings.refresh(&mut scene.rings, highlights, relight);
        cpu.points.refresh(&mut scene.points, highlights, relight);
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
        let shaper = shaper.expect("laying text out needs the shaper `init` is handed");
        // One lease for the whole scene's text. Measuring and placing are two
        // halves of laying a run out and both go through it — a second would be
        // a second borrow of the same shaper, which panics.
        let mut glyphs = shaper.glyphs();
        let mut laying = Laying {
            atlas,
            glyphs: &mut glyphs,
            scale: raster_scale,
        };
        cpu.texts
            .refresh(&mut scene.texts, &mut laying, highlights, relight);
    }

    /// Hand the device whatever the last refresh rewrote, building this
    /// mirror's own buffers first if it has none yet.
    ///
    /// `restarted` says [`Gpu::upload_sheet`] replaced the glyph sheet, which
    /// only a mirror that outlived the replacement owes a bind group for: one
    /// built here names the sheet as it already stands.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gpu: &Gpu,
        uniforms: &Uniforms,
        restarted: bool,
    ) {
        let Self { cpu, held, .. } = self;
        let held = match held {
            fresh @ None => fresh.insert(Held::new(device, gpu)),
            Some(held) => {
                if restarted {
                    held.rebind(device, gpu);
                }
                held
            }
        };
        held.upload(device, queue, cpu, uniforms);
    }

    /// Draw this mirror into the pass already open.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.held
            .as_ref()
            .expect("upload runs before draw")
            .draw(pass);
    }
}
