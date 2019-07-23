extern crate vulkano;
use vulkano::buffer::BufferAccess;
use vulkano::command_buffer::{
    AutoCommandBuffer, AutoCommandBufferBuilder, CommandBufferExecFuture, DynamicState,
};
use vulkano::descriptor::DescriptorSet;
use vulkano::device::{Device, Queue};
use vulkano::format::ClearValue;
use vulkano::framebuffer::FramebufferAbstract;
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::swapchain::{PresentFuture, Swapchain};
use vulkano::sync::{FenceSignalFuture, FlushError, GpuFuture};

use std::sync::Arc;

#[derive(Clone)]
pub struct ConcreteObject {
    pub pipeline: Arc<GraphicsPipelineAbstract + Send + Sync>,
    pub dynamic_state: DynamicState,
    pub vertex_buffer: Arc<dyn BufferAccess + Send + Sync + 'static>,
    pub uniform_set: Arc<DescriptorSet + Send + Sync>,
}

pub fn create_command_buffer(
    device: Arc<Device>,
    queue: Arc<Queue>,
    framebuffer: Arc<FramebufferAbstract + Send + Sync>,
    clear_values: &[ClearValue],
    objects: &[ConcreteObject],
) -> AutoCommandBuffer {
    let mut command_buffer =
        AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family())
            .unwrap()
            .begin_render_pass(framebuffer.clone(), false, clear_values.to_vec())
            .unwrap();

    for object in objects.iter() {
        command_buffer = command_buffer
            .draw(
                object.pipeline.clone(),
                &object.dynamic_state,
                vec![object.vertex_buffer.clone()],
                object.uniform_set.clone(),
                (),
            )
            .unwrap();
    }

    command_buffer.end_render_pass().unwrap().build().unwrap()
}

pub fn submit_command_buffer_to_swapchain<W, F>(
    device: Arc<Device>,
    queue: Arc<Queue>,
    future: F,
    swapchain: Arc<Swapchain<W>>,
    image_num: usize,
    command_buffer: AutoCommandBuffer,
) -> Result<
    FenceSignalFuture<PresentFuture<CommandBufferExecFuture<F, AutoCommandBuffer>, W>>,
    FlushError,
>
where
    F: GpuFuture + 'static,
{
    submit_command_buffer_partial(queue.clone(), future, command_buffer)
        .then_swapchain_present(queue, swapchain, image_num)
        .then_signal_fence_and_flush()
}

// pub fn check_if_swapchain_outdated

fn submit_command_buffer_partial<F>(
    queue: Arc<Queue>,
    future: F,
    command_buffer: AutoCommandBuffer,
) -> CommandBufferExecFuture<F, AutoCommandBuffer>
where
    F: GpuFuture + 'static,
{
    future.then_execute(queue.clone(), command_buffer).unwrap()
}
