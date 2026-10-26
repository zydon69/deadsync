//! Original GrooveAuthor playback policy for navigation.
pub mod dragging;
use crate::Error;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    stop_playback: bool,
    dragging: dragging::Preferences,
}
impl Preferences {
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
        Ok(Self {
            stop_playback,
            dragging: dragging::Preferences::read(fields)?,
        })
    }
    pub(crate) fn write(self, fields: &mut Map<String, Value>) {
        self.dragging.write(fields);
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
