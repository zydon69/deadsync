use super::super::{Panel, RawKeyboardEvent};
use super::*;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    key_binding_preferences::Preferences, playback::Clock as MusicClock, song_text::Field,
};
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
pub(in crate::screens::edition) fn fixture(language: &str) -> (tempfile::TempDir, State, u64) {
    crate::i18n::init(deadsync_assets::language::load_for_tests(language));
    let root = tempfile::tempdir().unwrap();
    for path in ["main.wav", "next.wav", "override.wav"] {
        std::fs::write(root.path().join(path), b"existing native asset descriptor").unwrap();
    }
    let path = root.path().join("song.ssc");
    std::fs::write(&path,b"#VERSION:0.83;#MUSIC:main.wav;#OFFSET:0.125;#SAMPLESTART:0;#SAMPLELENGTH:2;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
    let mut s = State::open(&path, 0).unwrap();
    s.configure_key_binding_preferences(Arc::new(Preferences::default()));
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
    s.bind_decoder_generation(Some(71));
    s.bind_decoder_asset_epoch(Some(owner), Some(83));
    let mut snapshot = GameplayAudioSnapshot::default();
    snapshot.editor.owner = owner;
    snapshot.editor.output = s.audio_output_snapshot();
    snapshot.stream_clock.valid_at = Instant::now();
    snapshot.stream_clock.has_music_mapping = true;
    snapshot.stream_clock.music_nanos = 1_250_000_000;
    snapshot.stream_clock.music_seconds_per_second = 1.;
    snapshot.stream_clock.stream_seconds = 12.;
    snapshot.editor.logical_music = Some(deadsync_gameplay::EditorLogicalMusicClock {
        generation: 71,
        asset_epoch: 83,
        music_nanos: 1_250_000_000,
    });
    s.update_audio(snapshot);
    s.take_audio_commands();
    assert!(s.chart_transport_is_playing());
    (root, s, owner)
}
fn open(s: &mut State) {
    press(s, KeyCode::F11);
    assert!(
        matches!(s.panel, Some(Panel::SongProperties(0))),
        "{}",
        s.status
    );
}
fn apply(s: &mut State) {
    s.panel = Some(Panel::SongProperties(1));
    press(s, KeyCode::Enter);
    assert!(s.song_properties.is_none(), "{}", s.status);
    assert!(s.panel.is_none());
}
pub(in crate::screens::edition) fn no_stop(commands: &[GameplayAudioCommand]) {
    assert!(
        !commands.iter().any(|c| matches!(
            c,
            GameplayAudioCommand::EditorStopMusic { .. }
                | GameplayAudioCommand::EditorPauseMusic { .. }
                | GameplayAudioCommand::EditorResumeMusic { .. }
                | GameplayAudioCommand::EditorStartMusic { .. }
                | GameplayAudioCommand::EditorEndAudio { .. }
        )),
        "{commands:?}"
    );
}
pub(in crate::screens::edition) fn row(s: &State) -> f64 {
    s.capture_timing_presentation().unwrap().row()
}
#[test]
fn raw_f11_numeric_offsets_retime_at_same_row_without_stopping_main_or_losing_pause_en_fr() {
    for language in ["en", "fr"] {
        for paused in [false, true] {
            for (index, value) in [(0, 0.375), (4, 0.25)] {
                let (_root, mut s, owner) = fixture(language);
                if paused {
                    s.toggle_audio();
                    assert!(s.paused_playback.is_some());
                }
                let before_row = row(&s);
                let origin = s.save_as_transport_stamp().1;
                let before_generation = s.chart_audio_generation();
                let before_epoch = s.playback_asset_epoch();
                let before_bytes = s.workspace.file().session.serialize().unwrap();
                s.take_audio_commands();
                open(&mut s);
                s.panel = Some(Panel::SongProperties(index));
                press(&mut s, KeyCode::Enter);
                assert!(s.wants_text_input());
                assert!(s.text_input(&value.to_string()));
                press(&mut s, KeyCode::Enter);
                assert_eq!(
                    s.workspace.file().session.serialize().unwrap(),
                    before_bytes
                );
                apply(&mut s);
                let after = s.focus_position.as_ref().unwrap();
                assert_eq!(after.row().to_bits(), before_row.to_bits());
                assert!(after.is_current(&s.workspace));
                assert_eq!(
                    after.music_time(),
                    MusicClock::capture(&s.workspace.file().session, 0)
                        .unwrap()
                        .chart_time_to_music(after.chart_time())
                        .unwrap()
                );
                assert_eq!(s.audio_session.scope.owner, owner);
                assert!(s.audio_output_snapshot().leased);
                assert_eq!(s.chart_audio_generation(), before_generation);
                assert_eq!(s.playback_asset_epoch(), before_epoch);
                assert_eq!(s.paused_playback.is_some(), paused);
                if paused {
                    assert_eq!(s.save_as_transport_stamp().1, origin);
                    assert_eq!(
                        s.paused_position().unwrap().music_time.to_bits(),
                        after.music_time().to_bits()
                    );
                }
                let expected_music = after.music_time() + s.audio_preferences.effective().offset;
                let commands = s.take_audio_commands();
                no_stop(&commands);
                if !paused {
                    assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorSeekMusic{owner:o,seconds} if *o==owner && seconds.to_bits()==expected_music.to_bits())),"{commands:?}");
                }
                let after_bytes = s.workspace.file().session.serialize().unwrap();
                for (command, expected) in [
                    (Command::Undo, &before_bytes),
                    (Command::Redo, &after_bytes),
                ] {
                    s.command(command);
                    assert_eq!(s.workspace.file().session.serialize().unwrap(), *expected);
                    assert_eq!(row(&s).to_bits(), before_row.to_bits());
                    assert_eq!(s.audio_session.scope.owner, owner);
                    assert_eq!(s.paused_playback.is_some(), paused);
                    no_stop(&s.take_audio_commands());
                }
            }
        }
    }
}
#[test]
fn music_replace_clear_and_history_use_canonical_swap_while_main_continues_en_fr() {
    for language in ["en", "fr"] {
        for paused in [false, true] {
            let (_root, mut s, owner) = fixture(language);
            if paused {
                s.toggle_audio();
            }
            let before_row = row(&s);
            let old = s.workspace.file().session.serialize().unwrap();
            s.take_audio_commands();
            for path in ["next.wav", ""] {
                open(&mut s);
                s.song_properties
                    .as_mut()
                    .unwrap()
                    .set_text(Field::Music, path)
                    .unwrap();
                apply(&mut s);
                assert_eq!(row(&s).to_bits(), before_row.to_bits());
                assert_eq!(s.audio_session.scope.owner, owner);
                assert_eq!(s.paused_playback.is_some(), paused);
                let commands = s.take_audio_commands();
                no_stop(&commands);
                assert!(
                    commands.iter().any(|command| match command {
                        GameplayAudioCommand::EditorSwapMusicAsset { owner: o, path: p } =>
                            !path.is_empty() && *o == owner && p.ends_with(path),
                        GameplayAudioCommand::EditorClearMusicAsset { owner: o } =>
                            path.is_empty() && *o == owner,
                        _ => false,
                    }),
                    "{commands:?}"
                );
            }
            s.command(Command::Undo);
            no_stop(&s.take_audio_commands());
            s.command(Command::Undo);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), old);
            assert_eq!(row(&s).to_bits(), before_row.to_bits());
            no_stop(&s.take_audio_commands());
        }
    }
}
#[test]
fn rejected_clock_and_cancel_do_not_publish_or_touch_transport_en_fr() {
    for language in ["en", "fr"] {
        let (_root, mut s, _owner) = fixture(language);
        let before = s.workspace.file().session.serialize().unwrap();
        let stamp = s.save_as_transport_stamp();
        let before_row = row(&s);
        open(&mut s);
        s.song_properties
            .as_mut()
            .unwrap()
            .set_sync_offset(f64::MAX)
            .unwrap();
        s.song_properties
            .as_mut()
            .unwrap()
            .set_music_offset(-f64::MAX)
            .unwrap();
        s.panel = Some(Panel::SongProperties(1));
        press(&mut s, KeyCode::Enter);
        assert!(s.song_properties.is_some());
        assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
        assert_eq!(s.save_as_transport_stamp(), stamp);
        assert_eq!(row(&s).to_bits(), before_row.to_bits());
        assert!(s.take_audio_commands().is_empty());
        s.panel = Some(Panel::SongProperties(2));
        press(&mut s, KeyCode::Enter);
        assert!(s.song_properties.is_none());
        assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
        assert_eq!(s.save_as_transport_stamp(), stamp);
    }
}
#[test]
fn chart_override_and_final_closed_view_keep_the_effective_asset_and_exact_row_or_time() {
    for language in ["en", "fr"] {
        for chartless in [false, true] {
            let (_root, mut s, owner) = fixture(language);
            // Install a real chart override through its canonical owner before the lot
            // under test, then bind its new current presentation without a fake clock.
            let mut draft =
                deadsync_editor::chart_properties::Draft::capture(&s.workspace.file().session, 0)
                    .unwrap();
            draft
                .set_text(deadsync_editor::chart_text::Field::Music, "override.wav")
                .unwrap();
            s.workspace.apply_chart_properties(&draft).unwrap();
            s.refresh();
            let preview = Preview::rebind_chart_asset(
                &s.workspace.file().session,
                0,
                s.workspace.file().resource_base(),
            )
            .unwrap();
            let focus = deadsync_editor::chart_focus::Position::at_time(
                &s.workspace.file().session,
                0,
                1.25,
            )
            .unwrap();
            s.apply_chart_focus_audio(Some((preview, true)), false, &focus);
            s.focus_position = Some(focus);
            s.sync_editor_asset_registration();
            if chartless {
                s.chart_views_apply(super::super::chart_views::ViewAction::Close);
                assert!(!s.workspace.has_chart());
            }
            let time = s.chart_focus_time().unwrap();
            s.take_audio_commands();
            open(&mut s);
            s.song_properties
                .as_mut()
                .unwrap()
                .set_text(Field::Music, "next.wav")
                .unwrap();
            apply(&mut s);
            assert_eq!(s.audio_session.scope.owner, owner);
            assert!(s.chart_transport_is_playing());
            assert_eq!(s.chart_focus_time().unwrap().to_bits(), time.to_bits());
            let commands = s.take_audio_commands();
            no_stop(&commands);
            if chartless {
                assert!(commands.iter().any(
                    |c| matches!(c,GameplayAudioCommand::EditorClearMusicAsset{owner:o}if *o==owner)
                ));
                assert!(s.editor_asset_paths()[0].is_none());
            } else {
                assert!(!commands.iter().any(|c| matches!(
                    c,
                    GameplayAudioCommand::EditorSwapMusicAsset { .. }
                        | GameplayAudioCommand::EditorClearMusicAsset { .. }
                )));
                assert!(
                    s.playback
                        .as_ref()
                        .unwrap()
                        .preview
                        .as_ref()
                        .unwrap()
                        .has_music_path(&s.workspace.file().resource_base().join("override.wav"))
                );
            }
        }
    }
}
#[test]
fn main_music_edit_stops_audition_but_offset_edit_does_not_steal_its_lease() {
    for language in ["en", "fr"] {
        let (_root, mut s, owner) = fixture(language);
        s.stop_audio();
        let registration = s.editor_asset_registration();
        s.observe_editor_assets(deadsync_gameplay::EditorAssetReadiness {
            owner: s.editor_asset_owner(),
            registration,
            main: true,
            dedicated_preview: false,
        });
        s.command_toggle_sample_audio();
        assert!(s.is_playing());
        assert!(!s.chart_transport_is_playing());
        s.take_audio_commands();
        open(&mut s);
        s.song_properties
            .as_mut()
            .unwrap()
            .set_sync_offset(0.25)
            .unwrap();
        apply(&mut s);
        assert!(s.is_playing());
        assert!(!s.chart_transport_is_playing());
        no_stop(&s.take_audio_commands());
        open(&mut s);
        s.song_properties
            .as_mut()
            .unwrap()
            .set_text(Field::Music, "next.wav")
            .unwrap();
        apply(&mut s);
        assert!(!s.is_playing());
        assert!(
            s.take_audio_commands()
                .iter()
                .any(|c| matches!(c,GameplayAudioCommand::EditorStopMusic{owner:o}if *o==owner))
        );
    }
}
#[test]
fn unchanged_apply_and_valid_staged_cancel_leave_clock_history_and_transport_untouched() {
    for language in ["en", "fr"] {
        for paused in [false, true] {
            let (_root, mut s, _owner) = fixture(language);
            if paused {
                s.toggle_audio();
            }
            let before = s.workspace.file().session.serialize().unwrap();
            let revision = s.workspace.file().session.revision();
            let stamp = s.save_as_transport_stamp();
            let position = s.capture_timing_presentation().unwrap().row();
            s.take_audio_commands();
            open(&mut s);
            apply(&mut s);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(s.workspace.file().session.revision(), revision);
            assert_eq!(s.save_as_transport_stamp(), stamp);
            let commands = s.take_audio_commands();
            no_stop(&commands);
            assert!(!commands.iter().any(|command| matches!(
                command,
                GameplayAudioCommand::SeekMusic(_)
                    | GameplayAudioCommand::EditorSeekMusic { .. }
                    | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                    | GameplayAudioCommand::EditorClearMusicAsset { .. }
            )));
            open(&mut s);
            s.song_properties
                .as_mut()
                .unwrap()
                .set_sync_offset(0.125)
                .unwrap();
            s.song_properties
                .as_mut()
                .unwrap()
                .set_text(Field::Music, "next.wav")
                .unwrap();
            s.panel = Some(Panel::SongProperties(2));
            press(&mut s, KeyCode::Enter);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(s.workspace.file().session.revision(), revision);
            assert_eq!(row(&s).to_bits(), position.to_bits());
            assert_eq!(s.save_as_transport_stamp(), stamp);
            let commands = s.take_audio_commands();
            no_stop(&commands);
            assert!(!commands.iter().any(|command| matches!(
                command,
                GameplayAudioCommand::SeekMusic(_)
                    | GameplayAudioCommand::EditorSeekMusic { .. }
                    | GameplayAudioCommand::EditorSwapMusicAsset { .. }
                    | GameplayAudioCommand::EditorClearMusicAsset { .. }
            )));
        }
    }
}
#[test]
fn chartless_cross_directory_save_as_rewrites_music_without_a_music_changed_notification() {
    for language in ["en", "fr"] {
        for paused in [false, true] {
            let (root, mut s, owner) = fixture(language);
            let mut draft =
                deadsync_editor::chart_properties::Draft::capture(&s.workspace.file().session, 0)
                    .unwrap();
            draft
                .set_text(deadsync_editor::chart_text::Field::Music, "override.wav")
                .unwrap();
            s.workspace.apply_chart_properties(&draft).unwrap();
            s.refresh();
            let preview = Preview::rebind_chart_asset(
                &s.workspace.file().session,
                0,
                s.workspace.file().resource_base(),
            )
            .unwrap();
            let focus = deadsync_editor::chart_focus::Position::at_time(
                &s.workspace.file().session,
                0,
                1.25,
            )
            .unwrap();
            s.apply_chart_focus_audio(Some((preview, true)), false, &focus);
            s.focus_position = Some(focus);
            s.sync_editor_asset_registration();
            s.chart_views_apply(super::super::chart_views::ViewAction::Close);
            assert!(!s.workspace.has_chart());
            if paused {
                s.toggle_audio();
                assert!(s.paused_playback.is_some());
            }
            let before = s.save_as_transport_stamp();
            let before_time = s.chart_focus_time().unwrap().to_bits();
            let old_music = s
                .workspace
                .file()
                .session
                .document()
                .text(None, "MUSIC")
                .unwrap();
            s.take_audio_commands();
            let destination_directory = root.path().join("relocated");
            std::fs::create_dir(&destination_directory).unwrap();
            let destination = destination_directory.join("copy.ssc");
            s.open_save_as();
            assert!(s.text_input(destination.to_str().unwrap()));
            press(&mut s, KeyCode::Enter);
            assert!(
                matches!(s.panel, Some(Panel::SaveAsReview(_))),
                "{}",
                s.status
            );
            s.panel = Some(Panel::SaveAsReview(0));
            press(&mut s, KeyCode::Enter);
            assert!(destination.exists(), "{}", s.status);
            assert_ne!(
                s.workspace
                    .file()
                    .session
                    .document()
                    .text(None, "MUSIC")
                    .unwrap(),
                old_music,
                "real SaveAs must rewrite the resident MUSIC path to exercise the guard"
            );
            assert_eq!(s.save_as_transport_stamp(), before);
            assert_eq!(s.chart_focus_time().unwrap().to_bits(), before_time);
            assert_eq!(s.paused_playback.is_some(), paused);
            assert_eq!(
                s.editor_asset_paths()[0]
                    .as_ref()
                    .unwrap()
                    .canonicalize()
                    .unwrap(),
                root.path().join("override.wav").canonicalize().unwrap()
            );
            let commands = s.take_audio_commands();
            no_stop(&commands);
            assert!(
                !commands.iter().any(|command| matches!(
                    command,
                    GameplayAudioCommand::EditorClearMusicAsset { .. }
                )),
                "{commands:?}"
            );
            // The relocation exemption must not suppress the next explicit notification.
            open(&mut s);
            s.song_properties
                .as_mut()
                .unwrap()
                .set_text(Field::Music, "../next.wav")
                .unwrap();
            apply(&mut s);
            assert_eq!(s.audio_session.scope.owner, owner);
            assert_eq!(s.chart_focus_time().unwrap().to_bits(), before_time);
            assert!(s.editor_asset_paths()[0].is_none());
            let commands = s.take_audio_commands();
            no_stop(&commands);
            assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorClearMusicAsset{owner:o}if *o==owner)));
        }
    }
}
