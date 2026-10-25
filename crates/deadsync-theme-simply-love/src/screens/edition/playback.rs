use super::State;
use crate::i18n;
use deadsync_editor::{playback::Preview, workspace::Command};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot, GameplayMusicCut};
use std::time::Instant;

pub(super) struct Playing {
    preview: Option<Preview>,
    requested: Instant,
    mapped: bool,
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
        self.waveform_preferences = file.waveform();
        self.audio_preferences_file = Some(file);
        self.apply_waveform_preferences();
        Ok(())
    }
    pub fn audio_preferences(&self) -> deadsync_editor::audio_preferences::Audio {
        self.audio_preferences
    }
    pub fn is_playing(&self) -> bool {
        self.playback.is_some()
    }
    pub fn take_audio_commands(&mut self) -> Vec<GameplayAudioCommand> {
        std::mem::take(&mut self.audio_commands)
    }
    pub(super) fn stop_audio(&mut self) {
        if self.playback.take().is_some() {
            self.audio_commands.push(GameplayAudioCommand::StopMusic);
            self.status = i18n::tr("Edition", "PlaybackStopped");
        }
    }
    pub(super) fn toggle_audio(&mut self) {
        if self.is_playing() {
            self.stop_audio();
            return;
        }
        let result = Preview::capture(
            &self.workspace.file().session,
            self.workspace.chart(),
            self.workspace.file().path(),
        )
        .and_then(|preview| {
            let time = preview.music_time(self.workspace.row())?;
            Ok((preview, time))
        });
        match result {
            Ok((preview, time)) => {
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
                    requested: Instant::now(),
                    mapped: false,
                });
                self.status = i18n::tr("Edition", "PlaybackPlaying");
            }
            Err(error) => {
                self.status =
                    i18n::tr_fmt("Edition", "PlaybackError", &[("error", &error.to_string())])
            }
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
                    requested: Instant::now(),
                    mapped: false,
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
        if clock.valid_at < playing.requested {
            return;
        }
        if !clock.has_music_mapping {
            if playing.mapped {
                self.stop_audio();
            }
            return;
        }
        playing.mapped = true;
        let result = preview.position(clock.music_nanos as f64 / 1_000_000_000.0);
        match result {
            Ok(row) => {
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
