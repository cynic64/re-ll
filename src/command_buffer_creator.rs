extern crate vulkano;
use vulkano::command_buffer::{AutoCommandBuffer, AutoCommandBufferBuilder, DynamicState};
use vulkano::device::{Device, Queue};
use vulkano::framebuffer::FramebufferAbstract;
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::descriptor::DescriptorSet;
use vulkano::format::ClearValue;
use vulkano::buffer::BufferAccess;

use std::sync::Arc;

#[derive(Clone)]
pub struct ConcreteObject {
    pub pipeline: Arc<GraphicsPipelineAbstract + Send + Sync>,
    pub dynamic_state: DynamicState,
    pub vertex_buffer: Arc<dyn BufferAccess + Send + Sync + 'static>,
    pub uniform_set: Arc<DescriptorSet + Send + Sync>,
}

pub fn create_command_buffer(device: Arc<Device>, queue: Arc<Queue>, framebuffer: Arc<FramebufferAbstract + Send + Sync>, clear_values: &[ClearValue], objects: &[ConcreteObject]) -> AutoCommandBuffer {
    let mut command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(
        device.clone(),
        queue.family(),
    )
    .unwrap()
    .begin_render_pass(
        framebuffer.clone(),
        false,
        clear_values.to_vec(),
    )
    .unwrap();

    for object in objects.iter() {
        command_buffer = command_buffer.draw(
            object.pipeline.clone(),
            &object.dynamic_state,
            vec![object.vertex_buffer.clone()],
            object.uniform_set.clone(),
            (),
        )
        .unwrap();
    }

    command_buffer
        .end_render_pass()
        .unwrap()
        .build()
        .unwrap()
}
