//! A GPU buffer that outlives the data in it.

use wgpu::util::DeviceExt;

/// pass can go a whole run with nothing to draw.
#[derive(Debug, Clone)]
pub(super) struct Retained {
    label: &'static str,
    usage: wgpu::BufferUsages,
    buffer: Option<wgpu::Buffer>,
    /// Bytes there is room for, which is at least what is in it.
    capacity: u64,
}

impl Retained {
    /// Empty, to be filled and grown by [`Retained::write`].
    pub(super) fn growable(label: &'static str, usage: wgpu::BufferUsages) -> Self {
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
        label: &'static str,
        usage: wgpu::BufferUsages,
        contents: &[u8],
    ) -> Self {
        Self {
            label,
            usage,
            buffer: Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                }),
            ),
            capacity: contents.len() as u64,
        }
    }

    pub(super) fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Overwrite from the start, growing first if `contents` no longer fits.
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
                label: Some(self.label),
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
