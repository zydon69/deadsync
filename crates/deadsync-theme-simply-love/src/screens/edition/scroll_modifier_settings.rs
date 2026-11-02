use super::*;
use deadsync_editor::key_binding_preferences::{
    Preferences,
    inspector::{Capture, CapturePhase},
    scroll_modifiers::{Kind, Query},
};
pub(super) struct Settings {
    before: Arc<Preferences>,
    after: Preferences,
    kind: Kind,
    alternative: usize,
    capture: Option<Capture>,
    return_keys: bool,
}
impl Settings {
    pub(super) fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }
    pub(super) fn release_input(&mut self) {
        self.capture = None;
    }
}
impl State {
    pub(super) fn open_scroll_modifiers(&mut self) {
        let return_keys = matches!(self.panel, Some(Panel::KeyBindingSettings(_)));
        let (before, after) = if return_keys {
            let Some(settings) = self.key_binding_draft.as_ref() else {
                return;
            };
            (
                settings.draft.baseline(),
                (*settings.draft.staged()).clone(),
            )
        } else {
            let before = Arc::clone(&self.command_bindings.preferences);
            let after = (*before).clone();
            (before, after)
        };
        self.release_input();
        self.scroll_modifier_settings = Some(Settings {
            after,
            before,
            kind: Kind::Zoom,
            alternative: 0,
            capture: None,
            return_keys,
        });
        self.panel = Some(Panel::ScrollModifierSettings(0));
    }
    pub(super) fn scroll_modifier_capture_event(&mut self, event: &RawKeyboardEvent) -> bool {
        let keys = self.scroll_modifier_capture_keys();
        let Some(settings) = self.scroll_modifier_settings.as_mut() else {
            return false;
        };
        let Some(capture) = settings.capture.as_mut() else {
            return false;
        };
        if capture.phase() == CapturePhase::Review {
            if event.pressed && !event.repeat {
                if event.code == KeyCode::Escape {
                    settings.capture = None;
                } else if matches!(event.code, KeyCode::Enter | KeyCode::NumpadEnter) {
                    let mut alternatives = settings
                        .after
                        .scroll_modifiers
                        .get(settings.kind)
                        .alternatives()
                        .to_vec();
                    if alternatives.is_empty() {
                        alternatives.push(vec![]);
                        settings.alternative = 0;
                    }
                    alternatives[settings.alternative] = capture.keys().to_vec();
                    settings
                        .after
                        .scroll_modifiers
                        .set(settings.kind, Query::from_alternatives(alternatives));
                    settings.capture = None;
                }
            }
        } else {
            capture.observe(&keys);
        }
        true
    }
    pub(super) fn scroll_modifier_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 7 && enter) {
            let return_keys = self
                .scroll_modifier_settings
                .as_ref()
                .is_some_and(|settings| settings.return_keys);
            self.scroll_modifier_settings = None;
            self.panel = if return_keys && self.key_binding_draft.is_some() {
                Some(Panel::KeyBindingSettings(0))
            } else {
                None
            };
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::ScrollModifierSettings(
                (index + if back { 7 } else { 1 }) % 8,
            ));
            return;
        }
        if !(enter || matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight)) {
            return;
        }
        let Some(settings) = self.scroll_modifier_settings.as_mut() else {
            return;
        };
        let mut alternatives = settings
            .after
            .scroll_modifiers
            .get(settings.kind)
            .alternatives()
            .to_vec();
        if alternatives.is_empty() {
            alternatives.push(vec![]);
            settings.alternative = 0;
        }
        match index {
            0 => {
                let current = Kind::ALL
                    .iter()
                    .position(|kind| *kind == settings.kind)
                    .expect("complete held-query kinds");
                settings.kind = Kind::ALL[(current
                    + if code == KeyCode::ArrowLeft {
                        Kind::ALL.len() - 1
                    } else {
                        1
                    })
                    % Kind::ALL.len()];
                settings.alternative = 0;
            }
            1 => {
                settings.alternative = (settings.alternative
                    + if code == KeyCode::ArrowLeft {
                        alternatives.len() - 1
                    } else {
                        1
                    })
                    % alternatives.len()
            }
            2 if enter => settings.capture = Some(Capture::new()),
            3 if enter => {
                alternatives.push(vec![]);
                settings.alternative = alternatives.len() - 1;
            }
            4 if enter => {
                if alternatives.len() > 1 {
                    alternatives.remove(settings.alternative);
                    settings.alternative = settings.alternative.min(alternatives.len() - 1);
                } else {
                    alternatives[0].clear();
                }
            }
            5 if enter => {
                settings.after.scroll_modifiers.set(
                    settings.kind,
                    if settings.after.source_order {
                        Query::defaults(settings.kind, cfg!(target_os = "macos"))
                    } else {
                        Preferences::default()
                            .scroll_modifiers
                            .get(settings.kind)
                            .clone()
                    },
                );
                settings.alternative = 0;
                return;
            }
            6 if enter => {
                let before = Arc::clone(&settings.before);
                let after = Arc::new(settings.after.clone());
                let result = (|| -> Result<(), deadsync_editor::Error> {
                    let file = self.capture_application_preferences_file()?;
                    self.workspace
                        .set_key_binding_preferences(before, after, Some(&file))?;
                    self.sync_key_binding_preferences_history();
                    Ok(())
                })();
                match result {
                    Ok(()) => {
                        if self
                            .scroll_modifier_settings
                            .as_ref()
                            .is_some_and(|settings| settings.return_keys)
                        {
                            self.key_binding_draft = None;
                        }
                        self.scroll_modifier_settings = None;
                        self.panel = None;
                    }
                    Err(error) => self.status = i18n::editor_error(&error).into(),
                }
                return;
            }
            _ => return,
        }
        if matches!(index, 3 | 4) {
            settings
                .after
                .scroll_modifiers
                .set(settings.kind, Query::from_alternatives(alternatives));
        }
    }
    pub(super) fn scroll_modifier_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let Some(settings) = self.scroll_modifier_settings.as_ref() else {
            return;
        };
        let x = screen_width() * 0.5;
        let top = screen_height() * 0.5 - 145.;
        actors
            .push(act!(quad:xy(x,top+130.):setsize(620.,340.):diffuse(0.04,0.06,0.1,0.98):z(8050)));
        let alternatives = settings
            .after
            .scroll_modifiers
            .get(settings.kind)
            .alternatives();
        let chord = alternatives.get(settings.alternative);
        for (row, key) in [
            "ScrollModifierKind",
            "KeyBindingAlternative",
            "KeyBindingCapture",
            "KeyBindingAdd",
            "KeyBindingDelete",
            "KeyBindingReset",
            "ScrollSettingsSave",
            "ScrollSettingsCancel",
        ]
        .iter()
        .enumerate()
        {
            let start = actors.len();
            let label = match row {
                0 => i18n::tr_fmt(
                    "Edition",
                    key,
                    &[(
                        "value",
                        &i18n::tr(
                            "Edition",
                            match settings.kind {
                                Kind::Zoom => "ScrollModifierZoom",
                                Kind::Spacing => "ScrollModifierSpacing",
                                Kind::SelectionMisc => "HeldSelectionMisc",
                                Kind::SelectionToggle => "HeldSelectionToggle",
                                Kind::SelectionExtend => "HeldSelectionExtend",
                                Kind::ReceptorAxis => "HeldReceptorAxis",
                            },
                        ),
                    )],
                ),
                1 => {
                    let keys = chord.map_or_else(
                        || i18n::tr("Edition", "KeyBindingUnbound").to_string(),
                        |chord| {
                            if chord.is_empty() {
                                i18n::tr("Edition", "ScrollModifierAlways").to_string()
                            } else {
                                super::command_bindings::chord_label(chord)
                            }
                        },
                    );
                    i18n::tr_fmt(
                        "Edition",
                        key,
                        &[
                            ("index", &(settings.alternative + 1).to_string()),
                            ("total", &alternatives.len().to_string()),
                            ("keys", &keys),
                        ],
                    )
                }
                _ => i18n::tr("Edition", key),
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+row as f32*30.):zoom(0.62):maxwidth(580.):diffuse(1.,if row==index{0.8}else{1.},if row==index{0.3}else{1.},1.):z(8051)));
            self.ui_row(
                actors,
                start,
                row == index && settings.capture.is_none(),
                30.,
            );
        }
        let start = actors.len();
        let help = if let Some(capture) = &settings.capture {
            i18n::tr_fmt(
                "Edition",
                if capture.phase() == CapturePhase::Review {
                    "KeyBindingReview"
                } else {
                    "KeyBindingRecording"
                },
                &[(
                    "keys",
                    &super::command_bindings::chord_label(capture.keys()),
                )],
            )
        } else {
            i18n::tr("Edition", "ScrollModifierHelp")
        };
        actors.push(act!(text:font("miso"):settext(help):xy(x,top+270.):zoom(0.5):maxwidth(580.):maxheight(55.):z(8052)));
        self.ui_row(actors, start, settings.capture.is_some(), 56.);
    }
}

#[cfg(test)]
mod capture_workflow_tests {
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
    fn raw_modifier_capture_reaches_review_and_commits_typed_query() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = State::without_song(root.path()).unwrap();
        state.panel = None;
        state.open_scroll_settings();
        press(&mut state, KeyCode::F11);
        assert!(matches!(
            state.panel,
            Some(Panel::ScrollModifierSettings(0))
        ));
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::Enter);
        assert_eq!(
            state
                .scroll_modifier_settings
                .as_ref()
                .unwrap()
                .capture
                .as_ref()
                .unwrap()
                .phase(),
            CapturePhase::Recording
        );
        press(&mut state, KeyCode::KeyA);
        assert_eq!(
            state
                .scroll_modifier_settings
                .as_ref()
                .unwrap()
                .capture
                .as_ref()
                .unwrap()
                .phase(),
            CapturePhase::Review
        );
        press(&mut state, KeyCode::Enter);
        assert_eq!(
            state
                .scroll_modifier_settings
                .as_ref()
                .unwrap()
                .after
                .scroll_modifiers
                .get(Kind::Zoom)
                .alternatives(),
            &[vec!["A".to_owned()]]
        );
        for _ in 0..4 {
            press(&mut state, KeyCode::ArrowDown);
        }
        press(&mut state, KeyCode::Enter);
        assert!(state.panel.is_none());
        assert_eq!(
            state
                .command_bindings
                .preferences
                .scroll_modifiers
                .get(Kind::Zoom)
                .alternatives(),
            &[vec!["A".to_owned()]]
        );
    }
}
#[cfg(test)]
mod held_query_inspector_tests {
    use super::*;
    fn press(s: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            s.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn fixture(language: &str, disk: bool) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let dir = tempfile::tempdir().unwrap();
        let mut s = State::without_song(dir.path()).unwrap();
        s.panel = None;
        if disk {
            let path = dir.path().join("prefs.json");
            std::fs::write(&path, br#"{"Version":1,"KeyBinds":{"Vendor":17}}"#).unwrap();
            s.configure_audio_preferences(&path).unwrap();
        }
        (dir, s)
    }
    fn open(s: &mut State, kind: usize) {
        s.open_key_binding_settings();
        press(s, KeyCode::F10);
        assert!(matches!(s.panel, Some(Panel::ScrollModifierSettings(0))));
        for _ in 0..kind {
            press(s, KeyCode::ArrowRight);
        }
        press(s, KeyCode::ArrowDown);
        press(s, KeyCode::ArrowDown);
        press(s, KeyCode::Enter);
    }
    #[test]
    fn four_held_queries_have_real_raw_capture_canonical_save_undo_redo_and_reopen_en_fr() {
        for language in ["en", "fr"] {
            for disk in [false, true] {
                for index in 2..6 {
                    let (_dir, mut s) = fixture(language, disk);
                    let before = Arc::clone(&s.command_bindings.preferences);
                    let bytes = s.workspace.file().session.serialize().unwrap();
                    open(&mut s, index);
                    let kind = Kind::ALL[index];
                    press(&mut s, KeyCode::F13);
                    assert_eq!(
                        s.scroll_modifier_settings
                            .as_ref()
                            .unwrap()
                            .capture
                            .as_ref()
                            .unwrap()
                            .phase(),
                        CapturePhase::Review
                    );
                    press(&mut s, KeyCode::Enter);
                    assert_eq!(s.command_bindings.preferences, before);
                    // Capture edits the selected alternative; it must not
                    // delete native Ctrl/Super or other untouched alternatives.
                    let mut expected = before.scroll_modifiers.get(kind).alternatives().to_vec();
                    expected[0] = vec!["F13".to_owned()];
                    assert_eq!(
                        s.scroll_modifier_settings
                            .as_ref()
                            .unwrap()
                            .after
                            .scroll_modifiers
                            .get(kind)
                            .alternatives(),
                        expected.as_slice()
                    );
                    for _ in 0..4 {
                        press(&mut s, KeyCode::ArrowDown);
                    }
                    press(&mut s, KeyCode::Enter);
                    assert!(s.panel.is_none(), "{}", s.status);
                    let after = Arc::clone(&s.command_bindings.preferences);
                    assert_ne!(after, before);
                    let owner = s.capture_application_preferences_file().unwrap();
                    assert_eq!(owner.key_bindings(), after);
                    assert!(!owner.has_changes());
                    if disk {
                        assert_eq!(
                            deadsync_editor::audio_preferences::File::open(owner.path())
                                .unwrap()
                                .key_bindings(),
                            after
                        );
                    }
                    for (redo, expected) in [(false, &before), (true, &after)] {
                        s.command(if redo { Command::Redo } else { Command::Undo });
                        assert_eq!(&s.command_bindings.preferences, expected, "{}", s.status);
                        assert_eq!(s.workspace.file().session.serialize().unwrap(), bytes);
                        assert!(!s.workspace.has_chart());
                    }
                }
            }
        }
    }
    #[test]
    fn held_query_subpanel_preserves_unsaved_callback_draft_on_cancel_and_publishes_both_on_save() {
        let (_dir, mut s) = fixture("en", false);
        let baseline = Arc::clone(&s.command_bindings.preferences);
        s.open_key_binding_settings();
        let action = s
            .key_binding_draft
            .as_ref()
            .unwrap()
            .draft
            .action()
            .id()
            .to_owned();
        s.key_binding_draft
            .as_mut()
            .unwrap()
            .draft
            .replace(vec!["F14".into()])
            .unwrap();
        press(&mut s, KeyCode::F10);
        press(&mut s, KeyCode::ArrowRight);
        press(&mut s, KeyCode::ArrowRight);
        s.scroll_modifier_settings
            .as_mut()
            .unwrap()
            .after
            .scroll_modifiers
            .set(
                Kind::SelectionMisc,
                Query::from_alternatives(vec![vec!["F13".into()]]),
            );
        press(&mut s, KeyCode::Escape);
        assert!(matches!(s.panel, Some(Panel::KeyBindingSettings(0))));
        assert_eq!(
            s.key_binding_draft
                .as_ref()
                .unwrap()
                .draft
                .staged()
                .commands[&action],
            vec![vec!["F14"]]
        );
        assert_eq!(s.command_bindings.preferences, baseline);
        press(&mut s, KeyCode::F10);
        s.scroll_modifier_settings
            .as_mut()
            .unwrap()
            .after
            .scroll_modifiers
            .set(
                Kind::SelectionMisc,
                Query::from_alternatives(vec![vec!["F13".into()]]),
            );
        s.panel = Some(Panel::ScrollModifierSettings(6));
        press(&mut s, KeyCode::Enter);
        assert!(s.key_binding_draft.is_none());
        assert_eq!(
            s.command_bindings.preferences.commands[&action],
            vec![vec!["F14"]]
        );
        assert_eq!(
            s.command_bindings
                .preferences
                .scroll_modifiers
                .get(Kind::SelectionMisc)
                .alternatives(),
            &[vec!["F13".to_owned()]]
        );
        s.command(Command::Undo);
        assert_eq!(s.command_bindings.preferences, baseline);
    }
}
