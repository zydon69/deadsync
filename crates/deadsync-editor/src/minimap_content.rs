//! GrooveAuthor MiniMap raster policy (48e18f71), with caller-supplied colors.
//! This module does not select notes, resolve palette preferences or upload GPU data.
use crate::{Error, minimap_projection::Projection};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddResult {
    AboveTop,
    InRange,
    BelowBottom,
}
/// Original packed BGR interpolation preserves the end color's alpha.
fn blend(start: u32, end: u32, percent: f32) -> u32 {
    let mut result = end & 0xff00_0000;
    for shift in [0, 8, 16] {
        let a = ((start >> shift) & 255) as f32;
        let delta = ((end >> shift) & 255) as i32 - ((start >> shift) & 255) as i32;
        result |= ((a + delta as f32 * percent) as u32) << shift;
    }
    result
}
#[derive(Clone, Debug)]
pub struct Raster {
    width: u32,
    height: u32,
    note_width: u32,
    lanes: Vec<u32>,
    pixels: Vec<u32>,
    quantize: bool,
}
impl Raster {
    /// Resource limits are adapter guards, not limits in the original application.
    pub fn new(
        width: u32,
        height: u32,
        lanes: u32,
        note_width: u32,
        note_spacing: u32,
        quantize: bool,
        background: u32,
    ) -> Result<Self, Error> {
        if !(2..=1024).contains(&width)
            || !(3..=16384).contains(&height)
            || lanes > 64
            || note_width > 1024
            || note_spacing > 1024
        {
            return Err(Error::InvalidEdit("Invalid minimap raster geometry".into()));
        }
        let total = width - 2;
        let maximum = if lanes < 2 {
            0
        } else {
            (total.wrapping_sub(lanes * note_width) as i32) / (lanes - 1) as i32
        };
        let spacing = maximum.min(note_spacing as i32);
        // C# promotes signed spacing * uint lanes to long, then casts to int.
        let span = (note_width as i64 * lanes as i64 + spacing as i64 * (lanes as i64 - 1))
            .min(total as i64);
        let start = 1 + ((total as i64 - span) >> 1);
        let positions = (0..lanes)
            .map(|lane| {
                (start + lane as i64 * (spacing as i64 + note_width as i64))
                    .min(i64::from(width) - 1 - i64::from(note_width))
                    .max(1) as u32
            })
            .collect();
        Ok(Self {
            width,
            height,
            note_width,
            lanes: positions,
            pixels: vec![background; width as usize * height as usize],
            quantize,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
    /// Replace a validated background without changing lane/geometry identity.
    pub fn replace_pixels(&mut self, pixels: Vec<u32>) -> Result<(), Error> {
        if pixels.len() != self.pixels.len() {
            return Err(Error::InvalidEdit("Invalid minimap background size".into()));
        }
        self.pixels = pixels;
        Ok(())
    }
    pub fn pixels_mut(&mut self) -> &mut [u32] { &mut self.pixels }
    pub fn lane_positions(&self) -> &[u32] {
        &self.lanes
    }
    pub fn project(
        projection: &Projection,
        coordinate: f64,
        force_unquantized: bool,
    ) -> Result<f64, Error> {
        if !coordinate.is_finite() {
            return Err(Error::InvalidEdit("Invalid minimap coordinate".into()));
        }
        let scale = f64::from(projection.visible_height) / projection.mini_range;
        let result = if projection.quantize && !force_unquantized {
            let value = (coordinate - projection.mini_start) * scale + 0.5;
            if !value.is_finite() || value < i64::MIN as f64 || value >= i64::MAX as f64 {
                return Err(Error::InvalidEdit("Minimap projection overflow".into()));
            }
            (value as i64) as f64
        } else {
            (coordinate - projection.mini_unquantized) * scale
        };
        if !result.is_finite() {
            return Err(Error::InvalidEdit("Minimap projection overflow".into()));
        }
        Ok(result)
    }
    fn valid_y(y: f64) -> Result<i32, Error> {
        if !y.is_finite() || y < i32::MIN as f64 || y >= i32::MAX as f64 {
            return Err(Error::InvalidEdit("Invalid minimap pixel".into()));
        }
        Ok(y.floor() as i32)
    }
    fn paint(&mut self, y: i32, x: u32, width: u32, color: u32) {
        let start = y as usize * self.width as usize + x as usize;
        self.pixels[start..start + width as usize].fill(color);
    }
    pub fn line(
        &mut self,
        y: f64,
        x: u32,
        width: u32,
        color: u32,
        force_unquantized: bool,
    ) -> Result<AddResult, Error> {
        let mut row = Self::valid_y(y)?;
        if x.checked_add(width).is_none_or(|end| end > self.width) {
            return Err(Error::InvalidEdit("Invalid minimap line width".into()));
        }
        if row < 1 {
            return Ok(AddResult::AboveTop);
        }
        if row >= self.height as i32 - 1 {
            return Ok(AddResult::BelowBottom);
        }
        if width == 0 {
            return Ok(AddResult::InRange);
        }
        if self.quantize && !force_unquantized {
            self.paint(row, x, width, color);
            return Ok(AddResult::InRange);
        }
        let percent = (1.0 - (y - row as f64)) as f32;
        // Preserve the original previousData=0 / color=0 cache semantics.
        for (row, percent) in [(row, percent), {
            row += 1;
            (row, 1.0 - percent)
        }] {
            if row >= self.height as i32 - 1 {
                break;
            }
            let mut previous = 0;
            let mut output = 0;
            for column in x..x + width {
                let index = row as usize * self.width as usize + column as usize;
                let background = self.pixels[index];
                if background != previous {
                    previous = background;
                    output = blend(background, color, percent);
                }
                self.pixels[index] = output;
            }
        }
        Ok(AddResult::InRange)
    }
    pub fn short_note(&mut self, lane: u32, y: f64, color: u32) -> Result<AddResult, Error> {
        let x = *self
            .lanes
            .get(lane as usize)
            .ok_or_else(|| Error::InvalidEdit("Invalid minimap lane".into()))?;
        self.line(y, x, self.note_width.min(self.width - 2), color, false)
    }
    /// The caller passes the original end projection; the hold adds one pixel.
    pub fn hold(
        &mut self,
        lane: u32,
        start: f64,
        end: f64,
        head: u32,
        body: u32,
    ) -> Result<AddResult, Error> {
        let x = *self
            .lanes
            .get(lane as usize)
            .ok_or_else(|| Error::InvalidEdit("Invalid minimap lane".into()))?;
        let width = self.note_width.min(self.width - 2);
        self.region(
            x,
            x + width.saturating_sub(1),
            start,
            end + 1.0,
            head,
            Some(body),
            width == 0,
        )
    }
    pub fn rect(
        &mut self,
        x: u32,
        end_x: u32,
        start: f64,
        end: f64,
        color: u32,
    ) -> Result<AddResult, Error> {
        self.region(x, end_x, start, end, color, None, end_x < x)
    }
    fn region(
        &mut self,
        x: u32,
        end_x: u32,
        start: f64,
        end: f64,
        head: u32,
        body: Option<u32>,
        empty: bool,
    ) -> Result<AddResult, Error> {
        let start_int = Self::valid_y(start)?;
        Self::valid_y(end)?;
        if x >= self.width || (!empty && end_x >= self.width) {
            return Err(Error::InvalidEdit("Invalid minimap rectangle".into()));
        }
        let mut y = start_int;
        if y >= self.height as i32 - 1 {
            return Ok(AddResult::BelowBottom);
        }
        if end < 1.0 {
            return Ok(AddResult::AboveTop);
        }
        if empty {
            return Ok(AddResult::InRange);
        }
        let mut i = 0i64;
        if y < 1 {
            i += 1i64 - y as i64;
            y = 1;
        }
        while (y as f64) < end && y < self.height as i32 - 1 {
            let mut note = if i == 0 { head } else { body.unwrap_or(head) };
            let mut color = note;
            if !self.quantize {
                if let Some(body) = body {
                    if i == 1 {
                        let space = if end < y as f64 + 1.0 {
                            end - y as f64
                        } else {
                            1.0
                        };
                        note = blend(body, head, ((start - start_int as f64) / space) as f32);
                        color = note;
                    }
                }
                let background = self.pixels[y as usize * self.width as usize + x as usize];
                if i == 0 {
                    color = blend(note, background, (start - start_int as f64) as f32);
                } else if y as f64 + 1.0 >= end {
                    color = blend(note, background, (1.0 - (end - y as f64)) as f32);
                }
            }
            self.paint(y, x, end_x - x + 1, color);
            i += 1;
            y += 1;
        }
        Ok(AddResult::InRange)
    }
}
