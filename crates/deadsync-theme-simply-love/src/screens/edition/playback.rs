mod chart_copy;
#[cfg(test)]
mod chart_copy_tests;
#[cfg(test)]
mod panel_open_tests;
mod source_visual;
mod timing_live;
pub(super) use timing_live::TimingPresentation;
mod live_preview_region;
use super::{
    State,
    decoder_transport::{Ack, Clock, Step, Transport},
};
use crate::i18n;
use deadsync_editor::{playback::Preview, workspace::Command};
use deadsync_gameplay::{GameplayAudioCommand, GameplayAudioSnapshot, GameplayMusicCut};
use std::{sync::Arc, time::Instant};

pub(super) struct Playing {
    sample_descriptor: Option<GameplayAudioCommand>,
    sample_cursor: Option<deadsync_gameplay::EditorPreviewCursor>,
    sample_registration: u64,
    pending_sample: Option<GameplayAudioCommand>,
    visual_clock: Option<deadsync_editor::source_visual_clock::SourceVisualClock>,
    visual_active: bool,
    visual_reference: Option<f64>,
    asset_epoch: Option<u64>,
    unfocused: bool,
    asset_replaced: bool,
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
    intent_origin: Option<Instant>,
    preview: Option<Preview>,
    asset_replaced: bool,
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    edit_row: u64,
    position: Position,
}
/// Owned proof input captured from validated native focus/transport before an edit.
/// Private fields prevent other panels from inventing a pause stamp or coordinates.
pub(super) struct SongPropertyPresentation {
    focus: deadsync_editor::chart_focus::Position,
    paused: Option<PauseStamp>,
}
struct PauseStamp {
    source: Arc<[u8]>,
    revision: u64,
    chart: usize,
    edit_row: u64,
    position: Position,
}
impl SongPropertyPresentation {
    pub(super) fn focus(&self) -> &deadsync_editor::chart_focus::Position {
        &self.focus
    }
    pub(super) fn music_time(&self) -> f64 {
        self.focus.music_time()
    }
    pub(super) fn row(&self) -> f64 {
        self.focus.row()
    }
}
impl State {
    /// Discard chart projections when the retained, chartless music source changes.
    pub(super) fn clear_unfocused_music_projection(&mut self) {
        if let Some(playing) = self.playback.as_mut().filter(|playing| playing.unfocused) {
            playing.preview = None;
        }
        if let Some(paused) = self
            .paused_playback
            .as_mut()
            .filter(|paused| paused.chart == usize::MAX)
        {
            paused.preview = None;
        }
    }

    #[cfg(test)]
    pub(super) fn playback_asset_epoch(&self) -> Option<u64> {
        self.playback
            .as_ref()
            .and_then(|playing| playing.asset_epoch)
    }
    pub(super) fn capture_song_property_presentation(
        &self,
    ) -> Result<SongPropertyPresentation, deadsync_editor::Error> {
        let session = &self.workspace.file().session;
        let chart = self.workspace.chart();
        let transport = self
            .playback
            .as_ref()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
            .and_then(|playing| playing.position)
            .or_else(|| self.paused_position());
        let current = self
            .focus_position
            .as_ref()
            .filter(|position| position.is_current(&self.workspace));
        let focus = if let Some(transport) = transport {
            if let Some(position) = current.filter(|position| {
                position.row().to_bits() == transport.row.to_bits()
                    && position.music_time().to_bits() == transport.music_time.to_bits()
            }) {
                position.clone()
            } else {
                let inverse = deadsync_editor::chart_focus::Position::at_time(
                    session,
                    chart,
                    transport.music_time,
                )?;
                if inverse.row().to_bits() == transport.row.to_bits() {
                    inverse
                } else {
                    // A chosen warp row can share the transport time. Validate it
                    // with the forward clock; never manufacture a row/time tuple.
                    let forward = deadsync_editor::chart_focus::Position::at_fractional_row(
                        session,
                        chart,
                        transport.row,
                    )?;
                    if forward.music_time().to_bits() != transport.music_time.to_bits() {
                        return Err(deadsync_editor::Error::InvalidEdit(
                            "Transport row/time do not describe one current clock position".into(),
                        ));
                    }
                    forward
                }
            }
        } else if let Some(position) = current {
            position.clone()
        } else {
            deadsync_editor::chart_focus::Position::at_fractional_row(
                session,
                chart,
                self.workspace.row() as f64,
            )?
        };
        if focus.edit_row() != self.workspace.row() {
            return Err(deadsync_editor::Error::InvalidEdit(
                "Presentation cursor is stale".into(),
            ));
        }
        let paused = self
            .paused_playback
            .as_ref()
            .filter(|_| self.paused_position().is_some())
            .map(|paused| PauseStamp {
                source: Arc::clone(&paused.source),
                revision: paused.revision,
                chart: paused.chart,
                edit_row: paused.edit_row,
                position: paused.position,
            });
        Ok(SongPropertyPresentation { focus, paused })
    }
    pub(super) fn rebind_song_property_presentation(
        &mut self,
        before: SongPropertyPresentation,
        receipt: &deadsync_editor::song_properties::PresentationReceipt,
    ) -> Result<(), deadsync_editor::Error> {
        let session = &self.workspace.file().session;
        let focus = before.focus.rebind_song_properties(session, receipt)?;
        if receipt.preserves_audio() {
            if let (Some(stamp), Some(paused)) = (before.paused, self.paused_playback.as_mut()) {
                if Arc::ptr_eq(&stamp.source, &paused.source)
                    && stamp.revision == paused.revision
                    && stamp.chart == paused.chart
                    && stamp.edit_row == paused.edit_row
                    && stamp.position.row.to_bits() == paused.position.row.to_bits()
                    && stamp.position.music_time.to_bits() == paused.position.music_time.to_bits()
                    && stamp.position.chart_time.to_bits() == paused.position.chart_time.to_bits()
                {
                    paused.source = Arc::clone(session.document().source());
                    paused.revision = session.content_revision();
                }
            }
        } else {
            self.stop_audio();
        }
        self.focus_position = Some(focus);
        Ok(())
    }

    /// Install the explicit application preference path; malformed files remain untouched.
    pub fn configure_audio_preferences(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), deadsync_editor::Error> {
        // Prepare every fallible resource before publishing a new owner. A
        // rejected file must leave the previous path, values and write guard intact.
        let mut file = deadsync_editor::audio_preferences::File::open(path)?;
        self.validate_source_preferences_effects(&file)?;
        let density_stream = file.density_stream()?;
        let minimap_content = file.minimap_content()?;
        let mut prepared_zoom = self.zoom.clone();
        file.zoom().initialize(&mut prepared_zoom)?;
        let previous_audio = self.audio_preferences;
        self.file_menu.install_recent(file.recent_songs().clone());
        self.configure_display_options(file.display_options());
        self.configure_general_options(file.general_options().clone())?;
        self.configure_autogen_preferences(file.autogen());
        self.configure_log_preferences(file.log_preferences());
        self.ftue = file.ftue().clone();
        let note_colors = file.note_colors_arc();
        let density = file.density();
        self.configure_receptor_preferences(file.receptors());
        self.configure_dark_preferences(file.dark());
        let streams = file.streams();
        self.configure_stream_preferences(streams)?;
        self.configure_performance_preferences(file.performance())?;
        if file.has_snap_preferences() {
            self.command(Command::SetSnap(file.snap().level.native_index()));
        }
        let zoom_preferences = file.zoom();
        self.zoom = prepared_zoom;
        self.zoom_preferences = zoom_preferences;
        self.audio_preferences = file.audio();
        self.install_scroll_preferences(file.scroll())?;
        self.selection_preferences = file.selection();
        self.workspace.set_auto_advance(
            file.note_entry_mode() == deadsync_editor::lane_entry::Mode::AdvanceBySnap,
        );
        self.configure_spacing_preference(file.spacing_preference());
        self.configure_key_binding_preferences(file.key_bindings());
        self.configure_minimap_preferences(file.minimap());
        self.configure_minimap_position_preferences(file.minimap_position());
        self.configure_note_color_arc(note_colors)?;
        self.configure_minimap_content_preferences(minimap_content);
        self.waveform_preferences = file.waveform();
        if file.source_archive().is_some() {
            self.workspace
                .source_preference_player(file.preferred_player())?;
            if self.workspace.has_chart() {
                file.set_preferred_player(self.workspace.player() as i32);
            }
        }
        self.source_preferences_import.memory = None;
        self.audio_preferences_path = Some(path.to_owned());
        self.audio_preferences_file = Some(file);
        self.source_density.stream = density_stream;
        self.configure_density_preferences(density)?;
        self.sync_editor_audio_settings(previous_audio);
        self.apply_waveform_preferences();
        Ok(())
    }
    pub fn audio_preferences(&self) -> deadsync_editor::audio_preferences::Audio {
        self.audio_preferences
    }
    /// Exact mapped chart time and fractional row for presentation only.
    pub(super) fn playback_position(&self) -> Option<(f64, f64)> {
        if !self.workspace.has_chart() {
            return None;
        }
        self.playback
            .as_ref()
            .and_then(|playing| playing.position)
            .or_else(|| self.paused_position())
            .map(|position| (position.chart_time, position.row))
            .or_else(|| {
                self.focus_position
                    .as_ref()
                    .filter(|position| position.is_current(&self.workspace))
                    .map(|position| (position.chart_time(), position.row()))
            })
    }
    /// Capture before stop_audio removes mapped/paused playback state. Sample
    /// audition has no chart Preview and must never drive chart focus.
    pub(super) fn transport_music_time(&self) -> Option<f64> {
        self.playback
            .as_ref()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
            .and_then(|playing| playing.position)
            .or_else(|| self.paused_position())
            .map(|position| position.music_time)
    }
    pub(super) fn chart_focus_music_time(&self) -> Result<f64, deadsync_editor::Error> {
        if let Some(time) = self.transport_music_time() {
            return Ok(time);
        }
        if let Some(position) = self
            .focus_position
            .as_ref()
            .filter(|position| position.is_current(&self.workspace))
        {
            return Ok(position.music_time());
        }
        deadsync_editor::playback::Clock::capture(
            &self.workspace.file().session,
            self.workspace.chart(),
        )?
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
    /// A scrollbar changes the editor position while the same decoded asset
    /// is paused. Keep that asset, but replace the exact resume position even
    /// when both positions share the nonnegative integral editing row.
    pub(super) fn retarget_paused_scroll_position(
        &mut self,
        focus: &deadsync_editor::chart_focus::Position,
    ) {
        let Some(paused) = self.paused_playback.as_mut() else {
            return;
        };
        let session = &self.workspace.file().session;
        if !Arc::ptr_eq(&paused.source, session.document().source())
            || paused.revision != session.content_revision()
            || paused.chart != self.workspace.chart()
        {
            return;
        }
        paused.edit_row = focus.edit_row();
        paused.position = Position {
            music_time: focus.music_time(),
            chart_time: focus.chart_time(),
            row: focus.row(),
        };
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
        let chart_transport = playing.preview.is_some() || playing.unfocused;
        let position = playing.position.or_else(|| {
            playing.preview.as_ref().and_then(|preview| {
                let music = preview.music_time(self.workspace.row()).ok()?;
                Some(Position {
                    music_time: music,
                    chart_time: preview.music_to_chart_time(music).ok()?,
                    row: preview.position(music).ok()?,
                })
            })
        });
        if chart_transport {
            self.paused_playback = position.map(|position| Paused {
                intent_origin: playing
                    .visual_clock
                    .as_ref()
                    .map(|clock| clock.intent_origin()),
                preview: playing.preview,
                asset_replaced: playing.asset_replaced,
                source: Arc::clone(self.workspace.file().session.document().source()),
                revision: self.workspace.file().session.content_revision(),
                chart: self.workspace.chart(),
                edit_row: self.workspace.row(),
                position,
            });
            if let Some(paused) = &self.paused_playback {
                self.audio_commands
                    .push(GameplayAudioCommand::EditorPauseMusic {
                        owner: self.audio_session.scope.owner,
                        seconds: paused.position.music_time
                            + self.audio_preferences.effective().offset,
                    });
            }
        } else {
            self.paused_playback = None;
            self.audio_commands.push(GameplayAudioCommand::StopMusic);
        }
        self.status = i18n::tr("Edition", "PlaybackPaused");
    }
    pub(super) fn chart_transport_is_playing(&self) -> bool {
        self.playback
            .as_ref()
            .is_some_and(|playing| playing.preview.is_some() || playing.unfocused)
    }
    /// Autoplay requires a validated chart position, including acknowledged
    /// silent continuation. Merely queued decoding, sample audition and pause
    /// never qualify. Legacy synthetic callers have no backend generation.
    pub(super) fn chart_autoplay_transport(&self) -> Option<(Option<u64>, f64)> {
        if !self.workspace.has_chart() {
            return None;
        }
        if let Some(time) = self.host_visual_transport() {
            return Some((None, time));
        }
        let playing = self
            .playback
            .as_ref()
            .filter(|playing| playing.preview.is_some() || playing.unfocused && playing.mapped)?;
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
            if !playing.asset_replaced {
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
    /// Source OnAudioOffsetChanged preserves the current visual song time and
    /// repositions audio to that time under the new offset. A sample audition
    /// is independent from MusicOffset; paused transport applies it on resume.
    pub(super) fn apply_audio_offset_change(&mut self) {
        if !self.chart_transport_is_playing() {
            return;
        }
        let now = Instant::now();
        self.update_source_visual_clock(now);
        let Some(time) = self
            .transport_music_time()
            .or_else(|| self.chart_focus_music_time().ok())
        else {
            return;
        };
        let Some(playing) = self.playback.as_mut() else {
            return;
        };
        let position = playing
            .position
            .or_else(|| {
                playing.preview.as_ref().and_then(|preview| {
                    Some(Position {
                        music_time: time,
                        chart_time: preview.music_to_chart_time(time).ok()?,
                        row: preview.position(time).ok()?,
                    })
                })
            })
            .unwrap_or(Position {
                music_time: time,
                chart_time: 0.,
                row: 0.,
            });
        playing.valid_after = now;
        playing.position = Some(position);
        playing.mapped = false;
        if let Some(clock) = playing.visual_clock.as_mut() {
            clock.seek(position.music_time, now);
        }
        self.audio_commands.push(GameplayAudioCommand::SeekMusic(
            position.music_time + self.audio_preferences.effective().offset,
        ));
    }

    pub(super) fn apply_playback_rate(&mut self) {
        self.apply_playback_rate_at(Instant::now());
    }
    pub(in crate::screens::edition) fn apply_playback_rate_at(&mut self, now: Instant) {
        if let Some(playing) = self
            .playback
            .as_mut()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
        {
            playing.valid_after = now;
            if let Some(clock) = playing.visual_clock.as_mut() {
                clock.set_rate(
                    self.audio_preferences.effective().rate,
                    playing.valid_after,
                    playing.visual_reference,
                );
            }
            playing.mapped = false;
            self.audio_commands
                .push(GameplayAudioCommand::EditorSetMusicRate {
                    owner: self.audio_session.scope.owner,
                    rate: self.audio_preferences.effective().rate,
                });
        }
    }
    pub fn take_audio_commands(&mut self) -> Vec<GameplayAudioCommand> {
        self.scoped_audio_commands()
    }
    pub(super) fn stop_audio(&mut self) {
        if self.workspace.has_chart() {
            if let Some(time) = self.transport_music_time() {
                self.focus_position = deadsync_editor::chart_focus::Position::at_time(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    time,
                )
                .ok();
            }
        }
        if self.playback.is_some() {
            self.pause_audio();
        }
        let paused = self.paused_playback.take().is_some();
        if paused {
            self.status = i18n::tr("Edition", "PlaybackStopped");
        }
    }
    pub(super) fn toggle_audio(&mut self) {
        if !self.has_song() {
            return;
        }
        self.rearm_editor_scope_for_transport();
        if self.chart_transport_is_playing() {
            self.pause_audio();
            return;
        }
        if self.is_playing() {
            self.stop_audio();
        }
        let focused = self.workspace.has_chart();
        let mut resume = self.paused_position().or_else(|| {
            self.focus_position
                .as_ref()
                .filter(|position| position.is_current(&self.workspace))
                .map(|position| Position {
                    music_time: position.music_time(),
                    chart_time: position.chart_time(),
                    row: position.row(),
                })
        });
        if !focused {
            resume = Some(Position {
                music_time: 0.,
                chart_time: 0.,
                row: 0.,
            });
        }
        let result=(||->Result<(Option<Preview>,Option<std::path::PathBuf>,f64,bool),deadsync_editor::Error>{
            if !focused{
                let path=self.unfocused_music.clone().or_else(||self.workspace.file().session.document().text(None,"MUSIC").filter(|name|!name.is_empty()).map(|name|self.workspace.file().resource_base().join(name.replace('\\',"/"))));
                let retained=path.is_some();return Ok((None,path,0.,retained));
            }
            let retained=self.paused_playback.as_ref().filter(|_|self.paused_position().is_some()).and_then(|paused|paused.preview.as_ref().and_then(|preview|preview.rebind_same_music(&self.workspace.file().session,self.workspace.chart(),self.workspace.file().resource_base()).ok().flatten()));
            let preview=match retained{Some(preview)=>preview,None=>Preview::rebind_chart_asset(&self.workspace.file().session,self.workspace.chart(),self.workspace.file().resource_base())?};
            let retained=preview.music_asset().is_some()&&self.unfocused_music.as_ref().is_some_and(|path|preview.has_music_path(path));
            let time=resume.map_or_else(||preview.music_time(self.workspace.row()),|position|Ok(position.music_time))?;let path=preview.music_asset().map(std::path::Path::to_path_buf);Ok((Some(preview),path,time,retained))
        })();
        match result {
            Ok((preview, path, time, retained)) => {
                self.unfocused_music = path.clone();
                self.paused_playback = None;
                if retained {
                    self.audio_commands
                        .push(GameplayAudioCommand::EditorResumeMusic {
                            owner: self.audio_session.scope.owner,
                            path: path.expect("retained asset exists"),
                            seconds: time + self.audio_preferences.effective().offset,
                            rate: self.audio_preferences.effective().rate,
                        });
                } else {
                    self.audio_commands
                        .push(GameplayAudioCommand::EditorStartMusic {
                            owner: self.audio_session.scope.owner,
                            path,
                            seconds: time + self.audio_preferences.effective().offset,
                            rate: self.audio_preferences.effective().rate,
                        });
                }
                let now = Instant::now();
                let resume = if self.audio_backend_available == Some(false) {
                    resume.or_else(|| {
                        preview.as_ref().and_then(|preview| {
                            Some(Position {
                                music_time: time,
                                chart_time: preview.music_to_chart_time(time).ok()?,
                                row: preview.position(time).ok()?,
                            })
                        })
                    })
                } else {
                    resume
                };
                self.playback = Some(Playing {
                    sample_descriptor: None,
                    sample_cursor: None,
                    sample_registration: 0,
                    pending_sample: None,
                    visual_clock: deadsync_editor::source_visual_clock::SourceVisualClock::start(
                        time,
                        self.audio_preferences.effective().rate,
                        now,
                    ),
                    visual_active: self.audio_backend_available == Some(false),
                    visual_reference: None,
                    asset_epoch: None,
                    unfocused: !focused,
                    asset_replaced: retained,
                    preview,
                    decoder: None,
                    decoder_ack: None,
                    clock_generation: None,
                    issued_generation: None,
                    precise_stream: None,
                    valid_after: now,
                    mapped: false,
                    mapped_generation: None,
                    position: resume,
                });
                self.status = i18n::tr(
                    "Edition",
                    if self.audio_backend_available == Some(false) {
                        "AudioDspUnavailable"
                    } else {
                        "PlaybackPlaying"
                    },
                );
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "PlaybackError",
                    &[("error", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    /// Keyboard navigation shares the original snapped-position and playback policy.
    pub(super) fn navigate_snap(&mut self, later: bool) {
        let current = self
            .playback_position()
            .map_or(self.workspace.row() as f64, |(_, row)| row);
        let result = (|| -> Result<f64, deadsync_editor::Error> {
            let signatures = self.signatures.as_ref().ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("Missing chart time signatures".into())
            })?;
            deadsync_editor::snap::Level::native(self.workspace.snap())?
                .navigation(current, later, signatures)
        })();
        match result {
            Ok(exact) => self.navigate_exact_position(exact, false),
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "PlaybackError",
                    &[("error", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    /// Shared precise scrolling policy: optional stop, otherwise seek the open
    /// transport and retain fractional/negative view position without reopening.
    pub(super) fn navigate_exact_position(&mut self, exact: f64, preserve_selection: bool) {
        self.wheel.cancel();
        let result = self.navigate_wheel_position(false, exact, preserve_selection);
        if let Err(error) = result {
            self.status = i18n::tr_fmt(
                "Edition",
                "PlaybackError",
                &[("error", &crate::i18n::editor_error(&error))],
            );
        }
    }
    pub(super) fn navigate_exact_time(
        &mut self,
        time: f64,
        preserve_selection: bool,
    ) -> Result<(), deadsync_editor::Error> {
        self.navigate_wheel_position(true, time, preserve_selection)
    }
    pub(super) fn navigate_wheel_position(
        &mut self,
        time: bool,
        value: f64,
        preserve_selection: bool,
    ) -> Result<(), deadsync_editor::Error> {
        let focus = if time {
            deadsync_editor::chart_focus::Position::at_time(
                &self.workspace.file().session,
                self.workspace.chart(),
                value,
            )?
        } else {
            deadsync_editor::chart_focus::Position::at_fractional_row(
                &self.workspace.file().session,
                self.workspace.chart(),
                value,
            )?
        };
        let destination = focus.row().max(0.0).floor() as u64;
        let chart_transport = self.chart_transport_is_playing();
        let sample_only = self.is_playing() && !chart_transport;
        let continuing = chart_transport && !self.scroll_preferences.stop_playback();
        // The captured clock and open decoder remain valid after pathname
        // changes; navigation must not re-open or re-parse the track.
        let audio = if continuing {
            let _preview = self
                .playback
                .as_ref()
                .and_then(|playing| playing.preview.as_ref())
                .ok_or_else(|| {
                    deadsync_editor::Error::InvalidEdit("No chart playback clock".into())
                })?;
            Some(Position {
                chart_time: focus.chart_time(),
                row: focus.row(),
                music_time: focus.music_time(),
            })
        } else {
            None
        };
        if !continuing && !sample_only {
            self.stop_audio();
        }
        if preserve_selection {
            self.workspace.seek_preserving_selection(destination)?;
            self.receptor_views.reset();
        } else {
            self.command(Command::Seek(destination));
        }
        self.focus_position = Some(focus);
        self.refresh();
        if let Some(position) = audio {
            self.paused_playback = None;
            let playing = self.playback.as_mut().expect("continued chart transport");
            playing.valid_after = Instant::now();
            playing.mapped = false;
            playing.position = Some(position);
            if let Some(clock) = playing.visual_clock.as_mut() {
                clock.seek(position.music_time, playing.valid_after);
            }
            self.audio_commands.push(GameplayAudioCommand::SeekMusic(
                position.music_time + self.audio_preferences.effective().offset,
            ));
            self.status = i18n::tr("Edition", "PlaybackPlaying");
        }
        Ok(())
    }
    /// The global source shortcut auditions the current document without opening
    /// or replacing a staged song-properties inspector.
    pub(super) fn command_toggle_sample_audio(&mut self) {
        let kind = self
            .workspace
            .song_properties()
            .as_ref()
            .map(Self::sample_asset_kind)
            .unwrap_or(deadsync_gameplay::EditorAssetKind::Main);
        let result = self.workspace.song_properties().and_then(|draft| {
            draft.sample_audio_from_directory(
                &self.workspace.file().session,
                self.workspace.chart(),
                self.workspace.file().resource_base(),
            )
        });
        self.toggle_sample_audio_role_result(kind, result);
    }
    pub(super) fn toggle_sample_audio(&mut self) {
        let kind = self
            .song_properties
            .as_ref()
            .map(Self::sample_asset_kind)
            .unwrap_or(deadsync_gameplay::EditorAssetKind::Main);
        let result = self
            .song_properties
            .as_ref()
            .ok_or_else(|| deadsync_editor::Error::InvalidEdit("No song draft".into()))
            .and_then(|draft| {
                draft.sample_audio_from_directory(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    self.workspace.file().resource_base(),
                )
            });
        self.toggle_sample_audio_role_result(kind, result);
    }
    #[cfg(test)]
    fn toggle_sample_audio_result(
        &mut self,
        result: Result<deadsync_editor::song_properties::SampleAudio, deadsync_editor::Error>,
    ) {
        let kind = self
            .workspace
            .song_properties()
            .as_ref()
            .map(Self::sample_asset_kind)
            .unwrap_or(deadsync_gameplay::EditorAssetKind::Main);
        self.toggle_sample_audio_role_result(kind, result);
    }
    pub(super) fn toggle_sample_audio_role_result(
        &mut self,
        kind: deadsync_gameplay::EditorAssetKind,
        result: Result<deadsync_editor::song_properties::SampleAudio, deadsync_editor::Error>,
    ) {
        self.rearm_editor_scope_for_transport();
        if self
            .playback
            .as_ref()
            .is_some_and(|playing| playing.preview.is_none())
        {
            self.stop_audio();
            return;
        }
        // Source Editor.OnTogglePlayPreview stops chart playback before trying
        // MusicManager.StartPreviewPlayback, even when the selected cache is pending.
        self.stop_audio();
        self.sync_editor_asset_registration();
        match result {
            Ok(sample) => {
                // Source media pickers commit immediately. The native inspector
                // stages paths, so require Apply before auditioning a new source.
                if !self.sample_path_is_registered(kind, &sample.path) {
                    self.status = i18n::tr("Edition", "SongSampleApplyMedia");
                    return;
                }
                if !self.editor_preview_ready(kind) {
                    self.status = i18n::tr("Edition", "SongSampleNotReady");
                    return;
                }
                let request = GameplayAudioCommand::EditorPlayPreview {
                    owner: self.editor_asset_owner(),
                    kind,
                    path: self
                        .configured_preview_path(kind)
                        .expect("validated registered preview source"),
                    cut: GameplayMusicCut {
                        start_sec: sample.start,
                        length_sec: sample.length.unwrap_or(f64::INFINITY),
                        fade_in_sec: self.audio_preferences.effective().fade_in,
                        fade_out_sec: self.audio_preferences.effective().fade_out,
                    },
                    looping: true,
                    rate: 1.0,
                };
                self.audio_commands.push(request.clone());
                let sample_registration = self.editor_asset_registration();
                self.playback = Some(Playing {
                    sample_descriptor: Some(request.clone()),
                    sample_cursor: None,
                    sample_registration,
                    pending_sample: (self.audio_backend_available == Some(false))
                        .then_some(request),
                    visual_clock: None,
                    visual_active: false,
                    visual_reference: None,
                    asset_epoch: None,
                    unfocused: false,
                    asset_replaced: false,
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
                self.status = i18n::tr(
                    "Edition",
                    if self.audio_backend_available == Some(false) {
                        "AudioDspUnavailable"
                    } else {
                        "SongSamplePlaying"
                    },
                );
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "PlaybackError",
                    &[("error", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    /// Bind the generation synchronously assigned to the last drained audio
    /// command. Repeated frame updates must not reset an existing anchor.
    pub fn bind_decoder_generation(&mut self, generation: Option<u64>) {
        if !self.audio_output_transport_ready() {
            return;
        }
        let Some(playing) = self
            .playback
            .as_ref()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
        else {
            return;
        };
        if generation.is_some() && playing.issued_generation == generation {
            return;
        }
        let Some(generation) = generation else {
            if self
                .playback
                .as_ref()
                .is_some_and(|playing| playing.visual_active)
            {
                return;
            }
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
        let issued = if playing.asset_replaced {
            decoder.issue_asset_generation(
                generation,
                start + self.audio_preferences.effective().offset,
                self.audio_preferences.effective().rate,
            )
        } else {
            decoder.issue(
                generation,
                start + self.audio_preferences.effective().offset,
            )
        };
        if !issued {
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
    pub(super) fn update_sample_fade_descriptors(&mut self, incoming: f64, outgoing: f64) {
        let update = |command: &mut GameplayAudioCommand| {
            if let GameplayAudioCommand::EditorPlayPreview { cut, .. }
            | GameplayAudioCommand::EditorResumePreview { cut, .. } = command
            {
                cut.fade_in_sec = incoming;
                cut.fade_out_sec = outgoing;
            }
        };
        if let Some(playing) = self.playback.as_mut() {
            if let Some(command) = playing.sample_descriptor.as_mut() {
                update(command);
            }
            if let Some(command) = playing.pending_sample.as_mut() {
                update(command);
            }
        }
    }
    pub fn bind_decoder_asset_epoch(&mut self, owner: Option<u64>, epoch: Option<u64>) {
        if let Some(playing) = self
            .playback
            .as_mut()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
        {
            if owner == Some(self.audio_session.scope.owner) {
                playing.asset_epoch = epoch;
            } else if owner.is_some() {
                self.stop_audio();
            }
        }
    }
    /// Optional editor-only f64 stream time from the exact same clock snapshot.
    /// Legacy gameplay snapshots and callers without this side channel keep their
    /// existing f32 contract. Stale identities or non-finite values are ignored.
    pub fn set_decoder_stream_time(&mut self, generation: u64, valid_at: Instant, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        if let Some(playing) = self
            .playback
            .as_mut()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
            && playing.issued_generation == Some(generation)
        {
            playing.precise_stream = Some((generation, valid_at, seconds));
        }
    }
    /// Install the backend acknowledgement before update_audio. The native
    /// adapter supplies the synchronous command generation for the clock too.
    pub fn set_decoder_status(&mut self, generation: u64, ack: Option<Ack>) {
        if let Some(playing) = self
            .playback
            .as_mut()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
        {
            playing.clock_generation = Some(generation);
            playing.decoder_ack = ack;
        }
    }
    /// Mapped audio or acknowledged silent stream time advances the cursor.
    /// Wall-clock time alone never claims decoding or playback succeeded.
    pub fn update_audio(&mut self, snapshot: GameplayAudioSnapshot) {
        self.observed_audio_owner = Some(snapshot.editor.owner);
        self.observe_editor_assets(snapshot.editor.assets);
        self.observe_editor_output(snapshot.editor.output);
        if self.audio_backend_available == Some(false) && self.host_visual_transport().is_some() {
            return;
        }
        if !self.audio_output_transport_ready() {
            return;
        }
        let Some(playing) = self.playback.as_mut() else {
            return;
        };
        if playing.preview.is_none() && !playing.unfocused {
            return;
        }
        let clock = snapshot.stream_clock;
        if playing.decoder.is_some() && playing.clock_generation != playing.issued_generation {
            return;
        }
        if clock.valid_at < playing.valid_after {
            return;
        }
        // Reject out-of-order snapshots as well as snapshots from before play.
        playing.valid_after = clock.valid_at;
        let logical = snapshot.editor.logical_music;
        if logical.is_some()
            && (!snapshot.editor.output.available || !snapshot.editor.output.leased)
        {
            return;
        }
        if logical.is_some_and(|phase| {
            phase.generation != playing.issued_generation.unwrap_or(0)
                || Some(phase.asset_epoch) != playing.asset_epoch
                || snapshot.editor.owner != self.audio_session.scope.owner
        }) {
            return;
        }
        if playing.decoder.is_some()
            && logical.is_none()
            && !playing.asset_replaced
            && let Some(ack) = playing
                .decoder_ack
                .filter(|ack| Some(ack.generation) == playing.issued_generation)
            && matches!(
                ack.phase,
                super::decoder_transport::Phase::Failed | super::decoder_transport::Phase::Stopped
            )
        {
            let failed = ack.phase == super::decoder_transport::Phase::Failed;
            self.stop_audio();
            if failed {
                self.status = i18n::tr("Edition", "PlaybackDecoderFailed");
            }
            return;
        }
        if logical.is_some() {
            playing.asset_replaced = true;
        }
        if logical.is_none() && playing.asset_epoch.is_some() && playing.asset_replaced {
            return;
        }
        if logical.is_some()
            && playing.decoder_ack.is_some_and(|ack| {
                Some(ack.generation) == playing.issued_generation
                    && ack.phase == super::decoder_transport::Phase::Failed
            })
        {
            self.status = i18n::tr("Edition", "PlaybackDecoderFailed");
        }
        let seconds = if let Some(phase) = logical {
            phase.music_nanos as f64 / 1e9
        } else if let Some(decoder) = playing.decoder.as_mut() {
            let Some(generation) = playing.clock_generation else {
                return;
            };
            let observed_clock = Clock {
                generation,
                valid_at: clock.valid_at,
                stream_seconds: playing
                    .precise_stream
                    .filter(|(precise_generation, stamp, _)| {
                        *precise_generation == generation && *stamp == clock.valid_at
                    })
                    .map_or_else(
                        || f64::from(clock.stream_seconds),
                        |(_, _, seconds)| seconds,
                    ),
                music_seconds: clock
                    .has_music_mapping
                    .then_some(clock.music_nanos as f64 / 1_000_000_000.0),
                rate: if clock.has_music_mapping {
                    f64::from(clock.music_seconds_per_second)
                } else {
                    self.audio_preferences.effective().rate
                },
            };
            let step = if playing.asset_replaced {
                decoder.observe_asset_phase(observed_clock)
            } else {
                decoder.observe(playing.decoder_ack, observed_clock)
            };
            match step {
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
                if playing.mapped {
                    self.pause_audio();
                }
                return;
            }
            clock.music_nanos as f64 / 1_000_000_000.0
        };
        playing.mapped = true;
        let actual_music_seconds = seconds;
        let seconds = seconds - self.audio_preferences.effective().offset;
        let seconds = if playing.visual_active {
            playing.visual_reference = Some(seconds);
            let Some(reading) = playing.visual_clock.as_mut().and_then(|clock| {
                clock.advance(Instant::now(), self.workspace.has_chart(), Some(seconds))
            }) else {
                return;
            };
            reading.music_time
        } else {
            seconds
        };
        let result = if self.workspace.has_chart() {
            playing
                .preview
                .as_ref()
                .ok_or_else(|| {
                    deadsync_editor::Error::InvalidEdit("No focused playback clock".into())
                })
                .and_then(|preview| {
                    preview
                        .position(seconds)
                        .and_then(|row| Ok((preview.music_to_chart_time(seconds)?, row)))
                })
        } else {
            Ok((0.0, 0.0))
        };
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
                if self.workspace.has_chart() && row != self.workspace.row() {
                    let _ = self.workspace.seek_preserving_selection(row);
                    self.refresh();
                }
            }
            Err(error) => {
                self.stop_audio();
                self.status = i18n::tr_fmt(
                    "Edition",
                    "PlaybackError",
                    &[("error", &crate::i18n::editor_error(&error))],
                );
            }
        }
        self.queue_editor_ticks(snapshot, actual_music_seconds);
    }
    pub(super) fn has_logical_music_phase(&self, snapshot: &GameplayAudioSnapshot) -> bool {
        snapshot.editor.output.available
            && snapshot.editor.output.leased
            && snapshot.editor.owner == self.audio_session.scope.owner
            && self.playback.as_ref().is_some_and(|playing| {
                snapshot.editor.logical_music.is_some_and(|phase| {
                    Some(phase.generation) == playing.issued_generation
                        && Some(phase.asset_epoch) == playing.asset_epoch
                })
            })
    }
    pub(super) fn chart_audio_generation(&self) -> Option<u64> {
        self.playback
            .as_ref()
            .filter(|playing| playing.preview.is_some() || playing.unfocused)
            .and_then(|playing| playing.issued_generation)
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
        assert!(
            state
                .take_audio_commands()
                .iter()
                .any(|c| matches!(c,GameplayAudioCommand::EditorBeginAudio{owner,..} if *owner!=0))
        );
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.assets = deadsync_gameplay::EditorAssetReadiness {
            owner: state.editor_asset_owner(),
            registration: state.editor_asset_registration(),
            main: true,
            dedicated_preview: false,
        };
        state.update_audio(snapshot);
        state.take_audio_commands();
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
#[cfg(test)]
mod song_timing_pause_tests {
    use super::*;
    #[test]
    fn timing_choice_undo_rebinds_pause_to_content_revision_after_application_preferences() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#STOPS:1=1;#WARPS:2=2;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#NOTES:0100;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.workspace.execute(Command::Seek(120)).unwrap();
        let focus = deadsync_editor::chart_focus::Position::at_fractional_row(
            &state.workspace.file().session,
            0,
            120.5,
        )
        .unwrap();
        state.focus_position = Some(focus.clone());
        state.paused_playback = Some(Paused {
            intent_origin: None,
            preview: None,
            asset_replaced: false,
            source: Arc::clone(state.workspace.file().session.document().source()),
            revision: state.workspace.file().session.content_revision(),
            chart: 0,
            edit_row: 120,
            position: Position {
                row: 120.5,
                music_time: focus.music_time(),
                chart_time: focus.music_time(),
            },
        });
        let before = deadsync_editor::dark_preferences::Preferences::default();
        let mut after = before;
        after.set_show(true);
        state
            .workspace
            .set_dark_preferences(before, after, None)
            .unwrap();
        assert_ne!(
            state.workspace.file().session.revision(),
            state.workspace.file().session.content_revision()
        );
        assert!(state.paused_position().is_some());
        let presentation = state.capture_song_property_presentation().unwrap();
        let mut draft = state.workspace.song_properties().unwrap();
        draft
            .set_timing_chart(&state.workspace.file().session, Some(1))
            .unwrap();
        let application = state
            .workspace
            .apply_song_properties_with_presentation_receipt(&draft)
            .unwrap();
        state
            .rebind_song_property_presentation(
                presentation,
                application.presentation_receipt().unwrap(),
            )
            .unwrap();
        state.apply_song_timing_history(false);
        assert!(state.paused_position().is_some());
        assert_eq!(
            state.paused_position().unwrap().row.to_bits(),
            120.5f64.to_bits()
        );
        assert_eq!(
            state.focus_position.as_ref().unwrap().row().to_bits(),
            120.5f64.to_bits()
        );
        assert!(state.workspace.take_dark_preferences_update().is_some());
    }
}

pub(super) struct HistoryRowAnchor {
    identity: deadsync_editor::generation::metadata::Guid,
    before: SongPropertyPresentation,
}
impl State {
    /// Source Undo does not replace EditorPosition's double cursor. The native
    /// validated clock reprojects that same row after a timing edit, keeping its
    /// fractional/negative anchor rather than falling back to the integer grid.
    pub(super) fn capture_history_row_anchor(&self) -> Option<HistoryRowAnchor> {
        if !self.has_song() || !self.workspace.has_chart() || self.chart_transport_is_playing() {
            return None;
        }
        let before = self.capture_song_property_presentation().ok()?;
        Some(HistoryRowAnchor {
            identity: self
                .workspace
                .file()
                .session
                .chart_ids()
                .get(self.workspace.chart())?
                .clone(),
            before,
        })
    }
    pub(super) fn restore_history_row_anchor(
        &mut self,
        anchor: HistoryRowAnchor,
    ) -> Result<(), deadsync_editor::Error> {
        if self.chart_transport_is_playing()
            || !self.workspace.has_chart()
            || self.workspace.chart() != anchor.before.focus.chart()
            || self.workspace.row() != anchor.before.focus.edit_row()
            || self
                .workspace
                .file()
                .session
                .chart_ids()
                .get(self.workspace.chart())
                != Some(&anchor.identity)
        {
            return Ok(());
        }
        let session = &self.workspace.file().session;
        let focus = deadsync_editor::chart_focus::Position::at_fractional_row(
            session,
            self.workspace.chart(),
            anchor.before.focus.row(),
        )?;
        if let (Some(stamp), Some(paused)) = (anchor.before.paused, self.paused_playback.as_mut()) {
            if Arc::ptr_eq(&stamp.source, &paused.source)
                && stamp.revision == paused.revision
                && stamp.chart == paused.chart
                && stamp.edit_row == paused.edit_row
                && stamp.position.row.to_bits() == paused.position.row.to_bits()
                && stamp.position.music_time.to_bits() == paused.position.music_time.to_bits()
            {
                paused.source = Arc::clone(session.document().source());
                paused.revision = session.content_revision();
                paused.position = Position {
                    row: focus.row(),
                    chart_time: focus.chart_time(),
                    music_time: focus.music_time(),
                };
            }
        }
        if let Some(views) = self.active_views.as_mut() {
            views.synchronize_focused_position(session, &focus)?;
        }
        self.focus_position = Some(focus);
        Ok(())
    }
}

#[cfg(test)]
mod history_anchor_tests {
    use super::*;
    #[test]
    fn undo_redo_preserve_fractional_and_negative_row_without_rounding_warp_inverse() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#OFFSET:0.25;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        for row in [-0.75_f64, 160.75] {
            let mut s = State::open(&path, 0).unwrap();
            let owner = s
                .take_audio_commands()
                .iter()
                .find_map(|command| match command {
                    deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio { owner, .. } => {
                        Some(*owner)
                    }
                    _ => None,
                })
                .unwrap();
            let mut ack = deadsync_gameplay::GameplayAudioSnapshot::default();
            ack.editor.owner = owner;
            ack.editor.output.available = true;
            ack.editor.output.leased = true;
            ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
            ack.editor.output.requested_period = Some(512);
            ack.editor.output.negotiated_period = Some(512);
            s.update_audio(ack);
            s.take_audio_commands();
            s.workspace
                .seek_preserving_selection(row.max(0.0).floor() as u64)
                .unwrap();
            s.focus_position = Some(
                deadsync_editor::chart_focus::Position::at_fractional_row(
                    &s.workspace.file().session,
                    0,
                    row,
                )
                .unwrap(),
            );
            s.refresh();
            let mut draft = deadsync_editor::timing_edit::region::Draft::capture(
                &s.workspace.file().session,
                0,
                deadsync_editor::timing_edit::Scope::Chart,
            )
            .unwrap();
            draft
                .set(deadsync_editor::timing_edit::region::Kind::Warp, 144, 48)
                .unwrap();
            s.workspace.apply_regions(&draft).unwrap();
            // Simulate the actual accepted quick-add/timing action retaining its
            // chosen row, then test the real command dispatcher history path.
            s.focus_position = Some(
                deadsync_editor::chart_focus::Position::at_fractional_row(
                    &s.workspace.file().session,
                    0,
                    row,
                )
                .unwrap(),
            );
            s.refresh();
            s.command(Command::Undo);
            assert_eq!(s.focus_position.as_ref().unwrap().row(), row);
            s.command(Command::Redo);
            assert_eq!(s.focus_position.as_ref().unwrap().row(), row);
            assert_eq!(
                s.active_views
                    .as_ref()
                    .unwrap()
                    .focused()
                    .unwrap()
                    .position()
                    .row(),
                row
            );
        }
    }
}

impl State {
    pub(super) fn prepare_chart_focus_audio(&self, chart: usize) -> Option<(Preview, bool)> {
        if self
            .playback
            .as_ref()
            .is_some_and(|playing| playing.preview.is_none() && !playing.unfocused)
            || (self.playback.is_none() && self.paused_playback.is_none())
        {
            return None;
        }
        let old = self
            .playback
            .as_ref()
            .and_then(|playing| playing.preview.as_ref())
            .or_else(|| {
                self.paused_playback
                    .as_ref()
                    .and_then(|paused| paused.preview.as_ref())
            });
        let session = &self.workspace.file().session;
        let directory = self.workspace.file().resource_base();
        let target = Preview::rebind_chart_asset(session, chart, directory).ok()?;
        let Some(old) = old else {
            return self.unfocused_music.as_ref().map(|path| {
                let replaced = !target.has_music_path(path);
                (target, replaced)
            });
        };
        match old.rebind_same_music(session, chart, directory).ok()? {
            Some(preview) => Some((preview, false)),
            None => Preview::rebind_chart_asset(session, chart, directory)
                .ok()
                .map(|preview| (preview, true)),
        }
    }
    pub(super) fn apply_chart_focus_audio(
        &mut self,
        preview: Option<(Preview, bool)>,
        was_paused: bool,
        focus: &deadsync_editor::chart_focus::Position,
    ) {
        let position = Position {
            music_time: focus.music_time(),
            chart_time: focus.chart_time(),
            row: focus.row(),
        };
        if was_paused {
            let intent_origin = self
                .paused_playback
                .as_ref()
                .and_then(|paused| paused.intent_origin);
            let replaced_before = self
                .paused_playback
                .as_ref()
                .is_some_and(|paused| paused.asset_replaced);
            let (preview, replaced) = preview.map_or((None, false), |(preview, replaced)| {
                (Some(preview), replaced)
            });
            if replaced {
                if let Some(preview) = &preview {
                    self.audio_commands.push(match preview.music_asset() {
                        Some(path) => GameplayAudioCommand::EditorSwapMusicAsset {
                            owner: self.audio_session.scope.owner,
                            path: path.to_path_buf(),
                        },
                        None => GameplayAudioCommand::EditorClearMusicAsset {
                            owner: self.audio_session.scope.owner,
                        },
                    });
                }
            }
            self.paused_playback = Some(Paused {
                intent_origin,
                preview,
                asset_replaced: replaced || replaced_before,
                source: Arc::clone(self.workspace.file().session.document().source()),
                revision: self.workspace.file().session.content_revision(),
                chart: self.workspace.chart(),
                edit_row: self.workspace.row(),
                position,
            });
        } else if let Some((preview, replaced)) = preview {
            let playing = self.playback.as_mut().expect("prepared active transport");
            playing.unfocused = false;
            if replaced {
                self.audio_commands.push(match preview.music_asset() {
                    Some(path) => GameplayAudioCommand::EditorSwapMusicAsset {
                        owner: self.audio_session.scope.owner,
                        path: path.to_path_buf(),
                    },
                    None => GameplayAudioCommand::EditorClearMusicAsset {
                        owner: self.audio_session.scope.owner,
                    },
                });
                playing.asset_replaced = true;
                playing.mapped_generation = None;
            }
            playing.preview = Some(preview);
            playing.position = Some(position);
        } else {
            self.stop_audio();
        }
    }
}

#[cfg(test)]
mod chart_audio_focus_tests {
    use super::*;
    fn state() -> (tempfile::TempDir, State, u64) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("music.wav"), b"command fixture").unwrap();
        std::fs::write(root.path().join("song.ssc"),"#VERSION:0.83;#MUSIC:music.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:music.wav;#OFFSET:0.5;#BPMS:0=60;#NOTES:1000;").unwrap();
        let mut s = State::open(&root.path().join("song.ssc"), 0).unwrap();
        let owner = s
            .take_audio_commands()
            .iter()
            .find_map(|c| {
                if let GameplayAudioCommand::EditorBeginAudio { owner, .. } = c {
                    Some(*owner)
                } else {
                    None
                }
            })
            .unwrap();
        let mut ack = GameplayAudioSnapshot::default();
        ack.editor.owner = owner;
        ack.editor.output.available = true;
        ack.editor.output.leased = true;
        ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        ack.editor.output.requested_period = Some(512);
        ack.editor.output.negotiated_period = Some(512);
        s.update_audio(ack);
        s.take_audio_commands();
        (root, s, owner)
    }
    fn update(s: &mut State, owner: u64, time: f64) {
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = s.audio_output_snapshot();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.stream_clock.has_music_mapping = true;
        snapshot.stream_clock.music_nanos = (time * 1e9) as i64;
        snapshot.stream_clock.music_seconds_per_second = 1.;
        s.update_audio(snapshot);
    }
    #[test]
    fn same_asset_focus_keeps_scoped_decoder_generation_clock_and_lease() {
        let (root, mut s, owner) = state();
        s.toggle_audio();
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorStartMusic{owner:actual,..}]if *actual==owner)
        );
        s.bind_decoder_generation(Some(7));
        s.set_decoder_status(
            7,
            Some(Ack {
                generation: 7,
                phase: super::super::decoder_transport::Phase::Producing,
                opened: true,
            }),
        );
        update(&mut s, owner, 1.25);
        assert!(s.chart_transport_is_playing());
        let output = s.audio_output_snapshot();
        // An already open asset survives pathname removal, just as the source
        // same-path early return does. Focus performs no reload or stat.
        std::fs::remove_file(root.path().join("music.wav")).unwrap();
        assert!(s.focus_chart_at_time(1, 1.25));
        assert!(s.chart_transport_is_playing());
        assert_eq!(s.chart_audio_generation(), Some(7));
        assert!(s.take_audio_commands().iter().all(|c| !matches!(
            c,
            GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPlayMusic { .. }
                | GameplayAudioCommand::EditorSeekMusic { .. }
                | GameplayAudioCommand::EditorEndAudio { .. }
        )));
        assert_eq!(s.audio_output_snapshot().leased, output.leased);
        let expected =
            deadsync_editor::chart_focus::Position::at_time(&s.workspace.file().session, 1, 1.25)
                .unwrap();
        assert_eq!(
            s.playback_position().unwrap(),
            (expected.chart_time(), expected.row())
        );
        update(&mut s, owner, 1.5);
        assert_eq!(s.chart_focus_time().unwrap(), 1.5);
    }
    #[test]
    fn retained_pause_resumes_deleted_asset_without_reopening_path() {
        let (root, mut s, owner) = state();
        s.toggle_audio();
        s.take_audio_commands();
        update(&mut s, owner, 1.25);
        s.toggle_audio();
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorPauseMusic{owner:actual,seconds}]if *actual==owner&&(*seconds-1.25-s.audio_preferences.effective().offset).abs()<1e-9)
        );
        std::fs::remove_file(root.path().join("music.wav")).unwrap();
        s.toggle_audio();
        assert!(s.is_playing());
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorResumeMusic{owner:actual,seconds,..}]if *actual==owner&&(*seconds-1.25-s.audio_preferences.effective().offset).abs()<1e-9)
        );
    }
    #[test]
    fn last_view_close_retains_music_and_reopening_uses_current_logical_instant() {
        let (_root, mut s, owner) = state();
        s.toggle_audio();
        s.take_audio_commands();
        s.bind_decoder_generation(Some(17));
        s.bind_decoder_asset_epoch(Some(owner), Some(29));
        s.set_decoder_status(
            17,
            Some(Ack {
                generation: 17,
                phase: super::super::decoder_transport::Phase::Producing,
                opened: true,
            }),
        );
        update(&mut s, owner, 1.25);
        s.take_audio_commands();
        s.chart_views_apply(super::super::chart_views::ViewAction::Close);
        assert!(!s.workspace.has_chart());
        assert!(s.is_playing());
        assert!(s.playback_position().is_none());
        assert!(s.take_audio_commands().iter().all(|c| !matches!(
            c,
            GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorEndAudio { .. }
        )));
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = s.audio_output_snapshot();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 17,
            asset_epoch: 29,
            music_nanos: 1_750_000_000,
        });
        s.update_audio(snapshot);
        assert!(s.is_playing());
        assert!(s.playback_position().is_none());
        assert_eq!(
            s.transport_music_time(),
            Some(1.75 - s.audio_preferences.effective().offset)
        );
        assert!(s.take_audio_commands().is_empty());
        let time = s.chart_focus_time().unwrap();
        assert!(s.focus_chart_at_time(1, time));
        assert_eq!(s.chart_focus_time().unwrap(), time);
        assert!(s.is_playing());
    }
    #[test]
    fn retained_resume_logical_clock_requires_owner_generation_and_asset_epoch() {
        let (_root, mut s, owner) = state();
        s.toggle_audio();
        s.take_audio_commands();
        update(&mut s, owner, 1.25);
        s.toggle_audio();
        s.take_audio_commands();
        s.toggle_audio();
        s.take_audio_commands();
        s.bind_decoder_generation(Some(41));
        s.bind_decoder_asset_epoch(Some(owner), Some(53));
        s.set_decoder_status(
            41,
            Some(Ack {
                generation: 41,
                phase: super::super::decoder_transport::Phase::Pending,
                opened: false,
            }),
        );
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = s.audio_output_snapshot();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 41,
            asset_epoch: 52,
            music_nanos: 2_000_000_000,
        });
        s.update_audio(snapshot);
        assert_eq!(s.transport_music_time(), Some(1.25));
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 41,
            asset_epoch: 53,
            music_nanos: 2_000_000_000,
        });
        s.update_audio(snapshot);
        assert_eq!(
            s.transport_music_time(),
            Some(2. - s.audio_preferences.effective().offset)
        );
        assert!(s.is_playing());
        assert_eq!(
            s.chart_autoplay_transport(),
            Some((Some(41), 2. - s.audio_preferences.effective().offset))
        );
    }
    #[test]
    fn paused_cursor_navigation_resumes_retained_asset_at_new_exact_focus() {
        let (root, mut s, owner) = state();
        s.toggle_audio();
        s.take_audio_commands();
        update(&mut s, owner, 1.25);
        s.toggle_audio();
        s.take_audio_commands();
        std::fs::remove_file(root.path().join("music.wav")).unwrap();
        s.navigate_exact_position(120.5, false);
        s.take_audio_commands();
        let expected = s.chart_focus_time().unwrap() + s.audio_preferences.effective().offset;
        s.toggle_audio();
        assert!(s.is_playing());
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorResumeMusic{seconds,..}]if (*seconds-expected).abs()<1e-9)
        );
    }
    #[test]
    fn no_focused_chart_restarts_retained_music_at_source_neutral_position() {
        let (_root, mut s, owner) = state();
        s.toggle_audio();
        s.take_audio_commands();
        update(&mut s, owner, 1.25);
        s.chart_views_apply(super::super::chart_views::ViewAction::Close);
        s.take_audio_commands();
        s.toggle_audio();
        s.take_audio_commands();
        assert!(!s.is_playing());
        s.toggle_audio();
        assert!(s.is_playing());
        assert!(s.playback_position().is_none());
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorResumeMusic{seconds,..}]if (*seconds-s.audio_preferences.effective().offset).abs()<1e-9)
        );
    }
    #[test]
    fn paused_focus_rebinds_stamp_and_resumes_exact_instant() {
        let (_root, mut s, owner) = state();
        s.toggle_audio();
        s.take_audio_commands();
        update(&mut s, owner, 1.25);
        s.toggle_audio();
        s.take_audio_commands();
        assert!(s.focus_chart_at_time(1, 1.25));
        assert!(!s.is_playing());
        assert_eq!(s.paused_position().unwrap().music_time, 1.25);
        let expected_start = 1.25 + s.audio_preferences.effective().offset;
        s.toggle_audio();
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorResumeMusic{seconds,..}] if (*seconds-expected_start).abs()<1e-9)
        );
    }
}

#[cfg(test)]
mod chart_asset_swap_tests {
    use super::*;
    #[test]
    fn changed_missing_asset_keeps_logical_clock_scope_and_timeline_generation() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.wav"), b"commandfixture").unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:a.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:missing.wav;#BPMS:0=60;#NOTES:1000;").unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let owner = s
            .take_audio_commands()
            .iter()
            .find_map(|c| {
                if let GameplayAudioCommand::EditorBeginAudio { owner, .. } = c {
                    Some(*owner)
                } else {
                    None
                }
            })
            .unwrap();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        s.update_audio(snapshot);
        s.take_audio_commands();
        s.toggle_audio();
        s.take_audio_commands();
        s.bind_decoder_generation(Some(91));
        s.set_decoder_status(
            91,
            Some(Ack {
                generation: 91,
                phase: super::super::decoder_transport::Phase::Producing,
                opened: true,
            }),
        );
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.stream_clock.has_music_mapping = true;
        snapshot.stream_clock.music_nanos = 1_250_000_000;
        snapshot.stream_clock.music_seconds_per_second = 1.;
        snapshot.stream_clock.stream_seconds = 10.;
        s.update_audio(snapshot);
        assert!(s.focus_chart_at_time(1, 1.25));
        assert!(s.chart_transport_is_playing());
        assert_eq!(s.chart_audio_generation(), Some(91));
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorSwapMusicAsset{owner:actual,path},GameplayAudioCommand::EditorRegisterAsset{owner:registered,kind:deadsync_gameplay::EditorAssetKind::Main,path:Some(asset)}] if *actual==owner && *registered==owner && path.ends_with("missing.wav") && asset==path)
        );
        // Current load failure is silent logical continuation, not a false
        // decoder success claim or a Stop. Only the acknowledged DSP advances.
        s.set_decoder_status(
            91,
            Some(Ack {
                generation: 91,
                phase: super::super::decoder_transport::Phase::Failed,
                opened: false,
            }),
        );
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.stream_clock.has_music_mapping = false;
        snapshot.stream_clock.stream_seconds = 10.5;
        s.update_audio(snapshot);
        assert!(s.chart_transport_is_playing());
        assert!((s.chart_focus_time().unwrap() - 1.75).abs() < 1e-9);
        assert!(s.take_audio_commands().is_empty());
        assert!(s.audio_output_snapshot().leased);
        assert_eq!(s.chart_autoplay_transport(), Some((Some(91), 1.75)));
    }
}

impl State {
    pub(super) fn close_focused_audio_view(&mut self) {
        if self.workspace.has_chart() {
            self.unfocused_music = Preview::rebind_chart_asset(
                &self.workspace.file().session,
                self.workspace.chart(),
                self.workspace.file().resource_base(),
            )
            .ok()
            .and_then(|preview| preview.music_asset().map(std::path::Path::to_path_buf));
        }
        if let Some(playing) = &mut self.playback {
            if playing.preview.is_some() {
                playing.unfocused = true;
            }
        }
        if let Some(paused) = &mut self.paused_playback {
            paused.chart = usize::MAX;
        }
        self.audio_commands
            .push(GameplayAudioCommand::EditorRetargetTicks {
                owner: self.audio_session.scope.owner,
            });
    }
}

#[cfg(test)]
mod initial_source_music_tests {
    use super::*;
    fn state(music: &str) -> (tempfile::TempDir, State, u64) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,format!("#VERSION:0.83;#MUSIC:{music};#BPMS:0=120;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;")).unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let owner = s
            .take_audio_commands()
            .iter()
            .find_map(|command| {
                if let GameplayAudioCommand::EditorBeginAudio { owner, .. } = command {
                    Some(*owner)
                } else {
                    None
                }
            })
            .unwrap();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        s.update_audio(snapshot);
        s.take_audio_commands();
        (root, s, owner)
    }
    fn logical(s: &mut State, owner: u64, seconds: f64) {
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = s.audio_output_snapshot();
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 71,
            asset_epoch: 83,
            music_nanos: (seconds * 1e9) as i64,
        });
        s.update_audio(snapshot);
    }
    #[test]
    fn missing_initial_music_uses_source_phase_while_real_failed_ack_stays_failed() {
        let (_root, mut s, owner) = state("missing.wav");
        s.toggle_audio();
        assert!(s.chart_transport_is_playing());
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorStartMusic{owner:actual,path:Some(path),seconds,..}]if *actual==owner&&path.ends_with("missing.wav")&&*seconds==s.audio_preferences.effective().offset)
        );
        s.bind_decoder_generation(Some(71));
        s.bind_decoder_asset_epoch(Some(owner), Some(83));
        s.set_decoder_status(
            71,
            Some(Ack {
                generation: 71,
                phase: super::super::decoder_transport::Phase::Pending,
                opened: false,
            }),
        );
        logical(&mut s, owner, 0.25);
        assert!(s.is_playing());
        assert_eq!(
            s.transport_music_time(),
            Some(0.25 - s.audio_preferences.effective().offset)
        );
        assert!(!s.playback.as_ref().unwrap().decoder_ack.unwrap().opened);
        assert_eq!(
            s.chart_autoplay_transport(),
            Some((Some(71), 0.25 - s.audio_preferences.effective().offset))
        );
        s.set_decoder_status(
            71,
            Some(Ack {
                generation: 71,
                phase: super::super::decoder_transport::Phase::Failed,
                opened: false,
            }),
        );
        logical(&mut s, owner, 0.5);
        assert!(s.is_playing());
        assert_eq!(
            s.transport_music_time(),
            Some(0.5 - s.audio_preferences.effective().offset)
        );
        assert_eq!(
            s.playback.as_ref().unwrap().decoder_ack.unwrap().phase,
            super::super::decoder_transport::Phase::Failed
        );
        assert!(!s.playback.as_ref().unwrap().decoder_ack.unwrap().opened);
        assert_eq!(s.status, i18n::tr("Edition", "PlaybackDecoderFailed"));
        assert!(!s.take_audio_commands().iter().any(|command| matches!(
            command,
            GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorStopMusic { .. }
        )));
    }
    #[test]
    fn empty_music_selection_is_explicit_silence_and_no_view_start_uses_zero() {
        let (_root, mut s, owner) = state("");
        s.toggle_audio();
        assert!(matches!(
            s.take_audio_commands().as_slice(),
            [GameplayAudioCommand::EditorStartMusic { path: None, .. }]
        ));
        s.bind_decoder_generation(Some(71));
        s.bind_decoder_asset_epoch(Some(owner), Some(83));
        logical(&mut s, owner, 0.25);
        assert_eq!(
            s.transport_music_time(),
            Some(0.25 - s.audio_preferences.effective().offset)
        );
        assert!(s.playback.as_ref().unwrap().decoder_ack.is_none());
        s.stop_audio();
        s.take_audio_commands();
        s.close_focused_audio_view();
        s.workspace.clear_active_chart();
        s.take_audio_commands();
        s.toggle_audio();
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorStartMusic{path:None,seconds,..}]if *seconds==s.audio_preferences.effective().offset)
        );
        assert!(s.playback_position().is_none());
    }
    #[test]
    fn focus_from_failed_asset_to_empty_music_clears_only_asset_and_keeps_clock() {
        let (root, _, _) = state("");
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:missing.wav;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let owner = s
            .take_audio_commands()
            .iter()
            .find_map(|c| {
                if let GameplayAudioCommand::EditorBeginAudio { owner, .. } = c {
                    Some(*owner)
                } else {
                    None
                }
            })
            .unwrap();
        let mut ack = GameplayAudioSnapshot::default();
        ack.editor.owner = owner;
        ack.editor.output.available = true;
        ack.editor.output.leased = true;
        ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        ack.editor.output.requested_period = Some(512);
        ack.editor.output.negotiated_period = Some(512);
        s.update_audio(ack);
        s.take_audio_commands();
        s.toggle_audio();
        s.take_audio_commands();
        s.bind_decoder_generation(Some(71));
        s.bind_decoder_asset_epoch(Some(owner), Some(83));
        logical(&mut s, owner, 0.25);
        s.take_audio_commands();
        let time = s.chart_focus_time().unwrap();
        assert!(s.focus_chart_at_time(1, time));
        assert!(s.is_playing());
        assert_eq!(s.chart_audio_generation(), Some(71));
        assert!(
            matches!(s.take_audio_commands().as_slice(),[GameplayAudioCommand::EditorClearMusicAsset{owner:actual},GameplayAudioCommand::EditorRegisterAsset{owner:registered,kind:deadsync_gameplay::EditorAssetKind::Main,path:None}]if *actual==owner && *registered==owner)
        );
    }
    #[test]
    fn silent_source_phase_schedules_ticks_without_claiming_decoded_pcm() {
        let (_root, mut s, owner) = state("");
        s.audio_preferences
            .set_field(
                deadsync_editor::audio_preferences::Field::UseAssistTick,
                deadsync_editor::audio_preferences::FieldValue::Bool(true),
            )
            .unwrap();
        s.toggle_audio();
        s.take_audio_commands();
        s.bind_decoder_generation(Some(71));
        s.bind_decoder_asset_epoch(Some(owner), Some(83));
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output = s.audio_output_snapshot();
        snapshot.editor.sample_rate = 48_000;
        snapshot.editor.assist_duration = Some(0.05);
        snapshot.editor.tick_generation = 5;
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 71,
            asset_epoch: 83,
            music_nanos: 0,
        });
        assert!(!snapshot.stream_clock.has_music_mapping);
        s.update_audio(snapshot);
        assert!(s.take_audio_commands().iter().any(|command|matches!(command,GameplayAudioCommand::EditorTickWindow{owner:actual,music_generation:71,..}if *actual==owner)));
        assert!(s.playback.as_ref().unwrap().decoder_ack.is_none());
    }
    #[test]
    fn source_phase_without_output_lease_cannot_claim_dsp_progress_or_ticks() {
        let (_root, mut s, owner) = state("missing.wav");
        s.toggle_audio();
        s.take_audio_commands();
        s.bind_decoder_generation(Some(71));
        s.bind_decoder_asset_epoch(Some(owner), Some(83));
        let before = s.transport_music_time();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.stream_clock.valid_at = Instant::now();
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 71,
            asset_epoch: 83,
            music_nanos: 2_000_000_000,
        });
        s.update_audio(snapshot);
        assert_eq!(s.transport_music_time(), before);
        assert!(
            s.take_audio_commands()
                .iter()
                .all(|command| !matches!(command, GameplayAudioCommand::EditorTickWindow { .. }))
        );
    }
}
#[cfg(test)]
mod initial_preview_transition_tests {
    use super::*;
    #[test]
    fn play_during_sample_stops_audition_then_starts_source_music_in_one_action() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(root.path().join("music.wav"), b"command fixture").unwrap();
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let owner = s
            .take_audio_commands()
            .iter()
            .find_map(|c| {
                if let GameplayAudioCommand::EditorBeginAudio { owner, .. } = c {
                    Some(*owner)
                } else {
                    None
                }
            })
            .unwrap();
        let mut ack = GameplayAudioSnapshot::default();
        ack.editor.owner = owner;
        ack.editor.output.available = true;
        ack.editor.output.leased = true;
        ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        ack.editor.output.requested_period = Some(512);
        ack.editor.output.negotiated_period = Some(512);
        s.update_audio(ack);
        s.take_audio_commands();
        s.observe_editor_assets(deadsync_gameplay::EditorAssetReadiness {
            owner,
            registration: s.editor_asset_registration(),
            main: true,
            dedicated_preview: true,
        });
        s.toggle_sample_audio_result(Ok(deadsync_editor::song_properties::SampleAudio {
            path: root.path().join("music.wav").canonicalize().unwrap(),
            start: 0.,
            length: Some(1.),
        }));
        s.take_audio_commands();
        assert!(s.is_playing());
        assert!(!s.chart_transport_is_playing());
        s.toggle_audio();
        assert!(s.chart_transport_is_playing());
        let commands = s.take_audio_commands();
        let stop = commands
            .iter()
            .position(|command| matches!(command, GameplayAudioCommand::EditorStopMusic { .. }))
            .unwrap();
        let start = commands
            .iter()
            .position(|command| matches!(command, GameplayAudioCommand::EditorStartMusic { .. }))
            .unwrap();
        assert!(stop < start);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, GameplayAudioCommand::EditorPauseMusic { .. }))
        );
    }
}

#[cfg(test)]
mod source_audio_offset_tests {
    use super::*;
    #[test]
    fn offset_apply_undo_redo_preserve_fractional_visual_time_and_retarget_audio() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:missing.wav;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.take_audio_commands();
        state.navigate_exact_time(1.234567, false).unwrap();
        state.toggle_audio();
        state.take_audio_commands();
        state.bind_decoder_generation(Some(41));
        let owner = state.editor_asset_owner();
        state.bind_decoder_asset_epoch(Some(owner), Some(53));
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
            generation: 41,
            asset_epoch: 53,
            music_nanos: 1_234_567_000,
        });
        state.update_audio(snapshot);
        state.take_audio_commands();
        let before = state.transport_music_time().unwrap();
        let row = state.playback_position().unwrap().1;
        let bytes = state.workspace.file().session.serialize().unwrap();
        let mut audio = state.audio_preferences;
        audio
            .set_field(
                deadsync_editor::audio_preferences::Field::AudioOffset,
                deadsync_editor::audio_preferences::FieldValue::Float(0.25),
            )
            .unwrap();
        assert!(state.commit_audio_preferences(audio).unwrap());
        for (expected, command) in [
            (before + 0.25, None),
            (before, Some(Command::Undo)),
            (before + 0.25, Some(Command::Redo)),
        ] {
            if let Some(command) = command {
                state.command(command);
            }
            assert!(state.is_playing());
            assert_eq!(state.transport_music_time(), Some(before));
            assert_eq!(state.playback_position().unwrap().1, row);
            let commands = state.take_audio_commands();
            assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSeekMusic{owner:o,seconds}if *o==owner&&(*seconds-expected).abs()<1e-12)));
            assert!(!commands.iter().any(|c| matches!(
                c,
                GameplayAudioCommand::EditorStopMusic { .. }
                    | GameplayAudioCommand::EditorPauseMusic { .. }
            )));
            assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        }
    }
}
#[cfg(test)]
mod final_chart_tick_tail_tests {
    use super::*;
    #[test]
    fn closing_final_chart_retargets_only_future_ticks_while_main_continues() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let owner = state.editor_asset_owner();
        state.take_audio_commands();
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        state.update_audio(snapshot);
        state.take_audio_commands();
        state.toggle_audio();
        state.take_audio_commands();
        assert!(state.chart_transport_is_playing());
        let bytes = state.workspace.file().session.serialize().unwrap();
        state.close_focused_audio_view();
        state.workspace.clear_active_chart();
        let commands = state.take_audio_commands();
        assert!(state.chart_transport_is_playing());
        assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorRetargetTicks{owner:actual}if *actual==owner)));
        assert!(!commands.iter().any(|command| matches!(
            command,
            GameplayAudioCommand::EditorInvalidateTicks { .. }
                | GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
        )));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
    }
}

impl State {
    /// A virtual generated chart navigates in its source clock while the real
    /// focused chart and open decoder stay bound to the same musical instant.
    pub(super) fn navigate_generation_preview_focus(
        &mut self,
        target: deadsync_editor::chart_focus::Position,
    ) -> Result<(), deadsync_editor::Error> {
        let focus = deadsync_editor::chart_focus::Position::at_time(
            &self.workspace.file().session,
            self.workspace.chart(),
            target.music_time(),
        )?;
        let continuing =
            self.chart_transport_is_playing() && !self.scroll_preferences.stop_playback();
        if self.chart_transport_is_playing() && !continuing {
            self.stop_audio();
        }
        self.wheel.cancel();
        self.workspace.seek_preserving_selection(focus.edit_row())?;
        self.retarget_paused_scroll_position(&focus);
        if continuing {
            let playing = self.playback.as_mut().expect("continued main transport");
            playing.valid_after = Instant::now();
            playing.mapped = false;
            playing.position = Some(Position {
                music_time: focus.music_time(),
                chart_time: focus.chart_time(),
                row: focus.row(),
            });
            if let Some(clock) = playing.visual_clock.as_mut() {
                clock.seek(focus.music_time(), playing.valid_after);
            }
            self.audio_commands.push(GameplayAudioCommand::SeekMusic(
                focus.music_time() + self.audio_preferences.effective().offset,
            ));
        }
        self.focus_position = Some(focus);
        self.receptor_views.set_preview_navigation(target);
        self.receptor_views.reset();
        self.refresh();
        Ok(())
    }
}

/// Captured before one validated structural gesture, never while opening a dialog.
pub(super) struct StructuralChartPresentation {
    time: f64,
    paused: bool,
    music: Option<std::path::PathBuf>,
}
impl State {
    pub(super) fn capture_structural_chart_presentation(
        &self,
    ) -> Result<StructuralChartPresentation, deadsync_editor::Error> {
        let time = if self.workspace.has_chart() {
            self.chart_focus_music_time()?
        } else {
            self.chart_focus_time()?
        };
        let music = if self.workspace.has_chart() {
            Preview::rebind_chart_asset(
                &self.workspace.file().session,
                self.workspace.chart(),
                self.workspace.file().resource_base(),
            )?
            .music_asset()
            .map(std::path::Path::to_path_buf)
        } else {
            self.unfocused_music.clone()
        };
        Ok(StructuralChartPresentation {
            time,
            paused: self.paused_playback.is_some(),
            music,
        })
    }
    pub(super) fn finish_structural_chart_presentation(
        &mut self,
        before: StructuralChartPresentation,
    ) -> Result<(), deadsync_editor::Error> {
        if self.workspace.has_chart() {
            let chart = self.workspace.chart();
            let position = deadsync_editor::chart_focus::Position::at_time(
                &self.workspace.file().session,
                chart,
                before.time,
            )?;
            let audio = self.prepare_chart_focus_audio(chart);
            self.workspace
                .seek_preserving_selection(position.edit_row())?;
            self.clear_unfocused_chart_selection();
            self.apply_chart_focus_audio(audio, before.paused, &position);
            self.focus_position = Some(position);
        } else {
            // Source CloseChart's final view clears focus without OnMusicChanged.
            // Keep the loaded override rather than falling back to Song.MUSIC.
            self.unfocused_music = before.music;
            self.close_focused_audio_view();
            if let Some(paused) = self.paused_playback.as_mut() {
                paused.source = Arc::clone(self.workspace.file().session.document().source());
                paused.revision = self.workspace.file().session.content_revision();
                paused.chart = usize::MAX;
                paused.edit_row = self.workspace.row();
            }
            self.clear_unfocused_chart_selection();
            self.focus_position = None;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "playback/chart_structure_tests.rs"]
mod chart_structure_tests;

/// Migration keeps the editor clock, including a chartless paused music source.
pub(super) struct MigrationPresentation {
    focused: Option<SongPropertyPresentation>,
    token: deadsync_editor::timing_edit::migration::PresentationOrigin,
    source: Arc<[u8]>,
    revision: u64,
}
impl State {
    pub(super) fn capture_migration_presentation(
        &self,
    ) -> Result<MigrationPresentation, deadsync_editor::Error> {
        let session = &self.workspace.file().session;
        Ok(MigrationPresentation {
            focused: if self.workspace.has_chart() {
                Some(self.capture_song_property_presentation()?)
            } else {
                None
            },
            token: deadsync_editor::timing_edit::migration::PresentationOrigin::capture(session),
            source: Arc::clone(session.document().source()),
            revision: session.content_revision(),
        })
    }
    pub(super) fn rebind_migration_presentation(
        &mut self,
        before: MigrationPresentation,
        receipt: &deadsync_editor::song_properties::PresentationReceipt,
    ) -> Result<(), deadsync_editor::Error> {
        receipt.validate_transition(&before.token, &self.workspace.file().session)?;
        if let Some(focused) = before.focused {
            return self.rebind_song_property_presentation(focused, receipt);
        }
        if self.workspace.has_chart() || !receipt.preserves_audio() {
            return Err(deadsync_editor::Error::InvalidEdit(
                "Migration changed the unfocused transport context".into(),
            ));
        }
        let session = &self.workspace.file().session;
        if let Some(paused) = self.paused_playback.as_mut().filter(|paused| {
            paused.chart == usize::MAX
                && paused.revision == before.revision
                && Arc::ptr_eq(&paused.source, &before.source)
        }) {
            paused.source = Arc::clone(session.document().source());
            paused.revision = session.content_revision();
        }
        Ok(())
    }
}

impl State {
    pub(super) fn save_as_transport_stamp(&self) -> (u64, Option<Instant>, bool, bool) {
        let origin = self
            .playback
            .as_ref()
            .and_then(|playing| playing.visual_clock.as_ref())
            .map(|clock| clock.intent_origin())
            .or_else(|| {
                self.paused_playback
                    .as_ref()
                    .and_then(|paused| paused.intent_origin)
            });
        (
            self.audio_session.scope.owner,
            origin,
            self.chart_transport_is_playing(),
            self.paused_playback.is_some(),
        )
    }
    /// Called only after a reviewed publication and validated focus decision.
    /// No fake decoder mapping is published.
    pub(super) fn rebind_chartless_current_audio(&mut self) {
        let target = self.source_editor_asset_paths()[0].clone();
        let old = self
            .playback
            .as_ref()
            .and_then(|playing| playing.preview.as_ref())
            .or_else(|| {
                self.paused_playback
                    .as_ref()
                    .and_then(|paused| paused.preview.as_ref())
            });
        let same = match (old, target.as_deref()) {
            (Some(preview), Some(path)) => preview.has_music_path(path),
            (Some(preview), None) => preview.music_asset().is_none(),
            (None, Some(path)) => self
                .unfocused_music
                .as_deref()
                .is_some_and(|old| Preview::paths_equal(old, path)),
            (None, None) => self.unfocused_music.is_none(),
        };
        if !same {
            self.audio_commands.push(match &target {
                Some(path) => GameplayAudioCommand::EditorSwapMusicAsset {
                    owner: self.audio_session.scope.owner,
                    path: path.clone(),
                },
                None => GameplayAudioCommand::EditorClearMusicAsset {
                    owner: self.audio_session.scope.owner,
                },
            });
        }
        self.unfocused_music = target;
        if let Some(playing) = self.playback.as_mut() {
            playing.preview = None;
            playing.unfocused = true;
            if !same {
                playing.asset_replaced = true;
                playing.mapped_generation = None;
            }
        }
        if let Some(paused) = self.paused_playback.as_mut() {
            paused.preview = None;
            paused.source = Arc::clone(self.workspace.file().session.document().source());
            paused.revision = self.workspace.file().session.content_revision();
            paused.chart = usize::MAX;
            paused.edit_row = self.workspace.row();
            paused.asset_replaced |= !same;
        }
    }
}
#[cfg(test)]
#[path = "playback/save_as_transport_tests.rs"]
mod save_as_transport_tests;
#[cfg(test)]
#[path = "playback/song_properties_live_tests.rs"]
pub(super) mod song_properties_live_tests;

#[cfg(test)]
#[path = "playback/song_metadata_history_tests.rs"]
mod song_metadata_history_tests;

#[cfg(test)]
mod held_axis_consumer_tests;
