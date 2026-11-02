//! Compile the production wrapper using actual headless desktop GL contexts.
use super::*;
use std::ffi::{CString, c_void};
use std::ptr;

#[link(name = "OpenGL", kind = "framework")]
unsafe extern "C" {
    fn CGLChoosePixelFormat(
        attributes: *const i32,
        format: *mut *mut c_void,
        count: *mut i32,
    ) -> i32;
    fn CGLDestroyPixelFormat(format: *mut c_void) -> i32;
    fn CGLCreateContext(format: *mut c_void, shared: *mut c_void, context: *mut *mut c_void)
    -> i32;
    fn CGLDestroyContext(context: *mut c_void) -> i32;
    fn CGLGetCurrentContext() -> *mut c_void;
    fn CGLSetCurrentContext(context: *mut c_void) -> i32;
}

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
}

struct Context {
    current: *mut c_void,
    previous: *mut c_void,
}

impl Context {
    fn new(path: GlPath) -> Self {
        // CGLPixelFormatAttribute: OpenGLProfile=99, Accelerated=73.
        let attributes = [
            99,
            if path == GlPath::Legacy {
                0x1000
            } else {
                0x3200
            },
            73,
            0,
        ];
        let mut format = ptr::null_mut();
        let mut count = 0;
        let mut current = ptr::null_mut();
        // SAFETY: CGL writes opaque handles and reads the zero-terminated list.
        unsafe {
            assert_eq!(
                CGLChoosePixelFormat(attributes.as_ptr(), &mut format, &mut count),
                0
            );
            assert!(!format.is_null());
            let status = CGLCreateContext(format, ptr::null_mut(), &mut current);
            CGLDestroyPixelFormat(format);
            assert_eq!(status, 0);
            let context = Self {
                current,
                previous: CGLGetCurrentContext(),
            };
            assert_eq!(CGLSetCurrentContext(context.current), 0);
            context
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: restore this thread's prior context before releasing our own.
        unsafe {
            CGLSetCurrentContext(self.previous);
            CGLDestroyContext(self.current);
        }
    }
}

#[test]
fn fxaa_production_resources_compile_and_link_on_legacy_and_modern_gl() {
    for path in [GlPath::Legacy, GlPath::Modern] {
        let _context = Context::new(path);
        // SAFETY: OpenGL.framework is linked above; RTLD_DEFAULT is -2 on
        // Darwin. The CGL context remains current until all resources are gone.
        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                let symbol = CString::new(name).unwrap();
                dlsym((-2isize) as *mut c_void, symbol.as_ptr()).cast_const()
            })
        };
        let resources = Resources::new(&gl, path, GlApi::Desktop)
            .unwrap_or_else(|error| panic!("{path:?} FXAA driver compilation: {error}"));
        assert_resized_draw(&gl, &resources);
        resources.destroy(&gl);
        // SAFETY: the same context is still current on this thread.
        assert_eq!(unsafe { gl.get_error() }, glow::NO_ERROR, "{path:?}");
    }
}

fn assert_resized_draw(gl: &glow::Context, resources: &Resources) {
    let color = [90u8, 140, 200, 51];
    let input = color.repeat(4 * 3);
    let mut output = vec![0u8; 7 * 5 * 4];
    // SAFETY: our context is current; both textures and the framebuffer are
    // exclusively owned here and released before the context is destroyed.
    unsafe {
        let source = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(source));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            4,
            3,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&input)),
        );
        let destination = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(destination));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            7,
            5,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        let framebuffer = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(destination),
            0,
        );
        assert_eq!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER),
            glow::FRAMEBUFFER_COMPLETE
        );
        let descriptor = SamplerDesc {
            filter: SamplerFilter::Linear,
            wrap: SamplerWrap::Clamp,
            mipmaps: false,
        };
        let source = Texture(
            TextureImages::Rgba(source),
            descriptor,
            AtomicU8::new(u8::MAX),
        );
        for alpha in [true, false] {
            resources.draw(
                gl,
                &source,
                [4, 3],
                framebuffer,
                [7, 5],
                deadlib_render_core::FxaaQuality {
                    subpix: 0.2,
                    edge_threshold: 0.166,
                    edge_threshold_min: 0.0833,
                },
                alpha,
            );
            gl.read_pixels(
                0,
                0,
                7,
                5,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut output)),
            );
            let expected = [
                color[0],
                color[1],
                color[2],
                if alpha { color[3] } else { 255 },
            ];
            assert!(
                output.chunks_exact(4).all(|pixel| pixel == expected),
                "resized FXAA must fill every output pixel with the requested alpha: {output:?}"
            );
        }
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.delete_framebuffer(framebuffer);
        gl.delete_texture(destination);
        gl.delete_texture(source.primary());
    }
}
