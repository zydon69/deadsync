//! MonoGame's packed Color consumer boundary for source public float vectors.
//! Keep raw preference values separate from this render-only conversion.
/// Original Color(float4)/Color(Vector4): multiply the float32 component by 255, cast to int32,
/// then clamp integer channels. Executed .NET10 source fixtures include the maximum finite float32 value.
pub fn quantize(raw: [f32; 4]) -> [u8; 4] {
    raw.map(|value| ((value * 255.0) as i32).clamp(0, 255) as u8)
}
pub fn rgba(bytes: [u8; 4]) -> [f32; 4] {
    bytes.map(|v| v as f32 / 255.0)
}
