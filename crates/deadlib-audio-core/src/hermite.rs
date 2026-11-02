//! Fumen's four-point float interpolation, with its original evaluation order.
#[inline]
pub(crate) fn interpolate(x0: f32, x1: f32, x2: f32, x3: f32, t: f32) -> f32 {
    let d = x1 - x2;
    let c1 = x2 - x0;
    let c3 = x3 - x0 + 3.0 * d;
    let c2 = -2.0 * d - c1 - c3;
    0.5 * ((c3 * t + c2) * t + c1) * t + x1
}
