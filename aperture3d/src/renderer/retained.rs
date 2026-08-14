//! A GPU buffer that outlives the data in it.

use wgpu::util::DeviceExt;

/// A buffer that outlives the data in it: emptied and refilled in place, grown
/// when what it is handed no longer fits, and never given back — an overlay
/// pass can go a whole run with nothing to draw.
///
/// The label is owned rather than borrowed because it is *derived*, from the
/// name of the pass that holds the buffer, and there is nowhere to borrow a
/// derived string from. It costs one allocation per buffer at startup and is
/// read again only when the buffer grows.
#[derive(Debug, Clone)]
pub(super) struct Retained {
    label: String,
    usage: wgpu::BufferUsages,
    buffer: Option<wgpu::Buffer>,
    /// Bytes there is room for, which is at least what is in it.
    capacity: u64,
}

impl Retained {
    /// Empty, to be filled and grown by [`Retained::write`].
    pub(super) fn growable(label: String, usage: wgpu::BufferUsages) -> Self {
        Self {
            label,
            usage,
            buffer: None,
            capacity: 0,
        }
    }

    /// Created already holding `contents`, for data that never changes. Wants
    /// no queue, which is what lets it be built before the first frame.
    pub(super) fn filled(
        device: &wgpu::Device,
        label: String,
        usage: wgpu::BufferUsages,
        contents: &[u8],
    ) -> Self {
        Self {
            buffer: Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&label),
                    contents,
                    usage,
                }),
            ),
            label,
            usage,
            capacity: contents.len() as u64,
        }
    }

    pub(super) fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Overwrite from the start, growing first if `contents` no longer fits.
    ///
    /// An empty slice writes nothing and leaves whatever the buffer last held:
    /// wgpu rejects a zero-sized write, and a buffer has no way to be told it is
    /// now empty. What keeps those stale bytes off the screen is the count
    /// beside them rather than anything here — [`Pass::upload_instances`] takes
    /// `instances` from the slice's own length, and [`Pass::draw`] draws nothing
    /// at zero.
    ///
    /// [`Pass::upload_instances`]: crate::renderer::pass::Pass::upload_instances
    /// [`Pass::draw`]: crate::renderer::pass::Pass::draw
    pub(super) fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, contents: &[u8]) {
        if contents.is_empty() {
            return;
        }
        let needed = contents.len() as u64;
        if needed > self.capacity {
            // Doubled rather than fitted exactly: geometry that creeps upward
            // a vertex at a time would otherwise reallocate on every edit,
            // which is the whole of what this type exists to avoid.
            self.capacity = needed.next_power_of_two();
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&self.label),
                size: self.capacity,
                usage: self.usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(
            self.buffer.as_ref().expect("a buffer was just ensured"),
            0,
            contents,
        );
    }
}
