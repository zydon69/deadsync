use super::*;
use deadsync_editor::waveform_preferences::Preferences;
impl State {
    pub(super) fn open_waveform_settings(&mut self) {
        self.waveform_preferences_draft = Some(self.waveform_preferences);
        self.panel = Some(Panel::WaveformSettings(0));
        self.status = i18n::tr("Edition", "WaveformSettingsHelp");
    }
    pub(super) fn waveform_settings_label(&self, index: usize, key: &str) -> Arc<str> {
        let settings = self
            .waveform_preferences_draft
            .unwrap_or(self.waveform_preferences);
        let value = match index {
            0 => i18n::tr(
                "Edition",
                if settings.show() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
            1 => i18n::tr(
                "Edition",
                if settings.enabled() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
            2 => format!("{:.1}", settings.channel_width() * 100.0),
            3 => format!("{:.2}", settings.dense_scale()),
            4 => i18n::tr(
                "Edition",
                &format!("WaveformScroll{}", settings.scroll_mode().name()),
            )
            .to_string(),
            5 | 6 => i18n::tr(
                "Edition",
                if if index == 5 {
                    settings.scale_with_zoom()
                } else {
                    settings.size_to_chart()
                } {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
            7 => i18n::tr(
                "Edition",
                &format!("WaveformLocation{}", settings.location().name()),
            )
            .to_string(),
            8 => settings.loading_parallelism().to_string(),
            9 => i18n::tr(
                "Edition",
                if settings.antialias() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
            10 => format!("{:.4}", settings.subpix()),
            11 => format!("{:.4}", settings.edge_threshold()),
            _ => format!("{:.4}", settings.edge_threshold_min()),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
    pub(super) fn waveform_settings_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::F2 {
            self.panel = Some(Panel::WaveformColors(0));
            self.status = i18n::tr("Edition", "WaveformColorsHelp");
            return;
        }
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 15 && enter) {
            self.waveform_preferences_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "WaveformSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::WaveformSettings(
                (index + if back { 15 } else { 1 }) % 16,
            ));
            return;
        }
        if index < 13
            && (matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            ) || ((index < 2 || (4..=7).contains(&index) || index == 9) && enter))
        {
            let Some(settings) = self.waveform_preferences_draft.as_mut() else {
                return;
            };
            let defaults = Preferences::default();
            let zero = matches!(code, KeyCode::Digit0 | KeyCode::Numpad0);
            let home = code == KeyCode::Home;
            let result = match index {
                0 => {
                    settings.set_show(if home {
                        defaults.show()
                    } else if zero {
                        false
                    } else {
                        !settings.show()
                    });
                    Ok(())
                }
                1 => {
                    settings.set_enabled(if home {
                        defaults.enabled()
                    } else if zero {
                        false
                    } else {
                        !settings.enabled()
                    });
                    Ok(())
                }
                2 => {
                    let delta = if self.shift { 0.001 } else { 0.01 };
                    let value = if home {
                        defaults.channel_width()
                    } else if zero {
                        0.0
                    } else {
                        settings.channel_width()
                            + if code == KeyCode::ArrowLeft {
                                -delta
                            } else {
                                delta
                            }
                    };
                    settings.set_channel_width(value.clamp(0.0, 1.0))
                }
                3 => {
                    let delta = if self.shift { 0.01 } else { 0.1 };
                    let value = if home {
                        defaults.dense_scale()
                    } else if zero {
                        0.0
                    } else {
                        settings.dense_scale()
                            + if code == KeyCode::ArrowLeft {
                                -delta
                            } else {
                                delta
                            }
                    };
                    settings.set_dense_scale(value.clamp(0.0, 10.0))
                }
                4 => {
                    settings.set_scroll_mode(if home {
                        defaults.scroll_mode()
                    } else if zero {
                        deadsync_editor::waveform_spacing::ScrollMode::CurrentTempo
                    } else {
                        settings.scroll_mode().cycle(code == KeyCode::ArrowLeft)
                    });
                    Ok(())
                }
                5 | 6 => {
                    let current = if index == 5 {
                        settings.scale_with_zoom()
                    } else {
                        settings.size_to_chart()
                    };
                    let value = if home {
                        if index == 5 {
                            defaults.scale_with_zoom()
                        } else {
                            defaults.size_to_chart()
                        }
                    } else if zero {
                        false
                    } else {
                        !current
                    };
                    if index == 5 {
                        settings.set_scale_with_zoom(value);
                    } else {
                        settings.set_size_to_chart(value);
                    }
                    Ok(())
                }
                7 => {
                    settings.set_location(if home {
                        defaults.location()
                    } else if zero {
                        deadsync_editor::waveform_preferences::DrawLocation::FocusedChart
                    } else {
                        settings.location().cycle(code == KeyCode::ArrowLeft)
                    });
                    Ok(())
                }
                8 => {
                    settings.set_loading_parallelism(if home {
                        defaults.loading_parallelism()
                    } else if zero {
                        1
                    } else {
                        settings
                            .loading_parallelism()
                            .saturating_add(if code == KeyCode::ArrowLeft { -1 } else { 1 })
                            .clamp(1, 128)
                    });
                    Ok(())
                }
                9 => {
                    settings.set_antialias(if home {
                        defaults.antialias()
                    } else if zero {
                        false
                    } else {
                        !settings.antialias()
                    });
                    Ok(())
                }
                _ => {
                    let current = match index {
                        10 => settings.subpix(),
                        11 => settings.edge_threshold(),
                        _ => settings.edge_threshold_min(),
                    };
                    let default = match index {
                        10 => defaults.subpix(),
                        11 => defaults.edge_threshold(),
                        _ => defaults.edge_threshold_min(),
                    };
                    let value = if home {
                        default
                    } else if zero {
                        0.0
                    } else {
                        (current
                            + if code == KeyCode::ArrowLeft {
                                if self.shift { -0.001 } else { -0.01 }
                            } else if self.shift {
                                0.001
                            } else {
                                0.01
                            })
                        .clamp(0.0, 1.0)
                    };
                    match index {
                        10 => settings.set_subpix(value),
                        11 => settings.set_edge_threshold(value),
                        _ => settings.set_edge_threshold_min(value),
                    }
                }
            };
            if let Err(error) = result {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
            }
            return;
        }
        if index == 13 && enter {
            self.waveform_preferences_draft = Some(Preferences::default());
        }
        if index != 14 || !enter {
            return;
        }
        let Some(settings) = self.waveform_preferences_draft else {
            return;
        };
        let result = self.apply_visual_preferences(
            deadsync_editor::visual_preferences::Snapshot::Waveform(self.waveform_preferences),
            deadsync_editor::visual_preferences::Snapshot::Waveform(settings),
        );
        match result {
            Ok(()) => {
                self.waveform_preferences_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "WaveformSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
}

#[cfg(test)]
mod source_preference_transport_tests {
    use super::*;
    #[test]
    fn opening_source_preference_panels_keeps_playback_and_does_not_issue_transport_commands() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.toggle_audio();
        assert!(state.is_playing());
        state.take_audio_commands();
        for open in [
            State::open_audio_settings,
            State::open_waveform_settings,
            State::open_minimap_settings,
            State::open_minimap_options,
            State::open_note_color_settings,
            State::open_receptor_settings,
            State::open_density_settings,
            State::open_stream_settings,
            State::open_scroll_settings,
            State::open_zoom_settings,
            State::open_scroll_modifiers,
            State::open_key_binding_settings,
        ] {
            let owner = state.editor_asset_owner();
            let before = state.transport_music_time();
            open(&mut state);
            assert!(state.is_playing());
            assert_eq!(state.editor_asset_owner(), owner);
            assert_eq!(state.transport_music_time(), before);
            assert!(!state.take_audio_commands().iter().any(|command| matches!(
                command,
                deadsync_gameplay::GameplayAudioCommand::StopMusic
                    | deadsync_gameplay::GameplayAudioCommand::EditorStopMusic { .. }
                    | deadsync_gameplay::GameplayAudioCommand::EditorPauseMusic { .. }
                    | deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic { .. }
            )));
        }
    }
}

#[cfg(test)]
mod loading_parallelism_ui_tests {
    use super::*;
    #[test]
    fn loading_parallelism_keyboard_bounds_save_undo_redo_and_restore_use_real_owner() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        let original = state.waveform_preferences;
        state.open_waveform_settings();
        for _ in 0..200 {
            state.waveform_settings_key(8, KeyCode::ArrowLeft);
        }
        assert_eq!(
            state
                .waveform_preferences_draft
                .unwrap()
                .loading_parallelism(),
            1
        );
        for _ in 0..200 {
            state.waveform_settings_key(8, KeyCode::ArrowRight);
        }
        assert_eq!(
            state
                .waveform_preferences_draft
                .unwrap()
                .loading_parallelism(),
            128
        );
        state.waveform_settings_key(14, KeyCode::Enter);
        assert_eq!(state.waveform_preferences.loading_parallelism(), 128);
        state.command(Command::Undo);
        assert_eq!(state.waveform_preferences, original);
        state.command(Command::Redo);
        assert_eq!(state.waveform_preferences.loading_parallelism(), 128);
        state.open_waveform_settings();
        state.waveform_settings_key(13, KeyCode::Enter);
        state.waveform_settings_key(14, KeyCode::Enter);
        assert_eq!(state.waveform_preferences, Preferences::default());
    }
}

#[cfg(test)]
mod fxaa_controls_tests {
    use super::*;
    #[test]
    fn keyboard_fxaa_controls_persist_restore_cancel_and_global_undo_in_both_languages() {
        for language in ["en", "fr"] {
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("song.ssc");
            std::fs::write(
                &path,
                "#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            )
            .unwrap();
            let preferences = root.path().join("preferences.json");
            std::fs::write(&preferences, "{\"Version\":1,\"Vendor\":{\"keep\":true}}").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.configure_audio_preferences(&preferences).unwrap();
            let before = state.waveform_preferences;
            state.open_waveform_settings();
            state.waveform_settings_key(9, KeyCode::Enter);
            for index in 10..=12 {
                state.waveform_settings_key(index, KeyCode::ArrowRight);
            }
            let after = state.waveform_preferences_draft.unwrap();
            assert!(!after.antialias());
            assert_ne!(after, before);
            state.waveform_settings_key(14, KeyCode::Enter);
            assert_eq!(state.waveform_preferences, after);
            assert_eq!(
                deadsync_editor::audio_preferences::File::open(&preferences)
                    .unwrap()
                    .waveform(),
                after
            );
            state.command(Command::Undo);
            assert_eq!(state.waveform_preferences, before);
            state.command(Command::Redo);
            assert_eq!(state.waveform_preferences, after);
            state.open_waveform_settings();
            state.waveform_settings_key(13, KeyCode::Enter);
            assert_eq!(state.waveform_preferences_draft, Some(before));
            state.waveform_settings_key(15, KeyCode::Enter);
            assert_eq!(state.waveform_preferences, after);
            state.open_waveform_settings();
            state.waveform_settings_key(13, KeyCode::Enter);
            state.waveform_settings_key(14, KeyCode::Enter);
            assert_eq!(state.waveform_preferences, before);
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(preferences).unwrap()).unwrap();
            assert_eq!(value["Vendor"]["keep"], true);
        }
    }
}
