//! MiniMap.UpdateBegin background/rim/editor region and UpdateEnd fade.
//! Projection is provided by the separately validated viewport model.
use crate::{Error, minimap_content::Raster, minimap_projection::Projection};
pub struct Background {
    pub pixels: Vec<u32>,
    pub content_start_y: f64,
    pub content_end_y: f64,
    pub cursor_y: f64,
    pub drawable: bool,
}
fn blend(start: u32, end: u32, percent: f32) -> u32 {
    let mut value = end & 0xff000000;
    for shift in [0, 8, 16] {
        let a = ((start >> shift) & 255) as f32;
        let delta = ((end >> shift) & 255) as i32 - ((start >> shift) & 255) as i32;
        value |= ((a + delta as f32 * percent) as u32) << shift;
    }
    value
}
fn floor(value: f64) -> Result<i32, Error> {
    if !value.is_finite() || value < i32::MIN as f64 || value >= i32::MAX as f64 {
        return Err(Error::InvalidEdit(
            "Minimap background coordinate overflow".into(),
        ));
    }
    Ok(value.floor() as i32)
}
fn interior(pixels: &mut [u32], width: u32, y: i32, color: u32) {
    let start = y as usize * width as usize + 1;
    pixels[start..start + width as usize - 2].fill(color);
}
pub fn build(
    projection: &Projection,
    width: u32,
    content: (f64, f64),
    cursor: f64,
    grabbed: bool,
    hovered: bool,
) -> Result<Background, Error> {
    let height = projection.visible_height;
    if !(3..=1024).contains(&width)
        || !(3..=16384).contains(&height)
        || projection.mini_range <= 0.0
        || !projection.mini_range.is_finite()
        || content.0 > content.1
    {
        return Err(Error::InvalidEdit(
            "Invalid minimap background geometry".into(),
        ));
    }
    let content_start_y = Raster::project(projection, content.0, false)?;
    let content_end_y = Raster::project(projection, content.1, false)?;
    let cursor_y = Raster::project(projection, cursor, true)?;
    if (projection.editor_end - projection.editor_start) / projection.mini_range >= 1.0 {
        return Ok(Background {
            pixels: Vec::new(),
            content_start_y,
            content_end_y,
            cursor_y,
            drawable: false,
        });
    }
    let mut pixels = vec![0xff1e1e1e; width as usize * height as usize];
    for y in 0..height {
        for x in 0..width {
            if y == 0 || y == height - 1 || x == 0 || x == width - 1 {
                pixels[y as usize * width as usize + x as usize] = 0xffffffff;
            }
        }
    }
    let end = (height - 1) as f64;
    let y_end = end.min(content_start_y);
    let y_end_int = floor(y_end)?;
    for y in 1..y_end_int {
        interior(&mut pixels, width, y, 0xff020202);
    }
    if (1..height - 1).contains(&y_end_int) {
        interior(
            &mut pixels,
            width,
            y_end_int,
            blend(
                0xff020202,
                0xff1e1e1e,
                (1.0 - (y_end - y_end_int as f64)) as f32,
            ),
        );
    }
    let y_start = 1.0f64.max(content_end_y + 1.0);
    let start_int = floor(y_start)?;
    if start_int > 1 && start_int < height - 1 {
        interior(
            &mut pixels,
            width,
            start_int,
            blend(0xff020202, 0xff1e1e1e, (y_start - start_int as f64) as f32),
        );
    }
    for y in (start_int.saturating_add(1)).max(1)..height - 1 {
        interior(&mut pixels, width, y, 0xff020202);
    }
    let editor_start = Raster::project(projection, projection.editor_start, true)?;
    let editor_end =
        Raster::project(projection, projection.editor_end, true)?.max(editor_start + 1.0);
    let start_int = floor(editor_start)?;
    let end_int = floor(editor_end)?;
    let color = if grabbed {
        0xff3e3e3e
    } else if hovered {
        0xff373737
    } else {
        0xff303030
    };
    let start_copy = start_int.saturating_add(1);
    let end_copy = end_int;
    if start_copy < height - 1 && end_copy >= 1 && end_copy >= start_copy {
        for y in start_copy.max(1)..end_copy.min(height - 1) {
            interior(&mut pixels, width, y, color);
        }
    }
    if (1..height - 1).contains(&start_int) {
        let background = pixels[start_int as usize * width as usize + 1];
        interior(
            &mut pixels,
            width,
            start_int,
            blend(color, background, (editor_start - start_int as f64) as f32),
        );
    }
    if (1..height - 1).contains(&end_int) {
        let background = pixels[end_int as usize * width as usize + 1];
        interior(
            &mut pixels,
            width,
            end_int,
            blend(
                color,
                background,
                (1.0 - (editor_end - end_int as f64)) as f32,
            ),
        );
    }
    Ok(Background {
        pixels,
        content_start_y,
        content_end_y,
        cursor_y,
        drawable: true,
    })
}
/// Apply only after marker/note composition, as in UpdateEnd. Returns false
/// when original Draw would suppress an editor range wider than the minimap.
pub fn fade(pixels: &mut [u32], editor_fraction: f64, fade_start: f64) -> Result<bool, Error> {
    if !editor_fraction.is_finite()
        || editor_fraction < 0.0
        || !fade_start.is_finite()
        || !(0.0..=1.0).contains(&fade_start)
    {
        return Err(Error::InvalidEdit("Invalid minimap fade".into()));
    }
    if editor_fraction >= 1.0 {
        return Ok(false);
    }
    if editor_fraction >= fade_start {
        let alpha = if (1.0f64).to_bits().abs_diff(fade_start.to_bits()) <= 1 {
            1.0
        } else {
            (1.0 - (editor_fraction - fade_start) / (1.0 - fade_start)).clamp(0.0, 1.0)
        };
        let packed = ((alpha * 255.0) as u8 as u32) << 24;
        for pixel in pixels {
            *pixel = (*pixel & 0x00ffffff) | packed;
        }
    }
    Ok(true)
}
