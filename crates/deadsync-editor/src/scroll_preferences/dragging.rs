//! Independent GrooveAuthor scrollbar capture policy (default: resume on release).
//! This is deliberately separate from StopPlaybackWhenScrolling's snapped moves.
use crate::Error;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    stop_playback: bool,
}
impl Preferences {
    pub fn stop_playback(self) -> bool {
        self.stop_playback
    }
    pub fn set_stop_playback(&mut self, stop: bool) {
        self.stop_playback = stop;
    }
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let stop_playback = match fields.get("StopPlaybackWhenDraggingScrollBars") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| {
                Error::InvalidFormat("Invalid StopPlaybackWhenDraggingScrollBars".into())
            })?,
        };
        Ok(Self { stop_playback })
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        fields.insert(
            "StopPlaybackWhenDraggingScrollBars".into(),
            self.stop_playback.into(),
        );
    }
}
