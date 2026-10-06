//! WgpuContext - Device and Queue wrapper
//!
//! Provides a convenient wrapper around wgpu's Device and Queue.

use std::sync::Arc;

/// Core wgpu context containing device and queue.
///
/// This is the fundamental building block for all GPU operations.
#[derive(Clone)]
pub struct WgpuContext {
    /// The wgpu device for creating GPU resources.
    pub device: Arc<wgpu::Device>,
    /// The wgpu queue for submitting commands.
    pub queue: Arc<wgpu::Queue>,
}

impl WgpuContext {
    /// Create a new context from existing device and queue.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
        }
    }

    /// Create a new context asynchronously with default settings.
    pub async fn new_async(compatible_surface: Option<&wgpu::Surface<'_>>) -> anyhow::Result<Self> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sard device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await?;

        Ok(Self::new(device, queue))
    }

    /// Create a new context synchronously (blocks).
    pub fn new_blocking(compatible_surface: Option<&wgpu::Surface<'_>>) -> anyhow::Result<Self> {
        pollster::block_on(Self::new_async(compatible_surface))
    }

    /// Submit command buffers to the queue.
    pub fn submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(&self, command_buffers: I) {
        self.queue.submit(command_buffers);
    }

    /// Create a command encoder.
    pub fn create_encoder(&self, label: Option<&str>) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label })
    }
}

impl std::fmt::Debug for WgpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuContext").finish()
    }
}
