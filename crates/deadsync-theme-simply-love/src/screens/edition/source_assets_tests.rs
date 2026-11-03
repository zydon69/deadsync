use super::*;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::{
    key_binding_preferences::{Action, Preferences},
    song_text::Field,
    workspace::Command,
};
use std::{sync::Arc, time::Instant};
fn key(state: &mut State, code: KeyCode) {
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
fn state(fr: bool, sm: bool, chartless: bool) -> (tempfile::TempDir, State) {
    if fr {
        i18n::init(deadsync_assets::language::load_for_tests("fr"));
    } else {
        i18n::init_for_tests();
    }
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(if sm { "song.sm" } else { "song.ssc" });
    let mut source = String::from(
        "#VERSION:0.83;#TITLE:Real title;#BACKGROUND:manual.png;#CDTITLE:manual-cd.png;#Opaque:private;#BPMS:0=120;",
    );
    if !chartless {
        source.push_str(if sm {
            "#NOTES:dance-single::Hard:9::1000;"
        } else {
            "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
        });
    }
    std::fs::write(&path, source).unwrap();
    for name in [
        "background.png",
        "banner.png",
        "cdtitle.png",
        "jacket.png",
        "cover-cd.png",
        "cover disc.png",
        "words.lrc",
    ] {
        std::fs::write(root.path().join(name), []).unwrap();
    }
    let mut state = State::open(&path, 0).unwrap();
    let mut preferences = Preferences::default();
    preferences.commands.insert(
        Action::AutoApplyAllSongAssets.id().into(),
        vec![vec!["F15".into()]],
    );
    preferences.commands.insert(
        Action::AutoApplyUnsetSongAssets.id().into(),
        vec![vec!["F16".into()]],
    );
    state.configure_key_binding_preferences(Arc::new(preferences));
    (root, state)
}
#[test]
fn auto_assets_shortcuts_apply_all_or_only_unset_with_one_undo_en_fr_sm_ssc_and_chartless() {
    for fr in [false, true] {
        for sm in [false, true] {
            for chartless in [false, true] {
                for unset in [false, true] {
                    let (_root, mut state) = state(fr, sm, chartless);
                    let before = state.workspace.file().session.serialize().unwrap();
                    state.audio_commands.clear();
                    // A leftover inspector draft must never be the source of a global action.
                    let mut abandoned = state.workspace.song_properties().unwrap();
                    abandoned
                        .set_text(Field::Title, "Unapplied inspector title")
                        .unwrap();
                    state.song_properties = Some(abandoned);
                    key(&mut state, if unset { KeyCode::F16 } else { KeyCode::F15 });
                    assert!(state.panel.is_none(), "{}", state.status);
                    let document = state.workspace.file().session.document();
                    assert_eq!(document.text(None, "TITLE").as_deref(), Some("Real title"));
                    assert_eq!(
                        document.text(None, "BACKGROUND").as_deref(),
                        Some(if unset {
                            "manual.png"
                        } else {
                            "background.png"
                        })
                    );
                    assert_eq!(
                        document.text(None, "CDTITLE").as_deref(),
                        Some(if unset {
                            "manual-cd.png"
                        } else {
                            "cdtitle.png"
                        })
                    );
                    for (tag, value) in [
                        ("BANNER", "banner.png"),
                        ("JACKET", "jacket.png"),
                        ("CDIMAGE", "cover-cd.png"),
                        ("DISCIMAGE", "cover disc.png"),
                        ("LYRICSPATH", "words.lrc"),
                    ] {
                        assert_eq!(
                            document.text(None, tag).as_deref(),
                            Some(value),
                            "{tag} sm={sm} chartless={chartless}: {}",
                            state.status
                        );
                    }
                    assert_eq!(document.text(None, "Opaque").as_deref(), Some("private"));
                    assert!(state.audio_commands.is_empty());
                    let after = state.workspace.file().session.serialize().unwrap();
                    assert_ne!(before, after);
                    state.command(Command::Undo);
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                    assert!(state.workspace.file().session.undo_label().is_none());
                    state.command(Command::Redo);
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
                    key(&mut state, if unset { KeyCode::F16 } else { KeyCode::F15 });
                    assert_eq!(state.status, i18n::tr("Edition", "SongAssetsUnchanged"));
                    state.command(Command::Undo);
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                }
            }
        }
    }
}
#[test]
fn no_song_refuses_auto_assets_without_recreating_a_document() {
    for fr in [false, true] {
        let (root, mut state) = state(fr, false, false);
        state.command_file_close();
        assert!(!state.has_song());
        let before = state.workspace.file().session.serialize().unwrap();
        let count = std::fs::read_dir(root.path()).unwrap().count();
        key(&mut state, KeyCode::F15);
        assert!(!state.has_song());
        assert_eq!(state.status, i18n::tr("Edition", "FileNoSongHelp"));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), count);
    }
}

#[test]
fn auto_assets_preserve_fractional_focus_without_audio_commands() {
    for sm in [false, true] {
        let (_root, mut state) = state(false, sm, false);
        let focus = deadsync_editor::chart_focus::Position::at_fractional_row(
            &state.workspace.file().session,
            0,
            60.5,
        )
        .unwrap();
        state.command(Command::Seek(focus.edit_row()));
        let row = focus.row();
        let time = focus.music_time();
        state.focus_position = Some(focus);
        state.audio_commands.clear();
        state.command_auto_song_assets(false);
        let focus = state.focus_position.as_ref().unwrap();
        assert!(focus.is_current(&state.workspace));
        assert_eq!(focus.row(), row);
        assert_eq!(focus.music_time(), time);
        assert!(state.audio_commands.is_empty());
    }
}
