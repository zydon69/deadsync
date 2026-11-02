use super::*;

#[test]
fn measure_label_and_chart_edge_callbacks_keep_exact_positions_and_document() {
    for locale in ["en", "fr"] {
        crate::i18n::init(deadsync_assets::language::load_for_tests(locale));
        for (action, from, expected) in [
            (Action::PreviousMeasure, 49.25, -94.75),
            (Action::NextMeasure, 49.25, 193.25),
            (Action::PreviousMeasure, 193.25, 25.25),
            (Action::NextMeasure, -0.25, 143.75),
            (Action::PreviousLabel, 49.25, 48.0),
            (Action::NextLabel, 49.25, 144.0),
            (Action::PreviousLabel, -0.25, 144.0),
            (Action::NextLabel, 144.25, 48.0),
            (Action::ChartStart, 49.25, 0.0),
            (Action::ChartEnd, 49.25, 384.0),
        ] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("source-navigation.ssc");
            std::fs::write(&path, b"#VERSION:0.83;#BPMS:0=120;#TIMESIGNATURES:0=3=4,4=7=8;#LABELS:1=first,3=last;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0000,0001;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.navigate_exact_position(from, true);
            assert_eq!(state.source_navigation_position().unwrap(), from);
            let before = state.workspace.file().session.serialize().unwrap();
            let revision = state.workspace.file().session.revision();
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            assert_eq!(
                state.source_navigation_position().unwrap(),
                expected,
                "{} {locale}",
                action.id()
            );
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(state.workspace.file().session.revision(), revision);
            assert!(state.workspace.file().session.undo_label().is_none());
        }
    }
}

#[test]
fn undo_redo_callbacks_restore_exact_document_for_each_quick_add_non_pattern_kind() {
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        for action in Action::ALL
            .into_iter()
            .filter(|a| a.id().starts_with("AddEvent") && *a != Action::AddEventPattern)
        {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("history.ssc");
            std::fs::write(
                &path,
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0001;",
            )
            .unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.command(Command::Seek(48));
            let before = state.workspace.file().session.serialize().unwrap();
            state.command_binding_dispatch(vec![Effect::Press(action.id().into())], Instant::now());
            state.quick_add_key(3, KeyCode::Enter);
            let after = state.workspace.file().session.serialize().unwrap();
            assert_ne!(after, before, "{locale} {}", action.id());
            assert!(state.panel.is_none(), "{}", action.id());
            for _ in 0..2 {
                assert!(
                    state.command_binding_dispatch(
                        vec![Effect::Press(Action::Undo.id().into())],
                        Instant::now()
                    ),
                    "undo route {locale} {} allowed={} text={} help={} preview={} job={} catalog={} analysis={}",
                    action.id(),
                    state.command_bindings_allowed(),
                    state.wants_text_input(),
                    state.help,
                    state.preview.is_some(),
                    state.job.is_some(),
                    state.catalog_job.is_some(),
                    state.analysis_job.is_some()
                );
                assert_eq!(
                    state.workspace.file().session.serialize().unwrap(),
                    before,
                    "undo {locale} {}",
                    action.id()
                );
                assert!(state.workspace.file().session.undo_label().is_none());
                assert!(state.command_binding_dispatch(
                    vec![Effect::Press(Action::Redo.id().into())],
                    Instant::now()
                ));
                assert_eq!(
                    state.workspace.file().session.serialize().unwrap(),
                    after,
                    "redo {locale} {}",
                    action.id()
                );
            }
        }
    }
}

#[test]
fn keep_chart_open_callback_is_idempotent_and_retains_views_when_focus_changes() {
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("views.ssc");
        std::fs::write(&path, b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0001;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let before = state.workspace.file().session.serialize().unwrap();
        for _ in 0..3 {
            assert!(state.command_binding_dispatch(
                vec![Effect::Press(Action::KeepTab.id().into())],
                Instant::now()
            ));
            let views = state.open_chart_views().unwrap();
            assert_eq!(views.entries().len(), 1);
            assert!(views.focused().unwrap().dedicated());
            assert_eq!(views.focused().unwrap().source_index(), 0);
        }
        assert!(state.focus_chart_at_time(1, 0.0));
        assert_eq!(state.open_chart_views().unwrap().entries().len(), 2);
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::PreviousTab.id().into())],
            Instant::now()
        ));
        assert_eq!(state.workspace.active_chart(), Some(0));
        assert!(
            state
                .open_chart_views()
                .unwrap()
                .focused()
                .unwrap()
                .dedicated()
        );
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::NextTab.id().into())],
            Instant::now()
        ));
        assert_eq!(state.workspace.active_chart(), Some(1));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(state.workspace.file().session.undo_label().is_none());
    }
}

#[test]
fn chart_tab_callbacks_move_without_wrap_cycle_focus_and_close_adjacent_views() {
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("tabs.ssc");
        std::fs::write(&path, b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0010;").unwrap();
        let mut state = State::open(&path, 2).unwrap();
        let before = state.workspace.file().session.serialize().unwrap();
        // Newly focused charts are inserted at the front by the source.
        for index in (0..3).rev() {
            assert!(state.focus_chart_at_time(index, 0.0));
            assert!(state.command_binding_dispatch(
                vec![Effect::Press(Action::KeepTab.id().into())],
                Instant::now()
            ));
        }
        assert!(state.focus_chart_at_time(2, 0.0));
        for (action, focused, order) in [
            (Action::MoveTabRight, Some(2), vec![0, 1, 2]),
            (Action::MoveTabLeft, Some(2), vec![0, 2, 1]),
            (Action::MoveTabLeft, Some(2), vec![2, 0, 1]),
            (Action::MoveTabLeft, Some(2), vec![2, 0, 1]),
            (Action::MoveTabRight, Some(2), vec![0, 2, 1]),
            (Action::PreviousTab, Some(0), vec![0, 2, 1]),
            (Action::PreviousTab, Some(1), vec![0, 2, 1]),
            (Action::NextTab, Some(0), vec![0, 2, 1]),
            (Action::NextTab, Some(2), vec![0, 2, 1]),
            (Action::CloseTab, Some(1), vec![0, 1]),
            (Action::CloseTab, Some(0), vec![0]),
            (Action::CloseTab, None, vec![]),
        ] {
            assert!(
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                ),
                "{locale} {}",
                action.id()
            );
            assert_eq!(
                state.workspace.active_chart(),
                focused,
                "{locale} {}",
                action.id()
            );
            let actual: Vec<_> = state
                .open_chart_views()
                .unwrap()
                .entries()
                .iter()
                .map(|v| v.source_index())
                .collect();
            assert_eq!(actual, order, "{locale} {}", action.id());
            assert!(state.has_song());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert!(state.workspace.file().session.undo_label().is_none());
        }
        assert!(state.focus_chart_at_time(1, 0.0));
        assert_eq!(state.workspace.active_chart(), Some(1));
        assert_eq!(state.open_chart_views().unwrap().entries().len(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn previous_next_chart_callbacks_follow_difficulty_order_wrap_and_reopen_without_focus() {
    struct Order;
    impl deadsync_editor::chart_order::Orderer for Order {
        fn order(
            &mut self,
            snapshot: &deadsync_editor::chart_order::Snapshot,
        ) -> Result<Vec<usize>, deadsync_editor::Error> {
            snapshot.indices_with(str::cmp)
        }
    }
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("ordered.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#NOTES:0010;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.navigate_exact_position(49.25, true);
        let time = state.chart_focus_time().unwrap();
        let before = state.workspace.file().session.serialize().unwrap();
        for (action, expected) in [
            (Action::NextChart, 2),
            (Action::NextChart, 1),
            (Action::PreviousChart, 2),
            (Action::PreviousChart, 0),
        ] {
            assert!(
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                )
            );
            assert!(state.process_chart_order(&mut Order));
            assert_eq!(
                state.workspace.active_chart(),
                Some(expected),
                "{locale} {}",
                action.id()
            );
            assert_eq!(state.chart_focus_time().unwrap(), time);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        }
        for (action, expected) in [(Action::NextChart, 1), (Action::PreviousChart, 2)] {
            assert!(state.command_binding_dispatch(
                vec![Effect::Press(Action::CloseTab.id().into())],
                Instant::now()
            ));
            assert_eq!(state.workspace.active_chart(), None);
            assert!(
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                )
            );
            assert!(state.process_chart_order(&mut Order));
            assert_eq!(state.workspace.active_chart(), Some(expected));
            assert_eq!(state.chart_focus_time().unwrap(), time);
        }
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(state.workspace.file().session.undo_label().is_none());
    }
}

#[test]
fn snap_cycle_callbacks_visit_every_level_both_directions_without_moving_chart() {
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("snap.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.command(Command::SetSnap(9));
        state.navigate_exact_position(-49.25, true);
        let before = state.workspace.file().session.serialize().unwrap();
        for (action, indices) in [
            (Action::IncreaseSnap, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            (Action::DecreaseSnap, vec![8, 7, 6, 5, 4, 3, 2, 1, 0, 9]),
        ] {
            for expected in indices {
                assert!(state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                ));
                assert_eq!(
                    state.workspace.snap(),
                    expected,
                    "{locale} {}: {}",
                    action.id(),
                    state.status
                );
                assert_eq!(state.source_navigation_position().unwrap(), -49.25);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                assert!(state.workspace.file().session.undo_label().is_none());
            }
        }
    }
}

#[test]
fn routine_owner_callbacks_preserve_exact_selected_notes_attributes_and_other_heads() {
    use deadsync_editor::notes::{Kind, Note, Position};
    use std::collections::BTreeMap;
    let cases = [
        (Action::ConvertSelectedNotesToPlayer1, 0, None),
        (Action::ConvertSelectedNotesToPlayer2, 1, None),
        (Action::ConvertSelectedNotesToPlayer3, 2, None),
        (Action::ConvertSelectedNotesToPlayer4, 3, None),
        (Action::SwapSelectedPlayer1And2Notes, 0, Some(1)),
        (Action::SwapSelectedPlayer1And3Notes, 0, Some(2)),
        (Action::SwapSelectedPlayer1And4Notes, 0, Some(3)),
        (Action::SwapSelectedPlayer2And3Notes, 1, Some(2)),
        (Action::SwapSelectedPlayer2And4Notes, 1, Some(3)),
        (Action::SwapSelectedPlayer3And4Notes, 2, Some(3)),
    ];
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        for (action, a, b) in cases {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("owners.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000\n&\n00000000\n&\n00000000;").unwrap();
            let mut file = deadsync_editor::storage::FileSession::open(&path).unwrap();
            let mut original = BTreeMap::new();
            for (player, kind) in [
                (0, Kind::Hold),
                (1, Kind::Tap),
                (2, Kind::Roll),
                (3, Kind::Mine),
            ] {
                original.insert(
                    Position {
                        row: 0,
                        lane: player as u16,
                        player,
                    },
                    Note {
                        kind,
                        end: matches!(kind, Kind::Hold | Kind::Roll).then_some(96),
                        attributes: Arc::from("[7]"),
                        tail_attributes: if matches!(kind, Kind::Hold | Kind::Roll) {
                            Arc::from("[8]")
                        } else {
                            Arc::from("")
                        },
                    },
                );
            }
            original.insert(
                Position {
                    row: 192,
                    lane: 7,
                    player: 3,
                },
                Note::tap(),
            );
            file.session
                .edit_notes(
                    0,
                    "Fixture",
                    original.iter().map(|(&p, n)| (p, Some(n.clone()))),
                )
                .unwrap();
            let workspace = deadsync_editor::workspace::Workspace::from_file(file, 0).unwrap();
            let mut state = State::from_workspace(workspace).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            let mut selection = state.workspace.event_selection_between(0, 0).unwrap();
            let note_indices: Vec<_> = selection
                .events()
                .iter()
                .enumerate()
                .filter_map(|(index, event)| {
                    matches!(event, deadsync_editor::event_clipboard::Event::Note(_))
                        .then_some(index)
                })
                .collect();
            selection.select_all(false);
            for index in note_indices {
                selection.set_selected(index, true).unwrap();
            }
            assert_eq!(selection.selected_count(), 4);
            state.replace_region_event_selection(selection);
            let mut expected = BTreeMap::new();
            for (&position, note) in &original {
                let mut target = position;
                if position.row == 0 {
                    target.player = match b {
                        None => a,
                        Some(b) if position.player == a => b,
                        Some(_) if position.player == b.unwrap() => a,
                        _ => position.player,
                    };
                }
                expected.insert(target, note.clone());
            }
            assert!(
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                )
            );
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            state.selected_transform_review_key(6, KeyCode::Enter);
            assert!(
                !matches!(
                    state.panel,
                    Some(super::super::Panel::SelectedTransformReview(_))
                ),
                "{locale} {}: {}",
                action.id(),
                state.status
            );
            let actual: BTreeMap<_, _> = state
                .workspace
                .grid()
                .iter()
                .map(|(p, n)| (p, n.clone()))
                .collect();
            assert_eq!(actual, expected, "{locale} {}", action.id());
            let after = state.workspace.file().session.serialize().unwrap();
            state.command(Command::Undo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            state.command(Command::Redo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
            assert_eq!(
                state
                    .workspace
                    .grid()
                    .iter()
                    .map(|(p, n)| (p, n.clone()))
                    .collect::<BTreeMap<_, _>>(),
                expected
            );
        }
    }
}

#[test]
fn audio_tick_and_rate_callbacks_match_source_values_and_history_contracts() {
    use deadsync_editor::audio_preferences::{Field, FieldValue};
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("audio.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let bytes = state.workspace.file().session.serialize().unwrap();
        for (action, field) in [
            (Action::ToggleAssistTick, Field::UseAssistTick),
            (Action::ToggleBeatTick, Field::UseBeatTick),
        ] {
            let before = state.audio_preferences;
            let revision = state.workspace.file().session.revision();
            let FieldValue::Bool(value) = before.value(field) else {
                panic!("not bool")
            };
            let mut expected = before;
            expected.set_field(field, FieldValue::Bool(!value)).unwrap();
            assert!(
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                )
            );
            assert_eq!(state.audio_preferences, expected);
            assert_eq!(state.workspace.file().session.revision(), revision);
            assert!(
                state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                )
            );
            assert_eq!(state.audio_preferences, before);
            assert_eq!(state.workspace.file().session.revision(), revision);
        }
        for (from, decreased, increased) in [
            (0.25, 0.25, 0.3),
            (0.26, 0.25, 0.3),
            (0.99, 0.9, 1.0),
            (1.0, 0.9, 1.1),
            (1.05, 1.0, 1.1),
            (3.99, 3.9, 4.0),
            (4.0, 3.9, 4.0),
        ] {
            for (action, target) in [
                (Action::DecreaseRate, decreased),
                (Action::IncreaseRate, increased),
            ] {
                let mut audio = state.audio_preferences;
                audio.set_music_rate(from).unwrap();
                state.commit_audio_preferences(audio).unwrap();
                let revision = state.workspace.file().session.revision();
                assert!(state.command_binding_dispatch(
                    vec![Effect::Press(action.id().into())],
                    Instant::now()
                ));
                assert_eq!(
                    state.audio_preferences.music_rate(),
                    target,
                    "{locale} {} from {from}",
                    action.id()
                );
                if target != from {
                    state.command(Command::Undo);
                    assert_eq!(state.audio_preferences.music_rate(), from);
                    state.command(Command::Redo);
                    assert_eq!(state.audio_preferences.music_rate(), target);
                } else {
                    assert_eq!(state.workspace.file().session.revision(), revision);
                }
                assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
            }
        }
    }
}

#[test]
fn toggle_player_callback_wraps_four_owners_and_replaces_peer_player_memory() {
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("players.ssc");
        let chart = "#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000\n&\n00000000\n&\n00000000;";
        std::fs::write(&path, format!("#VERSION:0.83;#BPMS:0=120;{chart}{chart}")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let before = state.workspace.file().session.serialize().unwrap();
        assert!(state.focus_chart_at_time(1, 0.0));
        state.command(Command::SelectPlayer(3));
        assert!(state.focus_chart_at_time(0, 0.0));
        // Explicit selection propagates its default to charts with no cache.
        assert_eq!(state.workspace.player(), 3);
        for expected in [0, 1, 2, 3, 0, 1] {
            assert!(state.command_binding_dispatch(
                vec![Effect::Press(Action::TogglePlayer.id().into())],
                Instant::now()
            ));
            assert_eq!(state.workspace.player(), expected);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        }
        assert!(state.focus_chart_at_time(1, 0.0));
        assert_eq!(state.workspace.player(), 1);
        assert!(state.workspace.file().session.undo_label().is_none());
    }
}

#[test]
fn all_ten_lane_callbacks_author_taps_mines_holds_and_rolls_in_the_exact_column() {
    use deadsync_editor::notes::{Kind, Position};
    let lanes = [
        Action::Arrow0,
        Action::Arrow1,
        Action::Arrow2,
        Action::Arrow3,
        Action::Arrow4,
        Action::Arrow5,
        Action::Arrow6,
        Action::Arrow7,
        Action::Arrow8,
        Action::Arrow9,
    ];
    fn raw(state: &mut State, code: KeyCode, pressed: bool) {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        for (lane, action) in lanes.into_iter().enumerate() {
            for (extend, modified, kind) in [
                (false, false, Kind::Tap),
                (false, true, Kind::Mine),
                (true, false, Kind::Hold),
                (true, true, Kind::Roll),
            ] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("lanes.ssc");
                std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-double;#NOTES:0000000000\n0000000000\n0000000000\n0000000000;").unwrap();
                let mut state = State::open(&path, 0).unwrap();
                assert_eq!(state.workspace.grid().columns(), 10);
                state.command(Command::SetSnap(0));
                let mut prefs = Preferences::default();
                prefs
                    .commands
                    .insert(action.id().into(), vec![vec!["F13".into()]]);
                prefs.commands.insert(
                    Action::ArrowModification.id().into(),
                    vec![vec!["F14".into()]],
                );
                state.configure_key_binding_preferences(Arc::new(prefs));
                let before = state.workspace.file().session.serialize().unwrap();
                if modified {
                    raw(&mut state, KeyCode::F14, true);
                }
                raw(&mut state, KeyCode::F13, true);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                state.raw_key(&RawKeyboardEvent {
                    code: KeyCode::F13,
                    pressed: true,
                    repeat: true,
                    timestamp: Instant::now(),
                    host_nanos: 0,
                });
                if extend {
                    raw(&mut state, KeyCode::ArrowDown, true);
                    raw(&mut state, KeyCode::ArrowDown, false);
                    assert_eq!(state.workspace.row(), 48);
                }
                raw(&mut state, KeyCode::F13, false);
                if modified {
                    raw(&mut state, KeyCode::F14, false);
                }
                let notes: Vec<_> = state
                    .workspace
                    .grid()
                    .iter()
                    .map(|(p, n)| (p, n.clone()))
                    .collect();
                assert_eq!(
                    notes.len(),
                    1,
                    "{locale} {} {kind:?}: {}",
                    action.id(),
                    state.status
                );
                assert_eq!(
                    notes[0].0,
                    Position {
                        row: 0,
                        lane: lane as u16,
                        player: 0
                    }
                );
                assert_eq!(notes[0].1.kind, kind);
                assert_eq!(notes[0].1.end, extend.then_some(48));
                let after = state.workspace.file().session.serialize().unwrap();
                state.command(Command::Undo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                state.command(Command::Redo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
                assert_eq!(std::fs::read(&path).unwrap(), before);
            }
        }
    }
}

#[test]
fn transport_callbacks_pause_resume_toggle_preview_and_cancel_before_clearing_selection() {
    use deadsync_gameplay::{EditorAssetReadiness, GameplayAudioCommand};
    for locale in ["en", "fr"] {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("transport.ssc");
        std::fs::write(root.path().join("music.wav"), b"command-only fixture").unwrap();
        std::fs::write(&path,b"#VERSION:0.83;#MUSIC:music.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#SAMPLESTART:0.5;#SAMPLELENGTH:1;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let before = state.workspace.file().session.serialize().unwrap();
        let mut selection = state.workspace.event_selection_between(0, 0).unwrap();
        selection.select_all(true);
        state.replace_region_event_selection(selection);
        let count = state
            .event_selection
            .as_ref()
            .unwrap()
            .selection
            .selected_count();
        for playing in [true, false, true] {
            assert!(state.command_binding_dispatch(
                vec![Effect::Press(Action::PlayPause.id().into())],
                Instant::now()
            ));
            assert_eq!(state.chart_transport_is_playing(), playing);
        }
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::CancelGoBack.id().into())],
            Instant::now()
        ));
        assert!(!state.is_playing());
        assert_eq!(
            state
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .selected_count(),
            count
        );
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::CancelGoBack.id().into())],
            Instant::now()
        ));
        assert!(state.event_selection.is_none());
        let mut ack = deadsync_gameplay::GameplayAudioSnapshot::default();
        ack.editor.owner = state.editor_asset_owner();
        ack.editor.output.available = true;
        ack.editor.output.leased = true;
        ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        ack.editor.output.requested_period = Some(512);
        ack.editor.output.negotiated_period = Some(512);
        state.update_audio(ack);
        state.observe_editor_assets(EditorAssetReadiness {
            owner: state.editor_asset_owner(),
            registration: state.editor_asset_registration(),
            main: true,
            dedicated_preview: true,
        });
        state.take_audio_commands();
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::TogglePreview.id().into())],
            Instant::now()
        ));
        assert!(state.is_playing(), "{locale}: {}", state.status);
        assert!(!state.chart_transport_is_playing());
        let commands = state.take_audio_commands();
        assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorPlayPreview{cut,looping:true,rate,..} if cut.start_sec==0.5 && cut.length_sec==1.0 && *rate==1.0)),"{commands:?}");
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::TogglePreview.id().into())],
            Instant::now()
        ));
        assert!(!state.is_playing());
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::TogglePreview.id().into())],
            Instant::now()
        ));
        assert!(state.is_playing());
        assert!(state.command_binding_dispatch(
            vec![Effect::Press(Action::CancelGoBack.id().into())],
            Instant::now()
        ));
        assert!(!state.is_playing());
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(state.workspace.file().session.undo_label().is_none());
    }
}
