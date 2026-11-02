//! Window-free access to the actual software prepare/raster stages for small,
//! solid-frame contract tests. No alternate blending implementation is used.
use super::*;
pub const WHITE_TEXTURE_HANDLE: TextureHandle = 1;
struct WhiteTexture(Texture);
impl TextureLookup for WhiteTexture {
    fn software_texture(&self, handle: TextureHandle) -> Option<&Texture> {
        (handle == WHITE_TEXTURE_HANDLE).then_some(&self.0)
    }
}
pub fn render_solid_frame(
    frame: &RenderFrame,
    projection: deadlib_render_core::ProjectionMatrix,
    width: usize,
    height: usize,
) -> Vec<[u8; 4]> {
    assert!((1..=512).contains(&width) && (1..=512).contains(&height));
    assert!(frame.render_targets.is_empty());
    let image = RgbaImage::from_raw(1, 1, vec![255; 4]).unwrap();
    let textures = WhiteTexture(create_texture(&image, SamplerDesc::default()).unwrap());
    for op in &frame.ops {
        if let DrawOp::Sprite(run) = op {
            assert_eq!(
                run.texture_handle, WHITE_TEXTURE_HANDLE,
                "solid-frame tests must bind the real white texture"
            );
        }
    }
    let (mut prepared, mut meshes, mut tmeshes) = (Vec::new(), Vec::new(), Vec::new());
    let fixed = prepare_objects(
        frame.into(),
        projection,
        &textures,
        width,
        height,
        &mut prepared,
        &mut meshes,
        &mut tmeshes,
        false,
    );
    let mut pixels = vec![pack_rgba(frame.clear_color); width * height];
    let mut depth = vec![1.0; width * height];
    draw_rows(
        frame.into(),
        &prepared,
        None,
        &meshes,
        &tmeshes,
        &textures,
        width,
        height,
        0,
        height,
        &mut byte_writer(&mut pixels),
        fixed,
        &mut depth,
    );
    pixels
        .into_iter()
        .map(|pixel| {
            [
                (pixel >> 16) as u8,
                (pixel >> 8) as u8,
                pixel as u8,
                (pixel >> 24) as u8,
            ]
        })
        .collect()
}
