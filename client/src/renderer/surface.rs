use super::Renderer;
use crate::event::{self, Event};
use bitflags::bitflags;
use log::warn;
use std::sync::Arc;
use thiserror::Error;
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

pub(crate) struct Surface {
    raw: Option<wgpu::Surface<'static>>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pending: Pending,
    changes: Changes,
}

impl Surface {
    pub(super) fn new(raw: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration) -> Self {
        Self {
            raw: Some(raw),
            config,
            pending: Pending::INIT | Pending::CONFIGURE,
            changes: Default::default(),
        }
    }

    pub(crate) fn recreate(
        &mut self,
        window: Arc<dyn Window>,
        instance: &wgpu::Instance,
        Renderer { adapter, .. }: &Renderer,
    ) -> Result<(), Unsupported> {
        let raw = Self::create_raw(instance, window);
        if !adapter.is_surface_supported(&raw) {
            return Err(Unsupported);
        }

        self.raw = Some(raw);
        self.pending.insert(Pending::CONFIGURE);
        Ok(())
    }

    pub(crate) fn is_available(&self) -> bool {
        self.raw.is_some()
    }

    pub(crate) fn get_current_texture(&self) -> Option<wgpu::CurrentSurfaceTexture> {
        if self.pending.contains(Pending::CONFIGURE) {
            return None;
        }

        self.raw.as_ref().map(wgpu::Surface::get_current_texture)
    }

    pub(crate) fn request_configure(&mut self) {
        self.pending.insert(Pending::CONFIGURE);
    }

    pub(crate) fn destroy(&mut self) {
        self.raw = None;
    }

    pub(crate) fn is_resized(&self) -> bool {
        self.changes.contains(Changes::RESIZED)
    }

    pub(crate) fn is_reformatted(&self) -> bool {
        self.changes.contains(Changes::REFORMATTED)
    }

    fn configure(
        &mut self,
        window: &dyn Window,
        Renderer {
            adapter, device, ..
        }: &Renderer,
    ) -> Option<Changes> {
        let raw = self.raw.as_ref()?;
        let mut changes = Changes::default();

        let PhysicalSize { width, height } = window.surface_size();
        if width == 0 || height == 0 {
            return None;
        }
        if self.config.width != width || self.config.height != height {
            self.config.width = width;
            self.config.height = height;
            changes.insert(Changes::RESIZED);
        }

        let caps = raw.get_capabilities(adapter);
        if !caps.formats.contains(&self.config.format) {
            let format = *caps.formats.first().unwrap();
            self.config.format = format;
            warn!("surface format changed; new value: {format:?}");
            changes.insert(Changes::REFORMATTED);
        }

        raw.configure(device, &self.config);
        Some(changes)
    }

    pub(super) fn create_raw(
        instance: &wgpu::Instance,
        window: Arc<dyn Window>,
    ) -> wgpu::Surface<'static> {
        instance
            .create_surface(window)
            .expect("surface should be creatable")
    }
}

impl event::Handler for Surface {
    type Context<'a> = (&'a dyn Window, &'a Renderer);

    fn handle(&mut self, event: &Event, (window, renderer): Self::Context<'_>) {
        self.changes = Default::default();

        if let Event::WindowEvent(event) = event {
            match event {
                WindowEvent::SurfaceResized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.pending.insert(Pending::CONFIGURE);
                }
                WindowEvent::RedrawRequested => {
                    if self.pending.contains(Pending::CONFIGURE)
                        && let Some(changes) = self.configure(window, renderer)
                    {
                        self.changes = changes;
                        self.pending.remove(Pending::CONFIGURE);

                        if self.pending.contains(Pending::INIT) {
                            self.changes.insert(Changes::RESIZED);
                            self.pending.remove(Pending::INIT);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

bitflags! {
    #[derive(Default)]
    struct Pending: u8 {
        const INIT = 1 << 0;
        const CONFIGURE = 1 << 1;
    }

    #[derive(Default)]
    struct Changes: u8 {
        const RESIZED = 1 << 0;
        const REFORMATTED = 1 << 1;
    }
}

#[derive(Debug, Error)]
#[error("surface not supported by adapter")]
pub(crate) struct Unsupported;
