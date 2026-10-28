use eframe::{egui, wgpu};

pub(crate) struct GpuFrame {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pub id: egui::TextureId,
    pub width: u32,
    pub height: u32,
    nearest: bool,
}

/// Persistent live GPU texture.
#[derive(Default)]
pub(crate) struct LiveTexture {
    pub gpu: Option<GpuFrame>,
    dirty: bool,
}

impl LiveTexture {
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn upload(
        &mut self,
        frame: &eframe::Frame,
        packed: Option<(&[u8], usize, usize, bool)>,
    ) {
        if !self.dirty {
            return;
        }
        let Some((rgba, width, height, nearest)) = packed else {
            return;
        };
        if width == 0 || height == 0 || rgba.len() != width.saturating_mul(height).saturating_mul(4)
        {
            return;
        }
        let Some(state) = frame.wgpu_render_state() else {
            return;
        };
        let width = width as u32;
        let height = height as u32;
        let filter = if nearest {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        };
        let recreate = self
            .gpu
            .as_ref()
            .is_none_or(|gpu| gpu.width != width || gpu.height != height);
        if recreate {
            let texture = state.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("hololive-live"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut renderer = state.renderer.write();
            let id = match self.gpu.as_ref().map(|gpu| gpu.id) {
                Some(id) => {
                    renderer.update_egui_texture_from_wgpu_texture(
                        &state.device,
                        &view,
                        filter,
                        id,
                    );
                    id
                }
                None => renderer.register_native_texture(&state.device, &view, filter),
            };
            drop(renderer);
            self.gpu = Some(GpuFrame {
                texture,
                view,
                id,
                width,
                height,
                nearest,
            });
        } else if let Some(gpu) = &mut self.gpu
            && gpu.nearest != nearest
        {
            state
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(&state.device, &gpu.view, filter, gpu.id);
            gpu.nearest = nearest;
        }
        write_rgba(
            &state.queue,
            &self.gpu.as_ref().expect("live texture allocated").texture,
            width,
            height,
            rgba,
        );
        self.dirty = false;
    }
}

fn write_rgba(queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32, rgba: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}
