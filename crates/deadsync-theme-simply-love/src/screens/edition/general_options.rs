//! Keyboard General Options with live consumers; deferred capability fields
//! remain preserved in the typed model until their own consumers are ported.
use super::*;
use deadsync_editor::options_preferences::General;
pub(super) const GENERAL_KEYS: [&str; 6] =
    ["GeneralField", "GeneralValue", "GeneralVisibleDefaults", "GeneralAllDefaults", "SourcePreferencesOpen", "SelectionPreferencesOpen"];
impl State {
    pub(super) fn commit_song_notification_suppression(&mut self,value:bool)->Result<bool,deadsync_editor::Error>{
        let file=self.capture_application_preferences_file()?;
        let changed=self.workspace.set_suppress_external_song_notifications(file.suppress_external_song_notifications(),value,&file)?;
        self.sync_restore_options_history()?;Ok(changed)
    }
    pub(super) fn commit_recent_files_history_size(&mut self, value: usize) -> Result<bool,deadsync_editor::Error> {
        let file=self.capture_application_preferences_file()?;
        let changed=self.workspace.set_recent_files_history_size(self.file_menu.recent_preferences().maximum,value,&file)?;
        self.sync_restore_options_history()?;
        Ok(changed)
    }
    pub fn general_options(&self) -> &General {
        &self.general_options
    }
    pub fn configure_general_options(
        &mut self,
        value: General,
    ) -> Result<(), deadsync_editor::Error> {
        value.validate()?;
        self.workspace
            .set_history_capacity(value.effective_history_size())?;
        self.general_options = value;
        Ok(())
    }
    pub(super) fn sync_general_options_history(&mut self) {
        if let Some(update) = self.workspace.take_general_options_update() {
            // effective size already applied by the core action, including Undo/Redo.
            self.general_options = update.preferences;
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
        }
    }
    pub(super) fn commit_general_options(
        &mut self,
        value: General,
    ) -> Result<bool, deadsync_editor::Error> {
        if matches!(self.panel, Some(Panel::Ftue(_))) {
            // Source UIFTUE passes undoable=false to these General controls.
            value.validate()?;
            if value == self.general_options { return Ok(false); }
            let current = self.capture_application_preferences_file()?;
            let before = self.general_options.clone();
            let mut owner = Some(current.clone());
            let merged = deadsync_editor::audio_preferences::publication::publish(
                &mut owner, Some(&current), value.clone(), |current| {
                    let mut file = current.clone();
                    let merged = file.general_options().merge_changes(&before, &value)?;
                    file.set_general_options(merged.clone())?;
                    Ok((merged, file))
                },
            )?;
            self.configure_general_options(merged)?;
            if let Some(file) = owner {self.publish_application_preferences_owner(file);}
            return Ok(true);
        }
        let file = self.capture_application_preferences_file()?;
        let changed = self.workspace.set_general_options(
            self.general_options.clone(),
            value,
            Some(&file),
        )?;
        self.sync_general_options_history();
        Ok(changed)
    }
    pub(super) fn open_general_options(&mut self) {
        self.lane_entry_release_input();
        self.general_field = 0;
        self.panel = Some(Panel::GeneralOptions(0));
        self.status = i18n::tr("Edition", "GeneralOptionsHelp");
    }
    pub(super) fn general_options_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if index==5&&enter{self.open_selection_settings();return;}
        if index==4&&enter{self.open_source_preferences_import();return;}
        if self.startup_graph_key(index, code) {
            return;
        }
        if code == KeyCode::Escape {
            self.panel = None;
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::GeneralOptions(
                (index + if back { 5 } else { 1 }) % 6,
            ));
            return;
        }
        // A disabled value cannot be edited, but it must never trap keyboard
        // focus or prevent closing the panel.
        if index == 1 && self.general_field == 8 && !self.general_options.custom_dpi {
            return;
        }
        if index >= 4{return;}
        if index == 3 { if enter {self.restore_all_options();}return;}
        if index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            self.general_field =
                (self.general_field + if code == KeyCode::ArrowLeft { 13 } else { 1 }) % 14;
            self.status = i18n::tr(
                "Edition",
                if self.general_field == 6 {
                    "GeneralStartupGraphHelp"
                } else {
                    "GeneralOptionsHelp"
                },
            );
            return;
        }
        if matches!(code, KeyCode::KeyI | KeyCode::KeyD) && !matches!(self.general_field, 1 | 2) {
            return;
        }
        if (index==1 || index==2 && enter) && self.general_field==13 {
            let value=match code {
                _ if index==2=>false,
                KeyCode::Home=>false,
                KeyCode::ArrowLeft|KeyCode::ArrowRight|KeyCode::Enter|KeyCode::NumpadEnter=>!self.options_snapshot().suppress_external_song,
                _=>return,
            };
            self.status=match self.commit_song_notification_suppression(value){Ok(_)=>i18n::tr("Edition","GeneralOptionsSaved"),Err(error)=>i18n::editor_error(&error).into()};return;
        }
        if (index==1 || index==2 && enter) && self.general_field==12 {
            let current=self.file_menu.recent_preferences().maximum;
            let value=match code {
                _ if index==2=>20,
                KeyCode::Home=>20,
                KeyCode::ArrowLeft=>current.saturating_sub(1).min(100),
                KeyCode::ArrowRight=>current.saturating_add(1).min(100),
                _=>return,
            };
            self.status=match self.commit_recent_files_history_size(value){Ok(_)=>i18n::tr("Edition","GeneralOptionsSaved"),Err(error)=>i18n::editor_error(&error).into()};
            return;
        }
        let mut value = self.general_options.clone();
        let defaults = General::default();
        if index == 2 && enter {
            value.undo_history_size = defaults.undo_history_size;
            value.new_sync = defaults.new_sync;
            value.open_sync = defaults.open_sync;
            value.open_last = defaults.open_last;
            value.default_steps_type = defaults.default_steps_type;
            value.default_difficulty = defaults.default_difficulty;
            value.startup_graphs = defaults.startup_graphs;
            value.custom_dpi=defaults.custom_dpi;value.dpi_scale=defaults.dpi_scale;value.suppress_pack=defaults.suppress_pack;value.suppress_update=defaults.suppress_update;value.suppress_opus=defaults.suppress_opus;
        } else if index == 1
            && matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::KeyI
                    | KeyCode::KeyD
                    | KeyCode::Enter
                    | KeyCode::NumpadEnter
            )
        {
            let reset = code == KeyCode::Home;
            let forward = code != KeyCode::ArrowLeft;
            match self.general_field {
                0 => {
                    if enter || matches!(code, KeyCode::KeyI | KeyCode::KeyD) {
                        return;
                    }
                    let step = if self.shift { 1 } else { 32 };
                    value.set_history_size(if reset {
                        defaults.undo_history_size
                    } else {
                        value
                            .undo_history_size
                            .saturating_add(if forward { step } else { -step })
                    });
                }
                1 | 2 => {
                    if enter {
                        return;
                    }
                    let offset = if self.general_field == 1 {
                        value.new_sync
                    } else {
                        value.open_sync
                    };
                    let next = match code {
                        KeyCode::Home | KeyCode::KeyI => 0.009,
                        KeyCode::KeyD => 0.,
                        _ => {
                            offset
                                + if forward {
                                    if self.shift { 0.0001 } else { 0.001 }
                                } else {
                                    if self.shift { -0.0001 } else { -0.001 }
                                }
                        }
                    };
                    let result = if self.general_field == 1 {
                        value.set_new_sync(next)
                    } else {
                        value.set_open_sync(next)
                    };
                    if result.is_err() {
                        return;
                    }
                }
                3 => value.open_last = if reset { true } else { !value.open_last },
                4 => {
                    if reset {
                        value.default_steps_type = 0
                    } else {
                        value.step_default_type(forward)
                    }
                }
                5 => {
                    if reset {
                        value.default_difficulty = 4
                    } else {
                        value.step_default_difficulty(forward)
                    }
                }
                7=>value.custom_dpi=if reset{false}else{!value.custom_dpi},
                8=>value.dpi_scale=if reset{1.}else{(value.dpi_scale+if forward{if self.shift{0.001}else{0.01}}else{if self.shift{-0.001}else{-0.01}}).clamp(0.25,8.)},
                9=>value.suppress_pack=if reset{false}else{!value.suppress_pack},
                10=>value.suppress_update=if reset{false}else{!value.suppress_update},
                11=>value.suppress_opus=if reset{false}else{!value.suppress_opus},
                _ => unreachable!("bounded General field"),
            }
        } else {
            return;
        }
        match self.commit_general_options(value) {
            Ok(_) => self.status = i18n::tr("Edition", "GeneralOptionsSaved"),
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "GeneralOptionsSaveFailedReason",
                    &[("error", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    pub(super) fn general_options_label(&self, index: usize, key: &str) -> Arc<str> {
        let field = i18n::tr(
            "Edition",
            [
                "GeneralUndoSize",
                "GeneralNewSync",
                "GeneralOpenSync",
                "GeneralOpenLast",
                "GeneralDefaultType",
                "GeneralDefaultDifficulty",
                "GeneralStartupGraphs",
                "GeneralCustomDpi",
                "GeneralDpiScale",
                "GeneralSuppressPack",
                "GeneralSuppressUpdate",
                "GeneralSuppressOpus",
                "GeneralRecentFilesMaximum",
                "GeneralSuppressSong",
            ][self.general_field],
        );
        if index == 0 {
            return i18n::tr_fmt("Edition", key, &[("value", &field)]);
        }
        if index != 1 {
            return i18n::tr("Edition", key);
        }
        let p = &self.general_options;
        let text = match self.general_field {
            0 => p.undo_history_size.to_string(),
            1 => format!("{:.6} s", p.new_sync),
            2 => format!("{:.6} s", p.open_sync),
            3 => i18n::tr(
                "Edition",
                if p.open_last {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
            4 => usize::try_from(p.default_steps_type)
                .ok()
                .and_then(|i| deadsync_editor::document::chart_type::ALL.get(i))
                .map(|s| s.to_string())
                .unwrap_or_else(|| p.default_steps_type.to_string()),
            5 => usize::try_from(p.default_difficulty)
                .ok()
                .and_then(|i| deadsync_editor::chart_identity::Difficulty::ALL.get(i))
                .map(|d| {
                    i18n::tr("Edition", &format!("GeneralDifficulty{}", d.as_str())).to_string()
                })
                .unwrap_or_else(|| p.default_difficulty.to_string()),
            6 => self.startup_graph_label(),
            7=>i18n::tr("Edition",if p.custom_dpi{"WaveformYes"}else{"WaveformNo"}).to_string(),
            8=>format!("{:.3} — {}",p.dpi_scale,i18n::tr("Edition",if self.ui_dpi_pending_restart(){"UiDpiRestartRequired"}else{"UiDpiRestartHelp"})),
            9..=11=>i18n::tr("Edition",if [p.suppress_pack,p.suppress_update,p.suppress_opus][self.general_field-9]{"WaveformYes"}else{"WaveformNo"}).to_string(),
            12=>self.file_menu.recent_preferences().maximum.to_string(),
            13=>i18n::tr("Edition",if self.options_snapshot().suppress_external_song{"WaveformYes"}else{"WaveformNo"}).to_string(),
            _ => unreachable!(),
        };
        i18n::tr_fmt("Edition", key, &[("value", &text)])
    }
}

#[cfg(test)]
mod disabled_dpi_navigation_tests {
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
    fn disabled_custom_dpi_value_remains_navigable_and_escape_closes_en_fr() {
        for language in ["en", "fr"] {
            for navigation in [
                KeyCode::ArrowUp,
                KeyCode::ArrowDown,
                KeyCode::Tab,
                KeyCode::Escape,
            ] {
                i18n::init(deadsync_assets::language::load_for_tests(language));
                let root = tempfile::tempdir().unwrap();
                let mut state = State::without_song(root.path()).unwrap();
                assert!(!state.general_options.custom_dpi);
                let before = state.general_options.clone();
                let document = state.workspace.file().session.serialize().unwrap();
                state.open_general_options();
                for _ in 0..8 {
                    press(&mut state, KeyCode::ArrowRight);
                }
                press(&mut state, KeyCode::ArrowDown);
                assert_eq!(state.general_field, 8);
                assert!(matches!(state.panel, Some(Panel::GeneralOptions(1))));

                // Disabled scalar controls must not mutate preferences or open
                // a numeric draft, regardless of their normal edit shortcuts.
                for code in [
                    KeyCode::ArrowLeft,
                    KeyCode::ArrowRight,
                    KeyCode::Home,
                    KeyCode::Enter,
                    KeyCode::NumpadEnter,
                ] {
                    press(&mut state, code);
                    assert!(matches!(state.panel, Some(Panel::GeneralOptions(1))));
                    assert_eq!(state.general_options, before);
                }
                press(&mut state, navigation);
                match navigation {
                    KeyCode::Escape => assert!(state.panel.is_none()),
                    KeyCode::ArrowUp => {
                        assert!(matches!(state.panel, Some(Panel::GeneralOptions(0))))
                    }
                    _ => assert!(matches!(state.panel, Some(Panel::GeneralOptions(2)))),
                }
                assert_eq!(state.general_options, before);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), document);
                assert!(state.workspace.file().session.undo_label().is_none());
            }
        }
    }
}

#[cfg(test)]
mod song_notification_setting_tests {
    use super::*;
    fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0});}}
    #[test]
    fn song_modified_checkbox_is_reachable_chartless_reversible_and_has_its_own_undo(){
        for language in ["en","fr"] {for disk in [false,true] {
            i18n::init(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();if disk {let path=root.path().join("prefs.json");std::fs::write(&path,br#"{"Version":1,"Options":{"Future":7},"Vendor":{"keep":true}}"#).unwrap();state.configure_audio_preferences(&path).unwrap();}
            let file=state.capture_application_preferences_file().unwrap();let draft=state.workspace.prepare_source_preferences(&file,br#"{"VendorSource":{"keep":[1,9]},"PreferencesOptions":{"SuppressExternalPackModificationNotification":true,"SuppressUpdateNotification":true,"SuppressOpusCodecUsageNotification":true}}"#,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();state.workspace.import_source_preferences(draft,&file,true).unwrap();state.sync_source_preferences_history().unwrap();let general=state.general_options.clone();let song=state.workspace.file().session.serialize().unwrap();assert!(!state.options_snapshot().suppress_external_song);
            state.open_general_options();for _ in 0..13 {press(&mut state,KeyCode::ArrowRight);}assert_eq!(state.general_field,13);press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::Enter);assert!(state.options_snapshot().suppress_external_song,"{}",state.status);assert_eq!(state.general_options,general);
            let file=state.capture_application_preferences_file().unwrap();assert!(!file.has_changes());assert!(file.source_archive().is_some());state.command(Command::Undo);assert!(!state.options_snapshot().suppress_external_song);state.command(Command::Redo);assert!(state.options_snapshot().suppress_external_song);
            press(&mut state,KeyCode::Home);assert!(!state.options_snapshot().suppress_external_song);state.command(Command::Undo);assert!(state.options_snapshot().suppress_external_song);state.command(Command::Redo);assert!(!state.options_snapshot().suppress_external_song);assert_eq!(state.general_options,general);assert_eq!(state.workspace.file().session.serialize().unwrap(),song);
            if disk {let reopened=deadsync_editor::audio_preferences::File::open(file.path()).unwrap();assert!(!reopened.suppress_external_song_notifications());let out:serde_json::Value=serde_json::from_slice(&std::fs::read(file.path()).unwrap()).unwrap();assert_eq!(out["Options"]["Future"],7);assert_eq!(out["Vendor"]["keep"],true);}
        }}
    }
}
