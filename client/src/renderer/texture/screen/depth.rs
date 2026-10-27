use super::ScreenTexture;
use crate::renderer::{Renderer, Surface};
use std::ops::{Deref, DerefMut};

pub(crate) struct DepthTexture(ScreenTexture);

impl DepthTexture {
    pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub(crate) fn new(renderer: &Renderer, surface: &Surface) -> Self {
        Self(ScreenTexture::new(renderer, surface, Self::FORMAT))
    }
}

impl Deref for DepthTexture {
    type Target = ScreenTexture;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DepthTexture {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
