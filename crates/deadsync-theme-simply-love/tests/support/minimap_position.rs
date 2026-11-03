use deadlib_platform::input::{KeyCode,RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::minimap_position_preferences::{Mount,Preferences};
use deadsync_theme_simply_love::{i18n,screens::edition::State};
use std::{fs,time::Instant};
fn event(state:&mut State,code:KeyCode,pressed:bool) {state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:Instant::now(),host_nanos:0,
    });}
fn press(state:&mut State,code:KeyCode) {event(state,code,true);event(state,code,false);}
fn setup()->(tempfile::TempDir,State,u64) {
 i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
 deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0,
    ));
 let dir=tempfile::tempdir().unwrap();let song=dir.path().join("song.ssc");
 let notes=std::iter::repeat_n("1000\n0000\n0000\n0000",64).collect::<Vec<_>>().join(",");
 fs::write(&song,format!("#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{notes};")).unwrap();
 fs::write(dir.path().join("music.wav"),b"command-only asset").unwrap();
 let prefs=dir.path().join("preferences.json");fs::write(&prefs, b"{\"Version\":1,\"SnapIndex\":1}").unwrap();
 let mut state=State::open(&song,0).unwrap();state.configure_audio_preferences(&prefs).unwrap();
    state.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
    );let owner=publish_audio_fixture(&mut state);(dir,state,owner)
}
fn advanced(state:&mut State) {
 event(state,KeyCode::SuperLeft,true);event(state,KeyCode::ShiftLeft,true);press(state,KeyCode::KeyM);
 event(state,KeyCode::ShiftLeft,false);event(state,KeyCode::SuperLeft,false);
 press(state,KeyCode::ArrowUp);press(state,KeyCode::Enter);
}
#[test]
fn four_mounts_use_current_native_bounds_and_mesh_offsets_in_both_languages() {
 let (_dir,mut state, _owner)=setup();
 for mount in Mount::ALL {
  let mut preferences=Preferences::default();preferences.mount=mount;preferences.set_offset(mount,10);
  state.configure_minimap_position_preferences(preferences);
  let bounds=state.minimap_mount_bounds().unwrap();assert_eq!(bounds.y, 141);
  assert_eq!(bounds.visible_height, 254);
  let expected=match mount {Mount::RightSideOfWindow=>754,Mount::LeftSideOfWindow=>10,_=>641,
        };
  assert_eq!(bounds.x,expected);
  let mut actors=Vec::new();state.actors(&mut actors);
  assert!(actors.iter().any(|actor|matches!(actor,Actor::Mesh{offset,z:8011,..} if offset[0]==bounds.x as f32)));
  assert!(!state.workspace().file().session.is_dirty());
 }
}
#[test]
fn advanced_keyboard_draft_cancel_then_save_updates_source_groups_only() {
 let (dir,mut state, _owner)=setup();let before=state.minimap_position_preferences();
 advanced(&mut state);press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::ArrowRight);
 press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::ArrowRight);
 assert_eq!(state.minimap_position_preferences(),before);
 press(&mut state,KeyCode::Escape);assert_eq!(state.minimap_position_preferences(),before);
 advanced(&mut state);press(&mut state,KeyCode::ArrowRight); // Hide, draft only.
 for _ in 0..14 {press(&mut state,KeyCode::ArrowDown);}
 press(&mut state,KeyCode::Enter);
 let json:serde_json::Value=serde_json::from_slice(&fs::read(dir.path().join("preferences.json")).unwrap()).unwrap();
 assert_eq!(json["MiniMap"]["ShowMiniMap"],false);
 assert!(json["Options"]["MiniMapMaxNotesToDraw"].is_u64());
 let mut actors=Vec::new();state.actors(&mut actors);
 assert!(!actors.iter().any(|actor|matches!(actor,Actor::Mesh{z:8011,..})));
 assert!(!state.workspace().file().session.is_dirty());
}

fn focus_minimap(state:&mut State) {
 event(state,KeyCode::SuperLeft,true);press(state,KeyCode::KeyM);event(state,KeyCode::SuperLeft,false);
}
#[test]
fn selected_area_mode_changes_distant_destination_but_preserves_small_step_navigation() {
 use deadsync_editor::minimap_position_preferences::SelectMode;
 let mut rows=Vec::new();
 for mode in [SelectMode::MoveToCursor,SelectMode::MoveToSelectedArea] {
  let (dir,mut state, _owner)=setup();
  let path=dir.path().join("preferences.json");
  fs::write(&path,b"{\"Version\":1,\"MiniMap\":{\"MiniMapVisibleTimeRange\":10,\"MiniMapVisibleRowRange\":1920}}").unwrap();
  state.configure_audio_preferences(&path).unwrap();
        state.configure_spacing_preference(
            deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
        );
  let mut preferences=state.minimap_position_preferences();preferences.select_mode=mode;
  state.configure_minimap_position_preferences(preferences);
  focus_minimap(&mut state);press(&mut state,KeyCode::End);
  rows.push(state.workspace().row());
  assert!(!state.workspace().file().session.is_dirty());
 }
 assert_ne!(rows[0],rows[1],"recognized source select modes must not be silently equivalent on a scrolling map");
 let (_dir,mut state, _owner)=setup();let mut preferences=state.minimap_position_preferences();preferences.select_mode=SelectMode::MoveToSelectedArea;state.configure_minimap_position_preferences(preferences);
 focus_minimap(&mut state);let initial=state.workspace().row();press(&mut state,KeyCode::ArrowDown);
 assert!(state.workspace().row()>initial,"a single small keyboard step must remain useful");
}

#[test]
fn selected_area_capture_extends_active_source_range_and_resumes_exact_destination() {
 use deadsync_editor::minimap_position_preferences::SelectMode;
 use deadsync_gameplay::{GameplayAudioCommand,GameplayAudioSnapshot};
 let (_dir,mut state,owner)=setup();
 let mut preferences=state.minimap_position_preferences();preferences.select_mode=SelectMode::MoveToSelectedArea;
 state.configure_minimap_position_preferences(preferences);
 press(&mut state,KeyCode::Space);press(&mut state,KeyCode::ArrowDown);
 let selection=state.workspace().selection();assert_eq!(selection,Some((0,48)));
 press(&mut state,KeyCode::Enter);state.take_audio_commands();
 let mut snapshot=GameplayAudioSnapshot::default();snapshot.stream_clock.valid_at=Instant::now();snapshot.stream_clock.has_music_mapping=true;snapshot.stream_clock.music_nanos=626_000_000;
 state.update_audio(snapshot);let before=state.chart_focus_time().unwrap();
 focus_minimap(&mut state);event(&mut state,KeyCode::ArrowDown,true);
 assert!(!state.is_playing());assert_eq!(state.take_audio_commands(),vec![GameplayAudioCommand::EditorPauseMusic {owner,
            seconds: 0.626
        }]);
 let destination=state.chart_focus_time().unwrap();assert!(destination>before);
 assert_eq!(state.workspace().selection(),
        Some((selection.unwrap().0, state.workspace().row()))
    );
 event(&mut state,KeyCode::ArrowDown,false);assert!(state.is_playing());
 let commands=state.take_audio_commands();assert_eq!(commands.len(),1);
 assert!(matches!(&commands[0],GameplayAudioCommand::EditorResumeMusic{owner:actual,seconds,..} if *actual==owner && (*seconds-destination).abs()<1e-10));
 assert!(!state.workspace().file().session.is_dirty());
}


fn publish_audio_fixture(state:&mut State)->u64 {
    let commands=state.take_audio_commands();
    let owners:Vec<_>=commands.iter().filter_map(|c|match c{deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio{owner,..}=>Some(*owner),_=>None,
        }).collect();
    assert_eq!(owners.len(),1,"one published lazy session");
    assert_ne!(owners[0],0);
    let mut snapshot = deadsync_gameplay::GameplayAudioSnapshot::default();
    let paths = state.editor_asset_paths();
    snapshot.editor.assets = deadsync_gameplay::EditorAssetReadiness {
        owner: owners[0],
        registration: state.editor_asset_registration(),
        main: paths[0].is_some(),
        dedicated_preview: paths[1].is_some(),
    };
    state.update_audio(snapshot);
    assert!(!state.take_audio_commands().iter().any(|c|matches!(c,deadsync_gameplay::GameplayAudioCommand::EditorInitializeOutput{..})),"unavailable command-only host needs no DSP request");
    owners[0]
}
