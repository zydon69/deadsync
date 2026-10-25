use super::State;
use crate::i18n;
use deadsync_editor::{playback::Preview, workspace::Command};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot, GameplayMusicCut};
use std::{sync::Arc, time::Instant};

pub(super) struct Playing {
    preview: Option<Preview>,
    valid_after: Instant,
    mapped: bool,
    position: Option<Position>,
}
#[derive(Clone, Copy)]
struct Position {
    chart_time: f64,
    row: f64,
    music_time: f64,
}
/// A pause belongs to one immutable document revision and integral edit cursor.
/// Retaining the original music time avoids an offset round trip on resume.
pub(super) struct Paused {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    edit_row: u64,
    position: Position,
}
impl State {
    /// Install the explicit application preference path; malformed files remain untouched.
    pub fn configure_audio_preferences(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), deadsync_editor::Error> {
        self.audio_preferences_path = Some(path.to_owned());
        self.audio_preferences_file = None;
        let file = deadsync_editor::audio_preferences::File::open(path)?;
        let zoom_preferences = file.zoom();
        zoom_preferences.apply(&mut self.zoom)?;
        self.zoom_preferences = zoom_preferences;
        self.audio_preferences = file.audio();
        self.scroll_preferences = file.scroll();
        self.waveform_preferences = file.waveform();
        self.audio_preferences_file = Some(file);
        self.apply_waveform_preferences();
        Ok(())
    }
    pub fn audio_preferences(&self) -> deadsync_editor::audio_preferences::Audio {
        self.audio_preferences
    }
    /// Exact mapped chart time and fractional row for presentation only.
    pub(super) fn playback_position(&self) -> Option<(f64, f64)> {
        self.playback
            .as_ref()
            .and_then(|playing| playing.position)
            .or_else(|| self.paused_position())
            .map(|position| (position.chart_time, position.row))
    }
    fn paused_position(&self) -> Option<Position> {
        let paused = self.paused_playback.as_ref()?;
        let session = &self.workspace.file().session;
        (Arc::ptr_eq(&paused.source, session.document().source())
            && paused.revision == session.revision()
            && paused.chart == self.workspace.chart()
            && paused.edit_row == self.workspace.row())
        .then_some(paused.position)
    }
    fn pause_audio(&mut self) {
        let Some(playing) = self.playback.take() else {
            return;
        };
        self.paused_playback = if playing.preview.is_some() {
            playing.position.map(|position| Paused {
                source: Arc::clone(self.workspace.file().session.document().source()),
                revision: self.workspace.file().session.revision(),
                chart: self.workspace.chart(),
                edit_row: self.workspace.row(),
                position,
            })
        } else {
            None
        };
        self.audio_commands.push(GameplayAudioCommand::StopMusic);
        self.status = i18n::tr("Edition", "PlaybackPaused");
    }
    pub fn is_playing(&self) -> bool {
        self.playback.is_some()
    }
    pub fn take_audio_commands(&mut self) -> Vec<GameplayAudioCommand> {
        std::mem::take(&mut self.audio_commands)
    }
    pub(super) fn stop_audio(&mut self) {
        let paused = self.paused_playback.take().is_some();
        let playing = self.playback.take().is_some();
        if playing {
            self.audio_commands.push(GameplayAudioCommand::StopMusic);
        }
        if playing || paused {
            self.status = i18n::tr("Edition", "PlaybackStopped");
        }
    }
    pub(super) fn toggle_audio(&mut self) {
        if self.is_playing() {
            self.pause_audio();
            return;
        }
        let resume = self.paused_position();
        let result = Preview::capture(
            &self.workspace.file().session,
            self.workspace.chart(),
            self.workspace.file().path(),
        )
        .and_then(|preview| {
            let time = if let Some(position) = resume {
                position.music_time
            } else {
                preview.music_time(self.workspace.row())?
            };
            Ok((preview, time))
        });
        match result {
            Ok((preview, time)) => {
                self.paused_playback = None;
                self.audio_commands.push(GameplayAudioCommand::PlayMusic {
                    path: preview.music().to_path_buf(),
                    cut: GameplayMusicCut {
                        start_sec: time,
                        ..Default::default()
                    },
                    looping: false,
                    rate: 1.0,
                });
                self.playback = Some(Playing {
                    preview: Some(preview),
                    valid_after: Instant::now(),
                    mapped: false,
                    position: resume,
                });
                self.status = i18n::tr("Edition", "PlaybackPlaying");
            }
            Err(error) => {
                self.status =
                    i18n::tr_fmt("Edition", "PlaybackError", &[("error", &error.to_string())])
            }
        }
    }
    /// Keyboard navigation shares the original snapped-position and playback policy.
    pub(super) fn navigate_snap(&mut self, later: bool) {
        let current = self
            .playback_position()
            .map_or(self.workspace.row() as f64, |(_, row)| row);
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let destination = deadsync_editor::scroll_preferences::snap_destination(
                current,
                deadsync_editor::workspace::SNAP_ROWS[self.workspace.snap()],
                later,
            )?
            .max(0) as u64;
            let continuing = self.is_playing() && !self.scroll_preferences.stop_playback();
            // The captured clock and open decoder remain valid after pathname
            // changes; navigation must not re-open or re-parse the track.
            let audio = if continuing {
                let preview = self
                    .playback
                    .as_ref()
                    .and_then(|playing| playing.preview.as_ref())
                    .ok_or_else(|| {
                        deadsync_editor::Error::InvalidEdit("No chart playback clock".into())
                    })?;
                Some(Position {
                    chart_time: preview.chart_time(destination)?,
                    row: destination as f64,
                    music_time: preview.music_time(destination)?,
                })
            } else {
                None
            };
            self.command(Command::Seek(destination));
            if let Some(position) = audio {
                self.paused_playback = None;
                let playing = self.playback.as_mut().expect("continued chart transport");
                playing.valid_after = Instant::now();
                playing.mapped = false;
                playing.position = Some(position);
                self.audio_commands
                    .push(GameplayAudioCommand::SeekMusic(position.music_time));
                self.status = i18n::tr("Edition", "PlaybackPlaying");
            } else {
                self.stop_audio();
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.status =
                i18n::tr_fmt("Edition", "PlaybackError", &[("error", &error.to_string())]);
        }
    }
    pub(super) fn toggle_sample_audio(&mut self) {
        if self
            .playback
            .as_ref()
            .is_some_and(|playing| playing.preview.is_none())
        {
            self.stop_audio();
            return;
        }
        self.stop_audio();
        let result = self
            .song_properties
            .as_ref()
            .ok_or_else(|| deadsync_editor::Error::InvalidEdit("No song draft".into()))
            .and_then(|draft| {
                draft.sample_audio(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    self.workspace.file().path(),
                )
            });
        match result {
            Ok(sample) => {
                self.audio_commands.push(GameplayAudioCommand::PlayMusic {
                    path: sample.path,
                    cut: GameplayMusicCut {
                        start_sec: sample.start,
                        length_sec: sample.length.unwrap_or(f64::INFINITY),
                        fade_in_sec: self.audio_preferences.fade_in(),
                        fade_out_sec: self.audio_preferences.fade_out(),
                    },
                    looping: true,
                    rate: 1.0,
                });
                self.playback = Some(Playing {
                    preview: None,
                    valid_after: Instant::now(),
                    mapped: false,
                    position: None,
                });
                self.status = i18n::tr("Edition", "SongSamplePlaying");
            }
            Err(error) => {
                self.status =
                    i18n::tr_fmt("Edition", "PlaybackError", &[("error", &error.to_string())])
            }
        }
    }
    /// Only mapped audio advances the cursor. Wall-clock time alone never
    /// claims that decoding or playback succeeded.
    pub fn update_audio(&mut self, snapshot: GameplayAudioSnapshot) {
        let Some(playing) = self.playback.as_mut() else {
            return;
        };
        let Some(preview) = playing.preview.as_ref() else {
            return;
        };
        let clock = snapshot.stream_clock;
        if clock.valid_at < playing.valid_after {
            return;
        }
        // Reject out-of-order snapshots as well as snapshots from before play.
        playing.valid_after = clock.valid_at;
        if !clock.has_music_mapping {
            if playing.mapped {
                self.pause_audio();
            }
            return;
        }
        playing.mapped = true;
        let seconds = clock.music_nanos as f64 / 1_000_000_000.0;
        let result = preview
            .position(seconds)
            .and_then(|row| Ok((preview.music_to_chart_time(seconds)?, row)));
        match result {
            Ok((time, row)) => {
                playing.position = Some(Position {
                    chart_time: time,
                    row,
                    music_time: seconds,
                });
                // Notes use nonnegative integral rows; the transport clock
                // itself retains fractional/negative positions.
                let row = row.max(0.0).min(i32::MAX as f64).floor() as u64;
                if row != self.workspace.row() {
                    let _ = self.workspace.execute(Command::Seek(row));
                    self.refresh();
                }
            }
            Err(error) => {
                self.stop_audio();
                self.status =
                    i18n::tr_fmt("Edition", "PlaybackError", &[("error", &error.to_string())]);
            }
        }
    }
}
