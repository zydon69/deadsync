//! Global selection rules can be configured without a chart or a selection gesture.
use super::*;
use deadsync_editor::selection_preferences::{Overlap, Preferences, RegionMode};
const KEYS: [&str; 5] = [
    "SelectionPreferencesDrag",
    "SelectionPreferencesClick",
    "ScrollSettingsDefaults",
    "ScrollSettingsSave",
    "ScrollSettingsCancel",
];
impl State {
    pub(super) fn open_selection_settings(&mut self) {
        self.minimap_release_input();
        self.receptor_views.release_input();
        self.lane_entry_release_input();
        self.selection_preferences_draft = Some(self.selection_preferences);
        self.panel = Some(Panel::SelectionSettings(0));
        self.status = i18n::tr("Edition", "SelectionPreferencesHelp");
    }
    pub(super) fn selection_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || index == 4 && enter {
            self.selection_preferences_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || code == KeyCode::Tab && self.shift;
            self.panel = Some(Panel::SelectionSettings(
                (index + if back { 4 } else { 1 }) % 5,
            ));
            return;
        }
        if index < 2
            && (enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
                ))
        {
            let Some(draft) = self.selection_preferences_draft.as_mut() else {
                return;
            };
            let reset = code == KeyCode::Home;
            let back = code == KeyCode::ArrowLeft;
            if index == 0 {
                draft.overlap = if reset {
                    Preferences::default().overlap
                } else {
                    Overlap::ALL[(draft.overlap.index() + if back { 2 } else { 1 }) % 3]
                };
            } else {
                draft.region = if reset {
                    Preferences::default().region
                } else {
                    RegionMode::ALL[(draft.region.index() + 1) % 2]
                };
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 2 {
            self.selection_preferences_draft = Some(Preferences::default());
            return;
        }
        if index != 3 {
            return;
        }
        let Some(after) = self.selection_preferences_draft else {
            return;
        };
        let result = self
            .capture_application_preferences_file()
            .and_then(|file| {
                self.workspace.set_selection_preferences(
                    self.selection_preferences,
                    after,
                    Some(&file),
                )
            });
        match result {
            Ok(_) => {
                self.sync_selection_preferences_history();
                self.selection_preferences_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "SelectionPreferencesSaved");
            }
            Err(error) => self.status = i18n::editor_error(&error).into(),
        }
    }
    pub(super) fn selection_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let p = self
            .selection_preferences_draft
            .unwrap_or(self.selection_preferences);
        let x = screen_width() * 0.5;
        let y = screen_height() * 0.5;
        actors.push(act!(quad:xy(x,y):zoomto(620.0,224.0):diffuse(0.04,0.06,0.1,0.98):z(8020)));
        for (row, key) in KEYS.into_iter().enumerate() {
            let label = if row < 2 {
                let value = if row == 0 {
                    match p.overlap {
                        Overlap::Any => i18n::tr("Edition", "RegionAny"),
                        Overlap::Center => i18n::tr("Edition", "RegionCenter"),
                        Overlap::All => i18n::tr("Edition", "RegionAll"),
                        Overlap::Unknown(raw) => i18n::tr_fmt(
                            "Edition",
                            "SelectionPreferencesRaw",
                            &[("value", &raw.to_string())],
                        ),
                    }
                } else {
                    match p.region {
                        RegionMode::TimeOrPosition => i18n::tr("Edition", "RegionTimeOnly"),
                        RegionMode::TimeOrPositionAndLane => {
                            i18n::tr("Edition", "RegionTimeAndLanes")
                        }
                        RegionMode::Unknown(raw) => i18n::tr_fmt(
                            "Edition",
                            "SelectionPreferencesRaw",
                            &[("value", &raw.to_string())],
                        ),
                    }
                };
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            } else {
                i18n::tr("Edition", key)
            };
            let c = if row == index {
                [1.0, 0.85, 0.25, 1.0]
            } else {
                [0.8, 0.8, 0.85, 1.0]
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,y-78.0+row as f32*36.0):zoom(0.65):maxwidth(590.0):diffuse(c[0],c[1],c[2],c[3]):z(8021)));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
    #[test]
    fn independent_selection_preferences_are_reachable_chartless_and_preserve_selection_and_song() {
        for language in ["en", "fr"] {
            for disk in [false, true] {
                for has_chart in [false, true] {
                    i18n::init(deadsync_assets::language::load_for_tests(language));
                    let root = tempfile::tempdir().unwrap();
                    let path = root.path().join("song.ssc");
                    std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100;").unwrap();
                    let mut state = if has_chart {
                        State::open(&path, 0).unwrap()
                    } else {
                        State::without_song(root.path()).unwrap()
                    };
                    if disk {
                        let prefs = root.path().join("prefs.json");
                        std::fs::write(
                            &prefs,
                            br#"{"Version":1,"Selection":{"Future":7},"Vendor":{"keep":true}}"#,
                        )
                        .unwrap();
                        state.configure_audio_preferences(&prefs).unwrap();
                    }
                    let file = state.capture_application_preferences_file().unwrap();
                    let draft=state.workspace.prepare_source_preferences(&file,br#"{"PreferencesSelection":{"Mode":"OverlapAll","RegionMode":"TimeOrPositionAndLane"},"VendorSource":{"keep":[1,9]}}"#,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();
                    state
                        .workspace
                        .import_source_preferences(draft, &file, true)
                        .unwrap();
                    state.sync_source_preferences_history().unwrap();
                    if has_chart {
                        let mut selection = state.workspace.event_selection().unwrap();
                        selection.select_all(true);
                        state.replace_region_event_selection(selection);
                    }
                    let selected = state
                        .event_selection
                        .as_ref()
                        .map(|s| s.selection.selected_events().collect::<Vec<_>>());
                    let range = state.workspace.selection();
                    let song = state.workspace.file().session.serialize().unwrap();
                    let before = state.selection_preferences;
                    let revision = state.workspace.file().session.revision();
                    state.open_general_options();
                    for _ in 0..5 {
                        press(&mut state, KeyCode::ArrowDown);
                    }
                    press(&mut state, KeyCode::Enter);
                    assert!(matches!(state.panel, Some(Panel::SelectionSettings(0))));
                    let mut actors = Vec::new();
                    state.actors(&mut actors);
                    assert!(!actors.is_empty());
                    press(&mut state, KeyCode::ArrowRight);
                    state.panel = Some(Panel::SelectionSettings(4));
                    press(&mut state, KeyCode::Enter);
                    assert_eq!(state.selection_preferences, before);
                    assert_eq!(state.workspace.file().session.revision(), revision);
                    state.open_selection_settings();
                    state.panel = Some(Panel::SelectionSettings(2));
                    press(&mut state, KeyCode::Enter);
                    assert_eq!(
                        state.selection_preferences_draft,
                        Some(Preferences::default())
                    );
                    state.panel = Some(Panel::SelectionSettings(3));
                    press(&mut state, KeyCode::Enter);
                    assert_eq!(state.selection_preferences, Preferences::default());
                    let owner = state.capture_application_preferences_file().unwrap();
                    assert_eq!(owner.selection(), Preferences::default());
                    assert!(owner.source_archive().is_some());
                    assert!(!owner.has_changes());
                    if disk {
                        let out: serde_json::Value =
                            serde_json::from_slice(&std::fs::read(owner.path()).unwrap()).unwrap();
                        assert_eq!(out["Selection"]["Future"], 7);
                        assert_eq!(out["Vendor"]["keep"], true);
                    }
                    state.command(Command::Undo);
                    assert_eq!(state.selection_preferences, before);
                    state.command(Command::Redo);
                    assert_eq!(state.selection_preferences, Preferences::default());
                    assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
                    assert_eq!(state.workspace.selection(), range);
                    assert_eq!(
                        state
                            .event_selection
                            .as_ref()
                            .map(|s| s.selection.selected_events().collect::<Vec<_>>()),
                        selected
                    );
                    assert_eq!(state.workspace.has_chart(), has_chart);
                }
            }
        }
    }
    #[test]
    fn global_click_mode_saved_without_a_gesture_changes_the_real_next_shift_selection() {
        use deadsync_editor::event_clipboard::region::Modifiers;
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("click.ssc");
        std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0001\n0100\n0000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let selected = |state: &mut State| {
            let mut choices = state.workspace.event_selection_between(0, 192).unwrap();
            let events = choices
                .events()
                .iter()
                .copied()
                .filter(|event| matches!(event, deadsync_editor::event_clipboard::Event::Note(_)))
                .collect::<Vec<_>>();
            assert_eq!(events.len(), 3);
            let session = &state.workspace.file().session;
            choices
                .finish_region(
                    session,
                    &events[..1],
                    Modifiers::default(),
                    state.selection_preferences.region,
                )
                .unwrap();
            choices
                .finish_region(
                    session,
                    &events[2..],
                    Modifiers {
                        shift: true,
                        ..Modifiers::default()
                    },
                    state.selection_preferences.region,
                )
                .unwrap();
            choices.selected_count()
        };
        assert_eq!(selected(&mut state), 3);
        state.open_selection_settings();
        state.panel = Some(Panel::SelectionSettings(1));
        press(&mut state, KeyCode::ArrowRight);
        state.panel = Some(Panel::SelectionSettings(3));
        press(&mut state, KeyCode::Enter);
        assert_eq!(selected(&mut state), 2);
        state.command(Command::Undo);
        assert_eq!(selected(&mut state), 3);
        state.command(Command::Redo);
        assert_eq!(selected(&mut state), 2);
    }
}
