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
    pub stats: GpuStats,
}

fn node_color(c: [u8; 4]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

fn to_vello(scene: &hamesh_scene::Scene) -> VelloScene {
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
    }
    v
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
        let vscene = to_vello(scene);
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
}
