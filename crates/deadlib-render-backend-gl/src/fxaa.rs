//! Source FXAA draw resources; this context owns their entire lifetime.
use super::*;

#[cfg(all(test, target_os = "macos"))]
mod driver_tests;
pub(super) struct Resources {
    program: glow::Program,
    vbo: glow::Buffer,
    vao: Option<glow::VertexArray>,
    reciprocal: UniformLocation,
    quality: UniformLocation,
    sampler: UniformLocation,
}
impl Resources {
    pub fn new(gl: &glow::Context, path: GlPath, api: GlApi) -> Result<Self, String> {
        let version = api
            .shaders(path)
            .sprite_vert
            .lines()
            .next()
            .unwrap_or("#version 110");
        let legacy = path == GlPath::Legacy;
        let vertex = format!(
            "{version}\n{} vec2 a_pos;\n{} vec2 v_uv;\nvoid main(){{gl_Position=vec4(a_pos,0.0,1.0);v_uv=(a_pos+1.0)*0.5;}}",
            if legacy { "attribute" } else { "in" },
            if legacy { "varying" } else { "out" }
        );
        let mut body = include_str!("../../../assets/shaders/editor/fxaa-quality39.glsl")
            .to_owned()
            .replace("* 1)", "* 1.0)");
        if legacy {
            body = body.replace("texture(", "texture2D(");
        }
        let output = if legacy { "gl_FragColor" } else { "out_color" };
        let fragment = format!(
            "{version}\n#ifdef GL_ES\nprecision highp float;\n#endif\n{} vec2 v_uv;\nuniform sampler2D u_texture;\nuniform vec2 u_reciprocal;\nuniform vec4 u_quality;\n{}\n{body}\nvoid main(){{{output}=fxaa_quality39(v_uv,u_texture,u_reciprocal,u_quality.x,u_quality.y,u_quality.z);if(u_quality.w!=0.0){{{output}.a=1.0;}}}}",
            if legacy { "varying" } else { "in" },
            if legacy { "" } else { "out vec4 out_color;" }
        );
        // SAFETY: the caller owns the current GL context; all temporary shaders
        // are deleted after linking and every failure releases acquired handles.
        unsafe {
            let compile = |kind, source: &str| -> Result<glow::Shader, String> {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let error = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    Err(error)
                } else {
                    Ok(shader)
                }
            };
            let vertex = compile(glow::VERTEX_SHADER, &vertex)?;
            let fragment = match compile(glow::FRAGMENT_SHADER, &fragment) {
                Ok(value) => value,
                Err(error) => {
                    gl.delete_shader(vertex);
                    return Err(error);
                }
            };
            let program = match gl.create_program() {
                Ok(value) => value,
                Err(error) => {
                    gl.delete_shader(vertex);
                    gl.delete_shader(fragment);
                    return Err(error);
                }
            };
            gl.bind_attrib_location(program, 0, "a_pos");
            gl.attach_shader(program, vertex);
            gl.attach_shader(program, fragment);
            gl.link_program(program);
            gl.detach_shader(program, vertex);
            gl.detach_shader(program, fragment);
            gl.delete_shader(vertex);
            gl.delete_shader(fragment);
            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(error);
            }
            let locations = (
                gl.get_uniform_location(program, "u_reciprocal"),
                gl.get_uniform_location(program, "u_quality"),
                gl.get_uniform_location(program, "u_texture"),
            );
            let (Some(reciprocal), Some(quality), Some(sampler)) = locations else {
                gl.delete_program(program);
                return Err("Source FXAA uniforms missing".into());
            };
            let vbo = match gl.create_buffer() {
                Ok(value) => value,
                Err(error) => {
                    gl.delete_program(program);
                    return Err(error);
                }
            };
            let vao = if path == GlPath::Modern {
                match gl.create_vertex_array() {
                    Ok(value) => Some(value),
                    Err(error) => {
                        gl.delete_buffer(vbo);
                        gl.delete_program(program);
                        return Err(error);
                    }
                }
            } else {
                None
            };
            if let Some(vao) = vao {
                gl.bind_vertex_array(Some(vao));
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&[-1f32, -1., 3., -1., -1., 3.]),
                glow::STATIC_DRAW,
            );
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            Ok(Self {
                program,
                vbo,
                vao,
                reciprocal,
                quality,
                sampler,
            })
        }
    }
    pub fn draw(
        &self,
        gl: &glow::Context,
        source: &Texture,
        source_size: [u32; 2],
        framebuffer: glow::Framebuffer,
        destination: [u32; 2],
        quality: deadlib_render_core::FxaaQuality,
        alpha: bool,
    ) {
        // SAFETY: source and destination are distinct context-owned attachments.
        // Binding caches start fresh in each subsequent normal draw pass.
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.viewport(0, 0, destination[0] as i32, destination[1] as i32);
            gl.color_mask(true, true, true, true);
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(false);
            gl.disable(glow::BLEND);
            gl.use_program(Some(self.program));
            if let Some(vao) = self.vao {
                gl.bind_vertex_array(Some(vao));
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(source.primary()));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            let desc = SamplerDesc {
                filter: SamplerFilter::Linear,
                wrap: SamplerWrap::Clamp,
                mipmaps: false,
            };
            source.2.store(desc.slot() as u8, Ordering::Relaxed);
            gl.uniform_1_i32(Some(&self.sampler), 0);
            gl.uniform_2_f32(
                Some(&self.reciprocal),
                1.0 / source_size[0] as f32,
                1.0 / source_size[1] as f32,
            );
            gl.uniform_4_f32(
                Some(&self.quality),
                quality.subpix,
                quality.edge_threshold,
                quality.edge_threshold_min,
                if alpha { 0.0 } else { 1.0 },
            );
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.enable(glow::BLEND);
        }
    }
    pub fn destroy(self, gl: &glow::Context) {
        // SAFETY: cleanup runs on this context before its destruction, once.
        unsafe {
            gl.delete_program(self.program);
            gl.delete_buffer(self.vbo);
            if let Some(vao) = self.vao {
                gl.delete_vertex_array(vao);
            }
        }
    }
}
