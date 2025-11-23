use std::{fs, path::Path};

pub fn read_wgsl<P: AsRef<Path>>(path: P) -> wgpu::ShaderModuleDescriptor<'static> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()));
    wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(contents.into()),
    }
}
