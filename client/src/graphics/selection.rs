use ash::{Device, vk};
use vk_shader_macros::include_glsl;

use super::{Base, as_bytes};
use common::{defer, graph_ray_casting::GraphCastHit};

const VERT: &[u32] = include_glsl!("shaders/selection.vert");
const FRAG: &[u32] = include_glsl!("shaders/selection.frag");

#[repr(C)]
#[derive(Clone, Copy)]
struct PushConstants {
    chunk_to_clip: na::Matrix4<f32>,
    voxel_and_dimension: [u32; 4],
    color: [f32; 4],
}

/// Draws short-lived voxel geometry used by selection and future geometry tools.
pub struct Selection {
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl Selection {
    pub fn new(gfx: &Base) -> Self {
        let device = &*gfx.device;
        unsafe {
            let vert = device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(VERT), None)
                .unwrap();
            let v_guard = defer(|| device.destroy_shader_module(vert, None));
            let frag = device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(FRAG), None)
                .unwrap();
            let f_guard = defer(|| device.destroy_shader_module(frag, None));

            let push_size = std::mem::size_of::<PushConstants>() as u32;
            debug_assert!(push_size <= 128);
            let pipeline_layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&[
                        vk::PushConstantRange {
                            stage_flags: vk::ShaderStageFlags::VERTEX,
                            offset: 0,
                            size: push_size,
                        },
                    ]),
                    None,
                )
                .unwrap();

            let entry_point = cstr!("main").as_ptr();
            let pipeline = device
                .create_graphics_pipelines(
                    gfx.pipeline_cache,
                    &[vk::GraphicsPipelineCreateInfo::default()
                        .stages(&[
                            vk::PipelineShaderStageCreateInfo {
                                stage: vk::ShaderStageFlags::VERTEX,
                                module: vert,
                                p_name: entry_point,
                                ..Default::default()
                            },
                            vk::PipelineShaderStageCreateInfo {
                                stage: vk::ShaderStageFlags::FRAGMENT,
                                module: frag,
                                p_name: entry_point,
                                ..Default::default()
                            },
                        ])
                        .vertex_input_state(&vk::PipelineVertexInputStateCreateInfo::default())
                        .input_assembly_state(
                            &vk::PipelineInputAssemblyStateCreateInfo::default()
                                .topology(vk::PrimitiveTopology::TRIANGLE_LIST),
                        )
                        .viewport_state(
                            &vk::PipelineViewportStateCreateInfo::default()
                                .scissor_count(1)
                                .viewport_count(1),
                        )
                        .rasterization_state(
                            &vk::PipelineRasterizationStateCreateInfo::default()
                                .cull_mode(vk::CullModeFlags::NONE)
                                .polygon_mode(vk::PolygonMode::FILL)
                                .line_width(1.0),
                        )
                        .multisample_state(
                            &vk::PipelineMultisampleStateCreateInfo::default()
                                .rasterization_samples(vk::SampleCountFlags::TYPE_1),
                        )
                        .depth_stencil_state(
                            &vk::PipelineDepthStencilStateCreateInfo::default()
                                .depth_test_enable(true)
                                .depth_write_enable(false)
                                .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL),
                        )
                        .color_blend_state(
                            &vk::PipelineColorBlendStateCreateInfo::default().attachments(&[
                                vk::PipelineColorBlendAttachmentState {
                                    blend_enable: vk::TRUE,
                                    src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
                                    dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                                    color_blend_op: vk::BlendOp::ADD,
                                    src_alpha_blend_factor: vk::BlendFactor::ONE,
                                    dst_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                                    alpha_blend_op: vk::BlendOp::ADD,
                                    color_write_mask: vk::ColorComponentFlags::RGBA,
                                },
                            ]),
                        )
                        .dynamic_state(
                            &vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&[
                                vk::DynamicState::VIEWPORT,
                                vk::DynamicState::SCISSOR,
                            ]),
                        )
                        .layout(pipeline_layout)
                        .render_pass(gfx.render_pass)
                        .subpass(0)],
                    None,
                )
                .unwrap()
                .remove(0);
            gfx.set_name(pipeline, cstr!("geometry selection"));
            v_guard.invoke();
            f_guard.invoke();

            Self {
                pipeline_layout,
                pipeline,
            }
        }
    }

    pub unsafe fn draw(
        &self,
        device: &Device,
        cmd: vk::CommandBuffer,
        projection: &na::Matrix4<f32>,
        dimension: u32,
        hit: &GraphCastHit,
    ) {
        let constants = PushConstants {
            chunk_to_clip: projection * hit.chunk_to_view,
            voxel_and_dimension: [
                u32::from(hit.voxel_coords.0[0]),
                u32::from(hit.voxel_coords.0[1]),
                u32::from(hit.voxel_coords.0[2]),
                dimension,
            ],
            color: [0.18, 0.78, 1.0, 0.34],
        };
        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                as_bytes(&constants),
            );
            device.cmd_draw(cmd, 36, 1, 0, 0);
        }
    }

    pub unsafe fn destroy(&mut self, device: &Device) {
        unsafe {
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
