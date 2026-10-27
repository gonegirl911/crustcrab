pub(super) mod buffer;
pub(super) mod instance;
pub(super) mod post_process;
pub(super) mod render_pass;
pub(super) mod render_pipeline;
pub(super) mod surface;
pub(super) mod texture;
mod traits;
pub(super) mod types;
pub(super) mod uniform;

pub(super) use traits::{Immediates, Vertex};

use log::{info, warn};
use std::sync::{
    Arc,
    atomic::{self, AtomicBool},
};
use surface::Surface;
use winit::{dpi::PhysicalSize, window::Window};

pub(super) struct Renderer {
    adapter: wgpu::Adapter,
    pub(super) mut(self) device: wgpu::Device,
    pub(super) mut(self) queue: wgpu::Queue,
    is_device_lost: Arc<AtomicBool>,
}

impl Renderer {
    pub(super) async fn new(window: Arc<dyn Window>, instance: &wgpu::Instance) -> (Self, Surface) {
        let PhysicalSize { width, height } = window.surface_size();
        let surface = Surface::create_raw(instance, window);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("adapter should be available");
        let config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::IMMEDIATES
                    | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
                    | wgpu::Features::TEXTURE_BINDING_ARRAY
                    | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM,
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .expect("device should be available");
        let is_device_lost = Arc::new(AtomicBool::new(false));

        device.set_device_lost_callback({
            let is_device_lost = is_device_lost.clone();
            move |reason, msg| match reason {
                wgpu::DeviceLostReason::Unknown => {
                    warn!("device lost: {msg}");
                    is_device_lost.store(true, atomic::Ordering::Relaxed);
                }
                wgpu::DeviceLostReason::Destroyed => {}
            }
        });

        let adapter_info = adapter.get_info();
        info!(
            "renderer initialized; name: {}, backend: {:?}, surface format: {:?}, present mode: {:?}",
            adapter_info.name, adapter_info.backend, config.format, config.present_mode,
        );

        (
            Self {
                adapter,
                device,
                queue,
                is_device_lost,
            },
            Surface::new(surface, config),
        )
    }

    pub(super) fn is_device_lost(&self) -> bool {
        self.is_device_lost.load(atomic::Ordering::Relaxed)
    }
}
