//! Deterministic animation state translated from GrooveAuthor Receptor.cs
//! (MIT, pinned 48e18f71). Caller supplies monotonic animation seconds;
//! chart-time deltas only offset hit animations, never advance audio.
use crate::{Error, autoplay::ActionKind};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub autoplay_rim: bool,
    pub autoplay_glow: bool,
    pub autoplay_shrink: bool,
    pub tap_rim: bool,
    pub tap_shrink: bool,
    pub pulse: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            autoplay_rim: true,
            autoplay_glow: true,
            autoplay_shrink: true,
            tap_rim: true,
            tap_shrink: true,
            pulse: true,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub brightness: f32,
    pub tap_scale: f32,
    pub rim_scale: f32,
    pub rim_alpha: f32,
    pub glow_alpha: f32,
}
#[derive(Clone, Copy, Debug)]
struct Timer {
    start: f64,
    offset: f64,
}
impl Timer {
    fn age(self, now: f64) -> f32 {
        (now - self.start + self.offset) as f32
    }
}
#[derive(Clone, Debug, Default)]
pub struct State {
    held: bool,
    autoplay_held: bool,
    tap: Option<Timer>,
    rim: Option<Timer>,
    glow: Option<Timer>,
}
impl State {
    fn rim_held(&self, p: Preferences) -> bool {
        (p.tap_rim && self.held) || (p.autoplay_rim && self.autoplay_held)
    }
    pub fn autoplay(
        &mut self,
        kind: ActionKind,
        now: f64,
        offset: f64,
        p: Preferences,
    ) -> Result<(), Error> {
        finite(now, offset)?;
        let was = self.rim_held(p);
        let timer = Some(Timer { start: now, offset });
        match kind {
            ActionKind::Down => {
                self.autoplay_held = true;
                if p.autoplay_shrink {
                    self.tap = timer;
                }
                if p.autoplay_glow {
                    self.glow = timer;
                }
            }
            ActionKind::Up => {
                self.autoplay_held = false;
                if p.autoplay_rim && was && !self.rim_held(p) {
                    self.rim = timer;
                }
                if p.autoplay_glow {
                    self.glow = timer;
                }
            }
            ActionKind::Cancel => {
                self.autoplay_held = false;
                if p.autoplay_rim && was && !self.rim_held(p) {
                    self.rim = Some(Timer {
                        start: now,
                        offset: 0.0,
                    });
                }
                self.glow = None;
            }
        }
        Ok(())
    }
    pub fn manual(&mut self, pressed: bool, now: f64, p: Preferences) -> Result<(), Error> {
        finite(now, 0.0)?;
        let was = self.rim_held(p);
        self.held = pressed;
        if pressed && p.tap_shrink {
            self.tap = Some(Timer {
                start: now,
                offset: 0.0,
            });
        }
        if !pressed && p.tap_rim && was && !self.rim_held(p) {
            self.rim = Some(Timer {
                start: now,
                offset: 0.0,
            });
        }
        Ok(())
    }
    pub fn sample(
        &mut self,
        now: f64,
        playing: bool,
        chart_row: f64,
        p: Preferences,
    ) -> Result<Visual, Error> {
        finite(now, chart_row)?;
        let mut visual = Visual {
            brightness: 0.8,
            tap_scale: 1.0,
            rim_scale: 1.0,
            rim_alpha: if self.rim_held(p) { 1.0 } else { 0.0 },
            glow_alpha: 0.0,
        };
        if let Some(timer) = self.tap {
            let t = timer.age(now);
            if t > 0.02f32 + 0.08f32 {
                self.tap = None;
            } else if t < 0.02 {
                visual.tap_scale = lerp(1.0, 0.9, t / 0.02);
            } else {
                visual.tap_scale = lerp(0.9, 1.0, (t - 0.02) / 0.08);
            }
        }
        if let Some(timer) = self.rim {
            let t = timer.age(now);
            if t > 0.2 {
                self.rim = None;
            } else {
                visual.rim_alpha = lerp(1.0, 0.0, t / 0.2);
                visual.rim_scale = lerp(1.0, 1.2, t / 0.2);
            }
        }
        if let Some(timer) = self.glow {
            let t = timer.age(now);
            if t > 0.3 {
                self.glow = None;
            } else {
                visual.glow_alpha = lerp(1.0, 0.0, t / 0.3);
            }
        }
        if playing && p.pulse {
            // Same positive beat phase as the source's negative-row adjustment,
            // without narrowing an arbitrary finite lead-in to an i32 cycle count.
            let beat = (chart_row.rem_euclid(48.0) / 48.0) as f32;
            visual.brightness = lerp(1.0, 0.8, beat / 0.3);
        }
        Ok(visual)
    }
}
fn finite(a: f64, b: f64) -> Result<(), Error> {
    if a.is_finite() && b.is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidEdit(
            "Nonfinite receptor animation time".into(),
        ))
    }
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    a + (b - a) * t
}
/// Receptor.GetAlpha: start fading at .02 size zoom; invisible at .01.
pub fn alpha(size_zoom: f64) -> Result<f32, Error> {
    finite(size_zoom, 0.0)?;
    Ok(((size_zoom - 0.01) / 0.01).clamp(0.0, 1.0) as f32)
}
