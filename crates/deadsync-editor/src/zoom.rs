//! GrooveAuthor ZoomManager numerical controller, pinned 48e18f71 (MIT).
//! Input and preference notifications are explicit, without UI/global state.
use crate::{Error, number::reference_equal};

pub const MIN_ZOOM: f64 = 0.000001;
pub const MAX_ZOOM: f64 = 1000000.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpacingMode {
    ConstantTime,
    ConstantRow,
    Variable,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Value {
    min: f64,
    max: f64,
    current: f64,
    desired: f64,
    initial: f64,
    start: f64,
}
impl Value {
    fn new(min: f64, max: f64, current: f64) -> Self {
        Self {
            min,
            max,
            current,
            desired: current,
            initial: current,
            start: 0.0,
        }
    }
    fn set(&mut self, value: f64) {
        self.current = value.clamp(self.min, self.max);
        self.desired = self.current;
    }
    fn bounds(&mut self, min: f64, max: f64) {
        self.min = min;
        self.max = max;
        // Match the original: a pending desired value is not reclamped when
        // the current value is already inside the new bounds.
        if self.current < min || self.current > max {
            self.set(self.current);
        }
    }
    fn scroll(&mut self, now: f64, multiplier: f64) {
        self.desired = if multiplier > 0.0 {
            self.desired * multiplier
        } else {
            self.desired / -multiplier
        }
        .clamp(self.min, self.max);
        self.start = now;
        self.initial = self.current;
    }
    fn update(&mut self, now: f64, duration: f64) {
        if reference_equal(self.current, self.desired) {
            return;
        }
        let end = self.start + duration;
        let value = if reference_equal(self.start, end) {
            if now >= end {
                self.desired
            } else {
                self.initial
            }
        } else {
            (self.initial + (now - self.start) / (end - self.start) * (self.desired - self.initial))
                .clamp(
                    self.initial.min(self.desired),
                    self.initial.max(self.desired),
                )
        };
        self.current = value.clamp(self.min, self.max);
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Manager {
    zoom: Value,
    time: Value,
    row: Value,
    variable: Value,
    cap: f64,
    limited: bool,
    duration: f64,
    multiplier: f64,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            zoom: Value::new(MIN_ZOOM, MAX_ZOOM, 1.0),
            time: Value::new(10.0, 100000.0, 300.0),
            row: Value::new(0.1, 10000.0, 2.0),
            variable: Value::new(10.0, 100000.0, 300.0),
            cap: 1.0,
            limited: false,
            duration: 0.1,
            multiplier: 1.2,
        }
    }
}
impl Manager {
    pub fn spacing_zoom(&self) -> f64 {
        self.zoom.current
    }
    pub fn size_zoom(&self) -> f64 {
        self.cap.min(self.zoom.current.min(1.0))
    }
    pub fn size_cap(&self) -> f64 {
        self.cap
    }
    pub fn spacing(&self, mode: SpacingMode) -> f64 {
        self.value(mode).current
    }
    pub fn interpolation_duration(&self) -> f64 {
        self.duration
    }
    pub fn zoom_multiplier(&self) -> f64 {
        self.multiplier
    }
    pub fn limit_to_size(&self) -> bool {
        self.limited
    }
    pub fn set_zoom(&mut self, value: f64) -> Result<(), Error> {
        finite(value)?;
        self.zoom.set(value);
        Ok(())
    }
    pub fn set_size_cap(&mut self, value: f64) -> Result<(), Error> {
        finite(value)?;
        self.cap = value.clamp(MIN_ZOOM, 1.0);
        self.refresh_bounds();
        Ok(())
    }
    pub fn set_limit_to_size(&mut self, value: bool) {
        self.limited = value;
        self.refresh_bounds();
    }
    /// External spacing notification: replace both current and desired values.
    pub fn set_spacing(&mut self, mode: SpacingMode, value: f64) -> Result<(), Error> {
        finite(value)?;
        self.value_mut(mode).set(value);
        Ok(())
    }
    pub fn set_interpolation_duration(&mut self, value: f64) -> Result<(), Error> {
        range(value, 0.0, 1.0)?;
        self.duration = value;
        Ok(())
    }
    pub fn set_zoom_multiplier(&mut self, value: f64) -> Result<(), Error> {
        range(value, 1.0, 4.0)?;
        self.multiplier = value;
        Ok(())
    }
    /// Zoom takes precedence over default-spacing changes when both are held.
    /// A keyboard zoom step can use delta +1/-1 through the same controller.
    pub fn process_input(
        &mut self,
        now: f64,
        delta: f32,
        zoom: bool,
        spacing: bool,
        mode: SpacingMode,
    ) -> Result<bool, Error> {
        finite(now)?;
        finite(f64::from(delta))?;
        if delta.abs() <= 0.0000001 || (!zoom && !spacing) {
            return Ok(false);
        }
        let magnitude =
            (1.0 + f64::from(delta.abs()) * (self.multiplier - 1.0)).clamp(1.0, self.multiplier);
        let multiplier = if delta < 0.0 { -magnitude } else { magnitude };
        if zoom {
            self.zoom.scroll(now, multiplier);
        } else {
            self.value_mut(mode).scroll(now, multiplier);
        }
        Ok(true)
    }
    pub fn update(&mut self, now: f64) -> Result<(), Error> {
        finite(now)?;
        for value in [
            &mut self.zoom,
            &mut self.time,
            &mut self.row,
            &mut self.variable,
        ] {
            value.update(now, self.duration);
        }
        Ok(())
    }
    fn refresh_bounds(&mut self) {
        self.zoom
            .bounds(MIN_ZOOM, if self.limited { self.cap } else { MAX_ZOOM });
    }
    fn value(&self, mode: SpacingMode) -> &Value {
        match mode {
            SpacingMode::ConstantTime => &self.time,
            SpacingMode::ConstantRow => &self.row,
            SpacingMode::Variable => &self.variable,
        }
    }
    fn value_mut(&mut self, mode: SpacingMode) -> &mut Value {
        match mode {
            SpacingMode::ConstantTime => &mut self.time,
            SpacingMode::ConstantRow => &mut self.row,
            SpacingMode::Variable => &mut self.variable,
        }
    }
}
fn finite(value: f64) -> Result<(), Error> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidEdit("Zoom input must be finite".into()))
    }
}
fn range(value: f64, min: f64, max: f64) -> Result<(), Error> {
    finite(value)?;
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(Error::InvalidEdit(
            "Zoom preference is outside its supported range".into(),
        ))
    }
}
