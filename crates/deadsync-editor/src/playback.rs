//! Immutable playback clock from the working document, including unsaved edits.
use crate::{
    Error, chart_properties,
    generation::{editor_timing::EditorTimeline, timing::GenerationTiming},
    history::Session,
    notes::Grid,
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub struct Preview {
    music: PathBuf,
    offset: f64,
    sync_offset: f64,
    timeline: EditorTimeline,
}
impl Preview {
    pub fn capture(session: &Session, chart: usize, simfile: &Path) -> Result<Self, Error> {
        let document = session.document();
        if chart >= document.charts().len() {
            return Err(Error::InvalidEdit("Invalid playback chart".into()));
        }
        let name = document
            .text(Some(chart), "MUSIC")
            .filter(|name| !name.trim().is_empty())
            .or_else(|| document.text(None, "MUSIC"))
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| Error::InvalidEdit("No music file selected for this chart".into()))?;
        let music = simfile
            .parent()
            .unwrap_or(Path::new("."))
            .join(name.replace('\\', "/"));
        if !music.is_file() {
            return Err(Error::InvalidEdit(format!(
                "Music file not found: {}",
                music.display()
            )));
        }
        let offset = chart_properties::Draft::capture(session, chart)?.effective_music_offset();
        let sync_offset = crate::song_properties::sync_offset(document)?;
        let timing = GenerationTiming::from_document(document, chart)?;
        // Playback needs rate events, not note interpretation. An empty grid also
        // keeps the clock available for Couple/Routine and empty charts.
        let timeline =
            EditorTimeline::build(&timing, &Grid::new(4, 1)?, None, &AtomicBool::new(false))?;
        Ok(Self {
            music,
            offset,
            sync_offset,
            timeline,
        })
    }
    pub fn music(&self) -> &Path {
        &self.music
    }
    /// Audio-file seconds. Negative positions are valid silent lead-in.
    pub fn music_time(&self, row: u64) -> Result<f64, Error> {
        self.chart_time_to_music(self.timeline.position_time(row)?)
    }
    pub fn position(&self, music_seconds: f64) -> Result<f64, Error> {
        self.timeline
            .time_position(self.music_to_chart_time(music_seconds)?)
    }
    pub fn chart_time_to_music(&self, chart_seconds: f64) -> Result<f64, Error> {
        finite(chart_seconds - self.offset + self.sync_offset)
    }
    pub fn music_to_chart_time(&self, music_seconds: f64) -> Result<f64, Error> {
        finite(music_seconds + self.offset - self.sync_offset)
    }
}
fn finite(value: f64) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidEdit("Nonfinite playback time".into()))
    }
}
