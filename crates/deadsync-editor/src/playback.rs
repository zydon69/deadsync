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
    has_music:bool,
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
        Self::capture_at_directory(session,chart,simfile.parent().unwrap_or(Path::new(".")))
    }
    pub fn capture_from_directory(session:&Session,chart:usize,directory:&Path)->Result<Self,Error> {
        if !directory.is_absolute(){return Err(Error::InvalidEdit("Playback requires an absolute resource directory".into()));}
        Self::capture_at_directory(session,chart,directory)
    }
    fn capture_at_directory(session:&Session,chart:usize,directory:&Path)->Result<Self,Error> {
        let document = session.document();
        if chart >= document.charts().len() {
            return Err(Error::InvalidEdit("Invalid playback chart".into()));
        }
        let text = crate::chart_text::Text::read(document, chart)?;
        let name = text.get(crate::chart_text::Field::Music);
        let inherited = document.text(None, "MUSIC").unwrap_or_default();
        let name = if name.is_empty() {
            inherited.as_str()
        } else {
            name
        };
        if name.is_empty() {
            return Err(Error::InvalidEdit(
                "No music file selected for this chart".into(),
            ));
        }
        let music = directory.join(name.replace('\\', "/"));
        if !music.is_file() {
            return Err(Error::InvalidEdit(format!(
                "Music file not found: {}",
                music.display()
            )));
        }
        Ok(Self {
            has_music:true,
            music,
            clock: Clock::capture(session, chart)?,
        })
    }
    /// Rebind the immutable chart clock of an already open asset without any
    /// filesystem IO. Source Fumen Path.ArePathsEqual compares path spelling
    /// after slash normalization, not inode identity or canonicalization.
    pub fn rebind_same_music(&self, session: &Session, chart: usize, directory: &Path) -> Result<Option<Self>, Error> {
        let text = crate::chart_text::Text::read(session.document(), chart)?;
        let local = text.get(crate::chart_text::Field::Music);
        let inherited = session.document().text(None, "MUSIC").unwrap_or_default();
        let name = if local.is_empty() { inherited.as_str() } else { local };
        if name.is_empty(){return if self.has_music{Ok(None)}else{Ok(Some(Self{has_music:false,music:directory.to_path_buf(),clock:Clock::capture(session,chart)?}))};}
        let target = directory.join(name.replace('\\', "/"));
        if !source_music_paths_equal(&target.to_string_lossy(), &self.music.to_string_lossy()) { return Ok(None); }
        Ok(Some(Self {has_music:true, music: self.music.clone(), clock: Clock::capture(session, chart)? }))
    }
    pub fn rebind_chart_asset(session:&Session,chart:usize,directory:&Path)->Result<Self,Error>{
        let text=crate::chart_text::Text::read(session.document(),chart)?;
        let local=text.get(crate::chart_text::Field::Music);
        let inherited=session.document().text(None,"MUSIC").unwrap_or_default();
        let name=if local.is_empty(){inherited.as_str()}else{local};
        Ok(Self{has_music:!name.is_empty(),music:directory.join(name.replace('\\',"/")),clock:Clock::capture(session,chart)?})
    }
    pub fn music_asset(&self)->Option<&Path>{self.has_music.then_some(self.music.as_path())}
    /// Source Fumen spelling equality; performs no IO or dot/case normalization.
    pub fn paths_equal(a:&Path,b:&Path)->bool{source_music_paths_equal(&a.to_string_lossy(),&b.to_string_lossy())}
    pub fn has_music_path(&self,path:&Path)->bool{self.has_music&&Self::paths_equal(&self.music,path)}
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
    pub(crate) fn event_phase_time(&self,row:u64,phase:u8)->Result<f64,Error>{self.timeline.query(row,Some(phase))}
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
    /// Presentation cursor before same-row rate events; retains fractional rows
    /// and negative lead-in without quantizing an editor navigation gesture.
    pub fn fractional_chart_time(&self, row: f64) -> Result<f64, Error> {
        self.timeline.fractional_position_time(row)
    }
    pub fn fractional_music_time(&self, row: f64) -> Result<f64, Error> {
        self.chart_time_to_music(self.fractional_chart_time(row)?)
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

impl Clock {pub(crate) fn tick_mapping_end(&self,row:i64)->Result<i64,Error>{self.timeline.tick_mapping_end(row)}}

/// Exact Fumen.Path.ArePathsEqual contract (pin 0fe292f9). Intentionally
/// retains case and dot components, strips only the Win32 device prefix before
/// replacing backslashes; performs no filesystem canonicalization.
fn source_music_paths_equal(a:&str,b:&str)->bool {
    if a==b{return true;}
    let strip=|path:&str|path.strip_prefix(r"\\?\").unwrap_or(path).replace('\\',"/");
    strip(a)==strip(b)
}
#[cfg(test)]
mod source_music_path_tests {
    use super::source_music_paths_equal as equal;
    #[test]fn source_path_spelling_contract(){
        assert!(equal(r"C:\Music\song.ogg","C:/Music/song.ogg"));
        assert!(equal(r"\\?\C:\Music\song.ogg","C:/Music/song.ogg"));
        assert!(equal(r"\\?\C:\Music\song.ogg",r"\\?\C:\Music\song.ogg"));
        assert!(!equal("C:/Music/song.ogg","c:/Music/song.ogg"));
        assert!(!equal("C:/Music/./song.ogg","C:/Music/song.ogg"));
        assert!(!equal("//?/C:/Music/song.ogg","C:/Music/song.ogg"));
    }
}

#[cfg(test)]
mod public_preview_path_equality_tests {
    use super::Preview;
    use std::path::Path;
    #[test]
    fn public_boundary_preserves_exact_source_spelling_rules_without_path_eq_normalization(){
        for (a,b,expected) in [
            (r"C:\Music\song.ogg","C:/Music/song.ogg",true),
            (r"\\?\C:\Music\song.ogg","C:/Music/song.ogg",true),
            ("a/./b.ogg","a/b.ogg",false),
            ("A/b.ogg","a/b.ogg",false),
            ("//?/C:/Music/song.ogg","C:/Music/song.ogg",false),
        ] {assert_eq!(Preview::paths_equal(Path::new(a),Path::new(b)),expected);}
    }
}
