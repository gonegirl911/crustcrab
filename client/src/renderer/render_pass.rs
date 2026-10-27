pub(crate) fn color_pass<'a>(
    view: &wgpu::TextureView,
    encoder: &'a mut wgpu::CommandEncoder,
    load_op: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: load_op,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}
