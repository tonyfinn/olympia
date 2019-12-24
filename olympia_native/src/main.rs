use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::new();
    let window = create_window(&event_loop);
    let _surface = pixels::wgpu::Surface::create(&window);
}

fn create_window(event_loop: &EventLoop<()>) -> winit::window::Window {
    let size: LogicalSize = (256, 256).into();

    winit::window::WindowBuilder::new()
        .with_title("Olympia GB")
        .with_inner_size(size)
        .with_min_inner_size(size)
        .with_resizable(false)
        .build(&event_loop)
        .unwrap()
}
