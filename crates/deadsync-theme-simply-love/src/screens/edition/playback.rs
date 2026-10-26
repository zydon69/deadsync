use super::{State, decoder_transport::{Transport, Ack, Clock, Step}};
use crate::i18n;
use deadsync_editor::{playback::Preview, workspace::Command};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot, GameplayMusicCut};
use std::{sync::Arc, time::Instant};

pub(super) struct Playing {
    preview: Option<Preview>,
    decoder: Option<Transport>,
    decoder_ack: Option<Ack>,
    clock_generation: Option<u64>,
    issued_generation: Option<u64>,
    precise_stream: Option<(u64, Instant, f64)>,
    valid_after: Instant,
    mapped: bool,
    mapped_generation: Option<u64>,
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
        let previous_rate = self.audio_preferences.music_rate();
        self.audio_preferences_path = Some(path.to_owned());
        self.audio_preferences_file = None;
        let file = deadsync_editor::audio_preferences::File::open(path)?;
        let zoom_preferences = file.zoom();
        zoom_preferences.apply(&mut self.zoom)?;
        self.zoom_preferences = zoom_preferences;
        self.audio_preferences = file.audio();
        self.scroll_preferences = file.scroll();
        self.configure_minimap_preferences(file.minimap());
        self.waveform_preferences = file.waveform();
        self.audio_preferences_file = Some(file);
        if previous_rate != self.audio_preferences.music_rate() {
            self.apply_playback_rate();
        }
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
            .or_else(|| self.focus_position.as_ref()
                .filter(|position| position.is_current(&self.workspace))
                .map(|position| (position.chart_time(), position.row())))
    }
    /// Capture before stop_audio removes mapped/paused playback state. Sample
    /// audition has no chart Preview and must never drive chart focus.
    fn transport_music_time(&self) -> Option<f64> {
        self.playback.as_ref().filter(|playing| playing.preview.is_some())
            .and_then(|playing| playing.position)
            .or_else(|| self.paused_position())
            .map(|position| position.music_time)
    }
    pub(super) fn chart_focus_music_time(&self) -> Result<f64, deadsync_editor::Error> {
        if let Some(time) = self.transport_music_time() { return Ok(time); }
        if let Some(position) = self.focus_position.as_ref().filter(|position| position.is_current(&self.workspace)) {
            return Ok(position.music_time());
        }
        deadsync_editor::playback::Clock::capture(&self.workspace.file().session, self.workspace.chart())?
            .music_time(self.workspace.row())
    }
    fn paused_position(&self) -> Option<Position> {
        let paused = self.paused_playback.as_ref()?;
        let session = &self.workspace.file().session;
        (Arc::ptr_eq(&paused.source, session.document().source())
            && paused.revision == session.content_revision()
            && paused.chart == self.workspace.chart()
            && paused.edit_row == self.workspace.row())
        .then_some(paused.position)
    }
    pub(super) fn pause_for_minimap(&mut self) {
        if self.chart_transport_is_playing() {
            self.pause_audio();
        }
    }
    fn pause_audio(&mut self) {
        let Some(playing) = self.playback.take() else {
            return;
        };
        self.paused_playback = if playing.preview.is_some() {
            playing.position.map(|position| Paused {
                source: Arc::clone(self.workspace.file().session.document().source()),
                revision: self.workspace.file().session.content_revision(),
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
    pub(super) fn chart_transport_is_playing(&self) -> bool {
        self.playback
            .as_ref()
            .is_some_and(|playing| playing.preview.is_some())
    }
    /// Autoplay requires a validated chart position, including acknowledged
    /// silent continuation. Merely queued decoding, sample audition and pause
    /// never qualify. Legacy synthetic callers have no backend generation.
    pub(super) fn chart_autoplay_transport(&self) -> Option<(Option<u64>, f64)> {
        let playing = self
            .playback
            .as_ref()
            .filter(|playing| playing.preview.is_some() && playing.mapped)?;
        let position = playing.position.filter(|position| {
            position.music_time.is_finite()
                && position.chart_time.is_finite()
                && position.row.is_finite()
        })?;
        let generation = if playing.decoder.is_some() {
            let generation = playing.issued_generation?;
            if playing.clock_generation != Some(generation)
                || playing.mapped_generation != Some(generation)
            {
                return None;
            }
            let ack = playing
                .decoder_ack
                .filter(|ack| ack.generation == generation && ack.opened)?;
            if !matches!(
                ack.phase,
                super::decoder_transport::Phase::Opened
                    | super::decoder_transport::Phase::Producing
                    | super::decoder_transport::Phase::Exhausted
            ) {
                return None;
            }
            Some(generation)
        } else {
            None
        };
        Some((generation, position.music_time))
    }
    pub fn is_playing(&self) -> bool {
        self.playback.is_some()
    }
    pub(super) fn apply_playback_rate(&mut self) {
        if let Some(playing) = self
            .playback
            .as_mut()
            .filter(|playing| playing.preview.is_some())
        {
            playing.valid_after = Instant::now();
            playing.mapped = false;
            self.audio_commands.push(GameplayAudioCommand::SetMusicRate(
                self.audio_preferences.music_rate() as f32,
            ));
        }
    }
    pub fn take_audio_commands(&mut self) -> Vec<GameplayAudioCommand> {
        std::mem::take(&mut self.audio_commands)
    }
    pub(super) fn stop_audio(&mut self) {
        if let Some(time) = self.transport_music_time() {
            self.focus_position = deadsync_editor::chart_focus::Position::at_time(
                &self.workspace.file().session, self.workspace.chart(), time,
            ).ok();
        }
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
        let resume = self.paused_position().or_else(|| self.focus_position.as_ref()
            .filter(|position| position.is_current(&self.workspace))
            .map(|position| Position { music_time: position.music_time(), chart_time: position.chart_time(), row: position.row() }));
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
                    rate: self.audio_preferences.music_rate() as f32,
                });
                self.playback = Some(Playing {
                    preview: Some(preview),
                    decoder: None,
                    decoder_ack: None,
                    clock_generation: None,
                    issued_generation: None,
                    precise_stream: None,
                    valid_after: Instant::now(),
                    mapped: false,
                    mapped_generation: None,
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
            if !continuing { self.stop_audio(); }
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
                    decoder: None,
                    decoder_ack: None,
                    clock_generation: None,
                    issued_generation: None,
                    precise_stream: None,
                    valid_after: Instant::now(),
                    mapped: false,
                    mapped_generation: None,
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
    /// Bind the generation synchronously assigned to the last drained audio
    /// command. Repeated frame updates must not reset an existing anchor.
    pub fn bind_decoder_generation(&mut self, generation: Option<u64>) {
        let Some(playing) = self.playback.as_ref().filter(|playing| playing.preview.is_some()) else { return; };
        if generation.is_some() && playing.issued_generation == generation { return; }
        let Some(generation) = generation else {
            self.stop_audio();
            self.status = i18n::tr("Edition", "PlaybackDecoderUnavailable");
            return;
        };
        let Ok(start) = self.chart_focus_music_time() else {
            self.stop_audio();
            self.status = i18n::tr("Edition", "PlaybackDecoderFailed");
            return;
        };
        let playing = self.playback.as_mut().expect("chart transport exists");
        let decoder = playing.decoder.get_or_insert_with(Transport::default);
        if !decoder.issue(generation, start) {
            self.stop_audio();
            self.status = i18n::tr("Edition", "PlaybackDecoderFailed");
            return;
        }
        playing.issued_generation = Some(generation);
        playing.mapped_generation = None;
        playing.clock_generation = Some(generation);
        playing.decoder_ack = None;
        playing.precise_stream = None;
    }
    /// Optional editor-only f64 stream time from the exact same clock snapshot.
    /// Legacy gameplay snapshots and callers without this side channel keep their
    /// existing f32 contract. Stale identities or non-finite values are ignored.
    pub fn set_decoder_stream_time(&mut self, generation: u64, valid_at: Instant, seconds: f64) {
        if !seconds.is_finite() { return; }
        if let Some(playing) = self.playback.as_mut().filter(|playing| playing.preview.is_some())
            && playing.issued_generation == Some(generation) {
            playing.precise_stream = Some((generation, valid_at, seconds));
        }
    }
    /// Install the backend acknowledgement before update_audio. The native
    /// adapter supplies the synchronous command generation for the clock too.
    pub fn set_decoder_status(&mut self, generation: u64, ack: Option<Ack>) {
        if let Some(playing) = self.playback.as_mut().filter(|playing| playing.preview.is_some()) {
            playing.clock_generation = Some(generation);
            playing.decoder_ack = ack;
        }
    }
    /// Mapped audio or acknowledged silent stream time advances the cursor.
    /// Wall-clock time alone never claims decoding or playback succeeded.
    pub fn update_audio(&mut self, snapshot: GameplayAudioSnapshot) {
        let Some(playing) = self.playback.as_mut() else {
            return;
        };
        let Some(preview) = playing.preview.as_ref() else {
            return;
        };
        let clock = snapshot.stream_clock;
        if playing.decoder.is_some() && playing.clock_generation != playing.issued_generation { return; }
        if clock.valid_at < playing.valid_after {
            return;
        }
        // Reject out-of-order snapshots as well as snapshots from before play.
        playing.valid_after = clock.valid_at;
        if playing.decoder.is_some()
            && let Some(ack) = playing.decoder_ack.filter(|ack| Some(ack.generation) == playing.issued_generation)
            && matches!(ack.phase, super::decoder_transport::Phase::Failed | super::decoder_transport::Phase::Stopped) {
            let failed = ack.phase == super::decoder_transport::Phase::Failed;
            self.stop_audio();
            if failed { self.status = i18n::tr("Edition", "PlaybackDecoderFailed"); }
            return;
        }
        let seconds = if let Some(decoder) = playing.decoder.as_mut() {
            let Some(generation) = playing.clock_generation else { return; };
            match decoder.observe(playing.decoder_ack, Clock {
                generation, valid_at: clock.valid_at,
                stream_seconds: playing.precise_stream
                    .filter(|(precise_generation, stamp, _)| *precise_generation == generation && *stamp == clock.valid_at)
                    .map_or_else(|| f64::from(clock.stream_seconds), |(_, _, seconds)| seconds),
                music_seconds: clock.has_music_mapping.then_some(clock.music_nanos as f64 / 1_000_000_000.0),
                rate: if clock.has_music_mapping { f64::from(clock.music_seconds_per_second) } else { self.audio_preferences.music_rate() },
            }) {
                Step::Position { music_seconds, .. } => music_seconds,
                Step::Wait => return,
                Step::Stopped => {
                    self.stop_audio();
                    return;
                }
                Step::Failed => {
                    self.stop_audio();
                    self.status = i18n::tr("Edition", "PlaybackDecoderFailed");
                    return;
                }
            }
        } else {
            if !clock.has_music_mapping {
                if playing.mapped { self.pause_audio(); }
                return;
            }
            clock.music_nanos as f64 / 1_000_000_000.0
        };
        playing.mapped = true;
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
                playing.mapped_generation = playing.issued_generation;
                // Notes use nonnegative integral rows; the transport clock
                // itself retains fractional/negative positions.
                let row = row.max(0.0).min(i32::MAX as f64).floor() as u64;
                if row != self.workspace.row() {
                    let _ = self.workspace.seek_preserving_selection(row);
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

#[cfg(test)]
mod autoplay_transport_tests {
    use super::super::decoder_transport::Phase;
    use super::*;
    use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
    use std::{fs, sync::Mutex};
    static SERIAL: Mutex<()> = Mutex::new(());
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn setup() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("music.wav"), b"command-only fixture").unwrap();
        fs::write(dir.path().join("song.ssc"),"#VERSION:0.83;#MUSIC:music.wav;#SAMPLESTART:0;#SAMPLELENGTH:1;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&dir.path().join("song.ssc"), 0).unwrap();
        press(&mut state, KeyCode::Enter);
        state.take_audio_commands();
        (dir, state)
    }
    fn status(state: &mut State, generation: u64, phase: Phase, opened: bool) {
        state.set_decoder_status(
            generation,
            Some(Ack {
                generation,
                phase,
                opened,
            }),
        );
    }
    fn update(state: &mut State, music: Option<f64>, stream: f32) {
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.stream_clock.stream_seconds = stream;
        snapshot.stream_clock.has_music_mapping = music.is_some();
        snapshot.stream_clock.music_nanos = (music.unwrap_or_default() * 1e9).round() as i64;
        snapshot.stream_clock.music_seconds_per_second = 1.0;
        state.update_audio(snapshot);
    }
    #[test]
    fn autoplay_transport_requires_current_opened_generation_and_validated_position() {
        let _guard = SERIAL.lock().unwrap();
        let (_dir, mut state) = setup();
        assert!(state.chart_autoplay_transport().is_none());
        state.bind_decoder_generation(Some(1));
        status(&mut state, 1, Phase::Pending, false);
        update(&mut state, Some(0.25), 1.0);
        assert!(state.chart_autoplay_transport().is_none());
        status(&mut state, 1, Phase::Producing, true);
        update(&mut state, Some(0.5), 2.0);
        assert_eq!(state.chart_autoplay_transport(), Some((Some(1), 0.5)));
        status(&mut state, 99, Phase::Producing, true);
        assert!(state.chart_autoplay_transport().is_none());
        status(&mut state, 1, Phase::Producing, false);
        assert!(state.chart_autoplay_transport().is_none());
        status(&mut state, 1, Phase::Failed, true);
        assert!(state.chart_autoplay_transport().is_none());
        status(&mut state, 1, Phase::Stopped, true);
        assert!(state.chart_autoplay_transport().is_none());
    }
    #[test]
    fn autoplay_transport_rebind_rejects_old_position_even_after_new_opened_ack() {
        let _guard = SERIAL.lock().unwrap();
        let (_dir, mut state) = setup();
        state.bind_decoder_generation(Some(1));
        status(&mut state, 1, Phase::Producing, true);
        update(&mut state, Some(0.5), 1.0);
        assert!(state.chart_autoplay_transport().is_some());
        state.bind_decoder_generation(Some(2));
        assert!(state.chart_autoplay_transport().is_none());
        status(&mut state, 2, Phase::Pending, false);
        assert!(state.chart_autoplay_transport().is_none());
        status(&mut state, 2, Phase::Opened, true);
        assert!(state.chart_autoplay_transport().is_none());
        update(&mut state, None, 2.0);
        assert_eq!(state.chart_autoplay_transport(), Some((Some(2), 0.5)));
        state
            .playback
            .as_mut()
            .unwrap()
            .position
            .as_mut()
            .unwrap()
            .music_time = f64::NAN;
        assert!(state.chart_autoplay_transport().is_none());
    }
    #[test]
    fn autoplay_transport_accepts_acknowledged_silent_time_but_excludes_pause_and_sample() {
        let _guard = SERIAL.lock().unwrap();
        let (_dir, mut state) = setup();
        state.bind_decoder_generation(Some(1));
        status(&mut state, 1, Phase::Exhausted, true);
        update(&mut state, None, 10.0);
        update(&mut state, None, 10.25);
        assert_eq!(state.chart_autoplay_transport(), Some((Some(1), 0.25)));
        press(&mut state, KeyCode::Enter);
        assert!(state.paused_playback.is_some());
        assert!(state.chart_autoplay_transport().is_none());
        // Audition requires the real Song Properties draft, opened through its keyboard UI.
        press(&mut state, KeyCode::F11);
        press(&mut state, KeyCode::F6);
        press(&mut state, KeyCode::F4);
        assert!(state.is_playing());
        assert!(state.playback.as_ref().unwrap().preview.is_none());
        assert!(state.chart_autoplay_transport().is_none());
    }
    #[test]
    fn autoplay_transport_legacy_synthetic_mapping_has_no_generation() {
        let _guard = SERIAL.lock().unwrap();
        let (_dir, mut state) = setup();
        update(&mut state, Some(0.125), 1.0);
        assert_eq!(state.chart_autoplay_transport(), Some((None, 0.125)));
        state.stop_audio();
        assert!(state.chart_autoplay_transport().is_none());
    }
}
