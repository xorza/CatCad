//! What the device holds for one mirror of a scene.

use crate::renderer::cpu::Cpu;
use crate::renderer::cpu::records::Records;
use crate::renderer::gpu::{Gpu, Twin};
use crate::renderer::pass::Pass;
use crate::renderer::record::Record;
use crate::renderer::uniforms::Uniforms;

/// The two passes one overlay kind is drawn through: its own, and the same
/// pipeline again holding only what a caller has singled out.
///
/// Paired for the reason [`Records`] pairs the buffers that feed them, and named
/// for the same two halves — the two are built together, uploaded together and
/// drawn one after the other, and they already share the triangle list their
/// [`Twin`] was built holding.
#[derive(Debug)]
pub(super) struct Passes {
    pub(super) ordinary: Pass,
    pub(super) lit: Pass,
}

impl Passes {
    /// One mirror's buffers for both halves of `kind`.
    ///
    /// Both labels are derived from the kind's one name, so a kind names itself
    /// once and every buffer of it follows.
    fn of(kind: &Twin) -> Self {
        Self {
            ordinary: Pass::fixed(
                &format!("aperture.{}", kind.name),
                kind.ordinary.clone(),
                kind.indices.clone(),
                kind.index_count,
            ),
            lit: Pass::fixed(
                &format!("aperture.{}.highlighted", kind.name),
                kind.lit.clone(),
                kind.indices.clone(),
                kind.index_count,
            ),
        }
    }

    /// Hand the GPU whatever the last refresh rewrote, and nothing it did not.
    ///
    /// Asks the records rather than being told: each buffer says whether it was
    /// rewritten and hands itself over in the same breath, so a still frame
    /// reaches the queue for neither.
    /// Takes the [`Records`] rather than whatever holds one, which is what lets
    /// this be written once: three kinds are a bare `Records` and text keeps its
    /// beside a raster scale and a scratch buffer, and neither is any of this
    /// method's business.
    fn upload<R: Record>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        records: &mut Records<R>,
    ) {
        if let Some(instances) = records.ordinary_to_upload() {
            self.ordinary.upload_instances(device, queue, instances);
        }
        if let Some(instances) = records.lit_to_upload() {
            self.lit.upload_instances(device, queue, instances);
        }
    }
}

/// One mirror's own half of the device: a buffer per kind, the uniform buffer
/// the mirror is drawn through, and the group that binds that buffer to the
/// glyph sheet every mirror shares.
///
/// The mirror of [`Cpu`], field for field, on the far side of the queue: what is
/// `curves` there is written into what is `curves` here. What is *not* here is
/// anything a second mirror could share — the pipelines and the sheet are
/// [`Gpu`]'s, and reached by handle.
#[derive(Debug)]
pub(super) struct Held {
    pub(super) solids: Pass,
    pub(super) faces: Pass,
    pub(super) ghosts: Pass,
    pub(super) gizmos: Passes,
    pub(super) curves: Passes,
    pub(super) rings: Passes,
    pub(super) points: Passes,
    pub(super) texts: Passes,
    /// What this mirror is drawn through, which is the whole of what makes two
    /// mirrors two: one camera each, landed in one rect each.
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

impl Held {
    pub(super) fn new(device: &wgpu::Device, gpu: &Gpu) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aperture.uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            bind: gpu.bind(device, &uniforms),
            uniforms,
            solids: Pass::mesh(&gpu.solids),
            faces: Pass::mesh(&gpu.faces),
            ghosts: Pass::mesh(&gpu.ghosts),
            gizmos: Passes::of(&gpu.gizmos),
            curves: Passes::of(&gpu.curves),
            rings: Passes::of(&gpu.rings),
            points: Passes::of(&gpu.points),
            texts: Passes::of(&gpu.texts),
        }
    }

    /// Name the sheet again, after it was started at a larger size.
    ///
    /// A group names a texture *view*, and the view it named no longer exists —
    /// see [`Gpu::upload_sheet`], which is what answers whether this is owed.
    /// Everything else about the group survives, the uniform buffer included.
    pub(super) fn rebind(&mut self, device: &wgpu::Device, gpu: &Gpu) {
        self.bind = gpu.bind(device, &self.uniforms);
    }

    /// Hand the GPU everything this mirror owes it, and nothing it does not.
    ///
    /// **The list every drawn kind has to appear on, and [`Held::draw`] below is
    /// the other one.** A kind uploaded and never drawn is invisible — it
    /// flattens, it uploads, and nothing asks for it — and a kind drawn without
    /// being uploaded goes on showing whatever it last held. Nothing checks
    /// that the two agree, so they are written one after the other beside the
    /// fields they walk, which is the most that can be done about it.
    ///
    /// Unconditional, all of them: each takes what it is owed and does nothing
    /// when that is nothing, which is what a still frame costs.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cpu: &mut Cpu,
        uniforms: &Uniforms,
    ) {
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));
        self.solids.upload_mesh(device, queue, &mut cpu.solids);
        self.faces.upload_mesh(device, queue, &mut cpu.faces);
        self.ghosts.upload_mesh(device, queue, &mut cpu.ghosts);
        self.gizmos.upload(device, queue, &mut cpu.gizmos);
        self.curves.upload(device, queue, &mut cpu.curves);
        self.rings.upload(device, queue, &mut cpu.rings);
        self.points.upload(device, queue, &mut cpu.points);
        self.texts.upload(device, queue, &mut cpu.texts.records);
    }

    /// Draw this mirror into the pass already open.
    ///
    /// **The other half of [`Held::upload`]'s list**, and the one that decides
    /// the order.
    ///
    /// Everything opaque first, then what is see-through, then what is blended —
    /// which is the order transparency has to be drawn in and the whole of why
    /// the passes go in this sequence rather than the ladder's. A blend mixes
    /// with what is *already* in the target, so whatever should show through a
    /// surface has to have been drawn before it. The opaque kinds write depth
    /// and can go in any order among themselves, because there the depth test is
    /// the whole answer.
    ///
    /// Takes the pass rather than opening one, so that several mirrors can be
    /// drawn into one — which is what keeps the multisampled buffer discarded
    /// rather than stored.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_bind_group(0, &self.bind, &[]);
        self.solids.draw(pass);
        // The ghosts straight after the solids, which is what they are shown
        // *through*: a blend mixes with what is already in the target, so the
        // model has to be there first. Before the drawing's own strokes and
        // markers rather than after, because what a preview must never do is
        // dim the thing being drawn — a ghost takes no depth test, so nothing
        // else could put those back over it.
        self.ghosts.draw(pass);
        // Every ordinary pass before any highlight, rather than each kind's two
        // together: a highlight has to read over anything it doubles whatever
        // kind that is, and not merely over its own kind.
        //
        // Named once and walked twice, so the two halves cannot disagree.
        let opaque = [&self.gizmos, &self.curves, &self.rings, &self.points];
        for kind in opaque {
            kind.ordinary.draw(pass);
        }
        for kind in opaque {
            kind.lit.draw(pass);
        }
        // The faces after all of it, because a face is the one see-through thing
        // here: drawn earlier it would mix with a target the drawing had not
        // reached yet, and everything behind it would be missing from the
        // mixture rather than dimmed by it. Drawn now, a stroke it crosses in
        // front of shows through it shaded, and a stroke of its *own* sketch —
        // coplanar, and a ladder rung above — beats it on depth and reads over
        // it untouched.
        self.faces.draw(pass);
        // Text last of all. It is the one alpha-blended pass, so what it reads
        // over has to be there already — and it writes no depth, so nothing
        // after it could be sorted against it anyway.
        self.texts.ordinary.draw(pass);
        self.texts.lit.draw(pass);
    }
}
