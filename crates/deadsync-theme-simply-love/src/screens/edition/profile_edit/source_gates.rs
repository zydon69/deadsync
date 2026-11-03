use super::super::RawKeyboardEvent;
use super::*;
fn open(kind: Kind) -> (tempfile::TempDir, State) {
    crate::i18n::init_for_tests();
    let root = tempfile::tempdir().unwrap();
    let song = root.path().join("song.ssc");
    std::fs::write(
        &song,
        b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
    )
    .unwrap();
    let mut state = State::open(&song, 0).unwrap();
    state.open_profile_manager();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while state.catalog_job.is_some() {
        state.update();
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let row = match kind {
        Kind::Pattern => 0,
        Kind::Performed => 1,
        Kind::Expressed => 2,
    };
    state.open_profile_edit(kind, Panel::Profiles(row));
    assert!(state.profile_edit.is_some(), "{}", state.status);
    (root, state)
}
fn index(state: &State, name: &str) -> usize {
    state
        .profile_edit
        .as_ref()
        .unwrap()
        .editor
        .fields()
        .iter()
        .position(|field| field.name() == name)
        .unwrap()
}
fn force_false(state: &mut State, name: &str) {
    let i = index(state, name);
    for _ in 0..3 {
        let draft = state.profile_edit.as_mut().unwrap();
        if draft.editor.value(i).unwrap() == &serde_json::json!(false) {
            return;
        }
        draft.editor.step(i, true, false).unwrap();
    }
    panic!("flag did not reach false");
}
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
    }
}
fn values(state: &State) -> Vec<serde_json::Value> {
    let editor = &state.profile_edit.as_ref().unwrap().editor;
    (0..editor.fields().len())
        .map(|i| editor.value(i).unwrap().clone())
        .collect()
}
#[test]
fn raw_disabled_control_intentions_do_not_mutate_draft_document_history_or_start_input() {
    for (kind, flag, field) in [
        (
            Kind::Pattern,
            "LimitSameArrowsInARowPerFoot",
            "MaxSameArrowsInARowPerFoot",
        ),
        (
            Kind::Performed,
            "SpeedTighteningEnabled",
            "SpeedTighteningMinDistance",
        ),
        (Kind::Performed, "Enabled", "RelativeNPS"),
    ] {
        let (_root, mut state) = open(kind);
        force_false(&mut state, flag);
        let i = index(&state, field);
        // In Performed, the first generic Enabled belongs to LateralTightening.
        assert!(!state.profile_edit.as_ref().unwrap().row_enabled(i));
        let before = values(&state);
        let document = state.workspace.file().session.serialize().unwrap();
        let revision = state.workspace.file().session.revision();
        for code in [
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::Enter,
            KeyCode::Digit1,
            KeyCode::Delete,
            KeyCode::Backspace,
        ] {
            state.panel = Some(Panel::ProfileEdit(i));
            press(&mut state, code);
            assert_eq!(values(&state), before);
            assert!(state.profile_edit.as_ref().unwrap().input.is_none());
        }
        assert_eq!(
            state.workspace.file().session.serialize().unwrap(),
            document
        );
        assert_eq!(state.workspace.file().session.revision(), revision);
        assert!(
            state
                .profile_edit_label(i, "ProfileEditField")
                .contains("inacti")
        );
    }
}
#[test]
fn hidden_rows_are_absent_from_render_mapping_and_skipped_by_raw_navigation_then_reappear_intact() {
    let (_root, mut state) = open(Kind::Pattern);
    let selector = index(&state, "StartingFootChoice");
    let specified = index(&state, "StartingFootSpecified");
    // Configure source Automatic explicitly, independent of selected preset.
    for _ in 0..3 {
        if state
            .profile_edit
            .as_ref()
            .unwrap()
            .editor
            .value(selector)
            .unwrap()
            == &serde_json::json!("automatic")
        {
            break;
        }
        state
            .profile_edit
            .as_mut()
            .unwrap()
            .editor
            .step(selector, true, false)
            .unwrap();
    }
    assert!(
        !state
            .profile_edit
            .as_ref()
            .unwrap()
            .visible_rows()
            .contains(&specified)
    );
    let preserved = values(&state)[specified].clone();
    state.panel = Some(Panel::ProfileEdit(selector));
    press(&mut state, KeyCode::ArrowDown);
    assert!(!matches!(state.panel,Some(Panel::ProfileEdit(i)) if i==specified));
    state.panel = Some(Panel::ProfileEdit(selector));
    press(&mut state, KeyCode::ArrowRight);
    assert_eq!(
        state
            .profile_edit
            .as_ref()
            .unwrap()
            .editor
            .availability(specified),
        Some(Availability::Active)
    );
    assert!(
        state
            .profile_edit
            .as_ref()
            .unwrap()
            .visible_rows()
            .contains(&specified)
    );
    press(&mut state, KeyCode::ArrowDown);
    assert!(matches!(state.panel,Some(Panel::ProfileEdit(i)) if i==specified));
    assert_eq!(values(&state)[specified], preserved);
}
#[test]
fn source_speed_gate_blocks_f4_panel_entry_and_stale_panel_edit_without_touching_bpm() {
    let (_root, mut state) = open(Kind::Performed);
    force_false(&mut state, "SpeedTighteningEnabled");
    let before = values(&state);
    state.panel = Some(Panel::ProfileEdit(0));
    press(&mut state, KeyCode::F4);
    assert!(matches!(state.panel, Some(Panel::ProfileEdit(_))));
    assert!(state.profile_edit.as_ref().unwrap().travel_editor.is_none());
    state.profile_edit.as_mut().unwrap().travel_editor = Some(TravelSpeed::editor_defaults());
    let speed = state.profile_edit.as_ref().unwrap().travel_editor.unwrap();
    state.profile_speed_key(1, KeyCode::Digit9);
    state.profile_speed_key(0, KeyCode::ArrowRight);
    state.profile_speed_key(5, KeyCode::Enter);
    assert_eq!(
        state.profile_edit.as_ref().unwrap().travel_editor,
        Some(speed)
    );
    assert!(state.profile_edit.as_ref().unwrap().travel_input.is_none());
    assert_eq!(values(&state), before);
    state.profile_speed_key(6, KeyCode::Enter);
    assert!(state.profile_edit.as_ref().unwrap().travel_editor.is_none());
}
