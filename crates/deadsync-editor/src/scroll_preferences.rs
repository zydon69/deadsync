//! Original GrooveAuthor playback policy for navigation.
pub(crate) mod history;
pub use history::Update;
pub mod dragging;
pub mod restore;
pub mod spacing;
pub mod wheel;
use crate::Error;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    stop_playback: bool,
    dragging: dragging::Preferences,
    speeds: [f64; 3],
    reverse: bool,
    wheel_time: f64,
    wheel_rows: i32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            stop_playback: false,
            dragging: Default::default(),
            speeds: [300., 2., 300.],
            reverse: false,
            wheel_time: 0.25,
            wheel_rows: 48,
        }
    }
}
impl Preferences {
    pub fn speeds(self) -> crate::spacing::Speeds {
        crate::spacing::Speeds {
            time: self.speeds[0],
            row: self.speeds[1],
            variable: self.speeds[2],
        }
    }
    pub fn speed(self, index: usize) -> Option<f64> {
        self.speeds.get(index).copied()
    }
    pub fn set_speed(&mut self, index: usize, value: f64) -> Result<(), Error> {
        if !value.is_finite() || value <= 0. || index >= 3 {
            return Err(Error::InvalidEdit("Invalid scrolling speed".into()));
        }
        let (min, max) = if index == 1 {
            (0.1, 10000.)
        } else {
            (10., 100000.)
        };
        let value = value.clamp(min, max);
        if !crate::number::reference_equal(self.speeds[index], value) {
            self.speeds[index] = value;
        }
        Ok(())
    }
    /// The source SliderFloat permits a manually typed finite value outside its
    /// drag bounds. Keep the preference raw; ZoomManager bounds its live value.
    pub fn set_speed_input(&mut self, index: usize, value: f32) -> Result<(), Error> {
        if !value.is_finite() || index >= 3 {
            return Err(Error::InvalidEdit("Invalid scrolling speed".into()));
        }
        let value = f64::from(value);
        if !crate::number::reference_equal(self.speeds[index], value) {
            self.speeds[index] = value;
        }
        Ok(())
    }
    pub fn reverse(self) -> bool {
        self.reverse
    }
    pub fn set_reverse(&mut self, value: bool) {
        self.reverse = value;
    }
    pub fn wheel_time(self) -> f64 {
        self.wheel_time
    }
    pub fn wheel_rows(self) -> i32 {
        self.wheel_rows
    }
    pub fn set_wheel_time(&mut self, value: f64) -> Result<(), Error> {
        if !value.is_finite() || !(0.0..=10.0).contains(&value) {
            return Err(Error::InvalidEdit("Invalid scroll wheel time".into()));
        }
        self.wheel_time = value;
        Ok(())
    }
    pub fn set_wheel_rows(&mut self, value: i32) -> Result<(), Error> {
        if !(0..=384).contains(&value) {
            return Err(Error::InvalidEdit("Invalid scroll wheel rows".into()));
        }
        self.wheel_rows = value;
        Ok(())
    }

    pub fn stop_playback(self) -> bool {
        self.stop_playback
    }
    pub fn set_stop_playback(&mut self, stop: bool) {
        self.stop_playback = stop;
    }
    pub fn stop_playback_when_dragging(self) -> bool {
        self.dragging.stop_playback()
    }
    pub fn set_stop_playback_when_dragging(&mut self, stop: bool) {
        self.dragging.set_stop_playback(stop);
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let stop_playback = match fields.get("StopPlaybackWhenScrolling") {
            None => false,
            Some(value) => value
                .as_bool()
                .ok_or_else(|| Error::InvalidFormat("Invalid StopPlaybackWhenScrolling".into()))?,
        };
        let mut next = Self {
            stop_playback,
            dragging: dragging::Preferences::read(fields)?,
            ..Self::default()
        };
        for (i, key) in [
            "TimeBasedPixelsPerSecond",
            "RowBasedPixelsPerRow",
            "VariablePixelsPerSecondAtDefaultBPM",
        ]
        .iter()
        .enumerate()
        {
            if let Some(value) = fields.get(*key) {
                let value = value
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| Error::InvalidFormat(format!("Invalid {key}")))?;
                if !crate::number::reference_equal(next.speeds[i], value) {
                    next.speeds[i] = value;
                }
            }
        }
        if let Some(value) = fields.get("Reverse") {
            next.reverse = value
                .as_bool()
                .ok_or_else(|| Error::InvalidFormat("Invalid Reverse".into()))?;
        }
        if let Some(value) = fields.get("ScrollWheelTime") {
            next.wheel_time = value
                .as_f64()
                .filter(|v| v.is_finite())
                .ok_or_else(|| Error::InvalidFormat("Invalid ScrollWheelTime".into()))?;
        }
        if let Some(value) = fields.get("ScrollWheelRows") {
            next.wheel_rows = value
                .as_i64()
                .and_then(|v| v.try_into().ok())
                .ok_or_else(|| Error::InvalidFormat("Invalid ScrollWheelRows".into()))?;
        }
        Ok(next)
    }
    pub(crate) fn write(self, fields: &mut Map<String, Value>) {
        self.dragging.write(fields);
        for (i, key) in [
            "TimeBasedPixelsPerSecond",
            "RowBasedPixelsPerRow",
            "VariablePixelsPerSecondAtDefaultBPM",
        ]
        .iter()
        .enumerate()
        {
            fields.insert((*key).into(), self.speeds[i].into());
        }
        fields.insert("Reverse".into(), self.reverse.into());
        fields.insert("ScrollWheelTime".into(), self.wheel_time.into());
        fields.insert("ScrollWheelRows".into(), self.wheel_rows.into());
        fields.insert(
            "StopPlaybackWhenScrolling".into(),
            self.stop_playback.into(),
        );
    }
}
/// Port of MoveEarlierInChart/MoveLaterInChart's nonzero-snap branch.
/// Original code truncates to int before signed division, including negatives.
pub fn snap_destination(position: f64, rows: u64, later: bool) -> Result<i64, Error> {
    if !position.is_finite()
        || position < i32::MIN as f64
        || position > i32::MAX as f64
        || rows == 0
        || rows > i32::MAX as u64
    {
        return Err(Error::InvalidEdit(
            "Invalid snapped navigation position".into(),
        ));
    }
    let position = position as i64;
    let rows = rows as i64;
    let mut next = position / rows * rows;
    if later {
        next += rows;
    } else if next == position {
        next -= rows;
    }
    if next < i32::MIN as i64 || next > i32::MAX as i64 {
        return Err(Error::InvalidEdit("Snapped navigation overflow".into()));
    }
    Ok(next)
}
