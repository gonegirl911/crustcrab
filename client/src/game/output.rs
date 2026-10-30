use crate::{
    event,
    renderer::{
        Renderer,
        post_process::blit::{self, Blit},
        render_pass::color_pass,
        surface::Surface,
    },
};

pub(super) struct Output(Blit);

impl Output {
    pub(super) fn new(
        renderer: &Renderer,
        surface: &Surface,
        input_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let format = surface.config.format;
        let operation = if format.is_srgb() {
            blit::Operation::Copy
        } else {
            blit::Operation::EncodeSrgb
        };
        Self(
            Blit::builder()
                .renderer(renderer)
                .input_bind_group_layout(input_bind_group_layout)
                .operation(operation)
                .format(format)
                .build(),
        )
    }

    pub(super) fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        input_bind_group: &wgpu::BindGroup,
    ) {
        self.0.draw(
            &mut color_pass(view, encoder, wgpu::LoadOp::Clear(Default::default())),
            input_bind_group,
        );
    }
}

impl event::Handler for Output {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, _: &event::Event, (renderer, surface): Self::Context<'_>) {
        if surface.is_reformatted() {
            *self = Self::new(renderer, surface, &self.0.get_bind_group_layout(0));
        }
    }
}
