//! The triangle lists the overlay passes draw every instance through.

/// Vertex pairs the ring band is built from. Stated here alone: `ring.wgsl`
/// declares it `override` and is handed this at pipeline creation, so the
/// indices below and the angles the shader walks cannot come apart.
pub(super) const RING_STEPS: usize = 32;

/// The band's triangles: a quad per step, wrapping at the last back to the
/// first. Inner and outer alternate, so step `s` owns vertices `2s` and
/// `2s + 1`.
pub(super) const RING_INDICES: [u32; RING_STEPS * 6] = ring_indices();

const fn ring_indices() -> [u32; RING_STEPS * 6] {
    let mut indices = [0; RING_STEPS * 6];
    let mut step = 0;
    while step < RING_STEPS {
        let inner = (step * 2) as u32;
        let next = ((step + 1) % RING_STEPS * 2) as u32;
        let base = step * 6;
        indices[base] = inner;
        indices[base + 1] = inner + 1;
        indices[base + 2] = next;
        indices[base + 3] = next;
        indices[base + 4] = inner + 1;
        indices[base + 5] = next + 1;
        step += 1;
    }
    indices
}

/// The two triangles every overlay quad is drawn through. Together they cover
/// the quad rather than overlapping, sharing the edge between the middle pair.
///
/// Each overlay *kind* is built holding its own copy rather than one being
/// shared between them: twenty-four bytes four times over, against a buffer
/// every kind would then have to be handed. What a kind holds is already shared
/// as far as sharing pays — both halves of it draw the one buffer, and so does
/// every mirror of the scene.
pub(super) const QUAD_INDICES: [u32; 6] = [0, 1, 2, 2, 1, 3];
