//! Prepared editor assets live across transport stop/pause and preview changes.
use super::*;
use crate::{
    editor_asset_service::{EditorAssetEvent, EditorAssetService},
    editor_asset_slots::AssetLoadToken,
    editor_asset_watch::EditorAssetKind,
};
use deadlib_audio_decode::prepared::PreparedAudioSource;
use std::sync::Arc;
#[derive(Clone, Copy)]
struct Active {
    kind: EditorAssetKind,
    main: bool,
    cut: Cut,
    looping: bool,
}
#[derive(Clone, Copy)]
struct Pending {
    token: AssetLoadToken,
    epoch: u64,
    preview_phase: Option<(f64, u64)>,
}
pub(super) struct Assets {
    owner: u64,
    service: Option<EditorAssetService>,
    active: Option<Active>,
    pending: Option<Pending>,
    preview_authority:Option<(AssetLoadToken,u64,u64)>,
    preview_suspended:Option<SuspendedPreview>,
}
impl Assets {
    pub(super) const fn new() -> Self {
        Self {
            owner: 0,
            service: None,
            active: None,
            pending: None,
            preview_authority:None,
            preview_suspended:None,
        }
    }
}
fn preview_frame_at_rate(frame:i64,from_hz:u32,to_hz:u32)->Option<i64>{
    if from_hz==0||to_hz==0{return None;}
    i64::try_from(i128::from(frame)*i128::from(to_hz)/i128::from(from_hz)).ok()
}
impl AudioControl {
    pub(super) fn begin_editor_asset_owner(&self, owner: u64) -> bool {
        if owner == 0 || owner == u64::MAX {
            return false;
        }
        let mut assets = self.editor.assets.borrow_mut();
        if assets.owner != 0 && assets.owner != owner {
            return false;
        }
        assets.owner = owner;
        true
    }
    pub(super) fn end_editor_asset_owner(&self, owner: u64) -> bool {
        let mut assets = self.editor.assets.borrow_mut();
        if owner == 0 || assets.owner != owner {
            return false;
        }
        assets.active = None;
        assets.preview_authority=None;
        assets.preview_suspended=None;
        assets.pending = None;
        assets.service = None;
        assets.owner = 0;
        true
    }
    /// Register source music/preview even while inactive. Disk observation and
    /// preparation are independent from the output lease and active transport.
    pub fn register_editor_asset(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: Option<PathBuf>,
    ) -> bool {
        if owner == 0 || self.editor.assets.borrow().owner != owner {
            return false;
        }
        let mut assets = self.editor.assets.borrow_mut();
        if assets.service.is_none() {
            match EditorAssetService::new(owner) {
                Ok(service) => assets.service = Some(service),
                Err(message) => {
                    log::error!("Editor asset service: {message}");
                    return false;
                }
            }
        }
        match assets.service.as_mut().unwrap().register(kind, path) {
            Ok(_) => true,
            Err(message) => {
                log::warn!("Editor asset watcher: {message}");
                false
            }
        }
    }
    pub fn editor_asset_ready(&self, owner: u64, kind: EditorAssetKind) -> bool {
        owner != 0
            && self.editor.assets.borrow().owner == owner
            && self
                .editor
                .assets
                .borrow()
                .service
                .as_ref()
                .is_some_and(|service| service.source(kind).is_some())
    }
    pub fn editor_asset_ready_for(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: &std::path::Path,
    ) -> bool {
        if owner == 0 {
            return false;
        }
        let assets = self.editor.assets.borrow();
        assets.owner == owner
            && assets.service.as_ref().is_some_and(|service| {
                service.source(kind).is_some()
                    && service
                        .path(kind)
                        .is_some_and(|held| retained_music_paths_equal(held, path))
            })
    }
    pub fn editor_prepared_asset_for(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: &std::path::Path,
    ) -> Option<crate::editor_asset_slots::PreparedAssetSnapshot> {
        let assets = self.editor.assets.borrow();
        if owner == 0 || assets.owner != owner {
            return None;
        }
        let service = assets.service.as_ref()?;
        if !service
            .path(kind)
            .is_some_and(|held| retained_music_paths_equal(held, path))
        {
            return None;
        }
        let token = service.current_load_token(kind)?;
        Some(crate::editor_asset_slots::PreparedAssetSnapshot {
            token,
            source: service.source(kind).cloned(),
        })
    }
    pub(super) fn stop_editor_asset_transport(&self) {
        let mut assets = self.editor.assets.borrow_mut();
        assets.active = None;
        assets.preview_authority=None;
        assets.preview_suspended=None;
        assets.pending = None;
    }
    pub(super) fn release_editor_assets(&self) {
        let mut assets = self.editor.assets.borrow_mut();
        assets.active = None;
        assets.preview_authority=None;
        assets.preview_suspended=None;
        assets.pending = None;
        assets.service = None;
        assets.owner = 0;
    }
    /// Called after SourceStart accepted its new timeline and asset epoch.
    pub(super) fn start_editor_prepared_main(
        &self,
        owner: u64,
        path: Option<PathBuf>,
        seconds: f64,
        rate: f64,
        exact_frame:Option<i64>,
    ) -> bool {
        if let Some(engine)=self.engine.as_ref(){if !deadlib_audio_core::configure_editor_main(deadlib_audio_core::music_map_generation(),deadlib_audio_core::music_asset_epoch(),exact_frame.unwrap_or_else(||(seconds*f64::from(engine.output.sample_rate_hz))as i64),rate){return false;}}
        self.register_editor_asset(owner, EditorAssetKind::Main, path.clone());
        let source = {
            let mut assets = self.editor.assets.borrow_mut();
            assets.preview_authority=None;
            assets.preview_suspended=None;
            assets.active = Some(Active {
                kind: EditorAssetKind::Main,
                main: true,
                cut: Cut::default(),
                looping: false,
            });
            assets.pending = assets
                .service
                .as_ref()
                .and_then(|service| service.current_load_token(EditorAssetKind::Main))
                .map(|token| Pending {
                    token,
                    epoch: deadlib_audio_core::music_asset_epoch(),
                    preview_phase: None,
                });
            assets
                .service
                .as_ref()
                .and_then(|service| service.source(EditorAssetKind::Main).cloned())
        };
        if let Some(source) = source {
            self.publish_editor_source(
                source,
                path.unwrap_or_default(),
                Active {
                    kind: EditorAssetKind::Main,
                    main: true,
                    cut: Cut {
                        start_sec: seconds,
                        ..Default::default()
                    },
                    looping: false,
                },
                deadlib_audio_core::music_asset_epoch(),
                None,
                rate as f32,
            );
        } else if let Some(engine) = self.engine.as_ref() {
            let _ = engine.command_sender.send(StreamCommand::ClearMusicAsset {
                asset_epoch: deadlib_audio_core::music_asset_epoch(),
            });
        }
        true
    }
    /// Preview has a distinct source role; never infer it from cut or path.
    /// Source StartPreview ignores requests until that source has loaded.
    pub fn editor_play_preview(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: PathBuf,
        cut: Cut,
        looping: bool,
        rate: f32,
    ) -> bool {
        if owner == 0 || self.editor.state.get().owner != owner || self.engine.is_none() {
            return false;
        }
        if self.editor.state.get().generation==u64::MAX{return false;}
        self.register_editor_asset(owner, kind, Some(path.clone()));
        let source = self
            .editor
            .assets
            .borrow()
            .service
            .as_ref()
            .and_then(|service| service.source(kind).cloned());
        let Some(source) = source else {
            return false;
        };
        if !cut.start_sec.is_finite() || !(cut.length_sec > 0.) {
            return false;
        }
        // Existing acceptance helper establishes gain/clock/epoch and scope.
        // It does not enqueue a pathname decoder for this prepared preview.
        if !self.invalidate_editor_ticks(owner){return false;}
        let _=rate;let rate = 1.0f32;
        self.music_request.set(Some(rate));
        let generation = reset_music_stream_clock();
        let epoch = deadlib_audio_core::advance_music_preview_asset_epoch();
        deadlib_audio_core::activate_music_track();
        deadlib_audio_core::resume_music_track();
        deadlib_audio_core::seed_music_asset_clock(cut.start_sec, rate);
        let initial=if kind==EditorAssetKind::DedicatedPreview{0}else{((cut.start_sec*f64::from(self.engine.as_ref().unwrap().output.sample_rate_hz))as i64).max(0)};
        deadlib_audio_core::initialize_editor_preview_clock(generation,epoch,initial,deadlib_audio_core::music_total_frames());
        self.editor.music_owned.set(true);
        self.editor.pending_asset.set(None);
        self.editor.music_path.replace(Some(path.clone()));
        self.editor.paused_position.set(None);
        self.editor.logical_music.set(false);
        begin_editor_unity_replaygain();
        let active = Active {
            kind,
            main: false,
            cut,
            looping,
        };
        {
            let mut assets = self.editor.assets.borrow_mut();
            assets.preview_suspended=None;
            assets.active = Some(active);
            assets.pending = None;
        }
        let _ = generation;
        self.publish_editor_source(source, path, active, epoch, None, rate);
        true
    }
    fn publish_editor_source(
        &self,
        source: Arc<PreparedAudioSource>,
        path: PathBuf,
        mut active: Active,
        epoch: u64,
        preview_phase: Option<(f64, u64)>,
        rate: f32,
    ) {
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        if epoch != deadlib_audio_core::music_asset_epoch() || !self.editor.music_owned.get() {
            return;
        }
        if !active.main&&self.editor.assets.borrow().preview_suspended.is_some(){return;}
        {
            let mut assets=self.editor.assets.borrow_mut();
            assets.preview_authority=if active.main{None}else{assets.service.as_ref().and_then(|service|service.current_load_token(active.kind)).map(|token|(token,deadlib_audio_core::music_map_generation(),epoch))};
        }
        if !active.main && active.kind == EditorAssetKind::DedicatedPreview {
            active.cut.length_sec = source.frames() as f64 / f64::from(source.sample_rate_hz);
        }
        if !active.main {
            deadlib_audio_core::set_editor_preview_envelope(epoch,active.cut.start_sec,active.cut.length_sec,active.cut.fade_in_sec,active.cut.fade_out_sec);
        }
        if !active.main{
            let _=engine.command_sender.send(StreamCommand::ReplaceEditorPreviewPrepared{source,path,cut:active.cut,looping:active.looping,generation:deadlib_audio_core::music_map_generation(),asset_epoch:epoch,preview_phase});
            return;
        }
        let _=rate;let _=preview_phase;
        let _=engine.command_sender.send(StreamCommand::ReplaceEditorMainPrepared{source,path,generation:deadlib_audio_core::music_map_generation(),asset_epoch:epoch});
    }
    /// Source region edits alter future callback bounds without seeking its cursor.
    pub fn editor_set_preview_region(&self,owner:u64,start:f64,length:f64)->bool{
        if owner==0||self.editor.state.get().owner!=owner||![start,length].into_iter().all(f64::is_finite)||length<0.{return false;}
        let mut assets=self.editor.assets.borrow_mut();
        let suspended=assets.preview_suspended.is_some();
        let Some(active)=assets.active.as_mut().filter(|active|!active.main)else{return false;};
        if active.kind==EditorAssetKind::DedicatedPreview{return true;}
        let epoch=deadlib_audio_core::music_asset_epoch();
        active.cut.start_sec=start;active.cut.length_sec=length;
        if !suspended{deadlib_audio_core::set_editor_preview_envelope(epoch,start,length,active.cut.fade_in_sec,active.cut.fade_out_sec);}
        true
    }
    /// Live source preference update: preserve decoder, phase, lease and asset authority.
    pub fn editor_set_preview_fades(&self,owner:u64,fade_in:f64,fade_out:f64)->bool {
        if owner==0 || self.editor.state.get().owner!=owner || !fade_in.is_finite() || !fade_out.is_finite(){return false;}
        let mut assets=self.editor.assets.borrow_mut();
        let Some(active)=assets.active.as_mut().filter(|active|!active.main)else{return false;};
        active.cut.fade_in_sec=fade_in;active.cut.fade_out_sec=fade_out;
        let active=*active;
        let length=if active.kind==EditorAssetKind::DedicatedPreview {
            assets.service.as_ref().and_then(|service|service.source(active.kind)).map(|source|source.frames()as f64/f64::from(source.sample_rate_hz)).unwrap_or(active.cut.length_sec)
        }else{active.cut.length_sec};
        if assets.preview_suspended.is_none(){
            deadlib_audio_core::set_editor_preview_envelope(deadlib_audio_core::music_asset_epoch(),active.cut.start_sec,length,active.cut.fade_in_sec,active.cut.fade_out_sec);
        }
        true
    }
    /// Service this before editor snapshots. No filesystem IO occurs here;
    /// only bounded watcher/results inboxes and transport commands are handled.
    pub fn poll_editor_assets(&self, clock: &mut crate::MusicClock) {
        if let Some(engine)=self.engine.as_ref(){engine.sfx_cache.reclaim_schedules();}
        self.poll_editor_asset_events(|| clock.snapshot_for_editor_time());
        self.restore_preview_after_output_handoff();
    }
    #[cfg(test)]
    fn poll_editor_assets_at_rendered_frame(&self, clock: &crate::MusicClock) {
        self.poll_editor_asset_events(|| clock.snapshot_at_rendered_frame());
    }
    fn poll_editor_asset_events(
        &self,
        mut clock: impl FnMut() -> (deadlib_audio_core::MusicStreamClockSnapshot, bool, f64),
    ) {
        let events = {
            let mut assets = self.editor.assets.borrow_mut();
            let Some(service) = assets.service.as_mut() else {
                return;
            };
            service.poll()
        };
        for event in events {
            match event {
                EditorAssetEvent::WatchFailed { message, .. } => {
                    log::warn!("Editor asset watcher: {message}")
                }
                EditorAssetEvent::Reset { token } => {
                    let active = {
                        let assets = self.editor.assets.borrow();
                        if !assets.service.as_ref().is_some_and(|s| s.is_current(token)) {
                            continue;
                        }
                        assets
                            .active
                            .filter(|active| active.kind == token.registration.kind)
                    };
                    if self.editor.assets.borrow().preview_suspended.is_some(){continue;}
                    let Some(active) = active else {
                        continue;
                    };
                    if self.editor.assets.borrow().pending.is_some_and(|pending| {
                        pending.token == token
                            && pending.epoch == deadlib_audio_core::music_asset_epoch()
                    }) {
                        continue;
                    }
                    if !self.editor.music_owned.get() {
                        continue;
                    }
                    let Some(engine) = self.engine.as_ref() else {
                        continue;
                    };
                    let exact_rate=self.exact_editor_main_rate().unwrap_or(f64::from(self.music_request.get().unwrap_or(1.)));
                    let rate=exact_rate as f32;
                    if active.main&&!deadlib_audio_core::editor_main_can_configure(){continue;}
                    let old_epoch=deadlib_audio_core::music_asset_epoch();
                    let preview_length=deadlib_audio_core::editor_preview_envelope(old_epoch).map(|value|value.length_seconds()).unwrap_or(active.cut.length_sec);
                    let main_callback=if active.main{self.freeze_editor_main_phase()}else{None};
                    let callback=if active.main{None}else{
                        let generation=deadlib_audio_core::music_map_generation();
                        let _=deadlib_audio_core::freeze_rendered_music_cursor(generation,old_epoch);
                        deadlib_audio_core::preview_callback_cursor().filter(|value|value.generation==generation&&value.asset_epoch==old_epoch)
                    };
                    let (snapshot, _, absolute) = clock();
                    let now = if active.main{self.editor.paused_position.get().unwrap_or(snapshot.music_nanos as f64/1e9)}else{callback.map(|value|value.next_music_seconds).unwrap_or(active.cut.start_sec)};
                    let physical_anchor=callback.map(|value|value.callback_frame).unwrap_or_else(deadlib_audio_core::music_total_frames);
                    let elapsed = absolute
                        - deadlib_audio_core::music_track_start_frame() as f64
                            / f64::from(engine.output.sample_rate_hz.max(1));
                    if active.main {
                        deadlib_audio_core::seed_music_asset_clock(
                            now - elapsed * f64::from(rate),
                            rate,
                        );
                        deadlib_audio_core::mark_music_track_started(
                            deadlib_audio_core::music_track_start_frame(),
                        );
                    }
                    let epoch = if active.main {
                        deadlib_audio_core::advance_music_asset_epoch()
                    } else {
                        deadlib_audio_core::advance_music_preview_asset_epoch()
                    };
                    if active.main{
                        let frame=main_callback.unwrap_or((now*f64::from(engine.output.sample_rate_hz))as i64);
                        if !deadlib_audio_core::configure_editor_main(deadlib_audio_core::music_map_generation(),epoch,frame,exact_rate){continue;}
                    }
                    if !active.main{
                        let elapsed=physical_anchor.saturating_sub(deadlib_audio_core::music_track_start_frame())as f64/f64::from(engine.output.sample_rate_hz.max(1));
                        deadlib_audio_core::seed_music_asset_clock(now-elapsed,1.);
                        let length=if active.kind==EditorAssetKind::DedicatedPreview{preview_length}else{active.cut.length_sec};

                        let initial=callback.map(|cursor|if cursor.sample_rate_hz==engine.output.sample_rate_hz{cursor.source_frame}else{(cursor.next_music_seconds*f64::from(engine.output.sample_rate_hz))as i64}).unwrap_or_else(||((active.cut.start_sec*f64::from(engine.output.sample_rate_hz))as i64).max(0));
                        deadlib_audio_core::initialize_editor_preview_clock(deadlib_audio_core::music_map_generation(),epoch,initial,physical_anchor);
                        deadlib_audio_core::set_editor_preview_envelope(epoch,active.cut.start_sec,length.max(0.),active.cut.fade_in_sec,active.cut.fade_out_sec);
                        self.editor.assets.borrow_mut().preview_authority=Some((token,deadlib_audio_core::music_map_generation(),epoch));
                    }
                    self.editor.pending_asset.set(Some(epoch));
                    crate::music_map::discard_music_asset_map();
                    begin_editor_unity_replaygain();
                    let phase =
                        (!active.main).then_some((now, physical_anchor));
                    self.editor.assets.borrow_mut().pending = Some(Pending {
                        token,
                        epoch,
                        preview_phase: phase,
                    });
                    let _ = engine
                        .command_sender
                        .send(StreamCommand::ClearMusicAsset { asset_epoch: epoch });
                }
                EditorAssetEvent::Loaded { token, source } => {
                    let publication = {
                        let assets = self.editor.assets.borrow();
                        let current = assets.service.as_ref().is_some_and(|s| s.is_current(token));
                        if !current {
                            continue;
                        }
                        assets.pending.filter(|p| p.token == token).and_then(|p| {
                            assets.active.map(|a| {
                                (
                                    p,
                                    a,
                                    assets
                                        .service
                                        .as_ref()
                                        .and_then(|s| s.path(a.kind))
                                        .map(std::path::Path::to_path_buf),
                                )
                            })
                        })
                    };
                    if let Some((pending, active, Some(path))) = publication {
                        if pending.epoch == deadlib_audio_core::music_asset_epoch() {
                            self.publish_editor_source(
                                source,
                                path,
                                active,
                                pending.epoch,
                                pending.preview_phase,
                                self.music_request.get().unwrap_or(1.),
                            );
                            self.editor.assets.borrow_mut().pending = None;
                        }
                    }
                }
                EditorAssetEvent::Failed { token, message } => {
                    let pending = self
                        .editor
                        .assets
                        .borrow()
                        .pending
                        .filter(|pending| pending.token == token);
                    if let Some(pending) = pending {
                        if pending.epoch == deadlib_audio_core::music_asset_epoch() {
                            if let Some(engine) = self.engine.as_ref() {
                                let _ = engine.command_sender.send(
                                    StreamCommand::EditorPreparedFailed {
                                        asset_epoch: pending.epoch,
                                        generation: deadlib_audio_core::music_map_generation(),
                                        message,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn editor_main_keeps_double_rate_through_start_rate_seek_and_asset_swap(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();let mut rig=Rig::new();let rate=0.30000000000000004f64;assert!(rig.audio.editor_start_music(rig.owner,None,0.,rate));assert_eq!(rig.audio.exact_editor_main_rate().unwrap().to_bits(),rate.to_bits());assert_ne!(rig.audio.exact_editor_main_rate().unwrap().to_bits(),f64::from(rate as f32).to_bits());
        let mut pcm=[0.;514];rig.render.render(OutputBufferMut::F32(&mut pcm),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(deadlib_audio_core::editor_main_cursor().unwrap().source_frame,77);
        let rate=1.3500000000000001f64;assert!(rig.audio.editor_set_music_rate_at(rig.owner,rate,&mut rig.clock));assert_eq!(rig.audio.exact_editor_main_rate().unwrap().to_bits(),rate.to_bits());assert!(rig.audio.editor_seek_music(rig.owner,32769.25/48000.));assert_eq!(rig.audio.exact_editor_main_rate().unwrap().to_bits(),rate.to_bits());
        rig.render.render(OutputBufferMut::F32(&mut pcm),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);let frame=deadlib_audio_core::editor_main_cursor().unwrap().source_frame;assert!(rig.audio.editor_clear_music_asset(rig.owner,&mut rig.clock));assert_eq!(deadlib_audio_core::editor_main_configuration().unwrap().frame,frame);assert_eq!(rig.audio.exact_editor_main_rate().unwrap().to_bits(),rate.to_bits());
        assert!(rig.audio.editor_set_music_rate(rig.owner,0.1));assert_eq!(rig.audio.exact_editor_main_rate(),Some(0.25));
    }
    #[test]
    fn main_prepared_actual_callbacks_match_source_grid_and_rate_seek_reuse_pcm_after_unlink(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();
        for input_hz in [24000,48000]{
            let directory=directory("main-source-callback");let path=directory.join("ramp.wav");wav(&path,true);
            if input_hz==24000{let mut bytes=std::fs::read(&path).unwrap();bytes[24..28].copy_from_slice(&24000u32.to_le_bytes());bytes[28..32].copy_from_slice(&96000u32.to_le_bytes());std::fs::write(&path,bytes).unwrap();}
            let mut rig=Rig::new();rig.audio.register_editor_asset(rig.owner,EditorAssetKind::Main,Some(path.clone()));rig.wait(|r|r.audio.editor_asset_ready(r.owner,EditorAssetKind::Main));
            let source=rig.audio.editor.assets.borrow().service.as_ref().unwrap().source(EditorAssetKind::Main).unwrap().clone();assert_eq!(source.sample_rate_hz,input_hz);
            let dry=deadlib_audio_decode::preview::resample_prepared(&source,48000,&std::sync::atomic::AtomicBool::new(false)).unwrap();
            let rate=0.3f64;assert!(rig.audio.editor_start_music(rig.owner,Some(path.clone()),37./48000.,rate));rig.wait(|r|r.ready());
            let epoch=deadlib_audio_core::music_asset_epoch();let mut frame=37;
            for frames in [1usize,257,2049,4097]{
                let mut expected=vec![0.;frames*2];deadlib_audio_decode::editor_time_stretch::process(frame,&dry,48000,2,&mut expected,f64::from(rate)).unwrap();
                let mut actual=vec![0.;frames*2];rig.render.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(actual,expected,"actual worker/render callback {frames}");
                frame+=(frames as f64*f64::from(rate))as i64;assert_eq!(deadlib_audio_core::editor_main_cursor().unwrap().source_frame,frame);
            }
            std::fs::remove_file(&path).unwrap();assert!(rig.audio.editor_set_music_rate_at(rig.owner,1.35,&mut rig.clock));
            while let Ok(command)=rig.commands.try_recv(){assert!(!matches!(command,StreamCommand::ReplaceEditorMainPrepared{..}),"rate must reuse prepared PCM");rig.runtime.handle(command);}
            let mut expected=vec![0.;514];deadlib_audio_decode::editor_time_stretch::process(frame,&dry,48000,2,&mut expected,1.35f64).unwrap();let mut actual=vec![0.;514];rig.render.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(actual,expected);assert_eq!(deadlib_audio_core::music_asset_epoch(),epoch);
            assert!(rig.audio.editor_seek_music(rig.owner,0.25));while let Ok(command)=rig.commands.try_recv(){assert!(!matches!(command,StreamCommand::ReplaceEditorMainPrepared{..}));rig.runtime.handle(command);}
            let mut expected=vec![0.;514];deadlib_audio_decode::editor_time_stretch::process(12000,&dry,48000,2,&mut expected,1.35f64).unwrap();rig.render.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(actual,expected);drop(rig);std::fs::remove_dir_all(directory).unwrap();
        }
    }
    #[test]
    fn delayed_main_completion_after_rate_uses_current_callback_phase_without_stale_ack(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();let directory=directory("main-source-pending-rate");let path=directory.join("ramp.wav");wav(&path,true);let mut rig=Rig::new();rig.audio.register_editor_asset(rig.owner,EditorAssetKind::Main,Some(path.clone()));rig.wait(|r|r.audio.editor_asset_ready(r.owner,EditorAssetKind::Main));
        let source=rig.audio.editor.assets.borrow().service.as_ref().unwrap().source(EditorAssetKind::Main).unwrap().clone();let dry=deadlib_audio_decode::preview::resample_prepared(&source,48000,&std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert!(rig.audio.editor_start_music(rig.owner,Some(path),0.,0.3));let old_generation=deadlib_audio_core::music_map_generation();let mut actual=[0.;514];rig.render.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(actual,[0.;514]);assert_eq!(deadlib_audio_core::editor_main_cursor().unwrap().source_frame,77);assert!(!rig.clock.snapshot_at_rendered_frame().1);assert!(rig.audio.decoder_status().is_none());
        assert!(rig.audio.editor_set_music_rate_at(rig.owner,1.35,&mut rig.clock));let generation=deadlib_audio_core::music_map_generation();assert_ne!(generation,old_generation);rig.poll();rig.wait(|r|r.ready());assert_eq!(rig.audio.decoder_status().unwrap().command_generation,generation);assert!(!rig.clock.snapshot_at_rendered_frame().1,"source ready is not callback PCM");let mut expected=[0.;514];deadlib_audio_decode::editor_time_stretch::process(77,&dry,48000,2,&mut expected,1.35f64).unwrap();rig.render.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(actual,expected);assert!(rig.clock.snapshot_at_rendered_frame().1);assert_eq!(deadlib_audio_core::rendered_music_cursor().unwrap().generation,generation);drop(rig);std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn live_main_preview_bounds_keep_real_pcm_phase_and_dedicated_slot_is_independent(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();
        for kind in [EditorAssetKind::Main,EditorAssetKind::DedicatedPreview]{
            let directory=directory("live-source-bounds");let path=directory.join("ramp.wav");wav(&path,true);
            let mut rig=Rig::new();rig.audio.register_editor_asset(rig.owner,kind,Some(path.clone()));rig.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));
            let cut=Cut{start_sec:if kind==EditorAssetKind::Main{0.05}else{0.},length_sec:0.02,..Default::default()};
            assert!(rig.audio.editor_play_preview(rig.owner,kind,path.clone(),cut,true,2.));rig.wait(|rig|rig.ready());
            assert_eq!(rig.audio.music_request.get(),Some(1.),"source preview ignores main MusicRate");
            let first=rig.pcm();assert!(first.iter().any(|v|*v>0));
            let epoch=deadlib_audio_core::music_asset_epoch();let generation=deadlib_audio_core::music_map_generation();
            let before=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();
            assert!(!rig.audio.editor_set_preview_region(rig.owner+1,0.08,0.02));
            assert!(rig.audio.editor_set_preview_region(rig.owner,0.08,0.02));
            assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap(),before);
            if kind==EditorAssetKind::Main{
                for index in 0..5{
                    let pcm=rig.pcm();assert!(pcm.iter().all(|v|*v==0),"source before new start must remain silent, callback={index}");
                }
                let pcm=rig.pcm();assert_eq!(pcm[0],1060);assert_eq!(pcm[1],1060);
                assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap().next_music_seconds,0.085);
                // Shrinking does not seek first: source samples the old absolute
                // cursor once, then wraps under the new end for the next frame.
                assert!(rig.audio.editor_set_preview_region(rig.owner,0.08,0.005));
                let pcm=rig.pcm();assert_eq!(&pcm[..4],&[1063,1063,1060,1060]);
                let frozen=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();
                assert!(rig.audio.editor_set_preview_region(rig.owner,0.08,0.));
                assert!(rig.pcm().iter().all(|v|*v==0));
                assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap().next_music_seconds,frozen.next_music_seconds);
                assert!(rig.audio.editor_set_preview_region(rig.owner,0.08,0.02));
                assert!(rig.pcm().iter().any(|v|*v>0));
            }else{
                let pcm=rig.pcm();assert_eq!(pcm[0],1003);assert!(pcm.iter().all(|v|*v>0));
                assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap().next_music_seconds,0.01);
            }
            assert_eq!(deadlib_audio_core::music_asset_epoch(),epoch);assert_eq!(deadlib_audio_core::music_map_generation(),generation);
            assert!(rig.commands.try_recv().is_err());
            drop(rig);std::fs::remove_dir_all(directory).unwrap();
        }
    }
    #[test]
    fn live_bounds_during_device_suspension_resume_absolute_frozen_cursor_not_modulo_new_cut(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();let directory=directory("suspended-live-source-bounds");let path=directory.join("ramp.wav");wav(&path,true);
        let mut rig=Rig::new();let kind=EditorAssetKind::Main;rig.audio.register_editor_asset(rig.owner,kind,Some(path.clone()));rig.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));
        let cut=Cut{start_sec:0.05,length_sec:0.02,..Default::default()};assert!(rig.audio.editor_play_preview(rig.owner,kind,path.clone(),cut,true,1.));rig.wait(|rig|rig.ready());assert!(rig.pcm().iter().any(|v|*v>0));
        assert!(rig.audio.editor_suspend_preview(rig.owner));rig.poll();let frozen=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();assert_eq!(frozen.next_music_seconds,0.055);
        assert!(rig.audio.editor_set_preview_region(rig.owner,0.08,0.02));
        assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap(),frozen);
        let changed=Cut{start_sec:0.08,length_sec:0.02,..Default::default()};
        assert!(rig.audio.resume_preview_after_output_lease(rig.owner,kind,path.clone(),changed,true,1.,frozen));rig.poll();rig.wait(|rig|rig.ready());
        assert!(rig.pcm().iter().all(|v|*v==0));assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap().next_music_seconds,0.06);
        drop(rig);std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn delayed_initial_preview_bounds_and_outage_keep_callback_receipt_without_fake_pcm_or_opened(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();let directory=directory("pending-live-source-bounds");let path=directory.join("ramp.wav");wav(&path,true);
        let mut rig=Rig::new();let kind=EditorAssetKind::Main;rig.audio.register_editor_asset(rig.owner,kind,Some(path.clone()));rig.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));
        let cut=Cut{start_sec:0.05,length_sec:0.02,..Default::default()};assert!(rig.audio.editor_play_preview(rig.owner,kind,path.clone(),cut,true,1.));
        // Hold the genuine manager command before preparation, while real DSP
        // callbacks continue. No decoder-open/readiness publication is invented.
        assert!(rig.audio.decoder_status().is_none());assert!(rig.pcm().iter().all(|v|*v==0));
        let initial=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();assert!(!initial.rendered);assert_eq!(initial.source_frame,Some(2640));
        assert!(rig.audio.editor_set_preview_region(rig.owner,0.08,0.02));assert!(rig.pcm().iter().all(|v|*v==0));
        assert!(rig.audio.editor_set_preview_region(rig.owner,0.07,0.02));
        assert!(rig.audio.decoder_status().is_none());assert!(rig.audio.editor_suspend_preview(rig.owner));
        let frozen=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();assert!(frozen.suspended);assert!(!frozen.rendered);assert_eq!(frozen.source_frame,Some(2880));assert_eq!(frozen.sample_rate_hz,48000);
        rig.poll();assert!(rig.audio.decoder_status().is_none(),"stale prepared command cannot open after suspension");
        for _ in 0..8{assert!(rig.pcm().iter().all(|v|*v==0));}assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap(),frozen);
        let changed=Cut{start_sec:0.07,length_sec:0.02,..Default::default()};assert!(rig.audio.resume_preview_after_output_lease(rig.owner,kind,path.clone(),changed,true,1.,frozen));rig.poll();rig.wait(|rig|rig.ready());
        assert!(rig.pcm().iter().all(|v|*v==0));assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap().source_frame,Some(3120));
        assert!(rig.pcm().iter().all(|v|*v==0));let pcm=rig.pcm();assert_eq!(pcm[0],1052);assert!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap().rendered);
        drop(rig);std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn preview_lease_resume_keeps_same_rate_integer_frame_and_converts_new_rate_rationally(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();
        assert_eq!(preview_frame_at_rate(i64::MAX,1,u32::MAX),None);assert_eq!(preview_frame_at_rate(1,0,48000),None);
        for target_hz in [44100,48000]{
            let directory=directory("preview-rate-handoff");let path=directory.join("ramp.wav");wav(&path,true);let mut rig=Rig::at_rate(44100);let kind=EditorAssetKind::Main;
            rig.audio.register_editor_asset(rig.owner,kind,Some(path.clone()));rig.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));
            let cut=Cut{start_sec:32768.25/44100.,length_sec:0.02,..Default::default()};assert!(rig.audio.editor_play_preview(rig.owner,kind,path.clone(),cut,true,1.));rig.wait(|rig|rig.ready());
            let mut pcm=[0i16;2];rig.render.render(OutputBufferMut::I16(&mut pcm),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert!(pcm.iter().any(|sample|*sample>0));
            assert!(rig.audio.editor_suspend_preview(rig.owner));rig.poll();let frozen=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();assert_eq!(frozen.source_frame,Some(32769));assert_eq!(frozen.sample_rate_hz,44100);
            let(stream,handle)=deadlib_audio_core::music_transport(target_hz,2);let runtime=stream::MusicStreamRuntime::new(stream.writer,OutputFormat{sample_rate_hz:target_hz,channels:2});
            rig.audio.engine.as_mut().unwrap().output.sample_rate_hz=target_hz;rig.audio.engine.as_mut().unwrap().decoder_status=runtime.decoder_status();
            rig.runtime=runtime;rig.clock=crate::MusicClock::new(stream.played_map,target_hz);rig.render=RenderState::new(handle,Arc::new(MixControls::new()),2);
            assert!(rig.audio.resume_preview_after_output_lease(rig.owner,kind,path.clone(),cut,true,1.,frozen));rig.poll();rig.wait(|rig|rig.ready());
            rig.render.render(OutputBufferMut::I16(&mut pcm),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert!(pcm.iter().any(|sample|*sample>0));
            let actual=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();let expected=i64::try_from(i128::from(32769)*i128::from(target_hz)/44100).unwrap();assert_eq!(actual.source_frame,Some(expected+1));assert_eq!(actual.sample_rate_hz,target_hz);assert!(actual.rendered);
            drop(rig);std::fs::remove_dir_all(directory).unwrap();
        }
    }
    #[test]
    fn dedicated_prepared_preview_nonloop_reports_eof_only_after_actual_callback_endpoint(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();let directory=directory("preview-real-eof");let path=directory.join("ramp.wav");wav(&path,true);let mut rig=Rig::new();let kind=EditorAssetKind::DedicatedPreview;
        rig.audio.register_editor_asset(rig.owner,kind,Some(path.clone()));rig.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));
        assert!(rig.audio.editor_play_preview(rig.owner,kind,path.clone(),Cut{length_sec:f64::INFINITY,..Default::default()},false,1.));rig.wait(|rig|rig.ready());
        assert_eq!(rig.audio.decoder_status().unwrap().phase,stream::lifecycle::DecoderPhase::Producing);
        for _ in 0..400{assert!(rig.pcm().iter().any(|sample|*sample>0));}
        let cursor=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();assert_eq!(cursor.source_frame,Some(96000));assert_eq!(cursor.next_music_seconds,2.);
        assert!(rig.pcm().iter().all(|sample|*sample==0));assert_eq!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap(),cursor);
        rig.wait(|rig|rig.audio.decoder_status().is_some_and(|status|status.phase==stream::lifecycle::DecoderPhase::Exhausted));
        assert_eq!(rig.audio.decoder_status().unwrap().final_music_seconds,Some(2.));drop(rig);std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn prepared_editor_main_and_preview_never_inherit_gameplay_pitch() {
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();
        let previous=preserve_pitch_enabled();set_preserve_pitch_enabled(true);
        let directory=directory("source-pitch");let path=directory.join("source.wav");wav(&path,true);
        let source=PreparedAudioSource::load(&path,crate::DECODE_OPTIONS,&std::sync::atomic::AtomicBool::new(false)).unwrap();
        let(audio,commands,_)=super::super::tests::control();let owner=1130;
        assert!(audio.begin_editor_audio(owner,EditorAudioGains{main:1.,music:1.,assist:1.,beat:1.}));
        assert!(audio.editor_play_music(owner,path.clone(),Cut::default(),false,2.));while commands.try_recv().is_ok(){}
        for main in [true,false] {
            let epoch=deadlib_audio_core::music_asset_epoch();
            audio.publish_editor_source(source.clone(),path.clone(),Active{kind:EditorAssetKind::Main,main,cut:Cut{start_sec:0.1,length_sec:0.5,..Default::default()},looping:!main},epoch,None,if main{2.}else{1.});
            let command=commands.try_recv().unwrap();
            if main{assert!(matches!(command,StreamCommand::ReplaceEditorMainPrepared{..}));}
            else{assert!(matches!(command,StreamCommand::ReplaceEditorPreviewPrepared{..}));}
        }
        audio.end_editor_audio(owner);set_preserve_pitch_enabled(previous);std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn preview_start_and_postlease_resume_refuse_at_max_without_rebinding_or_unfreezing(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();let directory=directory("preview-authority-max");let path=directory.join("preview.wav");wav(&path,true);let mut rig=Rig::new();let kind=EditorAssetKind::Main;rig.audio.register_editor_asset(rig.owner,kind,Some(path.clone()));rig.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));let cut=Cut{start_sec:0.05,length_sec:0.02,..Default::default()};assert!(rig.audio.editor_play_preview(rig.owner,kind,path.clone(),cut,true,1.));rig.wait(|rig|rig.ready());assert!(rig.pcm().iter().any(|sample|*sample>0));assert!(rig.audio.editor_suspend_preview(rig.owner));rig.poll();let frozen=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();rig.audio.editor.state.set(State{owner:rig.owner,generation:u64::MAX});
        let capture=||{let assets=rig.audio.editor.assets.borrow();let service=assets.service.as_ref().unwrap();((deadlib_audio_core::music_asset_epoch(),deadlib_audio_core::music_map_generation(),deadlib_audio_core::seeded_music_position(0.),deadlib_audio_core::music_track_paused(),rig.audio.music_request.get()),(service.path(kind).map(std::path::Path::to_path_buf),service.current_load_token(kind)),rig.audio.editor_preview_cursor(rig.owner,kind,&path))};let before=capture();
        assert!(!rig.audio.editor_play_preview(rig.owner,kind,path.with_file_name("must-not-rebind.wav"),cut,true,2.));assert_eq!(capture(),before);
        assert!(!rig.audio.resume_preview_after_output_lease(rig.owner,kind,path.clone(),cut,true,2.,frozen));assert_eq!(capture(),before);assert!(rig.commands.try_recv().is_err());assert!(rig.audio.editor_stop_music(rig.owner));assert!(rig.audio.editor_preview_cursor(rig.owner,kind,&path).is_none());assert!(rig.audio.music_request.get().is_none());assert!(!deadlib_audio_core::music_track_active());
    }

    #[test]
    fn live_fades_change_already_primed_prepared_pcm_without_restarting_transport() {
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();
        for input_hz in [24_000u32,48_000] {for dedicated in [false,true] {
            let directory=directory("live-fades");let path=directory.join("constant.wav");let count=input_hz/25;
            let mut bytes=Vec::new();bytes.extend_from_slice(b"RIFF");bytes.extend_from_slice(&(36+count*4).to_le_bytes());bytes.extend_from_slice(b"WAVEfmt ");bytes.extend_from_slice(&16u32.to_le_bytes());bytes.extend_from_slice(&1u16.to_le_bytes());bytes.extend_from_slice(&2u16.to_le_bytes());bytes.extend_from_slice(&input_hz.to_le_bytes());bytes.extend_from_slice(&(input_hz*4).to_le_bytes());bytes.extend_from_slice(&4u16.to_le_bytes());bytes.extend_from_slice(&16u16.to_le_bytes());bytes.extend_from_slice(b"data");bytes.extend_from_slice(&(count*4).to_le_bytes());for _ in 0..count*2{bytes.extend_from_slice(&10000i16.to_le_bytes());}std::fs::write(&path,bytes).unwrap();
            let kind=if dedicated{EditorAssetKind::DedicatedPreview}else{EditorAssetKind::Main};
            // Decode and render a separate dry run through the exact same real
            // resampler, loop resets and partial-resume path. A constant input
            // does not imply constant output near a fractional resampler cut.
            let (dry,dry_frozen_phase)={
                let mut reference=Rig::new();
                reference.audio.register_editor_asset(reference.owner,kind,Some(path.clone()));
                reference.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));
                let dry_cut=Cut{start_sec:if dedicated{0.}else{0.005012345},length_sec:if dedicated{0.04}else{0.02},..Default::default()};
                let frames=(dry_cut.length_sec*48000.)as usize;
                assert!(reference.audio.editor_play_preview(reference.owner,kind,path.clone(),dry_cut,true,1.));
                reference.wait(|rig|rig.ready());let mut dry=Vec::new();
                for _ in 0..frames/240*4+1{dry.extend(reference.pcm());}
                assert!(reference.audio.editor_suspend_preview(reference.owner));reference.poll();
                let frozen=reference.audio.editor_preview_cursor(reference.owner,kind,&path).unwrap();
                assert!(reference.audio.resume_preview_after_output_lease(reference.owner,kind,path.clone(),dry_cut,true,1.,frozen));
                reference.poll();reference.wait(|rig|rig.ready());
                for _ in 0..frames/240*2{dry.extend(reference.pcm());}
                (dry,frozen.next_music_seconds)
            };
            let dry_offset=std::cell::Cell::new(0usize);
            let mut rig=Rig::new();
            rig.audio.register_editor_asset(rig.owner,kind,Some(path.clone()));rig.wait(|rig|rig.audio.editor_asset_ready(rig.owner,kind));
            let mut cut=Cut{start_sec:if dedicated{0.}else{0.005012345},length_sec:if dedicated{0.04}else{0.02},fade_in_sec:0.005,fade_out_sec:0.005};
            assert!(rig.audio.editor_play_preview(rig.owner,kind,path.clone(),cut,true,1.));rig.wait(|rig|rig.ready());
            let epoch=deadlib_audio_core::music_asset_epoch();let generation=deadlib_audio_core::music_map_generation();let total=(cut.length_sec*48000.)as usize;
            let expected=|local:usize,incoming:usize,outgoing:usize|->f64{let start=total as i64-outgoing as i64;if outgoing>0&&local as i64>start{1.-(local as i64-start)as f64/outgoing as f64}else if incoming>0&&local<incoming{local as f64/incoming as f64}else{1.}};
            let check=|rig:&mut Rig,incoming:usize,outgoing:usize,loops:usize|{for n in 0..total/240*loops {let pcm=rig.pcm();for (i,samples)in pcm.chunks_exact(2).enumerate(){let local=(n*240+i)%total;let source=dry[dry_offset.get()+i*2];let target=(f64::from(source)*expected(local,incoming,outgoing)).round()as i16;assert!((i32::from(samples[0])-i32::from(target)).abs()<=3,"hz={input_hz} dedicated={dedicated} frame={local} dry={source} got={} expected={target}",samples[0]);assert_eq!(samples[0],samples[1]);}dry_offset.set(dry_offset.get()+pcm.len());}};
            check(&mut rig,240,240,1);
            // The next loop is already queued. Removing both fades must remove
            // the previous envelope, proving PCM wasn't baked by the worker.
            assert!(!rig.audio.editor_set_preview_fades(rig.owner+1,0.,0.));
            assert!(rig.audio.editor_set_preview_fades(rig.owner,0.,0.));check(&mut rig,0,0,1);
            assert!(rig.audio.editor_set_preview_fades(rig.owner,cut.length_sec*2.,cut.length_sec*2.));check(&mut rig,total*2,total*2,2);
            assert_eq!(deadlib_audio_core::music_asset_epoch(),epoch);assert_eq!(deadlib_audio_core::music_map_generation(),generation);
            // A fade edit during a device outage keeps the canonical frozen
            // callback cursor and is used by the resumed partial first loop.
            let partial=rig.pcm();assert!(partial.iter().any(|s|*s>0));dry_offset.set(dry_offset.get()+partial.len());
            assert!(rig.audio.editor_suspend_preview(rig.owner));rig.poll();let frozen=rig.audio.editor_preview_cursor(rig.owner,kind,&path).unwrap();
            // Cursor records consumed resampler mappings, not the idealized
            // unquantized Cut double. Fades must preserve its exact provenance.
            assert_eq!(frozen.next_music_seconds.to_bits(),dry_frozen_phase.to_bits(),"fades changed the actual rendered phase: hz={input_hz} dedicated={dedicated}");
            assert!(rig.audio.editor_set_preview_fades(rig.owner,0.,0.));cut.fade_in_sec=0.;cut.fade_out_sec=0.;
            assert!(rig.audio.resume_preview_after_output_lease(rig.owner,kind,path.clone(),cut,true,1.,frozen));rig.poll();rig.wait(|rig|rig.ready());check(&mut rig,0,0,2);
            let old_epoch=epoch;assert!(!deadlib_audio_core::editor_preview_envelope_enabled(old_epoch));
        }}
    }

    use super::*;
    use deadlib_audio_core::{
        CallbackClockSource, CallbackInfo, MixControls, OutputBufferMut, RenderState,
    };
    use std::{
        sync::mpsc::Receiver,
        time::{Duration, Instant},
    };
    #[test]
    fn editor_prepared_real_pcm_ignores_late_nonunity_replaygain_for_main_and_preview(){
        let _serial=crate::CLOCK_TEST_LOCK.lock().unwrap();
        let directory=directory("editor-replaygain");let path=directory.join("gain.wav");wav(&path,true);let mut rig=Rig::new();
        rig.audio.register_editor_asset(rig.owner,EditorAssetKind::Main,Some(path.clone()));rig.wait(|r|r.audio.editor_asset_ready(r.owner,EditorAssetKind::Main));
        let source=rig.audio.editor.assets.borrow().service.as_ref().unwrap().source(EditorAssetKind::Main).unwrap().clone();let dry=deadlib_audio_decode::preview::resample_prepared(&source,48000,&std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert!(rig.audio.editor_start_music(rig.owner,Some(path.clone()),0.,1.));rig.wait(|r|r.ready());
        let id=deadlib_audio_core::active_music_track_id();super::super::super::set_music_replaygain_if_matches(id,0.125);assert_eq!(deadlib_audio_core::music_target_gain(),1.);
        let mut actual=vec![0.;480];rig.render.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(&actual[..],&dry[..480]);
        assert!(rig.audio.editor_play_preview(rig.owner,EditorAssetKind::Main,path.clone(),Cut{start_sec:0.,length_sec:0.5,..Default::default()},true,1.));rig.wait(|r|r.ready());let id=deadlib_audio_core::active_music_track_id();super::super::super::set_music_replaygain_if_matches(id,2.);assert_eq!(deadlib_audio_core::music_target_gain(),1.);
        rig.render.render(OutputBufferMut::F32(&mut actual),CallbackInfo{anchor_nanos:deadlib_platform::host_time::instant_nanos(Instant::now()),clock:CallbackClockSource::Instant},[]);assert_eq!(&actual[..],&dry[..480],"preview actual PCM must retain unity ReplayGain");
        drop(rig);std::fs::remove_dir_all(directory).unwrap();
    }

    struct Rig {
        audio: AudioControl,
        commands: Receiver<StreamCommand>,
        runtime: stream::MusicStreamRuntime,
        clock: crate::MusicClock,
        render: RenderState,
        owner: u64,
    }
    impl Rig {
        fn new() -> Self {Self::at_rate(48_000)}
        fn at_rate(sample_rate_hz:u32) -> Self {
            let (mut audio, commands, _) = super::super::tests::control();
            audio.engine.as_mut().unwrap().output.sample_rate_hz=sample_rate_hz;
            let (stream, handle) = deadlib_audio_core::music_transport(sample_rate_hz, 2);
            let clock = crate::MusicClock::new(stream.played_map, sample_rate_hz);
            let render = RenderState::new(handle, Arc::new(MixControls::new()), 2);
            let runtime = stream::MusicStreamRuntime::new(
                stream.writer,
                OutputFormat {
                    sample_rate_hz,
                    channels: 2,
                },
            );
            audio.engine.as_mut().unwrap().decoder_status = runtime.decoder_status();
            let owner = 731;
            assert!(audio.begin_editor_audio(
                owner,
                EditorAudioGains {main:1.,
                    music: 1.,
                    assist: 1.,
                    beat: 1.
                }
            ));
            Self {
                audio,
                commands,
                runtime,
                clock,
                render,
                owner,
            }
        }
        fn poll(&mut self) {
            self.audio.poll_editor_assets_at_rendered_frame(&self.clock);
            while let Ok(command) = self.commands.try_recv() {
                self.runtime.handle(command);
            }
        }
        fn wait(&mut self, mut condition: impl FnMut(&Self) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !condition(self) {
                self.poll();
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        fn pcm(&mut self) -> Vec<i16> {
            let mut output = vec![0; 480];
            self.render.render(
                OutputBufferMut::I16(&mut output),
                CallbackInfo {
                    anchor_nanos: deadlib_platform::host_time::instant_nanos(Instant::now()),
                    clock: CallbackClockSource::Instant,
                },
                std::iter::empty(),
            );
            output
        }
        fn wait_watch(&mut self, kinds: &[EditorAssetKind]) {
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                self.poll();
                let ready = {
                    let assets = self.audio.editor.assets.borrow();
                    let service = assets.service.as_ref().unwrap();
                    kinds.iter().all(|kind| {
                        service.watch_ready(*kind)
                            && service.source(*kind).is_some()
                            && service
                                .current_load_token(*kind)
                                .is_some_and(|token| token.revision >= 2)
                    })
                };
                if ready {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "OS watch setup did not acknowledge arming"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        fn ready(&self) -> bool {
            self.runtime.decoder_status().snapshot().is_some_and(|s| {
                s.asset_epoch == deadlib_audio_core::music_asset_epoch()
                    && s.render_ready_generation == Some(deadlib_audio_core::music_map_generation())
            })
        }
    }
    impl Drop for Rig {
        fn drop(&mut self) {
            self.audio.end_editor_audio(self.owner);
            deadlib_audio_core::resume_music_track();
        }
    }
    fn wav(path: &std::path::Path, positive: bool) {
        let frames = 96_000u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + frames * 4).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&192_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(frames * 4).to_le_bytes());
        for frame in 0..frames {
            let value = if positive {
                1000 + (frame / 64) as i16
            } else {
                -3000i16
            };
            bytes.extend_from_slice(&value.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }
    fn directory(case: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "editor-prepared-{case}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn actual_last_write_main_delayed_reload_retains_source_now_then_paused_reload_resumes_new_asset()
     {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        deadlib_audio_core::resume_music_track();
        let directory = directory("main");
        let path = directory.join("main.wav");
        wav(&path, false);
        let mut rig = Rig::new();
        assert!(rig.audio.register_editor_asset(
            rig.owner,
            EditorAssetKind::Main,
            Some(path.clone())
        ));
        rig.wait(|rig| {
            rig.audio
                .editor_asset_ready(rig.owner, EditorAssetKind::Main)
        });
        rig.wait_watch(&[EditorAssetKind::Main]);
        assert!(
            rig.audio
                .editor_start_music(rig.owner, Some(path.clone()), 0., 1.)
        );
        rig.wait(|rig| rig.ready());
        assert!(rig.pcm().iter().any(|v| *v < 0));
        let generation = deadlib_audio_core::music_map_generation();
        let start = deadlib_audio_core::music_track_start_frame();
        let gain = deadlib_audio_core::music_target_gain();
        let epoch = deadlib_audio_core::music_asset_epoch();
        let (entered, release) = crate::editor_asset_service::gate_next_load(path.clone());
        wav(&path, true);
        rig.wait(|_| deadlib_audio_core::music_asset_epoch() > epoch);
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        for _ in 0..100 {
            rig.poll();
            assert!(rig.pcm().iter().all(|v| *v == 0));
        }
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
        assert_eq!(deadlib_audio_core::music_track_start_frame(), start);
        assert_eq!(deadlib_audio_core::music_target_gain(), gain);
        release.send(()).unwrap();
        rig.wait(|rig| rig.ready());
        let now = deadlib_audio_core::music_total_frames().saturating_sub(start) as f64 / 48_000.;
        let pcm = rig.pcm();
        let first = pcm.iter().find(|v| **v > 0).unwrap();
        assert!((*first - (1000 + (now * 48_000. / 64.) as i16)).abs() < 12);
        assert!(rig.audio.editor_pause_music(rig.owner, 0.75));
        rig.poll();
        let epoch = deadlib_audio_core::music_asset_epoch();
        wav(&path, false);
        rig.wait(|rig| {
            deadlib_audio_core::music_asset_epoch() > epoch
                && rig
                    .audio
                    .editor_asset_ready(rig.owner, EditorAssetKind::Main)
        });
        for _ in 0..20 {
            rig.poll();
            assert!(rig.pcm().iter().all(|v| *v == 0));
        }
        assert!(deadlib_audio_core::music_track_paused());
        assert!(
            rig.audio
                .editor_resume_music(rig.owner, path.clone(), 0.75, 1.)
        );
        rig.wait(|rig| rig.ready());
        assert!(rig.pcm().iter().any(|v| *v < -1000));
        assert_eq!(rig.audio.editor_music_owner(), Some(rig.owner));
        drop(rig);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn actual_preview_reload_after_many_loops_keeps_modulo_cut_and_inactive_main_cache() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        deadlib_audio_core::resume_music_track();
        let directory = directory("preview");
        let path = directory.join("preview.wav");
        let main = directory.join("main.wav");
        wav(&path, false);
        wav(&main, false);
        let mut rig = Rig::new();
        rig.audio
            .register_editor_asset(rig.owner, EditorAssetKind::Main, Some(main.clone()));
        rig.audio.register_editor_asset(
            rig.owner,
            EditorAssetKind::DedicatedPreview,
            Some(path.clone()),
        );
        rig.wait(|rig| {
            rig.audio
                .editor_asset_ready(rig.owner, EditorAssetKind::Main)
                && rig
                    .audio
                    .editor_asset_ready(rig.owner, EditorAssetKind::DedicatedPreview)
        });
        rig.wait_watch(&[EditorAssetKind::Main, EditorAssetKind::DedicatedPreview]);
        let cut = Cut {
            start_sec: 0.5,
            length_sec: 0.03,
            ..Default::default()
        };
        assert!(rig.audio.editor_play_preview(
            rig.owner,
            EditorAssetKind::Main,
            main.clone(),
            cut,
            true,
            1.
        ));
        rig.wait(|rig| rig.ready());
        let mut heard = 0;
        let deadline = Instant::now() + Duration::from_secs(5);
        while heard < 48_000 {
            rig.poll();
            heard += rig.pcm().chunks_exact(2).filter(|v| v[0] < -1000).count();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let generation = deadlib_audio_core::music_map_generation();
        let origin = deadlib_audio_core::music_track_start_frame();
        let oldepoch = deadlib_audio_core::music_asset_epoch();
        let (entered, release) = crate::editor_asset_service::gate_next_load(main.clone());
        wav(&main, true);
        rig.wait(|_| deadlib_audio_core::music_asset_epoch() > oldepoch);
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        for _ in 0..20 {
            rig.poll();
            assert!(rig.pcm().iter().all(|v| *v == 0));
        }
        release.send(()).unwrap();
        rig.wait(|rig| rig.ready());
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut heard = 0;
        while heard < 3000 {
            rig.poll();
            let output = rig.pcm();
            assert!(output.iter().all(|v| *v >= -200));
            for value in output.iter().filter(|v| **v > 200) {
                assert!(
                    *value >= 1350 && *value <= 1420,
                    "preview PCM must remain in original nonzero cut, not absolute main clock"
                );
                heard += 1;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
        assert_eq!(deadlib_audio_core::music_track_start_frame(), origin);
        assert!(rig.audio.editor_logical_phase().is_none());
        // Inactive dedicated preview reload must not invalidate the active main-preview epoch.
        let active_epoch = deadlib_audio_core::music_asset_epoch();
        let old = rig
            .audio
            .editor
            .assets
            .borrow()
            .service
            .as_ref()
            .unwrap()
            .source(EditorAssetKind::DedicatedPreview)
            .cloned()
            .unwrap();
        wav(&path, true);
        rig.wait(|rig| {
            rig.audio
                .editor
                .assets
                .borrow()
                .service
                .as_ref()
                .unwrap()
                .source(EditorAssetKind::DedicatedPreview)
                .is_some_and(|new| !Arc::ptr_eq(&old, new))
        });
        assert_eq!(deadlib_audio_core::music_asset_epoch(), active_epoch);
        drop(rig);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn dedicated_preview_reloads_after_multiple_full_loops_without_invalidating_inactive_main() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        deadlib_audio_core::resume_music_track();
        let directory = directory("dedicated");
        let main = directory.join("main.wav");
        let preview = directory.join("preview.wav");
        wav(&main, false);
        wav(&preview, false);
        let mut rig = Rig::new();
        rig.audio
            .register_editor_asset(rig.owner, EditorAssetKind::Main, Some(main.clone()));
        rig.audio.register_editor_asset(
            rig.owner,
            EditorAssetKind::DedicatedPreview,
            Some(preview.clone()),
        );
        rig.wait(|rig| {
            rig.audio
                .editor_asset_ready(rig.owner, EditorAssetKind::Main)
                && rig
                    .audio
                    .editor_asset_ready(rig.owner, EditorAssetKind::DedicatedPreview)
        });
        rig.wait_watch(&[EditorAssetKind::Main, EditorAssetKind::DedicatedPreview]);
        assert!(rig.audio.editor_play_preview(
            rig.owner,
            EditorAssetKind::DedicatedPreview,
            preview.clone(),
            Cut::default(),
            true,
            1.
        ));
        rig.wait(|rig| rig.ready());
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut heard = 0;
        while heard < 200_000 {
            rig.poll();
            heard += rig.pcm().chunks_exact(2).filter(|v| v[0] < -1000).count();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let epoch = deadlib_audio_core::music_asset_epoch();
        let generation = deadlib_audio_core::music_map_generation();
        let origin = deadlib_audio_core::music_track_start_frame();
        let (entered, release) = crate::editor_asset_service::gate_next_load(preview.clone());
        wav(&preview, true);
        rig.wait(|_| deadlib_audio_core::music_asset_epoch() > epoch);
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        for _ in 0..20 {
            rig.poll();
            assert!(rig.pcm().iter().all(|v| *v == 0));
        }
        release.send(()).unwrap();
        rig.wait(|rig| rig.ready());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            rig.poll();
            if rig.pcm().iter().any(|v| *v > 1000) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
        assert_eq!(deadlib_audio_core::music_track_start_frame(), origin);
        assert!(rig.audio.editor_logical_phase().is_none());
        let epoch = deadlib_audio_core::music_asset_epoch();
        let old = rig
            .audio
            .editor
            .assets
            .borrow()
            .service
            .as_ref()
            .unwrap()
            .source(EditorAssetKind::Main)
            .cloned()
            .unwrap();
        wav(&main, true);
        rig.wait(|rig| {
            rig.audio
                .editor
                .assets
                .borrow()
                .service
                .as_ref()
                .unwrap()
                .source(EditorAssetKind::Main)
                .is_some_and(|new| !Arc::ptr_eq(new, &old))
        });
        assert_eq!(deadlib_audio_core::music_asset_epoch(), epoch);
        // MAIN selection restores SourceClock policy and starts at its requested
        // source time; the previous preview epoch cannot leak its modulo mode.
        assert!(
            rig.audio
                .editor_start_music(rig.owner, Some(main), 1.25, 1.)
        );
        rig.wait(|rig| rig.ready());
        let first = rig.pcm();
        assert!(first.iter().any(|v| *v > 1900));
        assert!(rig.audio.editor_logical_phase().is_some());
        drop(rig);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn stop_during_preparation_keeps_inactive_cache_but_never_publishes_late_pcm() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        deadlib_audio_core::resume_music_track();
        let directory = directory("stop");
        let path = directory.join("main.wav");
        wav(&path, true);
        let (entered, release) = crate::editor_asset_service::gate_next_load(path.clone());
        let mut rig = Rig::new();
        assert!(
            rig.audio
                .editor_start_music(rig.owner, Some(path.clone()), 0., 1.)
        );
        rig.poll();
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(rig.audio.editor_stop_music(rig.owner));
        rig.poll();
        let stopped_epoch = deadlib_audio_core::music_asset_epoch();
        release.send(()).unwrap();
        rig.wait(|rig| {
            rig.audio
                .editor_asset_ready(rig.owner, EditorAssetKind::Main)
        });
        assert_eq!(deadlib_audio_core::music_asset_epoch(), stopped_epoch);
        assert!(rig.audio.editor_music_owner().is_none());
        assert!(rig.audio.editor_logical_phase().is_none());
        assert!(rig.runtime.decoder_status().snapshot().is_none());
        assert!(rig.pcm().iter().all(|v| *v == 0));
        drop(rig);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn no_device_asset_owner_loads_sources_without_claiming_audio_lease_or_progress() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        let directory = directory("no-device");
        let path = directory.join("main.wav");
        wav(&path, true);
        let (mut audio, _commands, _) = super::super::tests::control();
        audio.engine = None;
        let owner = 735;
        let generation = deadlib_audio_core::music_map_generation();
        let epoch = deadlib_audio_core::music_asset_epoch();
        assert!(!audio.begin_editor_audio(
            owner,
            EditorAudioGains {main:1.,
                music: 1.,
                assist: 1.,
                beat: 1.
            }
        ));
        assert_eq!(audio.editor_audio_status().owner, 0);
        assert!(audio.register_editor_asset(owner, EditorAssetKind::Main, Some(path.clone())));
        let (stream, _) = deadlib_audio_core::music_transport(48_000, 2);
        let mut clock = crate::MusicClock::new(stream.played_map, 48_000);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !audio.editor_asset_ready_for(owner, EditorAssetKind::Main, &path) {
            audio.poll_editor_assets(&mut clock);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!audio.editor_asset_ready_for(
            owner,
            EditorAssetKind::Main,
            &directory.join("other.wav")
        ));
        assert!(!audio.editor_start_music(owner, Some(path.clone()), 0., 1.));
        assert!(audio.editor_logical_phase().is_none());
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
        assert_eq!(deadlib_audio_core::music_asset_epoch(), epoch);
        assert!(audio.end_editor_audio(owner));
        assert!(!audio.editor_asset_ready(owner, EditorAssetKind::Main));
        assert!(!audio.register_editor_asset(owner, EditorAssetKind::Main, Some(path)));
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn explicit_main_unregistration_silences_pcm_without_stopping_logical_transport() {
        let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
        deadlib_audio_core::resume_music_track();
        let directory = directory("unload");
        let path = directory.join("main.wav");
        wav(&path, true);
        let mut rig = Rig::new();
        rig.audio
            .register_editor_asset(rig.owner, EditorAssetKind::Main, Some(path.clone()));
        rig.wait(|rig| {
            rig.audio
                .editor_asset_ready(rig.owner, EditorAssetKind::Main)
        });
        assert!(rig.audio.editor_start_music(rig.owner, Some(path), 0., 1.));
        rig.wait(|rig| rig.ready());
        assert!(rig.pcm().iter().any(|value| *value > 0));
        let generation = deadlib_audio_core::music_map_generation();
        let origin = deadlib_audio_core::music_track_start_frame();
        let epoch = deadlib_audio_core::music_asset_epoch();
        assert!(
            rig.audio
                .register_editor_asset(rig.owner, EditorAssetKind::Main, None)
        );
        rig.poll();
        assert!(deadlib_audio_core::music_asset_epoch() > epoch);
        assert_eq!(deadlib_audio_core::music_map_generation(), generation);
        assert_eq!(deadlib_audio_core::music_track_start_frame(), origin);
        assert!(rig.pcm().iter().all(|value| *value == 0));
        assert!(rig.audio.editor_logical_phase().is_some());
        assert!(rig.audio.decoder_status().is_none());
        assert!(
            !rig.audio
                .editor_asset_ready(rig.owner, EditorAssetKind::Main)
        );
        drop(rig);
        std::fs::remove_dir_all(directory).unwrap();
    }
#[test]
fn rendered_preview_freezes_exact_callback_cursor_and_resumes_ramp_without_outage_time() {
    let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
    let directory = directory("preview-recovery");
    let path = directory.join("preview.wav");
    wav(&path, true);
    let mut rig = Rig::new();
    let kind = EditorAssetKind::Main;
    assert!(
        rig.audio
            .register_editor_asset(rig.owner, kind, Some(path.clone()))
    );
    rig.wait(|rig| rig.audio.editor_asset_ready(rig.owner, kind));
    let cut = Cut {
        start_sec: 0.05,
        length_sec: 0.02,
        ..Default::default()
    };
    assert!(
        rig.audio
            .editor_play_preview(rig.owner, kind, path.clone(), cut, true, 1.)
    );
    rig.wait(|rig| rig.ready());
    for _ in 0..3 {
        assert!(rig.pcm().iter().any(|s| *s > 0));
    }
    let cursor = rig
        .audio
        .editor_preview_cursor(rig.owner, kind, &path)
        .unwrap();
    assert!(!cursor.suspended);
    assert!((cursor.next_music_seconds - 0.065).abs() < 1e-12);
    assert_eq!(
        cursor.rendered_frame,
        deadlib_audio_core::music_total_frames()
    );
    assert!(rig.audio.editor_suspend_preview(rig.owner));
    let frozen = rig
        .audio
        .editor_preview_cursor(rig.owner, kind, &path)
        .unwrap();
    assert!(frozen.suspended);
    assert_eq!(frozen.next_music_seconds, cursor.next_music_seconds);
    rig.poll();
    // Physical silence callbacks and arbitrary host delay cannot advance
    // the captured preview cursor. No PCM mapping/decoder ack is invented.
    for _ in 0..100 {
        assert!(rig.pcm().iter().all(|s| *s == 0));
        assert_eq!(
            rig.audio.editor_preview_cursor(rig.owner, kind, &path),
            Some(frozen)
        );
    }
    assert!(
        !rig.audio
            .editor_resume_preview(rig.owner, kind, path.clone(), cut, true, 1., frozen),
        "fake DSP rig owns no output restoration lease"
    );
    let epoch = deadlib_audio_core::music_asset_epoch();
    let mut wrong = frozen;
    wrong.owner += 1;
    assert!(!rig.audio.resume_preview_after_output_lease(
        rig.owner,
        kind,
        path.clone(),
        cut,
        true,
        1.,
        wrong
    ));
    wrong = frozen;
    wrong.asset_epoch += 1;
    assert!(!rig.audio.resume_preview_after_output_lease(
        rig.owner,
        kind,
        path.clone(),
        cut,
        true,
        1.,
        wrong
    ));
    assert_eq!(deadlib_audio_core::music_asset_epoch(), epoch);
    // This private continuation is the exact production body AFTER the
    // public physical-lease guard. Lease rejection above and native lease
    // projection tests are separate evidence, not a fake hardware receipt.
    assert!(rig.audio.resume_preview_after_output_lease(
        rig.owner,
        kind,
        path.clone(),
        cut,
        true,
        1.,
        frozen
    ));
    rig.wait(|rig| rig.ready());
    for callback in 0..8 {
        let pcm = rig.pcm();
        for (frame, channels) in pcm.chunks_exact(2).enumerate() {
            let source = 2400 + ((720 + callback * 240 + frame) % 960);
            let expected = 1000 + (source / 64) as i16;
            assert_eq!(
                channels,
                [expected, expected],
                "callback={callback},frame={frame}"
            );
        }
    }
    let new = rig
        .audio
        .editor_preview_cursor(rig.owner, kind, &path)
        .unwrap();
    assert!(!new.suspended);
    assert!(new.asset_epoch > frozen.asset_epoch);
    assert!(new.generation > frozen.generation);
    assert!(!rig.audio.editor_suspend_preview_for(rig.owner, frozen));
    assert!(!rig.audio.resume_preview_after_output_lease(
        rig.owner,
        kind,
        path.clone(),
        cut,
        true,
        1.,
        frozen
    ));
    drop(rig);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn suspended_preview_stop_or_changed_registration_revokes_old_resume() {
    let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
    for changed_registration in [false, true] {
        let directory = directory("preview-revoke");
        let path = directory.join("a.wav");
        let b = directory.join("b.wav");
        wav(&path, true);
        wav(&b, false);
        let mut rig = Rig::new();
        let kind = EditorAssetKind::DedicatedPreview;
        rig.audio
            .register_editor_asset(rig.owner, kind, Some(path.clone()));
        rig.wait(|rig| rig.audio.editor_asset_ready(rig.owner, kind));
        let cut = Cut::default();
        assert!(
            rig.audio
                .editor_play_preview(rig.owner, kind, path.clone(), cut, true, 1.)
        );
        rig.wait(|rig| rig.ready());
        rig.pcm();
        assert!(rig.audio.editor_suspend_preview(rig.owner));
        let cursor = rig
            .audio
            .editor_preview_cursor(rig.owner, kind, &path)
            .unwrap();
        if changed_registration {
            rig.audio.register_editor_asset(rig.owner, kind, Some(b));
        } else {
            rig.audio.editor_stop_music(rig.owner);
        }
        let epoch = deadlib_audio_core::music_asset_epoch();
        assert!(
            !rig.audio
                .resume_preview_after_output_lease(rig.owner, kind, path, cut, true, 1., cursor)
        );
        assert_eq!(deadlib_audio_core::music_asset_epoch(), epoch);
        drop(rig);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
#[test]
fn suspended_preview_reloads_same_registration_without_advancing_or_publishing_pcm() {
    let _serial = crate::CLOCK_TEST_LOCK.lock().unwrap();
    let directory = directory("preview-frozen-reload");
    let path = directory.join("preview.wav");
    wav(&path, true);
    let mut rig = Rig::new();
    let kind = EditorAssetKind::Main;
    rig.audio
        .register_editor_asset(rig.owner, kind, Some(path.clone()));
    rig.wait(|rig| rig.audio.editor_asset_ready(rig.owner, kind));
    rig.wait_watch(&[kind]);
    let cut = Cut {
        start_sec: 0.05,
        length_sec: 0.02,
        ..Default::default()
    };
    assert!(
        rig.audio
            .editor_play_preview(rig.owner, kind, path.clone(), cut, true, 1.)
    );
    rig.wait(|rig| rig.ready());
    rig.pcm();
    assert!(rig.audio.editor_suspend_preview(rig.owner));
    rig.poll();
    let frozen = rig
        .audio
        .editor_preview_cursor(rig.owner, kind, &path)
        .unwrap();
    let epoch = deadlib_audio_core::music_asset_epoch();
    let (entered, release) = crate::editor_asset_service::gate_next_load(path.clone());
    wav(&path, false);
    rig.wait(|rig| {
        rig.audio
            .editor
            .assets
            .borrow()
            .service
            .as_ref()
            .unwrap()
            .current_load_token(kind)
            .is_some_and(|token| token.revision > frozen.revision)
    });
    entered.recv_timeout(Duration::from_secs(3)).unwrap();
    for _ in 0..10 {
        rig.poll();
        assert!(rig.pcm().iter().all(|v| *v == 0));
    }
    assert_eq!(
        rig.audio.editor_preview_cursor(rig.owner, kind, &path),
        Some(frozen)
    );
    assert_eq!(deadlib_audio_core::music_asset_epoch(), epoch);
    release.send(()).unwrap();
    rig.wait(|rig| rig.audio.editor_asset_ready(rig.owner, kind));
    assert!(rig.pcm().iter().all(|v| *v == 0));
    assert!(
        rig.audio
            .resume_preview_after_output_lease(rig.owner, kind, path, cut, true, 1., frozen)
    );
    rig.wait(|rig| rig.ready());
    assert!(rig.pcm().iter().any(|v| *v < -1000));
    drop(rig);
    std::fs::remove_dir_all(directory).unwrap();
}
}

#[derive(Clone, Copy)]
struct SuspendedPreview {
    cursor: EditorPreviewCursor,
    suspension_epoch: u64,
    handoff: bool,
    exact_frame:Option<(i64,u32)>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorPreviewCursor {
    pub suspended: bool,
    /// False means a validated initial preview with zero rendered PCM.
    pub rendered: bool,
    pub owner: u64,
    pub kind: EditorAssetKind,
    pub registration: u64,
    pub revision: u64,
    pub generation: u64,
    pub asset_epoch: u64,
    pub next_music_seconds: f64,
    pub rendered_frame: u64,
    pub source_frame:Option<i64>,
    pub sample_rate_hz:u32,
}
impl AudioControl {
    pub fn editor_preview_cursor(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: &std::path::Path,
    ) -> Option<EditorPreviewCursor> {
        self.preview_cursor_with_policy(owner, kind, path, false, false)
    }
    fn preview_cursor_with_policy(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: &std::path::Path,
        allow_reset: bool,
        allow_unrendered: bool,
    ) -> Option<EditorPreviewCursor> {
        let assets = self.editor.assets.borrow();
        if owner == 0
            || assets.owner != owner
            || self.editor.state.get().owner != owner
            || !self.editor.music_owned.get()
        {
            return None;
        }
        if let Some(frozen) = assets.preview_suspended {
            return (frozen.cursor.owner == owner
                && frozen.cursor.kind == kind
                && frozen.suspension_epoch == deadlib_audio_core::music_asset_epoch()
                && frozen.cursor.generation == deadlib_audio_core::music_map_generation())
            .then_some(frozen.cursor);
        }
        self.engine.as_ref()?;
        let active = assets.active.filter(|a| !a.main && a.kind == kind)?;
        let _ = active;
        let (token, generation, epoch) = assets.preview_authority?;
        let service = assets.service.as_ref()?;
        let reset = allow_reset
            && assets.pending.is_some_and(|pending| {
                pending.token.registration == token.registration
                    && pending.epoch == deadlib_audio_core::music_asset_epoch()
            });
        if (!reset
            && (service.current_load_token(kind) != Some(token)
                || epoch != deadlib_audio_core::music_asset_epoch()))
            || !service
                .path(kind)
                .is_some_and(|held| retained_music_paths_equal(held, path))
            || generation != deadlib_audio_core::music_map_generation()
        {
            return None;
        }
        let cursor=deadlib_audio_core::rendered_music_cursor().filter(|cursor|cursor.generation==generation&&cursor.asset_epoch==epoch);
        let callback=deadlib_audio_core::preview_callback_cursor().filter(|cursor|cursor.generation==generation&&cursor.asset_epoch==epoch);
        if cursor.is_none()&&callback.is_none()&&!allow_unrendered{return None;}
        Some(EditorPreviewCursor {
            suspended: false,
            rendered:cursor.is_some(),
            owner,
            kind,
            registration: token.registration.registration,
            revision: token.revision,
            generation,
            asset_epoch: epoch,
            next_music_seconds: callback.map(|cursor|cursor.next_music_seconds).or_else(||cursor.map(|cursor|cursor.next_music_seconds)).unwrap_or_else(||deadlib_audio_core::editor_preview_initial_frame(generation,epoch).map(|frame|frame as f64/f64::from(self.engine.as_ref().map_or(1,|engine|engine.output.sample_rate_hz.max(1)))).unwrap_or(active.cut.start_sec.max(0.))),
            rendered_frame: callback.map(|cursor|cursor.callback_frame).or_else(||cursor.map(|cursor|cursor.rendered_frame)).unwrap_or_else(deadlib_audio_core::music_total_frames),
            source_frame:callback.map(|cursor|cursor.source_frame),
            sample_rate_hz:callback.map(|cursor|cursor.sample_rate_hz).unwrap_or_else(||self.engine.as_ref().map_or(0,|engine|engine.output.sample_rate_hz)),
        })
    }
    pub(in crate::runtime) fn freeze_editor_preview_before_handoff(&self) {
        let owner = self.editor.state.get().owner;
        if self.editor_suspend_preview(owner) {
            if let Some(frozen) = self.editor.assets.borrow_mut().preview_suspended.as_mut() {
                frozen.handoff = true;
            }
        }
    }
    pub(in crate::runtime) fn restore_preview_after_output_handoff(&self) {
        let request = {
            let assets = self.editor.assets.borrow();
            assets
                .preview_suspended
                .filter(|frozen| frozen.handoff)
                .and_then(|frozen| {
                    assets.active.and_then(|active| {
                        assets
                            .service
                            .as_ref()
                            .and_then(|service| service.path(active.kind))
                            .map(|path| (frozen.cursor, active, path.to_path_buf()))
                    })
                })
        };
        if let Some((cursor, active, path)) = request {
            let _ = self.editor_resume_preview(
                cursor.owner,
                active.kind,
                path,
                active.cut,
                active.looping,
                self.music_request.get().unwrap_or(1.),
                cursor,
            );
        }
    }
    pub fn editor_suspend_preview_for(&self, owner: u64, expected: EditorPreviewCursor) -> bool {
        let valid = {
            let assets = self.editor.assets.borrow();
            assets.owner == owner
                && expected.owner == owner
                && assets
                    .preview_authority
                    .is_some_and(|(token, generation, epoch)| {
                        token.registration.kind == expected.kind
                            && token.registration.registration == expected.registration
                            && token.revision == expected.revision
                            && generation == expected.generation
                            && epoch == expected.asset_epoch
                    })
        };
        valid && self.editor_suspend_preview(owner)
    }
    pub fn editor_suspend_preview(&self, owner: u64) -> bool {
        let request = {
            let assets = self.editor.assets.borrow();
            if assets.preview_suspended.is_some() {
                return assets.owner == owner;
            }
            let Some(active) = assets.active.filter(|a| !a.main) else {
                return false;
            };
            let Some(path) = assets
                .service
                .as_ref()
                .and_then(|service| service.path(active.kind))
                .map(std::path::Path::to_path_buf)
            else {
                return false;
            };
            (active.kind, path)
        };
        let Some(mut cursor) = self.preview_cursor_with_policy(owner, request.0, &request.1, true, true)
        else {
            return false;
        };
        if let Some(completed)=deadlib_audio_core::freeze_rendered_music_cursor(cursor.generation,cursor.asset_epoch){
            cursor.rendered=true;cursor.next_music_seconds=completed.next_music_seconds;cursor.rendered_frame=completed.rendered_frame;
        }
        if let Some(callback)=deadlib_audio_core::preview_callback_cursor().filter(|callback|callback.generation==cursor.generation&&callback.asset_epoch==cursor.asset_epoch){
            cursor.next_music_seconds=callback.next_music_seconds;cursor.rendered_frame=callback.callback_frame;
            cursor.source_frame=Some(callback.source_frame);cursor.sample_rate_hz=callback.sample_rate_hz;
        }
        let exact_frame=cursor.source_frame.or_else(||deadlib_audio_core::editor_preview_initial_frame(cursor.generation,cursor.asset_epoch)).and_then(|frame|(cursor.sample_rate_hz>0).then_some((frame,cursor.sample_rate_hz)));
        cursor.suspended = true;
        let suspension_epoch = deadlib_audio_core::advance_music_preview_asset_epoch();
        self.editor.assets.borrow_mut().preview_suspended = Some(SuspendedPreview {
            cursor,
            suspension_epoch,
            handoff: false,
            exact_frame,
        });
        self.editor.assets.borrow_mut().pending = None;
        deadlib_audio_core::pause_music_track();
        if let Some(engine) = self.engine.as_ref() {
            let _ = engine.command_sender.send(StreamCommand::ClearMusicAsset {
                asset_epoch: suspension_epoch,
            });
        }
        true
    }
    pub fn editor_resume_preview(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: PathBuf,
        cut: Cut,
        looping: bool,
        rate: f32,
        cursor: EditorPreviewCursor,
    ) -> bool {
        if !self.editor_output_is_leased(owner) {
            return false;
        }
        self.resume_preview_after_output_lease(owner, kind, path, cut, looping, rate, cursor)
    }
    fn resume_preview_after_output_lease(
        &self,
        owner: u64,
        kind: EditorAssetKind,
        path: PathBuf,
        cut: Cut,
        looping: bool,
        rate: f32,
        cursor: EditorPreviewCursor,
    ) -> bool {
        if self.engine.is_none()
            || owner == 0
            || cursor.owner != owner
            || cursor.kind != kind
            || !cursor.next_music_seconds.is_finite()
        {
            return false;
        }
        let source = {
            let assets = self.editor.assets.borrow();
            let Some(active) = assets.active else {
                return false;
            };
            let Some((token, generation, epoch)) = assets.preview_authority else {
                return false;
            };
            let Some(frozen) = assets.preview_suspended else {
                return false;
            };
            let same_cut = |a: Cut, b: Cut| {
                a.start_sec.to_bits() == b.start_sec.to_bits()
                    && a.length_sec.to_bits() == b.length_sec.to_bits()
                    && a.fade_in_sec.to_bits() == b.fade_in_sec.to_bits()
                    && a.fade_out_sec.to_bits() == b.fade_out_sec.to_bits()
            };
            if assets.owner != owner
                || self.editor.state.get().owner != owner
                || !self.editor.music_owned.get()
                || active.main
                || active.kind != kind
                || active.looping != looping
                || !same_cut(active.cut, cut)
                || cursor.registration != token.registration.registration
                || cursor.revision != token.revision
                || !cursor.suspended
                || frozen.cursor.generation != cursor.generation
                || frozen.cursor.asset_epoch != cursor.asset_epoch
                || cursor.generation != generation
                || cursor.asset_epoch != epoch
                || generation != deadlib_audio_core::music_map_generation()
                || frozen.suspension_epoch != deadlib_audio_core::music_asset_epoch()
            {
                return false;
            }
            let Some(service) = assets.service.as_ref() else {
                return false;
            };
            if service.current_load_token(kind).map(|t| t.registration) != Some(token.registration)
                || !service
                    .path(kind)
                    .is_some_and(|held| retained_music_paths_equal(held, &path))
            {
                return false;
            }
            let Some(source) = service.source(kind).cloned() else {
                return false;
            };
            source
        };
        let length = if kind == EditorAssetKind::DedicatedPreview {
            source.frames() as f64 / f64::from(source.sample_rate_hz)
        } else {
            cut.length_sec
        };
        if !length.is_finite() || length <= 0. {
            return false;
        }
        let frozen = self
            .editor
            .assets
            .borrow()
            .preview_suspended
            .unwrap()
            .cursor;
        let hz=self.engine.as_ref().unwrap().output.sample_rate_hz.max(1);
        let exact=self.editor.assets.borrow().preview_suspended.unwrap().exact_frame;
        let initial_frame=if let Some((frame,old_hz))=exact{
            let Some(frame)=preview_frame_at_rate(frame,old_hz,hz)else{return false;};frame
        }else{(frozen.next_music_seconds*f64::from(hz))as i64};
        let phase=initial_frame as f64/f64::from(hz);
        // A resumed output gets a new authority, but the old frozen cursor must be
        // validated BEFORE changing it. The new physical counter anchors zero gap.
        if !self.invalidate_editor_ticks(owner){return false;}
        let _=rate;let rate = 1.0f32;
        self.music_request.set(Some(rate));
        let _ = reset_music_stream_clock();
        let epoch = deadlib_audio_core::advance_music_preview_asset_epoch();
        deadlib_audio_core::activate_music_track();
        deadlib_audio_core::resume_music_track();
        deadlib_audio_core::seed_music_asset_clock(phase, rate);
        deadlib_audio_core::initialize_editor_preview_clock(deadlib_audio_core::music_map_generation(),epoch,initial_frame,deadlib_audio_core::music_total_frames());
        self.editor.pending_asset.set(None);
        self.editor.logical_music.set(false);
        self.editor.paused_position.set(None);
        self.editor.assets.borrow_mut().preview_suspended = None;
        self.publish_editor_source(
            source,
            path,
            Active {
                kind,
                main: false,
                cut,
                looping,
            },
            epoch,
            Some((phase, deadlib_audio_core::music_total_frames())),
            rate,
        );
        true
    }
}
