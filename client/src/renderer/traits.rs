use bytemuck::Pod;
use std::slice;

pub(crate) trait Vertex: Pod {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;
    const ATTRIBS: &[wgpu::VertexAttribute];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: Self::STEP_MODE,
            attributes: Self::ATTRIBS,
        }
    }
}

pub(crate) trait Immediates: Pod {
    const SIZE: u32 = {
        let size = size_of::<Self>();
        assert!(usize::BITS <= u32::BITS || size <= u32::MAX as usize);
        size as u32
    };

    fn set(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_immediates(0, bytemuck::cast_slice(slice::from_ref(self)));
    }
}
