// TODO: check whether the names (get_framebuffer, etc.) conform to the rust API guidelines
// and maybe move queue into VkWindow, even if the only method that requires it is submit_command_buffer
// i think so bc while constantly changing queues can be useful, never when submitting to a swapchain

use vulkano::command_buffer::AutoCommandBuffer;
use vulkano::device::{Device, Queue};
use vulkano::framebuffer::{
    AttachmentDescription, Framebuffer, FramebufferAbstract, RenderPassAbstract, RenderPassDesc,
};
use vulkano::image::attachment::AttachmentImage;
use vulkano::image::SwapchainImage;
use vulkano::swapchain::{
    AcquireError, Capabilities, PresentMode, Surface, SurfaceTransform, Swapchain,
    SwapchainAcquireFuture, SwapchainCreationError,
};

use winit::Window;

use std::sync::Arc;

use crate::command_buffer;

pub struct VkWindow {
    device: Arc<Device>,
    swapchain: Arc<Swapchain<Window>>,
    framebuffers: Vec<Arc<FramebufferAbstract + Send + Sync>>,
    surface: Arc<Surface<Window>>,
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
    image_num: Option<usize>,
    future: Option<SwapchainAcquireFuture<Window>>,
}

impl VkWindow {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        surface: Arc<Surface<Window>>,
        render_pass: Arc<RenderPassAbstract + Send + Sync>,
        caps: Capabilities,
    ) -> Self {
        // create swapchain
        let (swapchain, images) = create_swapchain_and_images_from_scratch(
            device.clone(),
            queue.clone(),
            surface.clone(),
            caps,
        );

        // create framebuffers
        // currently makes some assumptions about the format of each image
        // there must be a way to do this based on the render pass
        // check RenderPassDesc
        let dims: (u32, u32) = surface
            .window()
            .get_inner_size()
            .unwrap()
            .to_physical(surface.window().get_hidpi_factor())
            .into();
        let framebuffers = create_framebuffers(
            device.clone(),
            [dims.0, dims.1],
            render_pass.clone(),
            images,
        );

        Self {
            device,
            swapchain,
            framebuffers,
            surface,
            render_pass,
            image_num: None,
            future: None,
        }
    }

    pub fn update_render_pass(&mut self, render_pass: Arc<RenderPassAbstract + Send + Sync>) {
        self.render_pass = render_pass;
    }

    pub fn next_framebuffer(&mut self) -> Arc<FramebufferAbstract + Send + Sync> {
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

        self.framebuffers[self.image_num.unwrap()].clone()
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
        // rebuilds swapchain and framebuffers
        let dimensions = self.get_dimensions();
        let result = match self.swapchain.recreate_with_dimension(dimensions) {
            Ok(r) => r,
            Err(SwapchainCreationError::UnsupportedDimensions) => {
                panic!("Unsupported dimensions: {:?}", dimensions);
            }
            Err(err) => panic!("{:?}", err),
        };

        self.swapchain = result.0;
        let images = result.1;
        self.framebuffers = create_framebuffers(
            self.device.clone(),
            dimensions,
            self.render_pass.clone(),
            images,
        );
    }

    pub fn submit_command_buffer(&mut self, queue: Arc<Queue>, command_buffer: AutoCommandBuffer) {
        if self.image_num.is_none() || self.future.is_none() {
            panic!("Image_num or future was none when trying to submit command buffer to swapchain. next_framebuffer was probably not called before.");
        }

        let result = command_buffer::submit_command_buffer_to_swapchain(
            queue.clone(),
            self.future.take().unwrap(),
            self.swapchain.clone(),
            self.image_num.take().unwrap(),
            command_buffer,
        );

        command_buffer::cleanup_swapchain_result(self.device.clone(), result);
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

fn create_framebuffers(
    device: Arc<Device>,
    dimensions: [u32; 2],
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
    images: Vec<Arc<SwapchainImage<Window>>>,
) -> Vec<Arc<FramebufferAbstract + Send + Sync>> {
    // this sucks.
    match render_pass.num_attachments() {
        0 => panic!("You provided an empty render pass to create_framebuffers"),
        1 => images
            .iter()
            .map(|image| {
                let fba: Arc<FramebufferAbstract + Send + Sync> = Arc::new(
                    Framebuffer::start(render_pass.clone())
                        .add(image.clone())
                        .unwrap()
                        .build()
                        .unwrap(),
                );

                fba
            })
            .collect(),
        2 => images
            .iter()
            .map(|image| {
                let attachment1 = create_image_for_desc(
                    device.clone(),
                    dimensions,
                    render_pass.attachment_desc(1).unwrap(),
                );
                let fba: Arc<FramebufferAbstract + Send + Sync> = Arc::new(
                    Framebuffer::start(render_pass.clone())
                        .add(image.clone())
                        .unwrap()
                        .add(attachment1)
                        .unwrap()
                        .build()
                        .unwrap(),
                );

                fba
            })
            .collect(),
        3 => images
            .iter()
            .map(|image| {
                let attachment1 = create_image_for_desc(
                    device.clone(),
                    dimensions,
                    render_pass.attachment_desc(1).unwrap(),
                );
                let attachment2 = create_image_for_desc(
                    device.clone(),
                    dimensions,
                    render_pass.attachment_desc(2).unwrap(),
                );
                let fba: Arc<FramebufferAbstract + Send + Sync> = Arc::new(
                    Framebuffer::start(render_pass.clone())
                        .add(image.clone())
                        .unwrap()
                        .add(attachment1)
                        .unwrap()
                        .add(attachment2)
                        .unwrap()
                        .build()
                        .unwrap(),
                );

                fba
            })
            .collect(),
        4 => images
            .iter()
            .map(|image| {
                let attachment1 = create_image_for_desc(
                    device.clone(),
                    dimensions,
                    render_pass.attachment_desc(1).unwrap(),
                );
                let attachment2 = create_image_for_desc(
                    device.clone(),
                    dimensions,
                    render_pass.attachment_desc(2).unwrap(),
                );
                let attachment3 = create_image_for_desc(
                    device.clone(),
                    dimensions,
                    render_pass.attachment_desc(3).unwrap(),
                );
                let fba: Arc<FramebufferAbstract + Send + Sync> = Arc::new(
                    Framebuffer::start(render_pass.clone())
                        .add(image.clone())
                        .unwrap()
                        .add(attachment1)
                        .unwrap()
                        .add(attachment2)
                        .unwrap()
                        .add(attachment3)
                        .unwrap()
                        .build()
                        .unwrap(),
                );

                fba
            })
            .collect(),
        _ => panic!("More than 1 attachment image is not supported"),
    }
}

fn create_image_for_desc(
    device: Arc<Device>,
    dimensions: [u32; 2],
    desc: AttachmentDescription,
) -> Arc<AttachmentImage> {
    AttachmentImage::transient_multisampled(device.clone(), dimensions, desc.samples, desc.format)
        .unwrap()
}

type SwapchainAndImages = (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>);
