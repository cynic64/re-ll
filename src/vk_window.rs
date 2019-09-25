// TODO: check whether the names (get_framebuffer, etc.) conform to the rust API guidelines
// and maybe move queue into VkWindow, even if the only method that requires it is submit_command_buffer
// i think so bc while constantly changing queues can be useful, never when submitting to a swapchain

use vulkano::device::{Device, Queue};
use vulkano::framebuffer::RenderPassAbstract;
use vulkano::image::SwapchainImage;
use vulkano::swapchain::{
    AcquireError, Capabilities, PresentMode, Surface, SurfaceTransform, Swapchain,
    SwapchainAcquireFuture, SwapchainCreationError,
};
use vulkano::sync;
use vulkano::sync::{FlushError, GpuFuture};

use winit::Window;

use std::sync::Arc;

pub struct VkWindow {
    device: Arc<Device>,
    swapchain: Arc<Swapchain<Window>>,
    images: Vec<Arc<SwapchainImage<Window>>>,
    surface: Arc<Surface<Window>>,
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    image_num: Option<usize>,
    future: Option<SwapchainAcquireFuture<Window>>,
}

impl VkWindow {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        surface: Arc<Surface<Window>>,
        render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
        caps: Capabilities,
    ) -> Self {
        // create swapchain
        let (swapchain, images) = create_swapchain_and_images_from_scratch(
            device.clone(),
            queue.clone(),
            surface.clone(),
            caps,
        );

        Self {
            device,
            swapchain,
            images,
            surface,
            render_pass,
            image_num: None,
            future: None,
        }
    }

    pub fn set_render_pass(&mut self, new_render_pass: Arc<dyn RenderPassAbstract + Send + Sync>) {
        self.render_pass = new_render_pass;
    }

    pub fn next_image(&mut self) -> Arc<SwapchainImage<Window>> {
        // TODO: this does more than the name suggests, which is not so great
        let mut idx_and_future = None;
        while idx_and_future.is_none() {
            idx_and_future = match vulkano::swapchain::acquire_next_image(
                self.swapchain.clone(),
                // timeout
                None,
            ) {
                Ok(r) => Some(r),
                Err(AcquireError::OutOfDate) => {
                    self.rebuild();
                    None
                }
                Err(err) => panic!("{:?}", err),
            };
        }

        let idx_and_future = idx_and_future.unwrap();
        self.image_num = Some(idx_and_future.0);
        self.future = Some(idx_and_future.1);

        self.images[idx_and_future.0].clone()
    }

    pub fn get_dimensions(&self) -> [u32; 2] {
        let dims: (u32, u32) = self
            .surface
            .window()
            .get_inner_size()
            .unwrap()
            .to_physical(self.surface.window().get_hidpi_factor())
            .into();
        [dims.0, dims.1]
    }

    pub fn rebuild(&mut self) {
        let dimensions = self.get_dimensions();
        let result = match self.swapchain.recreate_with_dimension(dimensions) {
            Ok(r) => r,
            Err(SwapchainCreationError::UnsupportedDimensions) => {
                panic!("Unsupported dimensions: {:?}", dimensions);
            }
            Err(err) => panic!("{:?}", err),
        };

        self.swapchain = result.0;
        self.images = result.1;
    }

    pub fn get_future(&mut self) -> SwapchainAcquireFuture<Window> {
        self.future.take().unwrap()
    }

    pub fn present_image<F>(&mut self, queue: Arc<Queue>, future: F)
    where
        F: GpuFuture + 'static,
    {
        if self.image_num.is_none() {
            panic!("Image_num was none when trying to submit command buffer to swapchain. next_image was probably not called before.");
        }

        let result = future
            .then_swapchain_present(queue, self.swapchain.clone(), self.image_num.unwrap())
            .then_signal_fence_and_flush();

        let mut new_fut: Box<dyn GpuFuture> = match result {
            Ok(new_fut) => Box::new(new_fut),
            Err(FlushError::OutOfDate) => Box::new(sync::now(self.device.clone())),
            Err(e) => {
                println!("{:?}", e);
                Box::new(sync::now(self.device.clone()))
            }
        };

        new_fut.cleanup_finished();
    }

    pub fn get_surface(&self) -> Arc<Surface<Window>> {
        self.surface.clone()
    }
}

fn create_swapchain_and_images_from_scratch(
    device: Arc<Device>,
    queue: Arc<Queue>,
    surface: Arc<Surface<Window>>,
    caps: Capabilities,
) -> SwapchainAndImages {
    let image_format = caps.supported_formats[0].0;
    // TODO: try using other get_dimensions implementation
    let dimensions = caps.current_extent.unwrap_or([1024, 768]);

    match Swapchain::new(
        device,
        surface,
        caps.min_image_count,
        image_format,
        dimensions,
        1,
        caps.supported_usage_flags,
        &queue,
        SurfaceTransform::Identity,
        caps.supported_composite_alpha.iter().next().unwrap(),
        PresentMode::Immediate,
        true,
        None,
    ) {
        Ok(r) => r,
        // TODO: add dimensions to err msg
        Err(SwapchainCreationError::UnsupportedDimensions) => panic!("SwapchainCreationError::UnsupportedDimensions when creating initial swapchain. Should never happen."),
        Err(err) => panic!("{:?}", err),
    }
}

type SwapchainAndImages = (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>);
