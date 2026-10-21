//! GPU presentation: wgpu 30 + vello 0.11, driven by the same Hamesh `Scene`.
//!
//! The scene is translated 1:1 into a `vello::Scene` (one rounded rect per
//! visible node, sRGB-ish colors as authored). This module owns ONLY the
//! GPU plumbing: surface, device, renderer, blit, resize recovery.
//! All layout/inspection/automation truth stays in the core crates.
//!
//! Failure model (from research brief task-1): surface Lost/Outdated ->
//! reconfigure; device-level failure -> report `GpuError::DeviceFailed`
//! and let the caller fall back to the CPU path.

use vello::kurbo::{Affine, RoundedRect};
use vello::peniko::{Brush, Color, Fill};
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene as VelloScene};

#[derive(Debug)]
pub enum GpuError {
    Vello(String),
    Surface(String),
    /// The wgpu device was lost (driver update, GPU switch, sleep/RDP).
    /// Caller should drop GPU state and fall back to the CPU path.
    DeviceFailed(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vello(m) | Self::Surface(m) | Self::DeviceFailed(m) => write!(f, "{m}"),
        }
    }
}

/// Frames presented + last error, for the Eye/Memory evidence.
#[derive(Debug, Default)]
pub struct GpuStats {
    pub frames_presented: u64,
    pub reconfigures: u64,
    pub last_error: Option<String>,
}

pub struct GpuState {
    context: RenderContext,
    surface: RenderSurface<'static>,
    renderer: Renderer,
    device_lost_rx: std::sync::mpsc::Receiver<String>,
    /// Text layouts keyed by node id + text + width. `FontContext::new`
    /// scans system fonts every construction, so uncached per-frame
    /// layout costs ~40ms/frame; the cache restores interactive rates.
    /// (The stateless CPU path in hamesh-render still lays out per frame;
    /// a shared cache is the known follow-up.)
    text_cache: std::collections::HashMap<String, hamesh_text::TextLayout>,
    pub stats: GpuStats,
}

fn node_color(c: [u8; 4]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

/// Translate the Hamesh scene: one rounded rect per visible node plus
/// one vello glyph run per text node (same positioned data as CPU).
/// Takes the text-layout cache explicitly so the free function stays
/// borrow-clean for both `paint` and `snapshot`.
fn build_vscene(
    cache: &mut std::collections::HashMap<String, hamesh_text::TextLayout>,
    scene: &hamesh_scene::Scene,
) -> VelloScene {
    let mut v = VelloScene::new();
    for node in &scene.nodes {
        if !node.visible {
            continue;
        }
        let r = &node.rect;
        let shape = RoundedRect::new(
            r.x as f64,
            r.y as f64,
            (r.x + r.w) as f64,
            (r.y + r.h) as f64,
            6.0,
        );
        v.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(node_color(node.color)),
            None,
            &shape,
        );
        // Real text: same parley layout as the CPU path, drawn with
        // vello glyph runs. `parley::FontData` IS vello's `FontData`
        // (single linebender_resource_handle 0.1.1 in the tree).
        // NOTE: caret draws even for empty text (focused empty composer)
        // so it lives outside the text check, like the CPU renderer.
        let layout = if node.text.trim().is_empty() {
            None
        } else {
            let avail = (r.w - 16.0).max(8.0);
            let key = format!("{}|{:.0}|{}", node.id, avail, node.text);
            if !cache.contains_key(&key) {
                let layout = hamesh_text::TextLayout::new(&node.text, avail, 16.0);
                cache.insert(key.clone(), layout);
            }
            cache.get(&key).filter(|l| l.height() > 0.0)
        };
        draw_text(&mut v, node, layout);
    }
    v
}

fn draw_text(
    v: &mut VelloScene,
    node: &hamesh_scene::SceneNode,
    layout: Option<&hamesh_text::TextLayout>,
) {
    // Empty text still lays out nothing, but the caret needs a line box.
    let lh = layout.map(|l| l.height()).unwrap_or(0.0).max(18.0);
    let dy = node.rect.y + ((node.rect.h - lh) / 2.0).max(0.0);
    let brush = Brush::Solid(node_color([235, 245, 243, 255]));
    if let Some(layout) = layout {
        for run in layout.runs() {
            v.draw_glyphs(&run.font)
                .font_size(run.size)
                .hint(true)
                .brush(&brush)
                .transform(Affine::translate(((node.rect.x + 8.0) as f64, dy as f64)))
                .draw(
                    Fill::NonZero,
                    run.glyphs.iter().map(|(id, x, y)| vello::Glyph {
                        id: *id,
                        x: *x,
                        y: *y,
                    }),
                );
        }
    }
    // Caret bar for the focused composer (mirrors the CPU renderer).
    if node.focused {
        if let Some(cx) = node.caret_x {
            let care = RoundedRect::new(
                (node.rect.x + 8.0 + cx) as f64,
                dy as f64,
                (node.rect.x + 8.0 + cx + 2.0) as f64,
                (dy + lh.max(8.0)) as f64,
                1.0,
            );
            v.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &Brush::Solid(node_color([120, 220, 200, 255])),
                None,
                &care,
            );
        }
    }
}

impl GpuState {
    pub fn new(
        window: std::sync::Arc<winit::window::Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let mut context = RenderContext::new();
        let surface = pollster::block_on(context.create_surface(
            window,
            width.max(1),
            height.max(1),
            wgpu::PresentMode::Fifo,
        ))
        .map_err(|e| GpuError::Vello(format!("create_surface: {e:?}")))?;
        let dev_id = surface.dev_id;
        let device = context.devices[dev_id].device.clone();
        // Device-loss recovery hook (research brief risk #3): the callback
        // is 'static, so it reports through a channel checked each frame.
        let (lost_tx, device_lost_rx) = std::sync::mpsc::channel();
        device.set_device_lost_callback(move |reason, msg| {
            let _ = lost_tx.send(format!("{reason:?}: {msg}"));
        });
        let renderer = Renderer::new(&device, RendererOptions::default())
            .map_err(|e| GpuError::Vello(format!("Renderer::new: {e:?}")))?;
        Ok(Self {
            context,
            surface,
            renderer,
            device_lost_rx,
            text_cache: std::collections::HashMap::new(),
            stats: GpuStats::default(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.context
            .resize_surface(&mut self.surface, width, height);
        self.stats.reconfigures += 1;
    }

    /// Render one Hamesh scene frame and present it. Returns the number of
    /// Vello nodes drawn (== visible Hamesh nodes).
    pub fn paint(&mut self, scene: &hamesh_scene::Scene) -> Result<usize, GpuError> {
        // A lost device never recovers in place: report it so the caller
        // can drop GPU state and fall back to the CPU path.
        if let Ok(msg) = self.device_lost_rx.try_recv() {
            return Err(GpuError::DeviceFailed(msg));
        }
        let width = scene.viewport_w.max(1.0) as u32;
        let height = scene.viewport_h.max(1.0) as u32;
        let dev_id = self.surface.dev_id;
        if self.surface.config.width != width || self.surface.config.height != height {
            self.resize(width, height);
        }
        let vscene = build_vscene(&mut self.text_cache, scene);
        let visible = scene.nodes.iter().filter(|n| n.visible).count();
        let device = self.context.devices[dev_id].device.clone();
        let queue = self.context.devices[dev_id].queue.clone();
        let params = RenderParams {
            base_color: node_color([10, 15, 17, 255]),
            width,
            height,
            antialiasing_method: AaConfig::Msaa16,
        };
        if let Err(e) = self.renderer.render_to_texture(
            &device,
            &queue,
            &vscene,
            &self.surface.target_view,
            &params,
        ) {
            let msg = format!("render_to_texture: {e:?}");
            self.stats.last_error = Some(msg.clone());
            return Err(GpuError::Vello(msg));
        }
        // wgpu 30: acquire returns a `CurrentSurfaceTexture` enum, not a Result.
        let surface_texture = match self.surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                // Skip this frame; the next redraw retries.
                return Ok(0);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.context
                    .resize_surface(&mut self.surface, width, height);
                self.stats.reconfigures += 1;
                match self.surface.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    other => {
                        return Err(GpuError::Surface(format!("re-acquire: {other:?}")));
                    }
                }
            }
            other => return Err(GpuError::Surface(format!("{other:?}"))),
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("hamesh-blit"),
        });
        self.surface.blitter.copy(
            &device,
            &mut encoder,
            &self.surface.target_view,
            &surface_view,
        );
        queue.submit([encoder.finish()]);
        // wgpu 30: presentation is scheduled on the queue, not the texture.
        queue.present(surface_texture);
        self.stats.frames_presented += 1;
        Ok(visible)
    }

    /// Render the scene to an offscreen texture, read the pixels back,
    /// and save a PNG. This is the GPU half of the CPU/GPU tolerance
    /// harness (decision 002): same `Scene` in, comparable PNG out.
    /// Returns the number of visible nodes drawn plus the pixel buffer.
    pub fn snapshot(
        &mut self,
        scene: &hamesh_scene::Scene,
        path: &str,
    ) -> Result<(usize, tiny_skia::Pixmap), GpuError> {
        let width = scene.viewport_w.max(1.0) as u32;
        let height = scene.viewport_h.max(1.0) as u32;
        let dev_id = self.surface.dev_id;
        let device = self.context.devices[dev_id].device.clone();
        let queue = self.context.devices[dev_id].queue.clone();
        let vscene = build_vscene(&mut self.text_cache, scene);
        let visible = scene.nodes.iter().filter(|n| n.visible).count();

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hamesh-snapshot"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let params = RenderParams {
            base_color: node_color([10, 15, 17, 255]),
            width,
            height,
            antialiasing_method: AaConfig::Msaa16,
        };
        self.renderer
            .render_to_texture(&device, &queue, &vscene, &view, &params)
            .map_err(|e| GpuError::Vello(format!("snapshot render: {e:?}")))?;

        // COPY_BYTES_PER_ROW_ALIGNMENT = 256.
        let padded = (width * 4).div_ceil(256) * 256;
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hamesh-snapshot-readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("hamesh-snapshot-copy"),
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GpuError::Surface(format!("poll: {e:?}")))?;
        rx.recv()
            .map_err(|e| GpuError::Surface(format!("map recv: {e:?}")))?
            .map_err(|e| GpuError::Surface(format!("map: {e:?}")))?;
        let data = {
            let mapped = buf
                .slice(..)
                .get_mapped_range()
                .map_err(|e| GpuError::Surface(format!("range: {e:?}")))?;
            mapped.to_vec()
        };
        buf.unmap();

        // Unpad rows; vello emits straight alpha, tiny-skia takes it.
        let mut pix = tiny_skia::Pixmap::new(width, height).expect("snapshot pixmap");
        let dst = pix.data_mut();
        for y in 0..height as usize {
            let src_row = y * padded as usize;
            let dst_row = y * width as usize * 4;
            dst[dst_row..dst_row + width as usize * 4]
                .copy_from_slice(&data[src_row..src_row + width as usize * 4]);
        }
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).expect("snapshot dir");
            }
        }
        pix.save_png(path)
            .map_err(|e| GpuError::Surface(format!("save_png: {e:?}")))?;
        Ok((visible, pix))
    }
}
