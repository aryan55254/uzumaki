use anyhow::{Context, Result};
use std::sync::Arc;
use vello::kurbo::{Affine, Circle, RoundedRect, Stroke};
use vello::peniko::Color;
use vello::{RenderParams, RendererOptions, Scene};

use winit::window::Window as WinitWindow;

use crate::gpu::GpuContext;

pub struct Window {
    pub(crate) winit_window: Arc<WinitWindow>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) surface_config: wgpu::SurfaceConfiguration,
    pub(crate) renderer: vello::Renderer,
    pub(crate) scene: Scene,
    valid_surface: bool,
}

impl Window {
    pub fn new(gpu: &GpuContext, winit_window: Arc<WinitWindow>) -> Result<Self> {
        let surface = gpu
            .instance
            .create_surface(winit_window.clone())
            .context("Error creating surface")?;

        let size = winit_window.inner_size();

        let valid_surface = size.width != 0 && size.height != 0;

        let surface_caps = surface.get_capabilities(&gpu.adapter);
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![format],
        };

        surface.configure(&gpu.device, &surface_config);

        let renderer = vello::Renderer::new(&gpu.device, RendererOptions::default())
            .context("Error creating renderer")?;

        let scene = Scene::new();

        Ok(Self {
            winit_window,
            renderer,
            surface,
            surface_config,
            scene,
            valid_surface,
        })
    }

    pub fn id(&self) -> winit::window::WindowId {
        self.winit_window.id()
    }

    pub(crate) fn paint_and_present(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.valid_surface {
            return;
        }

        let width = self.surface_config.width;
        let height = self.surface_config.height;

        // Build the scene
        self.scene.reset();
        Self::build_scene(&mut self.scene, width as f64, height as f64);

        // 1. Create an Rgba8Unorm intermediate texture with STORAGE_BINDING
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vello_target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        // 2. Render vello into the intermediate texture
        let render_params = RenderParams {
            base_color: Color::from_rgba8(30, 30, 46, 255),
            width,
            height,
            antialiasing_method: vello::AaConfig::Msaa16,
        };
        self.renderer
            .render_to_texture(device, queue, &self.scene, &target_view, &render_params)
            .expect("Failed to render");

        // 3. Blit from intermediate texture to surface
        let surface_texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => {
                self.surface.configure(device, &self.surface_config);
                match self.surface.get_current_texture() {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Failed to get surface texture: {e}");
                        return;
                    }
                }
            }
        };
        let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.surface_config.format),
            ..Default::default()
        });

        let blitter = wgpu::util::TextureBlitter::new(device, self.surface_config.format);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        blitter.copy(device, &mut encoder, &target_view, &surface_view);
        queue.submit([encoder.finish()]);
        surface_texture.present();
    }

    fn build_scene(scene: &mut Scene, w: f64, h: f64) {
        use vello::kurbo::Rect;

        // Background rounded rect
        let bg_rect = RoundedRect::from_rect(Rect::new(20.0, 20.0, w - 20.0, h - 20.0), 12.0);
        scene.fill(
            vello::peniko::Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(45, 45, 65, 255),
            None,
            &bg_rect,
        );
        scene.stroke(
            &Stroke::new(2.0),
            Affine::IDENTITY,
            Color::from_rgba8(100, 100, 180, 255),
            None,
            &bg_rect,
        );

        // Circle in center
        let cx = w / 2.0;
        let cy = h / 2.0;
        let radius = w.min(h) * 0.15;
        let circle = Circle::new((cx, cy), radius);
        scene.fill(
            vello::peniko::Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(180, 100, 255, 200),
            None,
            &circle,
        );
        scene.stroke(
            &Stroke::new(3.0),
            Affine::IDENTITY,
            Color::from_rgba8(220, 180, 255, 255),
            None,
            &circle,
        );

        // Smaller decorative circles
        for i in 0..4 {
            let angle = std::f64::consts::TAU * (i as f64) / 4.0;
            let dist = radius * 1.8;
            let small_cx = cx + angle.cos() * dist;
            let small_cy = cy + angle.sin() * dist;
            let small_circle = Circle::new((small_cx, small_cy), radius * 0.3);
            let colors = [
                Color::from_rgba8(255, 100, 100, 200),
                Color::from_rgba8(100, 255, 150, 200),
                Color::from_rgba8(100, 180, 255, 200),
                Color::from_rgba8(255, 220, 100, 200),
            ];
            scene.fill(
                vello::peniko::Fill::NonZero,
                Affine::IDENTITY,
                colors[i],
                None,
                &small_circle,
            );
        }
    }

    fn resize_surface(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(device, &self.surface_config);
    }

    pub(crate) fn on_resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        if width != 0 && height != 0 {
            self.resize_surface(device, width, height);
            self.valid_surface = true;
            true
        } else {
            self.valid_surface = false;
            false
        }
    }
}
