//! Published-editor lease and source tick windows. Commands remain inert until
//! the host drains the published State; loading a candidate touches no audio.
use super::*;
use deadsync_editor::audio_ticks::{ChartTicks, Input, VoiceWindows};
use deadsync_gameplay::{EditorAudioGains, EditorTickKind, GameplayAudioCommand as AudioCommand};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
pub(super) struct Scope {
    pub(super) owner: u64,
    pub(super) prior: Vec<AudioCommand>,
    begun: bool,
    released: bool,
    key: Option<(u64, u64, u64, usize)>,
    chart: Option<ChartTicks>,
    windows: VoiceWindows,
    held: Vec<AudioCommand>,
}
impl Default for Scope {
    fn default() -> Self {
        // Exhaustion is practically unreachable, but must not reuse another
        // live editor identity or the mixer's reserved publication sentinel.
        let owner = NEXT_OWNER
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1).filter(|next| *next < u64::MAX)
            })
            .expect("editor audio owner space exhausted");
        Self {
            owner,
            prior: Vec::new(),
            begun: false,
            released: false,
            key: None,
            chart: None,
            windows: Default::default(),
            held: Vec::new(),
        }
    }
}
#[derive(Default)]
pub(super) struct EditorAudioSession {
    pub(super) assets:super::editor_assets::Runtime,
    pub(super) scope: Scope,
    pub(super) output: super::audio_output::Output,
}
impl State {
    pub(super) fn transfer_editor_audio_session(&mut self, next: &mut State) {
        self.stop_audio();
        let commands = self.take_audio_commands();
        next.audio_commands.clear();
        let mut assets = std::mem::take(&mut next.audio_session.assets);
        assets.force_register = true;
        next.audio_session = std::mem::take(&mut self.audio_session);
        // Keep the new document's bindings, while retaining the published host
        // owner/lease. Force both registrations under that retained owner.
        next.audio_session.assets = assets;
        next.audio_commands.extend(commands);
        next.sync_editor_audio_settings(self.audio_preferences);
    }
    pub(super) fn restart_source_audio_after_device_return(&mut self){
        if self.audio_session.scope.released{return;}
        self.audio_session.scope.begun=false;self.audio_session.scope.held.clear();self.restart_source_output_after_device_return();
    }
    pub(super) fn rearm_editor_scope_for_transport(&mut self) {
        if self.rearm_audio_output_for_transport() {
            self.audio_session.scope.begun = false;
        }
    }
    fn editor_gains(&self) -> EditorAudioGains {
        let p = self.audio_preferences.effective();
        let gains = self.audio_preferences.mix_gains();
        EditorAudioGains {
            main: gains.main,
            music: gains.music,
            assist: if p.assist_enabled {gains.assist}else{0.},
            beat: if p.beat_enabled {gains.beat}else{0.},
        }
    }
    pub(super) fn sync_editor_audio_settings(
        &mut self,
        before: deadsync_editor::audio_preferences::Audio,
    ) {
        let old = before.effective();
        let new = self.audio_preferences.effective();
        self.audio_commands.push(AudioCommand::EditorUpdateAudio {
            owner: self.audio_session.scope.owner,
            gains: self.editor_gains(),
        });
        if (old.fade_in,old.fade_out)!=(new.fade_in,new.fade_out) {
            let update=|command:&mut AudioCommand| {
                if let AudioCommand::EditorPlayPreview{cut,..}|AudioCommand::EditorResumePreview{cut,..}=command {
                    cut.fade_in_sec=new.fade_in;cut.fade_out_sec=new.fade_out;
                }
            };
            self.update_sample_fade_descriptors(new.fade_in,new.fade_out);
            for command in &mut self.audio_session.scope.held {update(command);}
            for command in &mut self.audio_commands {update(command);}
            self.audio_commands.push(AudioCommand::EditorSetPreviewFades{owner:self.audio_session.scope.owner,fade_in:new.fade_in,fade_out:new.fade_out});
        }
        if old.offset != new.offset { self.apply_audio_offset_change(); }
        if old.rate != new.rate {
            self.apply_playback_rate();
        }
        if (
            old.assist_enabled,
            old.beat_enabled,
            old.assist_skip_beat,
            old.beat_skip_assist,
            old.assist_attack,
            old.beat_attack,
        ) != (
            new.assist_enabled,
            new.beat_enabled,
            new.assist_skip_beat,
            new.beat_skip_assist,
            new.assist_attack,
            new.beat_attack,
        ) {
            self.audio_commands
                .push(AudioCommand::EditorRetargetTicks {
                    owner: self.audio_session.scope.owner,
                });
            self.reset_editor_tick_windows();
        }
    }
    pub(super) fn retarget_editor_tick_windows(&mut self) {
        self.audio_commands.push(AudioCommand::EditorRetargetTicks { owner: self.audio_session.scope.owner });
        self.reset_editor_tick_windows();
    }
    fn reset_editor_tick_windows(&mut self) {
        self.audio_session.scope.key = None;
        self.audio_session.scope.chart = None;
        self.audio_session.scope.windows.reset();
    }
    /// All normal Exit/Close/Reload publication paths drain this release before
    /// discarding the editor. Stale release commands cannot stop a newer owner.
    pub(in crate::screens) fn take_editor_audio_close(&mut self) -> bool {
        self.take_file_close()
    }
    pub(in crate::screens) fn release_editor_audio_scope(&mut self) {
        if self.audio_session.scope.released {
            return;
        }
        self.stop_audio();
        if self.audio_session.scope.begun {
            self.audio_commands.push(AudioCommand::EditorEndAudio {
                owner: self.audio_session.scope.owner,
            });
        }
        self.audio_session.scope.released = true;
        self.reset_editor_tick_windows();
    }
    pub(super) fn scoped_audio_commands(&mut self) -> Vec<AudioCommand> {
        self.scoped_audio_commands_at(std::time::Instant::now())
    }
    pub(in crate::screens::edition) fn scoped_audio_commands_at(&mut self,now:std::time::Instant)->Vec<AudioCommand>{
        self.sync_editor_asset_registration();
        let owner=self.audio_session.scope.owner;
        if self.audio_commands.iter().any(|command|match command{
            AudioCommand::StopMusic=>true,
            AudioCommand::EditorPauseMusic{owner:actual,..}|AudioCommand::EditorStopMusic{owner:actual}|AudioCommand::EditorEndAudio{owner:actual}=>*actual==owner,
            _=>false,
        }){self.audio_session.scope.held.clear();}
        let mut pending = if self.audio_output_transport_ready()||self.has_pending_source_visual_audio_start() {
            std::mem::take(&mut self.audio_session.scope.held)
        } else {
            Vec::new()
        };
        pending.extend(std::mem::take(&mut self.audio_commands));
        self.coalesce_source_visual_audio_start(now,&mut pending);
        let mut result = std::mem::take(&mut self.audio_session.scope.prior);
        result.reserve(pending.len() + 1);
        if !self.audio_session.scope.begun && !self.audio_session.scope.released {
            self.audio_session.scope.begun = true;
            result.push(AudioCommand::EditorBeginAudio {
                owner: self.audio_session.scope.owner,
                gains: self.editor_gains(),
            });
        }
        let owner = self.audio_session.scope.owner;
        for command in pending {
            let command = match command {
                AudioCommand::StopMusic => {
                    self.reset_editor_tick_windows();
                    AudioCommand::EditorStopMusic { owner }
                }
                command @ AudioCommand::EditorSetMusicRate { owner: actual, .. } if actual == owner => {
                    self.reset_editor_tick_windows();
                    command
                }
                AudioCommand::SetMusicRate(rate) => {
                    self.reset_editor_tick_windows();
                    AudioCommand::EditorSetMusicRate { owner, rate: f64::from(rate) }
                }
                AudioCommand::SeekMusic(seconds) => {
                    self.reset_editor_tick_windows();
                    AudioCommand::EditorSeekMusic { owner, seconds }
                }
                AudioCommand::PlayMusic {
                    path,
                    cut,
                    looping,
                    rate,
                } => {
                    self.reset_editor_tick_windows();
                    AudioCommand::EditorPlayMusic {
                        owner,
                        path,
                        cut,
                        looping,
                        rate: f64::from(rate),
                    }
                }
                command => command,
            };
            if matches!(
                command,
                AudioCommand::EditorPlayMusic { .. }
                    | AudioCommand::EditorPlayPreview { .. }
                    | AudioCommand::EditorResumePreview { .. }
                    | AudioCommand::EditorSwapMusicAsset { .. }
                    | AudioCommand::EditorStartMusic { .. }
                    | AudioCommand::EditorClearMusicAsset { .. }
                    | AudioCommand::EditorResumeMusic { .. }
                    | AudioCommand::EditorSetMusicRate { .. }
                    | AudioCommand::EditorSeekMusic { .. }
            ) && (matches!(&command,AudioCommand::EditorResumePreview{kind,..}if !self.editor_preview_ready(*kind))||!self.audio_output_transport_ready()||((self.has_pending_source_visual_audio_start()||matches!(command,AudioCommand::EditorPlayPreview{..}|AudioCommand::EditorResumePreview{..}))&&!self.source_visual_audio_lease_ready()))
            {
                self.audio_session.scope.held.push(command);
            } else {
                if matches!(
                    command,
                    AudioCommand::EditorStopMusic { .. } | AudioCommand::EditorPauseMusic { .. } | AudioCommand::EditorEndAudio { .. }
                ) {
                    self.audio_session.scope.held.clear();
                }
                result.push(command);
            }
        }
        result
    }
    pub(super) fn queue_editor_ticks(
        &mut self,
        snapshot: deadsync_gameplay::GameplayAudioSnapshot,
        actual_music_seconds: f64,
    ) {
        let audio = self.audio_preferences.effective();
        if !self.workspace.has_chart() || self.audio_session.scope.released
            || snapshot.editor.owner != self.audio_session.scope.owner
            || (!snapshot.stream_clock.has_music_mapping&&!self.has_logical_music_phase(&snapshot))
            || snapshot.editor.sample_rate == 0
            || (!audio.assist_enabled && !audio.beat_enabled)
        {
            return;
        }
        let Some(generation) = self.chart_audio_generation() else {
            return;
        };
        let key = (
            generation,
            snapshot.editor.tick_generation,
            self.workspace.file().session.content_revision(),
            self.workspace.chart(),
        );
        if self.audio_session.scope.key != Some(key) {
            self.audio_session.scope.key = Some(key);
            self.audio_session.scope.windows.reset();
            self.audio_session.scope.chart =
                match ChartTicks::capture(&self.workspace.file().session, self.workspace.chart()) {
                    Ok(chart) => Some(chart),
                    Err(_) => {
                        self.status = i18n::tr("Edition", "AudioTicksUnavailable");
                        None
                    }
                };
        }
        let Some(chart) = &self.audio_session.scope.chart else {
            return;
        };
        if (audio.assist_enabled && snapshot.editor.assist_duration.is_none())
            || (audio.beat_enabled && snapshot.editor.beat_duration.is_none())
        {
            self.status = i18n::tr("Edition", "AudioTicksUnavailable");
            return;
        }
        let current = actual_music_seconds * f64::from(snapshot.editor.sample_rate);
        if !current.is_finite() || current < i64::MIN as f64 || current >= i64::MAX as f64 {
            return;
        }
        let result = chart
            .window(
                audio,
                Input {
                    current_music_samples: current as i64,
                    sample_rate: snapshot.editor.sample_rate,
                    assist_length: snapshot.editor.assist_duration.unwrap_or(0.),
                    beat_length: snapshot.editor.beat_duration.unwrap_or(0.),
                    budget: 2_000_000,
                },
            )
            .and_then(|window| {
                self.audio_session
                    .scope
                    .windows
                    .update(&window, current as i64)
            });
        match result {
            Ok((assist, beat)) => {
                for (kind, samples) in [
                    (EditorTickKind::Assist, assist),
                    (EditorTickKind::Beat, beat),
                ] {
                    self.audio_commands.push(AudioCommand::EditorTickWindow {
                        owner: self.audio_session.scope.owner,
                        tick_generation: snapshot.editor.tick_generation,
                        music_generation: generation,
                        kind,
                        samples: Arc::from(samples),
                    });
                }
            }
            Err(_) => {
                self.audio_session.scope.chart = None;
                self.status = i18n::tr("Edition", "AudioTicksUnavailable");
            }
        }
    }
}

impl State{pub(super) fn owns_audio_notification(&self,owner:Option<u64>)->bool{owner==Some(self.audio_session.scope.owner)&&self.audio_session.scope.begun&&!self.audio_session.scope.released}}
#[cfg(test)]
mod live_preview_preferences_tests {
    use super::*;
    use deadsync_editor::audio_preferences::{Field,FieldValue};
    #[test]
    fn fade_apply_undo_redo_updates_held_descriptor_without_transport_restart(){
        crate::i18n::init_for_tests();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("song.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();state.take_audio_commands();let owner=state.editor_asset_owner();let bytes=state.workspace.file().session.serialize().unwrap();
        state.audio_session.scope.held.push(AudioCommand::EditorPlayPreview{owner,kind:deadsync_gameplay::EditorAssetKind::Main,path:dir.path().join("music.wav"),cut:deadsync_gameplay::GameplayMusicCut{start_sec:2.,length_sec:3.,fade_in_sec:0.,fade_out_sec:1.5},looping:true,rate:1.});
        let mut audio=state.audio_preferences;audio.set_field(Field::PreviewFadeInTime,FieldValue::Float(0.12)).unwrap();audio.set_field(Field::PreviewFadeOutTime,FieldValue::Float(0.23)).unwrap();assert!(state.commit_audio_preferences(audio).unwrap());
        for (incoming,outgoing,command) in [(0.12,0.23,None),(0.,1.5,Some(deadsync_editor::workspace::Command::Undo)),(0.12,0.23,Some(deadsync_editor::workspace::Command::Redo))] {
            if let Some(command)=command{state.command(command);}
            assert!(state.audio_commands.iter().any(|command|matches!(command,AudioCommand::EditorSetPreviewFades{owner:actual,fade_in,fade_out}if *actual==owner&&*fade_in==incoming&&*fade_out==outgoing)));
            assert!(state.audio_session.scope.held.iter().any(|command|matches!(command,AudioCommand::EditorPlayPreview{cut,..}if cut.start_sec==2.&&cut.length_sec==3.&&cut.fade_in_sec==incoming&&cut.fade_out_sec==outgoing)));
            assert!(!state.audio_commands.iter().any(|command|matches!(command,AudioCommand::EditorStopMusic{..}|AudioCommand::EditorSeekMusic{..}|AudioCommand::EditorPauseMusic{..})));
            state.audio_commands.clear();assert_eq!(state.workspace.file().session.serialize().unwrap(),bytes);
        }
    }
}
#[cfg(test)]mod source_tick_preference_tail_tests{
 use super::*;
 #[test]fn tick_preference_apply_undo_redo_revokes_future_windows_without_hard_stop(){
  crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();state.take_audio_commands();let owner=state.editor_asset_owner();let bytes=state.workspace.file().session.serialize().unwrap();let mut audio=state.audio_preferences;audio.set_field(deadsync_editor::audio_preferences::Field::UseBeatTick,deadsync_editor::audio_preferences::FieldValue::Bool(true)).unwrap();assert!(state.commit_audio_preferences(audio).unwrap());
  for command in [None,Some(deadsync_editor::workspace::Command::Undo),Some(deadsync_editor::workspace::Command::Redo)]{
   if let Some(command)=command{state.command(command);}let commands=state.take_audio_commands();assert!(commands.iter().any(|command|matches!(command,AudioCommand::EditorRetargetTicks{owner:actual}if *actual==owner)));let enabled=state.audio_preferences.effective().beat_enabled;assert!(commands.iter().any(|command|matches!(command,AudioCommand::EditorUpdateAudio{gains,..}if gains.beat==if enabled{0.75}else{0.})));assert!(!commands.iter().any(|command|matches!(command,AudioCommand::EditorInvalidateTicks{..}|AudioCommand::EditorStopMusic{..}|AudioCommand::EditorSeekMusic{..})));assert_eq!(state.workspace.file().session.serialize().unwrap(),bytes);
  }
 }
}

impl State {
    pub(in crate::screens::edition) fn update_queued_preview_region(&mut self,owner:u64,start:f64,length:f64){
        for command in self.audio_commands.iter_mut().chain(self.audio_session.scope.held.iter_mut()) {
            if let AudioCommand::EditorPlayPreview{owner:actual,kind:deadsync_gameplay::EditorAssetKind::Main,cut,..}|AudioCommand::EditorResumePreview{owner:actual,kind:deadsync_gameplay::EditorAssetKind::Main,cut,..}=command {
                if *actual==owner {cut.start_sec=start;cut.length_sec=length;}
            }
        }
    }
}

#[cfg(test)]
mod live_preview_region_queue_tests {
    use super::*;
    #[test]
    fn staged_bounds_update_real_held_main_request_and_preserve_dedicated_stale_owner_and_resume_cursor(){
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");let music=root.path().join("main.wav");std::fs::write(&music,b"RIFF&\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x80\xbb\0\0\0w\x01\0\x02\0\x10\0data\x02\0\0\0\0\0").unwrap();std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#MUSIC:main.wav;#SAMPLESTART:2;#SAMPLELENGTH:3;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut state=State::open(&path,0).unwrap();state.take_audio_commands();let owner=state.editor_asset_owner();state.observe_editor_assets(deadsync_gameplay::EditorAssetReadiness{owner,registration:state.editor_asset_registration(),main:true,dedicated_preview:false});state.toggle_sample_audio_role_result(deadsync_gameplay::EditorAssetKind::Main,Ok(deadsync_editor::song_properties::SampleAudio{path:music.canonicalize().unwrap(),start:2.,length:Some(3.)}));state.take_audio_commands();
        assert!(state.audio_session.scope.held.iter().any(|c|matches!(c,AudioCommand::EditorPlayPreview{kind:deadsync_gameplay::EditorAssetKind::Main,cut,..}if cut.start_sec==2.&&cut.length_sec==3.)));
        let cut=deadsync_gameplay::GameplayMusicCut{start_sec:2.,length_sec:3.,fade_in_sec:0.125,fade_out_sec:0.25};let cursor=deadsync_gameplay::EditorPreviewCursor{suspended:true,rendered:true,owner,kind:deadsync_gameplay::EditorAssetKind::Main,registration:1,revision:2,generation:3,asset_epoch:4,next_music_seconds:2.5,rendered_frame:120000,source_frame:None,sample_rate_hz:48000};
        let dedicated=AudioCommand::EditorPlayPreview{owner,kind:deadsync_gameplay::EditorAssetKind::DedicatedPreview,path:music.clone(),cut,looping:true,rate:1.};let stale=AudioCommand::EditorPlayPreview{owner:owner+100,kind:deadsync_gameplay::EditorAssetKind::Main,path:music.clone(),cut,looping:true,rate:1.};state.audio_session.scope.held.push(dedicated.clone());state.audio_session.scope.held.push(stale.clone());state.audio_session.scope.held.push(AudioCommand::EditorResumePreview{owner,kind:deadsync_gameplay::EditorAssetKind::Main,path:music,cut,looping:true,rate:1.,cursor});
        state.open_song_properties();state.song_sample_key(0,KeyCode::ArrowRight);let start=state.song_properties.as_ref().unwrap().sample_start();let held=&state.audio_session.scope.held;
        assert!(held.contains(&dedicated));assert!(held.contains(&stale));assert!(held.iter().any(|c|matches!(c,AudioCommand::EditorPlayPreview{owner:actual,kind:deadsync_gameplay::EditorAssetKind::Main,cut,..}if *actual==owner&&cut.start_sec==start&&cut.length_sec==3.)));
        assert!(held.iter().any(|c|matches!(c,AudioCommand::EditorResumePreview{cut,cursor:actual,..}if cut.start_sec==start&&cut.length_sec==3.&&cut.fade_in_sec==0.125&&cut.fade_out_sec==0.25&&*actual==cursor)));
    }
}

#[cfg(test)]
mod separate_master_gain_tests {
    use super::*;
    use deadsync_editor::audio_preferences::{Field,FieldValue};
    #[test]
    fn actual_editor_command_producer_preserves_master_and_independent_bus_gains() {
        crate::i18n::init_for_tests();
        let root=tempfile::tempdir().unwrap();
        let mut state=State::without_song(root.path()).unwrap();
        let before=state.audio_preferences;
        for (field,value) in [(Field::MainVolume,0.625),(Field::MusicVolume,0.375),(Field::AssistTickVolume,0.5),(Field::BeatTickVolume,0.75)] {
            state.audio_preferences.set_field(field,FieldValue::Float(value)).unwrap();
        }
        state.audio_preferences.set_field(Field::UseAssistTick,FieldValue::Bool(true)).unwrap();
        state.audio_preferences.set_field(Field::UseBeatTick,FieldValue::Bool(false)).unwrap();
        state.audio_commands.clear();
        state.sync_editor_audio_settings(before);
        let gains=state.audio_commands.iter().find_map(|command|match command{AudioCommand::EditorUpdateAudio{gains,..}=>Some(*gains),_=>None}).unwrap();
        assert_eq!(gains,EditorAudioGains{main:0.625,music:0.375,assist:0.5,beat:0.});
    }
}
