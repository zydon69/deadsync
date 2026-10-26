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
    clock: Clock,
}

/// Position conversion does not require a selected or decodable audio file.
pub struct Clock {
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
        Ok(Self {
            music,
            clock: Clock::capture(session, chart)?,
        })
    }
    pub fn music(&self) -> &Path {
        &self.music
    }
    pub fn music_time(&self, row: u64) -> Result<f64, Error> {
        self.clock.music_time(row)
    }
    pub fn chart_time(&self, row: u64) -> Result<f64, Error> {
        self.clock.chart_time(row)
    }
    pub fn position(&self, seconds: f64) -> Result<f64, Error> {
        self.clock.position(seconds)
    }
    pub fn chart_time_to_music(&self, seconds: f64) -> Result<f64, Error> {
        self.clock.chart_time_to_music(seconds)
    }
    pub fn music_to_chart_time(&self, seconds: f64) -> Result<f64, Error> {
        self.clock.music_to_chart_time(seconds)
    }
}
impl Clock {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let document = session.document();
        let offset = chart_properties::Draft::capture(session, chart)?.effective_music_offset();
        let sync_offset = crate::song_properties::sync_offset(document)?;
        let timing = GenerationTiming::from_document(document, chart)?;
        // Playback needs rate events, not note interpretation. An empty grid also
        // keeps the clock available for Couple/Routine and empty charts.
        let timeline =
            EditorTimeline::build(&timing, &Grid::new(4, 1)?, None, &AtomicBool::new(false))?;
        Ok(Self {
            offset,
            sync_offset,
            timeline,
        })
    }
    pub(crate) fn capture_with_attacks(
        session: &Session,
        chart: usize,
        rows: &[u64],
    ) -> Result<Self, Error> {
        let document = session.document();
        let descriptor = document
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Invalid attack chart".into()))?;
        let parsed;
        let grid = if let Some(grid) = session.loaded_grid(chart) {
            grid
        } else {
            parsed = Grid::parse_layout(document.value(descriptor.notes.clone()), &descriptor.steps_type)?;
            &parsed
        };
        let offset = chart_properties::Draft::capture(session, chart)?.effective_music_offset();
        let sync_offset = crate::song_properties::sync_offset(document)?;
        let timing = GenerationTiming::from_document(document, chart)?;
        Ok(Self {
            offset,
            sync_offset,
            timeline: EditorTimeline::build_clock(&timing, grid, rows)?,
        })
    }
    pub(crate) fn attack_times(&self) -> impl Iterator<Item = (u64, f64)> + '_ {
        self.timeline.attack_times()
    }
    /// Ordered chart-time rate snapshots for visual spacing. Use
    /// chart_time_to_music when converting these times to audio-file seconds.
    pub fn rate_mappings(&self) -> impl Iterator<Item = (u8, crate::rate_mapping::RateMapping)> + '_ {
        self.timeline.rate_mappings()
    }
    pub fn spacing_sections(&self) -> impl Iterator<Item = crate::generation::editor_timing::SpacingSection> + '_ {
        self.timeline.spacing_sections()
    }
    /// Chart time of a speed event at its original ordering phase, not audio-file time.
    pub fn speed_event_time(&self, row: u64) -> Result<f64, Error> {
        self.timeline.speed_event_time(row)
    }
    /// Chart-time note position, including a delay at the note's row.
    pub fn note_time(&self, row: u64) -> Result<f64, Error> {
        self.timeline.note_time(row)
    }
    pub(crate) fn with_song_draft(mut self, offset: f64, sync_offset: f64) -> Self {
        self.offset = offset;
        self.sync_offset = sync_offset;
        self
    }
    /// Chart-time cursor position before same-row events. Visual rate selection
    /// uses this directly, avoiding a floating-point round trip through offsets.
    pub fn chart_time(&self, row: u64) -> Result<f64, Error> {
        self.timeline.position_time(row)
    }
    /// Audio-file seconds. Negative positions are valid silent lead-in.
    pub fn music_time(&self, row: u64) -> Result<f64, Error> {
        self.chart_time_to_music(self.chart_time(row)?)
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
