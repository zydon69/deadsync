//! Foot positions and facing from StepGraph.cs / PadData.cs (MIT, bdbc427f).
use super::graph::NodeState;
use crate::pad::Pad;

pub fn foot_position(pad: &Pad, lanes: [Option<u16>; 2]) -> (f32, f32, bool) {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut count = 0;
    for lane in lanes.into_iter().flatten() {
        let arrow = &pad.arrows[usize::from(lane)];
        x += arrow.x as f32 + 0.5;
        y += arrow.y as f32 + 0.5;
        count += 1;
    }
    if count > 1 {
        x /= count as f32;
        y /= count as f32;
    }
    (x, y, count > 1)
}

pub fn width(pad: &Pad) -> i32 {
    pad.arrows.iter().map(|arrow| arrow.x).max().unwrap()
        - pad.arrows.iter().map(|arrow| arrow.x).min().unwrap()
        + 1
}

pub fn side(pad: &Pad, state: NodeState, cutoff: f64) -> Option<(bool, [f64; 2])> {
    let min_x = pad.arrows.iter().map(|arrow| arrow.x).min().unwrap();
    let width = f64::from(width(pad));
    let mut side = None;
    let mut ys = [0.0; 2];
    let mut counts = [0; 2];
    for (foot, portions) in state.feet.iter().enumerate() {
        for portion in portions {
            if let Some(lane) = portion.lane {
                let arrow = &pad.arrows[usize::from(lane)];
                let fraction = (f64::from(arrow.x - min_x) + 0.5) / width;
                let left = fraction <= cutoff;
                let right = fraction >= 1.0 - cutoff;
                if !left && !right {
                    return None;
                }
                if side.is_some_and(|previous| previous != left) {
                    return None;
                }
                side = Some(left);
                ys[foot] += f64::from(arrow.y);
                counts[foot] += 1;
            }
        }
    }
    let side = side?;
    for foot in 0..2 {
        ys[foot] /= f64::from(counts[foot]);
    }
    Some((side, ys))
}

pub fn facing_inward(pad: &Pad, state: NodeState, cutoff: f64) -> bool {
    side(pad, state, cutoff)
        .is_some_and(|(left, ys)| if left { ys[0] < ys[1] } else { ys[0] > ys[1] })
}

pub fn facing_outward(pad: &Pad, state: NodeState, cutoff: f64) -> bool {
    side(pad, state, cutoff)
        .is_some_and(|(left, ys)| if left { ys[0] > ys[1] } else { ys[0] < ys[1] })
}

pub fn same_f64(a: f64, b: f64) -> bool {
    a == b
        || (a.is_sign_negative() == b.is_sign_negative() && a.to_bits().abs_diff(b.to_bits()) <= 1)
}
