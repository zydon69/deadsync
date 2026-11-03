//! Explicit review navigation belongs to the generated source clock. Publication
//! itself preserves focus; only these user actions retarget the musical instant.
use super::*;
impl State {
    pub(super) fn navigate_generation_preview(&mut self,key:KeyCode)->Result<(),deadsync_editor::Error>{
        let current=self.preview_position()?.ok_or_else(||deadsync_editor::Error::InvalidEdit("No generated preview position".into()))?;
        let preview=self.preview.as_ref().expect("captured preview position");
        let row=match key{
            KeyCode::Home=>0.0,
            KeyCode::End=>preview.grid().last_row()as f64,
            KeyCode::PageUp|KeyCode::PageDown=>{
                let level=deadsync_editor::snap::Level::native(self.workspace.snap())?;
                let spacing=if level.rows()==0{
                    deadsync_editor::timing_edit::signature::Signatures::from_document(self.workspace.file().session.document(),current.chart())?.measure_position(current.row().max(0.0).floor()as u64)?.rows_per_measure
                }else{level.rows()};
                let direction=if key==KeyCode::PageUp{-16.0}else{16.0};
                (current.row()+direction*spacing as f64).max(0.0)
            },
            _=>return Ok(()),
        };
        if !row.is_finite()||row>i32::MAX as f64{return Err(deadsync_editor::Error::InvalidEdit("Snap position outside chart range".into()));}
        let target=deadsync_editor::chart_focus::Position::at_fractional_row(&self.workspace.file().session,current.chart(),row)?;
        self.navigate_generation_preview_focus(target)
    }
}

#[cfg(test)]
mod source_context_navigation_tests{
    use super::*;
    use deadsync_gameplay::{GameplayAudioCommand as Audio,GameplayAudioSnapshot};
    use std::time::{Instant,Duration};
    fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:Instant::now(),host_nanos:0});}}
    fn fixture(language:&str,mode:u8)->(tempfile::TempDir,PathBuf,Vec<u8>,State){
        i18n::init(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let path=root.path().join("source.ssc");std::fs::write(root.path().join("main.wav"),b"command-level media fixture").unwrap();
        let bytes=b"#VERSION:0.83;#MUSIC:main.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:9;#TIMESIGNATURES:0=4=4;#NOTES:1000\n0100\n0010\n0001;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:5;#OFFSET:0.75;#BPMS:0=60;#TIMESIGNATURES:0=3=4;#NOTES:0001\n1000\n0100\n0010;".to_vec();std::fs::write(&path,&bytes).unwrap();let mut state=State::open(&path,0).unwrap();
        state.chart_views_apply(super::super::chart_views::ViewAction::Pin);assert!(state.focus_chart_at_time(1,0.));state.chart_views_apply(super::super::chart_views::ViewAction::Pin);assert!(state.focus_chart_at_time(0,0.));
        state.navigate_exact_position(96.5,true);state.workspace.execute(Command::SetSnap(deadsync_editor::snap::LEGACY_OFF_INDEX)).unwrap();state.scroll_preferences.set_stop_playback(false);
        if mode>0{
            state.take_audio_commands();let owner=state.editor_asset_owner();let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);state.update_audio(snapshot);state.take_audio_commands();state.toggle_audio();assert!(state.chart_transport_is_playing());state.take_audio_commands();state.bind_decoder_generation(Some(17));state.bind_decoder_asset_epoch(Some(owner),Some(29));
            snapshot.editor.logical_music=Some(deadsync_gameplay::EditorLogicalMusicClock{generation:17,asset_epoch:29,music_nanos:1_005_208_333});snapshot.stream_clock.valid_at=Instant::now();state.update_audio(snapshot);state.take_audio_commands();if mode==1{state.toggle_audio();assert!(state.paused_playback.is_some());}
        }
        state.take_audio_commands();(root,path,bytes,state)
    }
    fn publish_b(state:&mut State){
        let task=state.workspace.generation_task_at_with_profiles(1,deadsync_editor::generation::job::ChartOptions::balanced("dance-double"),None,Default::default()).unwrap();state.job=Some(GenerationJob::Chart(task.spawn().unwrap()));state.panel=Some(Panel::Generating);let deadline=Instant::now()+Duration::from_secs(20);while state.job.is_some(){state.update();assert!(Instant::now()<deadline,"{}",state.status);std::thread::yield_now();}assert!(matches!(state.preview,Some(GenerationPreview::Chart(_))),"{}",state.status);
    }
    #[test]
    fn physical_source_b_home_end_pages_retarget_a_and_transport_without_mutating_sources_en_fr(){
        deadlib_present::space::with_metrics(deadlib_present::space::Metrics::centered(1100.,760.),||{
            for language in ["en","fr"]{for mode in 0..3{for apply in [false,true]{
                let(_root,path,disk,mut state)=fixture(language,mode);let original=state.workspace.file().session.serialize().unwrap();let row=state.workspace.row();let instant=state.chart_focus_time().unwrap();let owner=state.editor_asset_owner();publish_b(&mut state);assert_eq!(state.workspace.active_chart(),Some(0));assert_eq!(state.workspace.row(),row);assert_eq!(state.chart_focus_time().unwrap().to_bits(),instant.to_bits());
                for key in [KeyCode::PageDown,KeyCode::Home,KeyCode::End,KeyCode::PageUp]{
                    let current=state.preview_position().unwrap().unwrap();let row=match key{KeyCode::Home=>0.,KeyCode::End=>state.preview.as_ref().unwrap().grid().last_row()as f64,KeyCode::PageDown=>current.row()+16.*144.,_ =>(current.row()-16.*144.).max(0.)};
                    let b=deadsync_editor::chart_focus::Position::at_fractional_row(&state.workspace.file().session,1,row).unwrap();let a=deadsync_editor::chart_focus::Position::at_time(&state.workspace.file().session,0,b.music_time()).unwrap();state.take_audio_commands();press(&mut state,key);
                    let shown=state.preview_position().unwrap().unwrap();assert_eq!(shown.chart(),1);assert_eq!(shown.row().to_bits(),row.to_bits(),"{language} mode{mode} {key:?}");assert_eq!(shown.music_time().to_bits(),b.music_time().to_bits());assert_eq!(state.workspace.active_chart(),Some(0));assert_eq!(state.workspace.row(),a.edit_row());let focused=state.focus_position.as_ref().unwrap();assert_eq!(focused.row().to_bits(),a.row().to_bits());assert_eq!(state.chart_focus_time().unwrap().to_bits(),b.music_time().to_bits());assert_eq!(state.workspace.file().session.serialize().unwrap(),original);
                    let commands=state.take_audio_commands();assert!(!commands.iter().any(|command|matches!(command,Audio::EditorStartMusic{..}|Audio::EditorStopMusic{..}|Audio::EditorPauseMusic{..}|Audio::EditorSwapMusicAsset{..}|Audio::EditorClearMusicAsset{..})));
                    if mode==2{assert!(state.chart_transport_is_playing());assert_eq!(state.chart_audio_generation(),Some(17));assert_eq!(state.playback_asset_epoch(),Some(29));assert_eq!(commands.iter().filter(|command|matches!(command,Audio::EditorSeekMusic{owner:actual,seconds}if *actual==owner&&seconds.to_bits()==(b.music_time()+state.audio_preferences.effective().offset).to_bits())).count(),1,"one owner-scoped exact B seek for {language} {key:?}: {commands:?}");assert!(!commands.iter().any(|command|matches!(command,Audio::SeekMusic(_))),"published commands must be owner scoped");}else{assert!(!state.is_playing());assert!(!commands.iter().any(|command|matches!(command,Audio::SeekMusic(_)|Audio::EditorSeekMusic{..})),"stopped/paused navigation must not seek the decoder: {commands:?}");assert_eq!(state.paused_playback.is_some(),mode==1);}
                    let viewport=state.chart_viewport();for geometry in state.visible_view_geometry().into_iter().filter(|geometry|!geometry.focused){let plan=state.source_viewport_plan(geometry,viewport).unwrap();let committed=state.workspace.file().session.loaded_grid(geometry.source).unwrap();assert!(plan.admission.notes().all(|position|committed.get(position).is_some()));}
                }
                let current=state.chart_focus_time().unwrap();press(&mut state,if apply{KeyCode::Enter}else{KeyCode::Escape});assert!(state.preview.is_none(),"{}",state.status);assert_eq!(state.editor_asset_owner(),owner);assert_eq!(state.chart_focus_time().unwrap().to_bits(),current.to_bits());assert_eq!(state.workspace.active_chart(),Some(if apply{2}else{0}));if !apply{assert_eq!(state.workspace.file().session.serialize().unwrap(),original);}
                if mode==1{assert!(state.paused_playback.is_some());state.take_audio_commands();state.toggle_audio();assert!(state.chart_transport_is_playing());assert!(state.take_audio_commands().iter().any(|command|matches!(command,Audio::EditorResumeMusic{seconds,..}if seconds.to_bits()==(current+state.audio_preferences.effective().offset).to_bits())));}
                assert_eq!(std::fs::read(path).unwrap(),disk);
            }}}
        });
    }
    #[test]
    fn explicit_preview_navigation_honors_scroll_stop_policy_but_still_uses_b_clock(){
        deadlib_present::space::with_metrics(deadlib_present::space::Metrics::centered(1100.,760.),||{
            let(_root,_path,_disk,mut state)=fixture("en",2);publish_b(&mut state);state.scroll_preferences.set_stop_playback(true);state.take_audio_commands();press(&mut state,KeyCode::Home);assert!(!state.is_playing());assert_eq!(state.preview_position().unwrap().unwrap().row(),0.);let b=deadsync_editor::chart_focus::Position::at_row(&state.workspace.file().session,1,0).unwrap();assert_eq!(state.chart_focus_time().unwrap().to_bits(),b.music_time().to_bits());assert!(state.take_audio_commands().iter().any(|command|matches!(command,Audio::EditorPauseMusic{..})));
        });
    }
}
