//! Source density color boundary: MonoGame Color(Vector4), then Fumen's
//! packed-byte interpolation. Raw preference vectors are never modified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub low: [u8; 4],
    pub high: [u8; 4],
    pub background: [u8; 4],
}
impl Palette {
    pub fn new(low: [f32; 4], high: [f32; 4], background: [f32; 4]) -> Self {
        Self {
            low: quantize(low),
            high: quantize(high),
            background: quantize(background),
        }
    }
    pub fn color(self, percent: f32) -> [f32; 4] {
        rgba(interpolate(self.low, self.high, percent))
    }
}
pub use crate::source_color::{quantize, rgba};
/// Preserve Fumen's packed uint expression, including cross-byte bits when
/// a negative measure duration produces a percentage outside0..1.
pub fn interpolate(low: [u8; 4], high: [u8; 4], percent: f32) -> [u8; 4] {
    let inverse = 1.0 - percent;
    let channels: [u32; 4] =
        std::array::from_fn(|i| (low[i] as f32 * inverse + high[i] as f32 * percent) as u32);
    (channels[0]
        | channels[1].wrapping_shl(8)
        | channels[2].wrapping_shl(16)
        | channels[3].wrapping_shl(24))
    .to_le_bytes()
}
