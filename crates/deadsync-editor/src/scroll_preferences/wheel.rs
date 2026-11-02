//! Decisions from Editor.ProcessInputForScrollingAndZooming (48e18f71).
//! Platform wheel normalization and chart-clock projection belong to the host.
use super::{Preferences, spacing::Preference};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intent {
    None,
    Zoom,
    /// Unknown enum values still consume spacing input in the original switch.
    Spacing(Option<crate::zoom::SpacingMode>),
    PlayingTime {
        delta: f64,
        stop: bool,
    },
    InterpolateTime(f64),
    InterpolateRows(f64),
    SnapEarlier,
    SnapLater,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub detents: f32,
    pub zoom_held: bool,
    pub spacing_held: bool,
    pub playing: bool,
    pub has_chart: bool,
    pub snapped: bool,
    pub spacing_zoom: f64,
    pub spacing_mode: Preference,
}
/// A finite, positive spacing zoom is guaranteed by ZoomManager. Reject invalid
/// host input instead of introducing NaN into the document's position clock.
pub fn decide(preferences: Preferences, input: Input) -> Intent {
    if !input.detents.is_finite()
        || input.detents.abs() <= 0.0000001
        || !input.spacing_zoom.is_finite()
        || input.spacing_zoom <= 0.0
    {
        return Intent::None;
    }
    if input.zoom_held {
        return Intent::Zoom;
    }
    if input.spacing_held {
        return Intent::Spacing(match input.spacing_mode {
            Preference::Unknown(_) => None,
            known => Some(known.effective()),
        });
    }
    let direction = if preferences.reverse() { -1.0 } else { 1.0 };
    let scale = -f64::from(input.detents) * direction / input.spacing_zoom;
    if input.playing {
        return if input.has_chart {
            Intent::PlayingTime {
                delta: preferences.wheel_time() * scale,
                stop: preferences.stop_playback(),
            }
        } else {
            Intent::None
        };
    }
    if input.snapped {
        return if (input.detents > 0.0) != preferences.reverse() {
            Intent::SnapEarlier
        } else {
            Intent::SnapLater
        };
    }
    if !input.has_chart {
        return Intent::None;
    }
    if input.spacing_mode == Preference::ConstantTime {
        Intent::InterpolateTime(preferences.wheel_time() * scale)
    } else {
        Intent::InterpolateRows(f64::from(preferences.wheel_rows()) * scale)
    }
}

/// One axis of EditorPosition's accumulating interpolation. The host resets
/// this value on explicit navigation and resolves its value through ChartClock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interpolation {
    current: f64,
    initial: f64,
    desired: f64,
    start: f64,
}
impl Interpolation {
    pub fn new(position: f64) -> Self {
        Self {
            current: position,
            initial: position,
            desired: position,
            start: 0.0,
        }
    }
    pub fn reset(&mut self, position: f64) {
        *self = Self::new(position);
    }
    pub fn begin(&mut self, previous_frame: f64, current: f64, delta: f64) {
        self.current = current;
        self.start = previous_frame;
        self.initial = current;
        self.desired += delta;
    }
    pub fn is_complete(self, current: f64) -> bool {
        crate::number::reference_equal(current, self.desired)
    }
    pub fn desired(self) -> f64 {
        self.desired
    }
    pub fn current(self) -> f64 {
        self.current
    }
    pub fn value(&mut self, now: f64, duration: f64) -> f64 {
        if crate::number::reference_equal(self.current, self.desired) {
            return self.current;
        }
        let end = self.start + duration;
        if crate::number::reference_equal(self.start, end) {
            let next = if now >= end {
                self.desired
            } else {
                self.initial
            };
            if !crate::number::reference_equal(self.current, next) {
                self.current = next;
            }
            return self.current;
        }
        let next = (self.initial
            + (now - self.start) / (end - self.start) * (self.desired - self.initial))
            .clamp(
                self.initial.min(self.desired),
                self.initial.max(self.desired),
            );
        // EditorPosition setters ignore changes within DoubleEquals tolerance.
        if !crate::number::reference_equal(self.current, next) {
            self.current = next;
        }
        self.current
    }
}

/// Native precise-wheel adaptation: 40 logical pixels per wheel detent.
/// Source SDL and Winit macOS expose different wheel units. This intentionally
/// documents a native sensitivity rather than claiming identical SDL rounding.
pub const LOGICAL_PIXELS_PER_DETENT: f64 = 40.0;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameInput {
    pending: f64,
}
impl FrameInput {
    pub fn lines(&mut self, vertical: f64) {
        self.add(vertical);
    }
    pub fn physical_pixels(&mut self, vertical: f64, window_scale: f64) {
        if window_scale.is_finite() && window_scale > 0.0 {
            self.add(vertical / window_scale / LOGICAL_PIXELS_PER_DETENT);
        }
    }
    fn add(&mut self, detents: f64) {
        let next = self.pending + detents;
        if detents.is_finite() && next.is_finite() && next.abs() <= f64::from(f32::MAX) {
            self.pending = next;
        }
    }
    /// Consume once per admitted frame. No rounding of fractional trackpad
    /// movement; snap mode's one-step-per-frame policy is applied by decide().
    pub fn take(&mut self) -> f32 {
        let pending = self.pending;
        self.pending = 0.0;
        pending as f32
    }
    /// Focus/modal/publication cancellation must not replay queued input later.
    pub fn cancel(&mut self) {
        self.pending = 0.0;
    }
}
