//! Fumen ColorUtils HSV arithmetic and GrooveAuthor PIU lane palette.
//! Inputs/outputs remain f32 until the separate XNA Color byte conversion.
fn equal(a: f32, b: f32) -> bool {
    a - 0.0000001 <= b && a + 0.0000001 >= b
}
pub fn desaturate([r, g, b]: [f32; 3], multiplier: f32) -> Result<[f32; 3], &'static str> {
    if ![r, g, b, multiplier].iter().all(|n| n.is_finite()) {
        return Err("Nonfinite PIU color");
    }
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let mut h = 0.0f32;
    let mut s = if equal(max, 0.0) {
        0.0
    } else {
        (max - min) / max
    };
    let v = max;
    if !equal(s, 0.0) {
        let d = max - min;
        h = if equal(r, max) {
            (g - b) / d
        } else if equal(g, max) {
            2.0 + (b - r) / d
        } else {
            4.0 + (r - g) / d
        };
        h *= (std::f64::consts::PI / 3.0) as f32;
        if h < 0.0 {
            h += (2.0 * std::f64::consts::PI) as f32;
        }
    }
    s = (s * multiplier).clamp(0.0, 1.0);
    if equal(s, 0.0) {
        return Ok([v, v, v]);
    }
    h = if equal(h, (std::f64::consts::PI * 2.0) as f32) {
        0.0
    } else {
        (f64::from(h * 3.0) / std::f64::consts::PI) as f32
    };
    let sextant = h.floor();
    let f = h - sextant;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    Ok(match sextant as i32 {
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        5 => [v, p, q],
        _ => [v, t, p],
    })
}
pub const TOP: [f32; 3] = [0.7019608, 0.1058824, 0.2156863];
pub const MIDDLE: [f32; 3] = [1.0, 0.9176471, 0.0];
pub const BOTTOM: [f32; 3] = [0.1058824, 0.2509804, 0.7019608];
pub fn lane_color(texture_index: usize) -> [f32; 3] {
    match texture_index % 5 {
        1 | 3 => TOP,
        2 => MIDDLE,
        _ => BOTTOM,
    }
}
