//! Data displayed by original UIPerformance, independent of native UI actors.
use super::{
    monitor::{Monitor, TIMINGS, Timing},
    preferences::{Preferences, Scale},
};

pub struct Summary {
    /// Source plot values are f32 even though frame averages use f64.
    pub frames: Vec<[f32; TIMINGS]>,
    pub current: [f32; TIMINGS],
    pub averages: [f32; TIMINGS],
    pub maxima: [f32; TIMINGS],
    pub frame_average: f64,
    pub frames_in_last_second: usize,
    /// f32::MAX is the source ImGui automatic-scaling sentinel.
    pub plot_scale: f32,
}

impl Summary {
    pub fn capture(
        monitor: &Monitor,
        preferences: Preferences,
        ticks_per_second: u64,
    ) -> Option<Self> {
        if ticks_per_second == 0 {
            return None;
        }
        let frequency = ticks_per_second as f64;
        let (mut total_seconds, mut frame_sum, mut recent, mut count) = (0.0, 0.0, 0, 0);
        for frame in monitor.frames() {
            let seconds = frame.ticks(Timing::Frame) as f64 / frequency;
            total_seconds += seconds;
            if total_seconds <= 1.0 {
                frame_sum += seconds;
                recent += 1;
            }
            count += 1;
            // Original increments before comparing, including imported <=0.
            if count as i64 >= i64::from(preferences.max_frames) {
                break;
            }
        }
        if count == 0 {
            return None;
        }
        let mut result = Self {
            frames: Vec::with_capacity(count),
            current: [0.0; TIMINGS],
            averages: [0.0; TIMINGS],
            maxima: [0.0; TIMINGS],
            frame_average: if recent > 0 {
                frame_sum / recent as f64
            } else {
                frame_sum
            },
            frames_in_last_second: recent,
            plot_scale: f32::MAX,
        };
        let mut greatest = 0.0_f32;
        for (index, frame) in monitor.frames().enumerate() {
            let values = Timing::ALL.map(|timing| (frame.ticks(timing) as f64 / frequency) as f32);
            for (timing, value) in values.into_iter().enumerate() {
                if index == 0 {
                    result.current[timing] = value;
                }
                result.maxima[timing] = result.maxima[timing].max(value);
                if index < recent {
                    result.averages[timing] += value;
                }
                if index < count {
                    greatest = greatest.max(value);
                }
            }
            if index < count {
                result.frames.push(values);
            }
        }
        // IEEE division is intentional: original produces NaN for zero samples.
        for average in &mut result.averages {
            *average /= recent as f32;
        }
        result.plot_scale = match preferences.scale {
            Scale::Shared => greatest,
            Scale::Explicit => preferences.explicit_seconds as f32,
            Scale::Independent | Scale::Unknown(_) => f32::MAX,
        };
        Some(result)
    }
}
