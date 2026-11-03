//! Host-only playback position when audio is unavailable. This is a UI timer,
//! never an opened decoder, musical PCM mapping, device lease or DSP reading.
use super::*;
use deadsync_editor::source_visual_clock::SourceVisualClock;
impl State {
    /// Called by the real host adapter before decoder synchronization. Unknown
    /// legacy snapshots never opt into this independent host-clock contract.
    pub fn observe_audio_backend_available(&mut self, available: bool) {
        self.observe_audio_backend_available_at(available, Instant::now());
    }
    pub(in crate::screens::edition) fn observe_audio_backend_available_at(&mut self,available:bool,now:Instant){
        let previous = self.audio_backend_available.replace(available);
        if !available {
            if previous!=Some(false){
                if let Some(playing)=self.playback.as_mut().filter(|playing|playing.sample_descriptor.is_some()){
                    if playing.sample_cursor.is_none(){playing.pending_sample=playing.sample_descriptor.clone();}
                    if let Some(cursor)=playing.sample_cursor{self.audio_commands.push(GameplayAudioCommand::EditorSuspendPreview{owner:self.audio_session.scope.owner,cursor});}
                    self.status=i18n::tr("Edition","AudioDspUnavailable");
                }
            }
            if let Some(playing) = self
                .playback
                .as_mut()
                .filter(|playing| playing.preview.is_some() || playing.unfocused)
            {
                if !playing.visual_active {
                    let time = playing
                        .position
                        .map(|position| position.music_time)
                        .or_else(|| {
                            playing
                                .preview
                                .as_ref()
                                .and_then(|preview| preview.music_time(self.workspace.row()).ok())
                        })
                        .unwrap_or(0.);
                    playing.visual_clock = SourceVisualClock::start(
                        time,
                        self.audio_preferences.effective().rate,
                        now,
                    );
                    playing.visual_active = true;
                }
                playing.asset_epoch = None;
                playing.decoder = None;
                playing.decoder_ack = None;
                playing.clock_generation = None;
                playing.issued_generation = None;
                playing.precise_stream = None;
                playing.mapped = false;
                playing.mapped_generation = None;
                playing.visual_reference = None;
            }
        } else if previous == Some(false) {
            self.update_source_visual_clock(now);
            self.restart_source_audio_after_device_return();
            // No device consumed any frames of this initial sample request.
            // Keep preview logical state; replay the exact typed descriptor only
            // after the output scope acquires its real lease.
            let registration=self.editor_asset_registration();
            if let Some(playing)=self.playback.as_mut().filter(|playing|playing.sample_registration==registration){
                if let (Some(GameplayAudioCommand::EditorPlayPreview{owner,kind,path,cut,looping,rate}),Some(cursor))=(playing.sample_descriptor.clone(),playing.sample_cursor){
                    playing.pending_sample=None;self.audio_commands.push(GameplayAudioCommand::EditorResumePreview{owner,kind,path,cut,looping,rate,cursor});
                }else if let Some(request)=playing.pending_sample.take(){self.audio_commands.push(request);}
            }
            if let Some(playing) = self
                .playback
                .as_ref()
                .filter(|playing| playing.preview.is_some() || playing.unfocused)
            {
                let time = playing.position.map_or(0., |position| position.music_time);
                let path = playing
                    .preview
                    .as_ref()
                    .and_then(|preview| preview.music_asset().map(std::path::Path::to_path_buf))
                    .or_else(|| self.unfocused_music.clone());
                self.audio_commands
                    .push(GameplayAudioCommand::EditorStartMusic {
                        owner: self.audio_session.scope.owner,
                        path,
                        seconds: time + self.audio_preferences.effective().offset,
                        rate: self.audio_preferences.effective().rate,
                    });
            }
        }
    }
pub fn editor_preview_request(
    &self,
) -> Option<(deadsync_gameplay::EditorAssetKind, std::path::PathBuf)> {
    match self.playback.as_ref()?.sample_descriptor.as_ref()? {
        GameplayAudioCommand::EditorPlayPreview { kind, path, .. } => Some((*kind, path.clone())),
        _ => None,
    }
}
/// Freeze during output loss. Only actual callback provenance can replace
/// this cursor; elapsed host time and decoded-ahead status cannot do so.
pub fn observe_rendered_preview_cursor(
    &mut self,
    cursor: Option<deadsync_gameplay::EditorPreviewCursor>,
) {
    if self.audio_backend_available == Some(false) && cursor.is_none_or(|cursor| !cursor.suspended)
    {
        return;
    }
    let owner = self.editor_asset_owner();
    let registration = self.editor_asset_registration();
    let output_ready =
        self.audio_backend_available == Some(true) && self.source_visual_audio_lease_ready();
    if let Some(playing) = self
        .playback
        .as_mut()
        .filter(|playing| playing.sample_registration == registration)
    {
        if let (Some(GameplayAudioCommand::EditorPlayPreview { kind, .. }), Some(cursor)) =
            (playing.sample_descriptor.as_ref(), cursor)
        {
            if cursor.owner == owner
                && cursor.kind == *kind
                && cursor.next_music_seconds.is_finite()
            {
                playing.sample_cursor = Some(cursor);
                if output_ready && !cursor.suspended && cursor.rendered {
                    self.status = i18n::tr("Edition", "SongSamplePlaying");
                }
            }
        }
    }
}

    pub(in crate::screens::edition) fn has_pending_source_visual_audio_start(&self)->bool{
        self.audio_backend_available==Some(true)&&self.playback.as_ref().is_some_and(|playing|playing.visual_active&&playing.visual_reference.is_none()&&(playing.preview.is_some()||playing.unfocused))
    }
    pub(in crate::screens::edition) fn source_visual_audio_lease_ready(&self)->bool{
        let output=self.audio_output_snapshot();self.audio_output_transport_ready()&&output.available&&output.leased&&self.observed_audio_owner==Some(self.audio_session.scope.owner)
    }
    /// Before the first real DSP phase after device recovery, the host clock
    /// and current UI descriptor are authoritative. Async lease setup can leave
    /// old Start/Seek/Rate/Swap commands queued for an arbitrary duration.
    pub(in crate::screens::edition) fn coalesce_source_visual_audio_start(
        &mut self,
        now: Instant,
        pending: &mut Vec<GameplayAudioCommand>,
    ) {
        let needs_start = self.has_pending_source_visual_audio_start();
        let owner = self.audio_session.scope.owner;
        let main_intent = |command: &GameplayAudioCommand| match command {
            GameplayAudioCommand::SeekMusic(_) | GameplayAudioCommand::SetMusicRate(_) => true,
            GameplayAudioCommand::EditorStartMusic { owner: actual, .. }
            | GameplayAudioCommand::EditorResumeMusic { owner: actual, .. }
            | GameplayAudioCommand::EditorSwapMusicAsset { owner: actual, .. }
            | GameplayAudioCommand::EditorClearMusicAsset { owner: actual }
            | GameplayAudioCommand::EditorSeekMusic { owner: actual, .. }
            | GameplayAudioCommand::EditorSetMusicRate { owner: actual, .. } => *actual == owner,
            _ => false,
        };
        if !needs_start || !pending.iter().any(main_intent) {
            return;
        }
        self.update_source_visual_clock(now);
        let Some(playing) = self.playback.as_ref() else { return; };
        let Some(position) = playing.position else { return; };
        let path = playing.preview.as_ref()
            .and_then(|preview| preview.music_asset().map(std::path::Path::to_path_buf))
            .or_else(|| self.unfocused_music.clone());
        pending.retain(|command| !main_intent(command));
        pending.push(GameplayAudioCommand::EditorStartMusic {
            owner,
            path,
            seconds: position.music_time + self.audio_preferences.effective().offset,
            rate: self.audio_preferences.effective().rate,
        });
    }

    pub(in crate::screens::edition) fn update_source_visual_clock(&mut self, now: Instant) {
        let focused = self.workspace.has_chart();
        let Some(playing) = self.playback.as_mut().filter(|playing| {
            playing.visual_active && (playing.preview.is_some() || playing.unfocused)
        }) else {
            return;
        };
        let Some(clock) = playing.visual_clock.as_mut() else {
            return;
        };
        let Some(reading) = clock.advance(now, focused, None) else {
            return;
        };
        let result = if focused {
            playing.preview.as_ref().and_then(|preview| {
                Some((
                    preview.music_to_chart_time(reading.music_time).ok()?,
                    preview.position(reading.music_time).ok()?,
                ))
            })
        } else {
            Some((0., 0.))
        };
        let Some((chart_time, row)) = result else {
            return;
        };
        playing.position = Some(Position {
            chart_time,
            row,
            music_time: reading.music_time,
        });
        if focused {
            let row = row.max(0.).min(i32::MAX as f64).floor() as u64;
            if row != self.workspace.row() {
                let _ = self.workspace.seek_preserving_selection(row);
                self.refresh();
            }
        }
    }
    pub(in crate::screens::edition) fn host_visual_transport(&self) -> Option<f64> {
        let playing = self
            .playback
            .as_ref()
            .filter(|playing| playing.visual_active && playing.visual_clock.is_some())?;
        playing
            .position
            .filter(|position| {
                position.music_time.is_finite()
                    && position.chart_time.is_finite()
                    && position.row.is_finite()
            })
            .map(|position| position.music_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn state() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:missing.wav;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.take_audio_commands();
        state.observe_audio_backend_available(false);
        state.update_audio(GameplayAudioSnapshot::default());
        state.take_audio_commands();
        (root, state)
    }
    fn clock(state: &mut State, now: Instant, music: f64) {
        let playing = state.playback.as_mut().unwrap();
        playing.visual_clock =
            SourceVisualClock::start(music, state.audio_preferences.effective().rate, now);
        playing.visual_active = true;
        playing.position = Some(Position {
            music_time: music,
            chart_time: music,
            row: music * 96.,
        });
    }
    #[test]
    fn main_start_and_live_rate_keep_original_double_precision_before_backend() {
        let (_root, mut state) = state();
        let initial = 1.23456789012345f64;
        assert_ne!(initial, f64::from(initial as f32));
        state.audio_preferences.set_music_rate(initial).unwrap();
        state.toggle_audio();
        let start = state.audio_commands.iter().find_map(|command| match command {
            GameplayAudioCommand::EditorStartMusic { rate, .. } => Some(*rate),
            _ => None,
        }).expect("Main starts through the typed editor command");
        assert_eq!(start.to_bits(), initial.to_bits());
        let next = 1.34567890123456f64;
        state.audio_preferences.set_music_rate(next).unwrap();
        state.apply_playback_rate_at(Instant::now());
        let change = state.audio_commands.iter().rev().find_map(|command| match command {
            GameplayAudioCommand::EditorSetMusicRate { rate, .. } => Some(*rate),
            _ => None,
        }).expect("Main rate changes through the typed editor command");
        assert_eq!(change.to_bits(), next.to_bits());
        assert!(state.audio_commands.iter().all(|command| !matches!(command, GameplayAudioCommand::SetMusicRate(_))));
    }
    #[test]
    fn no_device_start_advances_modal_visual_position_without_decoder_or_tick_claim() {
        let (_root, mut state) = state();
        let before = state.workspace.file().session.serialize().unwrap();
        state.toggle_audio();
        assert!(state.is_playing());
        state.take_audio_commands();
        state.bind_decoder_generation(None);
        assert!(state.is_playing());
        let now = Instant::now();
        clock(&mut state, now, 0.);
        state.open_chart_properties();
        assert!(state.chart_properties.is_some());
        state.take_audio_commands();
        state.update_source_visual_clock(now + Duration::from_millis(500));
        assert_eq!(state.transport_music_time(), Some(0.5));
        assert_eq!(state.chart_autoplay_transport(), Some((None, 0.5)));
        assert_eq!(state.workspace.row(), 48);
        let playing = state.playback.as_ref().unwrap();
        assert!(playing.decoder.is_none());
        assert!(playing.decoder_ack.is_none());
        assert!(playing.asset_epoch.is_none());
        assert!(playing.issued_generation.is_none());
        assert!(!playing.mapped);
        assert!(
            state
                .take_audio_commands()
                .iter()
                .all(|c| !matches!(c, GameplayAudioCommand::EditorTickWindow { .. }))
        );
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(state.workspace.file().session.undo_label().is_none());
        assert!(!state.workspace.file().session.is_dirty());
    }
    #[test]
    fn host_only_pause_resume_rate_and_seek_keep_exact_source_positions() {
        let (_root, mut state) = state();
        state.toggle_audio();
        state.take_audio_commands();
        let now = Instant::now();
        clock(&mut state, now, 0.);
        state.update_source_visual_clock(now + Duration::from_millis(500));
        state.toggle_audio();
        assert!(!state.is_playing());
        state.take_audio_commands();
        state.update_source_visual_clock(now + Duration::from_secs(1));
        assert_eq!(state.paused_position().unwrap().music_time, 0.5);
        state.toggle_audio();
        state.take_audio_commands();
        clock(&mut state, now + Duration::from_secs(1), 0.5);
        state.audio_preferences.set_music_rate(2.).unwrap();
        state.apply_playback_rate_at(now + Duration::from_secs(1));
        state.take_audio_commands();
        state.update_source_visual_clock(now + Duration::from_millis(1250));
        assert_eq!(state.transport_music_time(), Some(1.));
        // Rebase to a past, controlled host stamp before exercising the real
        // keyboard/navigation consumer, whose command stamp is sampled internally.
        clock(&mut state, Instant::now() - Duration::from_secs(1), 1.);
        state.scroll_preferences.set_stop_playback(false);
        let before = Instant::now();
        state.navigate_exact_position(120.5, false);
        let after = Instant::now();
        let expected = deadsync_editor::chart_focus::Position::at_fractional_row(
            &state.workspace.file().session,
            state.workspace.chart(),
            120.5,
        )
        .unwrap()
        .music_time();
        state.update_source_visual_clock(after + Duration::from_millis(500));
        let actual = state.transport_music_time().unwrap();
        assert!(actual >= expected + 1.);
        assert!(actual <= expected + 2. * (after.duration_since(before).as_secs_f64() + 0.5));
        assert!(state.is_playing());
        assert!(state.playback.as_ref().unwrap().issued_generation.is_none());
    }
    #[test]
    fn no_device_final_close_and_reopen_keep_host_clock_without_projecting_closed_chart() {
        let (_root, mut state) = state();
        state.toggle_audio();
        state.take_audio_commands();
        let now = Instant::now();
        clock(&mut state, now, 0.);
        state.update_source_visual_clock(now + Duration::from_millis(500));
        state.chart_views_apply(super::super::super::chart_views::ViewAction::Close);
        state.take_audio_commands();
        assert!(!state.workspace.has_chart());
        assert!(state.is_playing());
        let row = state.workspace.row();
        state.update_source_visual_clock(now + Duration::from_secs(1));
        assert_eq!(state.transport_music_time(), Some(1.));
        assert!(state.playback_position().is_none());
        assert_eq!(state.workspace.row(), row);
        assert!(state.chart_autoplay_transport().is_none());
        assert!(state.take_audio_commands().is_empty());
        assert!(state.focus_chart_at_time(1, 1.));
        assert_eq!(state.chart_focus_time().unwrap(), 1.);
        assert!(state.is_playing());
    }
    #[test]
    fn returning_device_reacquires_owner_and_waits_for_real_lease_before_music_start() {
        let (_root, mut state) = state();
        state.toggle_audio();
        state.take_audio_commands();
        let now = Instant::now();
        clock(&mut state, now, 0.);
        state.update_source_visual_clock(now + Duration::from_millis(500));
        state.observe_audio_backend_available(true);
        let commands = state.take_audio_commands();
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, GameplayAudioCommand::EditorBeginAudio { .. }))
        );
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, GameplayAudioCommand::EditorStartMusic { .. }))
        );
        assert!(state.is_playing());
        state.bind_decoder_generation(None);
        assert!(state.is_playing());
        let owner = state.audio_session.scope.owner;
        let mut snapshot = GameplayAudioSnapshot::default();
        snapshot.editor.owner = owner;
        snapshot.editor.output.available = true;
        snapshot.editor.output.leased = true;
        snapshot.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        snapshot.editor.output.requested_period = Some(512);
        snapshot.editor.output.negotiated_period = Some(512);
        state.update_audio(snapshot);
        let commands = state.take_audio_commands();
        assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorStartMusic{owner:actual,seconds,..}if *actual==owner&&*seconds==0.5+state.audio_preferences.effective().offset)));
        assert!(state.playback.as_ref().unwrap().decoder_ack.is_none());
        assert!(state.playback.as_ref().unwrap().issued_generation.is_none());
    }
}

#[cfg(test)]
mod device_return_tests {
    use super::*;
    use std::time::Duration;
    fn state() -> (tempfile::TempDir,State) {
        crate::i18n::init_for_tests();
        let root=tempfile::tempdir().unwrap();
        let path=root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:main.wav;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:other.wav;#NOTES:1000;").unwrap();
        let mut state=State::open(&path,0).unwrap();state.take_audio_commands();state.observe_audio_backend_available(false);state.update_audio(GameplayAudioSnapshot::default());state.take_audio_commands();state.toggle_audio();state.take_audio_commands();(root,state)
    }
    fn clock(state:&mut State,now:Instant){let playing=state.playback.as_mut().unwrap();playing.visual_clock=SourceVisualClock::start(0.,1.,now);playing.position=Some(Position{music_time:0.,chart_time:0.,row:0.});}
    fn ready(state:&mut State){let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=state.audio_session.scope.owner;snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);state.update_audio(snapshot);}
    #[test]
    fn device_return_and_delayed_lease_use_current_host_time_and_latest_main_intent(){
        let(_root,mut state)=state();let now=Instant::now();clock(&mut state,now);state.update_source_visual_clock(now+Duration::from_millis(500));
        state.observe_audio_backend_available_at(true,now+Duration::from_secs(1));assert_eq!(state.transport_music_time(),Some(1.));let first=state.scoped_audio_commands_at(now+Duration::from_secs(1));assert!(first.iter().any(|command|matches!(command,GameplayAudioCommand::EditorBeginAudio{..})));assert!(!first.iter().any(|command|matches!(command,GameplayAudioCommand::EditorStartMusic{..})));
        state.audio_preferences.set_music_rate(2.).unwrap();state.apply_playback_rate_at(now+Duration::from_secs(1));assert!(state.focus_chart_at_time(1,1.));state.audio_commands.push(GameplayAudioCommand::SeekMusic(0.25));let owner=state.audio_session.scope.owner;state.audio_commands.push(GameplayAudioCommand::EditorUpdateAudio{owner,gains:deadsync_gameplay::EditorAudioGains{main:1.,music:0.8,assist:0.2,beat:0.4}});state.scoped_audio_commands_at(now+Duration::from_secs(1));
        ready(&mut state);state.audio_commands.push(GameplayAudioCommand::EditorUpdateAudio{owner,gains:deadsync_gameplay::EditorAudioGains{main:1.,music:0.7,assist:0.2,beat:0.4}});let commands=state.scoped_audio_commands_at(now+Duration::from_secs(2));assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorUpdateAudio{gains,..} if gains.music==0.7)));let starts:Vec<_>=commands.iter().filter_map(|command|if let GameplayAudioCommand::EditorStartMusic{owner,path,seconds,rate}=command{Some((owner,path,seconds,rate))}else{None}).collect();assert_eq!(starts.len(),1);assert_eq!(*starts[0].0,owner);assert!(starts[0].1.as_ref().unwrap().ends_with("other.wav"));assert_eq!(*starts[0].2,3.+state.audio_preferences.effective().offset);assert_eq!(*starts[0].3,2.);assert!(commands.iter().all(|command|!matches!(command,GameplayAudioCommand::EditorSeekMusic{..}|GameplayAudioCommand::EditorSetMusicRate{..}|GameplayAudioCommand::EditorSwapMusicAsset{..}|GameplayAudioCommand::EditorClearMusicAsset{..}|GameplayAudioCommand::SeekMusic(_)|GameplayAudioCommand::SetMusicRate(_))));state.update_source_visual_clock(now+Duration::from_millis(2500));assert_eq!(state.transport_music_time(),Some(4.));assert!(state.playback.as_ref().unwrap().decoder_ack.is_none());assert!(state.playback.as_ref().unwrap().issued_generation.is_none());
    }
    #[test]
    fn another_owner_lease_cannot_release_source_recovery_start(){
        let(_root,mut state)=state();let now=Instant::now();clock(&mut state,now);state.observe_audio_backend_available_at(true,now);state.scoped_audio_commands_at(now);
        ready(&mut state);state.observed_audio_owner=Some(state.audio_session.scope.owner+1);
        let commands=state.scoped_audio_commands_at(now+Duration::from_secs(1));
        assert!(!commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorStartMusic{..})));
        assert!(state.playback.as_ref().unwrap().decoder_ack.is_none());
    }
    #[test]
    fn pause_before_lease_ready_cancels_held_start_before_it_can_be_published(){
        let(_root,mut state)=state();let now=Instant::now();clock(&mut state,now);state.observe_audio_backend_available_at(true,now+Duration::from_secs(1));state.scoped_audio_commands_at(now+Duration::from_secs(1));state.toggle_audio();assert!(!state.is_playing());ready(&mut state);let commands=state.scoped_audio_commands_at(now+Duration::from_secs(2));assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorPauseMusic{..})));assert!(!commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorStartMusic{..}|GameplayAudioCommand::EditorResumeMusic{..})));
    }
}

#[cfg(test)]
mod no_device_sample_tests {
    use super::*;
    fn sample()->(tempfile::TempDir,State){
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");std::fs::write(&path,"#VERSION:0.83;#MUSIC:main.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();state.take_audio_commands();state.observe_audio_backend_available(false);state.update_audio(GameplayAudioSnapshot::default());state.take_audio_commands();
        let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=state.editor_asset_owner();snapshot.editor.assets.owner=state.editor_asset_owner();snapshot.editor.assets.registration=state.editor_asset_registration();snapshot.editor.assets.main=true;state.update_audio(snapshot);
        let sample=deadsync_editor::song_properties::SampleAudio{path:state.configured_preview_path(deadsync_gameplay::EditorAssetKind::Main).unwrap(),start:0.25,length:Some(0.5)};state.toggle_sample_audio_role_result(deadsync_gameplay::EditorAssetKind::Main,Ok(sample));(root,state)
    }
    #[test]fn no_device_sample_keeps_logical_preview_without_host_or_dsp_claim_and_can_stop(){let(_root,mut state)=sample();assert!(state.is_playing());assert_eq!(state.status,i18n::tr("Edition","AudioDspUnavailable"));let playing=state.playback.as_ref().unwrap();assert!(playing.visual_clock.is_none());assert!(playing.decoder_ack.is_none());assert!(playing.issued_generation.is_none());assert!(playing.pending_sample.is_some());state.stop_audio();assert!(!state.is_playing());state.observe_audio_backend_available(true);assert!(!state.take_audio_commands().iter().any(|command|matches!(command,GameplayAudioCommand::EditorPlayPreview{..})));}
    #[test]fn no_device_sample_recovery_waits_for_real_owned_lease(){let(_root,mut state)=sample();state.take_audio_commands();state.observe_audio_backend_available(true);let commands=state.take_audio_commands();assert!(!commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorPlayPreview{..})));let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=state.editor_asset_owner();snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);state.update_audio(snapshot);let commands=state.take_audio_commands();assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorPlayPreview{cut,..}if cut.start_sec==0.25&&cut.length_sec==0.5)));assert!(state.playback.as_ref().unwrap().decoder_ack.is_none());}
}

#[cfg(test)]
mod preview_recovery_tests {
    use super::*;
    use std::time::Duration;
    fn snapshot(state: &State, owner: u64) -> GameplayAudioSnapshot {
        let mut s = GameplayAudioSnapshot::default();
        s.editor.owner = owner;
        s.editor.assets = deadsync_gameplay::EditorAssetReadiness {
            owner,
            registration: state.editor_asset_registration(),
            main: true,
            dedicated_preview: false,
        };
        s.editor.output.available = true;
        s.editor.output.leased = true;
        s.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        s.editor.output.requested_period = Some(512);
        s.editor.output.negotiated_period = Some(512);
        s
    }
    fn sample() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#MUSIC:main.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut s = State::open(&path, 0).unwrap();
        s.take_audio_commands();
        s.observe_audio_backend_available(true);
        let owner = s.editor_asset_owner();
        s.update_audio(snapshot(&s, owner));
        let request = deadsync_editor::song_properties::SampleAudio {
            path: s
                .configured_preview_path(deadsync_gameplay::EditorAssetKind::Main)
                .unwrap(),
            start: 0.25,
            length: Some(0.5),
        };
        s.toggle_sample_audio_role_result(deadsync_gameplay::EditorAssetKind::Main, Ok(request));
        assert!(
            s.take_audio_commands()
                .iter()
                .any(|c| matches!(c, GameplayAudioCommand::EditorPlayPreview { .. }))
        );
        (root, s)
    }
    fn cursor(s: &State, suspended: bool) -> deadsync_gameplay::EditorPreviewCursor {
        deadsync_gameplay::EditorPreviewCursor {
            suspended,
            rendered:true,
            owner: s.editor_asset_owner(),
            kind: deadsync_gameplay::EditorAssetKind::Main,
            registration: 41,
            revision: 7,
            generation: 13,
            asset_epoch: 29,
            next_music_seconds: 0.6,
            rendered_frame: 16800,
            source_frame: None,
            sample_rate_hz: 48000,
        }
    }
    #[test]
    fn callback_without_pcm_preserves_exact_signed_frame_through_suspend_and_resume() {
        let (_root, mut s) = sample();
        let owner=s.editor_asset_owner();
        let mut actual=cursor(&s,false);
        actual.rendered=false;
        actual.source_frame=Some(-32769);
        actual.sample_rate_hz=44100;
        actual.next_music_seconds=-32769.0/44100.0;
        s.observe_rendered_preview_cursor(Some(actual));
        assert_eq!(s.playback.as_ref().unwrap().sample_cursor,Some(actual));
        s.observe_audio_backend_available(false);
        s.take_audio_commands();
        actual.suspended=true;
        s.observe_rendered_preview_cursor(Some(actual));
        s.observe_audio_backend_available(true);
        s.take_audio_commands();
        s.update_audio(snapshot(&s,owner));
        assert!(s.take_audio_commands().iter().any(|command|matches!(command,GameplayAudioCommand::EditorResumePreview{cursor,..}if *cursor==actual&&cursor.source_frame==Some(-32769)&&cursor.sample_rate_hz==44100&&!cursor.rendered)));
        assert!(s.playback.as_ref().unwrap().decoder_ack.is_none());
    }
    #[test]
    fn interrupted_preview_uses_frozen_callback_receipt_and_actual_owner_lease() {
        let (_root, mut s) = sample();
        let owner = s.editor_asset_owner();
        let normal = cursor(&s, false);
        s.observe_rendered_preview_cursor(Some(normal));
        s.observe_audio_backend_available(false);
        assert!(s.take_audio_commands().iter().any(
            |c| matches!(c,GameplayAudioCommand::EditorSuspendPreview{owner:o,..}if *o==owner)
        ));
        let frozen = cursor(&s, true);
        s.observe_rendered_preview_cursor(Some(frozen));
        s.update_source_visual_clock(Instant::now() + Duration::from_secs(3600));
        assert!(s.playback.as_ref().unwrap().visual_clock.is_none());
        assert_eq!(s.playback.as_ref().unwrap().sample_cursor, Some(frozen));
        assert!(s.playback.as_ref().unwrap().decoder_ack.is_none());
        assert_eq!(s.status, i18n::tr("Edition", "AudioDspUnavailable"));
        s.observe_audio_backend_available(true);
        assert!(
            !s.take_audio_commands()
                .iter()
                .any(|c| matches!(c, GameplayAudioCommand::EditorResumePreview { .. }))
        );
        s.update_audio(snapshot(&s, owner + 1));
        assert!(
            !s.take_audio_commands()
                .iter()
                .any(|c| matches!(c, GameplayAudioCommand::EditorResumePreview { .. }))
        );
        s.update_audio(snapshot(&s, owner));
        let commands = s.take_audio_commands();
        assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorResumePreview{cursor,cut,..}if *cursor==frozen&&cut.start_sec==0.25&&cut.length_sec==0.5)));
        assert!(commands.iter().all(|c| !matches!(
            c,
            GameplayAudioCommand::EditorPlayPreview { .. }
                | GameplayAudioCommand::EditorStartMusic { .. }
        )));
        assert!(s.playback.as_ref().unwrap().decoder_ack.is_none());
    }
    #[test]
    fn interrupted_preview_stop_drops_descriptor_and_cannot_resume() {
        let (_root, mut s) = sample();
        let c = cursor(&s, false);
        s.observe_rendered_preview_cursor(Some(c));
        s.observe_audio_backend_available(false);
        s.take_audio_commands();
        s.stop_audio();
        assert!(!s.is_playing());
        s.observe_audio_backend_available(true);
        let owner = s.editor_asset_owner();
        s.update_audio(snapshot(&s, owner));
        assert!(s.take_audio_commands().iter().all(|c| !matches!(
            c,
            GameplayAudioCommand::EditorResumePreview { .. }
                | GameplayAudioCommand::EditorPlayPreview { .. }
        )));
    }
}
