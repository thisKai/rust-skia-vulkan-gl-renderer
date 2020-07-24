use {
    skulpin::{
        winit::{
            self,
            dpi::{LogicalSize, PhysicalSize},
            event_loop::EventLoopWindowTarget,
        },
        CoordinateSystem, CreateRendererError,
    },
    std::{cell::RefCell, convert::TryInto},
};

pub use skia_safe;

pub enum WindowRenderer {
    Skulpin(SkulpinRenderer),
    Gl(GlRenderer),
}

impl WindowRenderer {
    pub fn new(event_loop: &EventLoopWindowTarget<()>, size: LogicalSize<u32>) -> Self {
        SkulpinRenderer::new(event_loop, size)
            .map(Self::Skulpin)
            .unwrap_or_else(|e| {
                eprintln!(
                    "Error during skulpin renderer construction: {:?}, Using OpenGL.",
                    e
                );
                Self::Gl(GlRenderer::new(event_loop, size))
            })
    }
    pub fn resize(&self, size: PhysicalSize<u32>) {
        match self {
            Self::Skulpin(_) => {}
            Self::Gl(renderer) => renderer.resize(size),
        }
    }
    pub fn paint<F: FnOnce(&mut skia_safe::Canvas)>(&self, f: F) -> Result<(), PaintError> {
        match self {
            Self::Skulpin(renderer) => renderer.paint(f).map_err(PaintError::Skulpin),
            Self::Gl(renderer) => renderer.paint(f).map_err(PaintError::Gl),
        }
    }
    pub fn request_repaint(&self) {
        match self {
            Self::Skulpin(renderer) => renderer.request_repaint(),
            Self::Gl(renderer) => renderer.request_repaint(),
        }
    }
    pub fn scale_factor(&self) -> f64 {
        match self {
            Self::Skulpin(renderer) => renderer.scale_factor(),
            Self::Gl(renderer) => renderer.scale_factor(),
        }
    }
}

#[derive(Debug)]
pub enum PaintError {
    Skulpin(skulpin::ash::vk::Result),
    Gl(glutin::ContextError),
}

pub struct SkulpinRenderer {
    winit_window: winit::window::Window,
    renderer: RefCell<skulpin::Renderer>,
}
impl SkulpinRenderer {
    pub fn new(
        event_loop: &EventLoopWindowTarget<()>,
        size: LogicalSize<u32>,
    ) -> Result<Self, CreateRendererError> {
        let winit_window = winit::window::WindowBuilder::new()
            .with_title("Skulpin")
            .with_inner_size(size)
            .build(&event_loop)
            .expect("Failed to create window");
        let skulpin_window = skulpin::WinitWindow::new(&winit_window);
        let renderer = skulpin::RendererBuilder::new()
            .use_vulkan_debug_layer(true)
            .coordinate_system(CoordinateSystem::Logical)
            .build(&skulpin_window)?;

        Ok(Self {
            winit_window,
            renderer: RefCell::new(renderer),
        })
    }
    pub fn paint<F: FnOnce(&mut skia_safe::Canvas)>(
        &self,
        f: F,
    ) -> Result<(), skulpin::ash::vk::Result> {
        let window = skulpin::WinitWindow::new(&self.winit_window);

        self.renderer
            .borrow_mut()
            .draw(&window, |canvas, _coordinate_system_helper| f(canvas))
    }
    pub fn request_repaint(&self) {
        self.winit_window.request_redraw()
    }
    pub fn scale_factor(&self) -> f64 {
        self.winit_window.scale_factor()
    }
}

pub struct GlRenderer {
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    gr_context: RefCell<skia_safe::gpu::Context>,
    fb_info: skia_safe::gpu::gl::FramebufferInfo,
    backend_render_target: RefCell<skia_safe::gpu::BackendRenderTarget>,
    surface: RefCell<skia_safe::Surface>,
}
impl GlRenderer {
    pub fn new(event_loop: &EventLoopWindowTarget<()>, size: LogicalSize<u32>) -> Self {
        use gl::types::*;

        let wb = glutin::window::WindowBuilder::new()
            .with_title("GL")
            .with_inner_size(size);

        let cb = glutin::ContextBuilder::new()
            .with_depth_buffer(0)
            .with_stencil_buffer(8)
            .with_pixel_format(24, 8)
            .with_double_buffer(Some(true))
            .with_gl_profile(glutin::GlProfile::Core);

        let windowed_context = cb.build_windowed(wb, &event_loop).unwrap();
        let windowed_context = unsafe { windowed_context.make_current().unwrap() };

        let pixel_format = windowed_context.get_pixel_format();

        gl::load_with(|s| windowed_context.get_proc_address(&s));

        let mut gr_context = skia_safe::gpu::Context::new_gl(None).unwrap();

        let mut fboid: GLint = 0;
        unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid) };

        let fb_info = skia_safe::gpu::gl::FramebufferInfo {
            fboid: fboid.try_into().unwrap(),
            format: skia_safe::gpu::gl::Format::RGBA8.into(),
        };

        let size = windowed_context.window().inner_size();
        let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_gl(
            (
                size.width.try_into().unwrap(),
                size.height.try_into().unwrap(),
            ),
            pixel_format.multisampling.map(|s| s.try_into().unwrap()),
            pixel_format.stencil_bits.try_into().unwrap(),
            fb_info,
        );
        let mut surface = skia_safe::Surface::from_backend_render_target(
            &mut gr_context,
            &backend_render_target,
            skia_safe::gpu::SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            None,
            None,
        )
        .unwrap();

        let sf = windowed_context.window().scale_factor() as f32;
        surface.canvas().scale((sf, sf));
        Self {
            windowed_context,
            gr_context: RefCell::new(gr_context),
            fb_info,
            backend_render_target: RefCell::new(backend_render_target),
            surface: RefCell::new(surface),
        }
    }
    pub fn resize(&self, size: PhysicalSize<u32>) {
        self.windowed_context.resize(size);

        let pixel_format = self.windowed_context.get_pixel_format();

        *self.backend_render_target.borrow_mut() = skia_safe::gpu::BackendRenderTarget::new_gl(
            (
                size.width.try_into().unwrap(),
                size.height.try_into().unwrap(),
            ),
            pixel_format.multisampling.map(|s| s.try_into().unwrap()),
            pixel_format.stencil_bits.try_into().unwrap(),
            self.fb_info,
        );
        *self.surface.borrow_mut() = skia_safe::Surface::from_backend_render_target(
            &mut self.gr_context.borrow_mut(),
            &self.backend_render_target.borrow(),
            skia_safe::gpu::SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            None,
            None,
        )
        .unwrap();

        self.windowed_context.window().request_redraw();
    }
    pub fn paint<F: FnOnce(&mut skia_safe::Canvas)>(
        &self,
        f: F,
    ) -> Result<(), glutin::ContextError> {
        let mut surface = self.surface.borrow_mut();
        let mut canvas = surface.canvas();
        f(&mut canvas);
        canvas.flush();
        self.windowed_context.swap_buffers()
    }
    pub fn request_repaint(&self) {
        self.windowed_context.window().request_redraw()
    }
    pub fn scale_factor(&self) -> f64 {
        self.windowed_context.window().scale_factor()
    }
}
