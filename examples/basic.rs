use skia_vulkan_gl_renderer::{skia_safe, winit, WindowRenderer};
pub fn main() {
    let event_loop = winit::event_loop::EventLoop::new();

    let window_size = winit::dpi::LogicalSize::new(800, 600);
    let renderer = WindowRenderer::new(&event_loop, window_size);

    event_loop.run(move |event, _, control_flow| match event {
        winit::event::Event::WindowEvent {
            event: winit::event::WindowEvent::CloseRequested,
            ..
        } => {
            *control_flow = winit::event_loop::ControlFlow::Exit;
        }
        winit::event::Event::RedrawRequested(_) => renderer
            .paint(|canvas| {
                canvas.clear(skia_safe::Color::from_argb(255, 255, 255, 255));
            })
            .unwrap(),
        _ => {}
    })
}
