//! Text grid → texture → fullscreen quad with a CRT shader.
//!
//! The grid is rasterized on the CPU (cols*8 x rows*16 pixels, 8x8 glyphs
//! doubled vertically) and uploaded when it changes; the shader does the
//! curvature, scan lines, glow and letterboxing. Nothing here knows about
//! the kernel: it takes a [`Screen`] snapshot and draws it.

use kiddos_console::{font::glyph, Cell, Screen, PALETTE};
use std::sync::Arc;
use winit::window::Window;

pub const CELL_W: u32 = 8;
pub const CELL_H: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    screen_size: [f32; 2],
    tex_size: [f32; 2],
    time: f32,
    crt: f32,
    _pad: [f32; 2],
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    params_buf: wgpu::Buffer,
    cols: u16,
    rows: u16,
    pixels: Vec<u8>,
    last_generation: u64,
    last_cursor: Option<(u16, u16, bool)>,
    pub crt: bool,
}

impl Renderer {
    pub fn new(window: Arc<Window>, cols: u16, rows: u16) -> Result<Renderer, String> {
        pollster::block_on(Self::new_async(window, cols, rows))
    }

    async fn new_async(window: Arc<Window>, cols: u16, rows: u16) -> Result<Renderer, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("kiddos"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| e.to_string())?;
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface not supported")?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let tex_w = cols as u32 * CELL_W;
        let tex_h = rows as u32 * CELL_H;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("grid"),
            size: wgpu::Extent3d {
                width: tex_w,
                height: tex_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buf.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("crt"),
            source: wgpu::ShaderSource::Wgsl(include_str!("crt.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("crt"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Ok(Renderer {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            texture,
            params_buf,
            cols,
            rows,
            pixels: vec![0; (tex_w * tex_h * 4) as usize],
            last_generation: 0,
            last_cursor: None,
            crt: true,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Rasterize the grid into the CPU pixel buffer. Returns true if changed.
    fn rasterize(&mut self, screen: &Screen, cursor_on: bool) -> bool {
        let (cx, cy) = screen.cursor();
        let cursor = if screen.cursor_visible() && cursor_on {
            Some((cx, cy, true))
        } else {
            None
        };
        if screen.generation() == self.last_generation && cursor == self.last_cursor {
            return false;
        }
        self.last_generation = screen.generation();
        self.last_cursor = cursor;
        rasterize_into(screen, cursor_on, &mut self.pixels);
        true
    }

    pub fn draw(&mut self, screen: &Screen, cursor_on: bool, time: f32) -> Result<(), wgpu::SurfaceError> {
        let tex_w = self.cols as u32 * CELL_W;
        let tex_h = self.rows as u32 * CELL_H;
        if self.rasterize(screen, cursor_on) {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(tex_w * 4),
                    rows_per_image: Some(tex_h),
                },
                wgpu::Extent3d {
                    width: tex_w,
                    height: tex_h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let params = Params {
            screen_size: [self.config.width as f32, self.config.height as f32],
            tex_size: [tex_w as f32, tex_h as f32],
            time,
            crt: if self.crt { 1.0 } else { 0.0 },
            _pad: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&params));
        let frame = self.surface.get_current_texture()?;
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("crt"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

/// Draw `screen` as RGBA pixels, `cols*8` by `rows*16`, into `pixels`
/// (which must be exactly that size). Pure CPU; the renderer and the
/// screenshot test both use it.
pub fn rasterize_into(screen: &Screen, cursor_on: bool, pixels: &mut [u8]) {
    let (cols, rows) = (screen.cols() as usize, screen.rows() as usize);
    let tex_w = cols * CELL_W as usize;
    assert_eq!(pixels.len(), tex_w * rows * CELL_H as usize * 4);
    let (cx, cy) = screen.cursor();
    let show_cursor = screen.cursor_visible() && cursor_on;
    let cells = screen.cells();
    for y in 0..rows {
        for x in 0..cols {
            let Cell { ch, fg, bg } = cells[y * cols + x];
            let mut g = glyph(ch);
            if show_cursor && cx as usize == x && cy as usize == y {
                // underline cursor: light up the bottom two rows
                g[6] = 0xFF;
                g[7] = 0xFF;
            }
            let (fg_rgb, bg_rgb) = (PALETTE[fg as usize & 15], PALETTE[bg as usize & 15]);
            for (gy, bits) in g.iter().enumerate() {
                for sub in 0..2usize {
                    let py = y * CELL_H as usize + gy * 2 + sub;
                    let row = &mut pixels[(py * tex_w + x * CELL_W as usize) * 4..][..CELL_W as usize * 4];
                    for gx in 0..8usize {
                        let c = if bits & (1 << gx) != 0 { fg_rgb } else { bg_rgb };
                        row[gx * 4..gx * 4 + 3].copy_from_slice(&c);
                        row[gx * 4 + 3] = 255;
                    }
                }
            }
        }
    }
}

/// Write `screen` as a binary PPM (for screenshots in tests and tools).
pub fn screenshot_ppm(screen: &Screen, cursor_on: bool) -> Vec<u8> {
    let (cols, rows) = (screen.cols() as usize, screen.rows() as usize);
    let (w, h) = (cols * CELL_W as usize, rows * CELL_H as usize);
    let mut rgba = vec![0u8; w * h * 4];
    rasterize_into(screen, cursor_on, &mut rgba);
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for px in rgba.chunks(4) {
        out.extend_from_slice(&px[..3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_and_can_dump_a_screenshot() {
        let mut s = Screen::new(40, 6);
        s.write_str("\x1b[1;36mKidDOS\x1b[0m 0.1.0  (80 cols x 25 rows)\n");
        s.write_str("kid@kiddos:~$ echo \x1b[1;32mПривет\x1b[0m, мир! ┌─┐ █▓\n");
        s.write_str("\x1b[33mA\x1b[34mB\x1b[35mC\x1b[31mD\x1b[0m abc xyz 0123 ?!@#\n");
        let ppm = screenshot_ppm(&s, true);
        assert!(ppm.starts_with(b"P6\n320 96\n255\n"));
        assert_eq!(ppm.len(), 14 + 320 * 96 * 3);
        // the K of KidDOS has cyan pixels in the first cell
        let cyan = ppm[14..]
            .chunks(3)
            .take(320 * 16)
            .filter(|p| p == &[0x55, 0xFF, 0xFF])
            .count();
        assert!(cyan > 20, "{cyan}");
        if let Ok(dir) = std::env::var("KIDDOS_SHOT_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("screen.ppm"), &ppm).unwrap();
        }
    }
}
