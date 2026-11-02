//! Cached, feedback-free FXAA quality39. Raw capture remains distinct from the
//! public filtered texture so preserve frames never filter their previous result.
use super::*;
use deadlib_render_core::FxaaQuality;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Push {
    inverse_source: [f32; 2],
    subpix: f32,
    edge_threshold: f32,
    edge_threshold_min: f32,
    opaque: f32,
    padding: [f32; 2],
}
impl Push {
    fn new(width: u32, height: u32, quality: FxaaQuality, alpha: bool) -> Self {
        Self {
            inverse_source: [1.0 / width.max(1) as f32, 1.0 / height.max(1) as f32],
            subpix: quality.subpix,
            edge_threshold: quality.edge_threshold,
            edge_threshold_min: quality.edge_threshold_min,
            opaque: if alpha { 0.0 } else { 1.0 },
            padding: [0.0; 2],
        }
    }
}

/// Returns an earlier *public* target. None means the current target's retained
/// raw attachment. Validate before allocation/recording so failure cannot submit
/// a partial graph or accidentally read the current color attachment.
pub(super) fn source_index(
    passes: &[RenderTargetFrame],
    index: usize,
) -> Result<Option<usize>, Box<dyn Error>> {
    let Some(source) = passes[index].postprocess.and_then(|pass| pass.source) else {
        return Ok(None);
    };
    if !is_render_target_texture(source) {
        return Err("Vulkan FXAA source must be an earlier render target".into());
    }
    let source = render_target_base_handle(source);
    let Some(found) = passes[..index]
        .iter()
        .position(|pass| pass.texture_handle == source)
    else {
        return Err("Vulkan FXAA source is missing, current, or later in the render graph".into());
    };
    if passes
        .iter()
        .filter(|pass| pass.texture_handle == source)
        .count()
        != 1
    {
        return Err("Vulkan FXAA source aliases the destination or another target".into());
    }
    Ok(Some(found))
}

pub(super) fn create_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
) -> Result<PipelinePair, Box<dyn Error>> {
    let vert = create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/vulkan_fxaa.vert.spv")),
    )?;
    let frag = match create_shader_module(
        device,
        include_bytes!(concat!(env!("OUT_DIR"), "/vulkan_fxaa.frag.spv")),
    ) {
        Ok(frag) => frag,
        Err(error) => {
            unsafe { device.destroy_shader_module(vert, None) };
            return Err(error.into());
        }
    };
    let mut created_layout = None;
    let result = (|| {
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert)
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag)
                .name(c"main"),
        ];
        let vertex = vk::PipelineVertexInputStateCreateInfo::default();
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        // Fullscreen replacement: retain the source's filtered alpha, or force
        // opaque for RGB targets. Existing seven scene blend pipelines are untouched.
        let attachment = color_blend_for(BlendMode::Replace, true);
        let blend = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&attachment));
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let push = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .size(mem::size_of::<Push>() as u32);
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&set_layout))
            .push_constant_ranges(std::slice::from_ref(&push));
        // SAFETY: all referenced handles belong to the live device; slices live through each call.
        let layout = unsafe { device.create_pipeline_layout(&layout_info, None)? };
        created_layout = Some(layout);
        let depth = vk::PipelineDepthStencilStateCreateInfo::default();
        let pipeline = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex)
            .input_assembly_state(&assembly)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&multisample)
            .depth_stencil_state(&depth)
            .color_blend_state(&blend)
            .dynamic_state(&dynamic)
            .layout(layout)
            .render_pass(render_pass)
            .subpass(0);
        let pipe = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline], None)
                .map_err(|(partial, error)| {
                    for pipeline in partial {
                        if pipeline != vk::Pipeline::null() {
                            device.destroy_pipeline(pipeline, None);
                        }
                    }
                    error
                })?[0]
        };
        Ok(PipelinePair { layout, pipe })
    })();
    // SAFETY: creation has finished; pipelines retain no shader module ownership.
    unsafe {
        device.destroy_shader_module(vert, None);
        device.destroy_shader_module(frag, None);
        if result.is_err() {
            if let Some(layout) = created_layout {
                device.destroy_pipeline_layout(layout, None);
            }
        }
    }
    result
}

pub(super) fn record(
    state: &State,
    cmd: vk::CommandBuffer,
    destination: &OffscreenTarget,
    source: &OffscreenTarget,
    quality: FxaaQuality,
    alpha: bool,
    depth: bool,
) {
    let device = state.device.as_ref().unwrap();
    let capture = &state.capture_passes[destination.float_color as usize][depth as usize];
    let pipeline = &capture.fxaa;
    let extent = vk::Extent2D {
        width: destination.width,
        height: destination.height,
    };
    let clear = [
        vk::ClearValue {
            color: vk::ClearColorValue { float32: [0.0; 4] },
        },
        vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        },
    ];
    let begin = vk::RenderPassBeginInfo::default()
        .render_pass(capture.clear)
        .framebuffer(destination.framebuffer)
        .render_area(vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent,
        })
        .clear_values(&clear[..1 + depth as usize]);
    let descriptor = texture_descriptor_set(
        &source.texture,
        source.handle,
        false,
        Some(MeshSampler {
            filter: SamplerFilter::Linear,
            wrap: SamplerWrap::Clamp,
        }),
    );
    let push = Push::new(source.width, source.height, quality, alpha);
    // SAFETY: source is a distinct completed attachment in SHADER_READ layout.
    // The capture render pass dependencies publish its writes before this read
    // and this pass's writes before later consumers. Resources outlive submission.
    unsafe {
        device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
        // FXAA covers the entire allocation, including cleared viewport padding.
        // Its own fullscreen vertex shader uses top-left UVs with positive height.
        device.cmd_set_viewport(
            cmd,
            0,
            &[vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: extent.width as f32,
                height: extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }],
        );
        device.cmd_set_scissor(
            cmd,
            0,
            &[vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent,
            }],
        );
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline.pipe);
        device.cmd_bind_descriptor_sets(
            cmd,
            vk::PipelineBindPoint::GRAPHICS,
            pipeline.layout,
            0,
            &[descriptor],
            &[],
        );
        device.cmd_push_constants(
            cmd,
            pipeline.layout,
            vk::ShaderStageFlags::FRAGMENT,
            0,
            bytemuck::bytes_of(&push),
        );
        device.cmd_draw(cmd, 3, 1, 0, 0);
        device.cmd_end_render_pass(cmd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadlib_render_core::{
        FxaaPass, render_target_sample_handle, render_target_texture_handle,
    };
    fn quality() -> FxaaQuality {
        FxaaQuality {
            subpix: 0.4,
            edge_threshold: 0.125,
            edge_threshold_min: 0.01,
        }
    }
    fn target(
        id: u64,
        width: u32,
        height: u32,
        source: Option<Option<TextureHandle>>,
    ) -> RenderTargetFrame {
        RenderTargetFrame {
            texture_handle: render_target_texture_handle(id),
            width,
            height,
            viewport: [width, height],
            float_color: false,
            alpha: true,
            depth: false,
            preserve: false,
            postprocess: source.map(|source| FxaaPass {
                source,
                quality: quality(),
            }),
            cameras: vec![],
            sprite_instances: vec![],
            mesh_vertices: vec![],
            tmesh_instances: vec![],
            tmesh_geometries: vec![],
            ops: vec![],
        }
    }
    #[test]
    fn explicit_source_resolves_previous_published_target_in_same_frame_including_filtered_source()
    {
        let mut passes = vec![
            target(1, 1024, 256, Some(None)),
            target(2, 854, 256, Some(Some(render_target_texture_handle(1)))),
            target(
                3,
                1100,
                256,
                Some(Some(render_target_sample_handle(
                    render_target_texture_handle(2),
                    true,
                ))),
            ),
        ];
        passes[0].float_color = true;
        passes[1].float_color = false;
        assert_eq!(source_index(&passes, 0).unwrap(), None);
        assert_eq!(source_index(&passes, 1).unwrap(), Some(0));
        assert_eq!(source_index(&passes, 2).unwrap(), Some(1));
        assert_ne!(passes[0].texture_handle, passes[1].texture_handle);
        assert_eq!(
            Push::new(passes[0].width, passes[0].height, quality(), true).inverse_source,
            [1.0 / 1024.0, 1.0 / 256.0]
        );
        // The graph lookup remains allocation-free for cached, valid frames.
        crate::perf::assert_no_churn(|| {
            for _ in 0..64 {
                assert_eq!(source_index(&passes, 2).unwrap(), Some(1));
            }
        });
    }
    #[test]
    fn feedback_future_missing_assets_and_duplicate_source_handles_are_refused() {
        let h = render_target_texture_handle;
        for source in [h(2), h(3), h(99), 15] {
            let passes = [
                target(1, 1024, 32, None),
                target(2, 854, 32, Some(Some(source))),
                target(3, 1100, 32, None),
            ];
            assert!(source_index(&passes, 1).is_err());
        }
        let duplicate = [
            target(1, 1024, 32, None),
            target(1, 854, 32, Some(Some(h(1)))),
        ];
        assert!(source_index(&duplicate, 1).is_err());
    }
    #[test]
    fn push_layout_matches_glsl_and_uses_source_dimensions_not_destination_or_viewport() {
        let q = quality();
        let push = Push::new(1024, 128, q, false);
        assert_eq!(mem::size_of::<Push>(), 32);
        assert_eq!(mem::offset_of!(Push, inverse_source), 0);
        assert_eq!(mem::offset_of!(Push, subpix), 8);
        assert_eq!(mem::offset_of!(Push, edge_threshold), 12);
        assert_eq!(mem::offset_of!(Push, edge_threshold_min), 16);
        assert_eq!(mem::offset_of!(Push, opaque), 20);
        assert_eq!(push.inverse_source, [1.0 / 1024.0, 1.0 / 128.0]);
        assert_eq!(push.subpix, q.subpix);
        assert_eq!(push.edge_threshold, q.edge_threshold);
        assert_eq!(push.edge_threshold_min, q.edge_threshold_min);
        assert_eq!(push.opaque, 1.0);
        assert_eq!(Push::new(0, 0, q, true).inverse_source, [1.0, 1.0]);
        // Fullscreen interpolation gives destination pixel-center UV; quality
        // stepping remains one source texel even when the target is resampled.
        for destination_width in [854., 1100., 3440.] {
            let uv = 0.5 / destination_width;
            assert!(uv > 0.);
            assert_ne!(uv, push.inverse_source[0]);
        }
    }
    #[test]
    fn fragment_wrapper_uses_shared_pinned_kernel_and_preserves_source_alpha() {
        let shader = include_str!("shaders/vulkan_fxaa.frag");
        let build = include_str!("../build.rs");
        assert_eq!(shader.matches("// FXAA_QUALITY39_SOURCE").count(), 1);
        assert!(build.contains("../../assets/shaders/editor/fxaa-quality39.glsl"));
        assert!(shader.contains("fxaa_quality39(v_uv,u_tex,pc.inverse_source,pc.subpix,pc.edge_threshold,pc.edge_threshold_min)"));
        assert!(shader.contains("if(pc.opaque>0.5) outColor.a=1.0;"));
        assert!(!shader.contains("outColor.rgb/=outColor.a"));
    }
}
