//! Immutable interpretation chosen when a song is opened. It is not a mutable
//! application preference and is retained by snapshots and native recovery.
use super::{Document, Format};
use crate::{Error, history::Session};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadContext {
    open_sync_offset: f64,
    timing_steps: i32,
    timing_difficulty: i32,
}
impl Default for LoadContext {
    fn default() -> Self {
        Self {
            open_sync_offset: crate::song_properties::DEFAULT_SYNC_OFFSET,
            timing_steps: 0,
            timing_difficulty: 4,
        }
    }
}
impl LoadContext {
    pub fn new(open_sync_offset: f64) -> Result<Self, Error> {
        if !open_sync_offset.is_finite() {
            return Err(Error::InvalidFormat("Invalid open-song sync offset".into()));
        }
        Ok(Self {
            open_sync_offset,
            ..Default::default()
        })
    }
    pub(crate) fn with_timing_preferences(mut self, steps: i32, difficulty: i32) -> Self {
        self.timing_steps = steps;
        self.timing_difficulty = difficulty;
        self
    }
    pub(crate) fn timing_preferences(self) -> (i32, i32) {
        (self.timing_steps, self.timing_difficulty)
    }
    pub fn open_sync_offset(self) -> f64 {
        self.open_sync_offset
    }
}
/// Serialization of a view carries its interpretation without materializing
/// extra tags. Only explicit output preparation materializes the chosen sync.
#[derive(Clone)]
pub struct Snapshot {
    bytes: Arc<[u8]>,
    format: Format,
    context: LoadContext,
}
impl Snapshot {
    pub fn capture(session: &Session) -> Result<Self, Error> {
        Ok(Self {
            bytes: session.serialize()?.into(),
            format: session.document().format,
            context: session.document().load_context(),
        })
    }
    pub fn from_parts(bytes: Arc<[u8]>, format: Format, context: LoadContext) -> Self {
        Self {
            bytes,
            format,
            context,
        }
    }
    pub fn parse(self) -> Result<Document, Error> {
        Document::parse_with_context(self.bytes, self.format, self.context)
    }
    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
    pub fn format(&self) -> Format {
        self.format
    }
    pub fn context(&self) -> LoadContext {
        self.context
    }
}
