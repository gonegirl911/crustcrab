use super::definition::TEX_PATHS;
use crate::renderer::{
    Renderer,
    texture::image::{ImageTextureArray, load_rgba},
};
use std::ops::Deref;

struct Array(ImageTextureArray);

impl Array {
    fn new(renderer: &Renderer) -> Self {
        let images = TEX_PATHS
            .iter()
            .map(|path| load_rgba(format!("client/assets/textures/blocks/{path}")))
            .collect::<Vec<_>>();
        Self(
            ImageTextureArray::builder()
                .renderer(renderer)
                .images(&images)
                .mip_level_count(4)
                .is_srgb(true)
                .address_mode(wgpu::AddressMode::Repeat)
                .build(),
        )
    }
}

impl Deref for Array {
    type Target = ImageTextureArray;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
