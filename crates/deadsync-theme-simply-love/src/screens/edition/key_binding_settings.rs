//! Staged global key bindings. The panel keeps fixed navigation until Save.
use super::*;
use deadsync_editor::key_binding_preferences::inspector::{Capture, CapturePhase, Draft};
use std::sync::Arc;
pub(super) struct Settings {
    pub(super) draft: Draft,
    capture: Option<Capture>,
    conflicts: Vec<String>,
    profile_report: Option<deadsync_editor::key_binding_preferences::source_profile::Report>,
    profile_page: usize,
}
impl Settings {
    fn refresh_conflicts(&mut self) {
        self.conflicts = deadsync_editor::key_binding_preferences::physical::conflicts(
            &self.draft.staged(),
            self.draft.action().id(),
            &self.draft.alternatives()[self.draft.alternative],
        );
    }
    pub(super) fn release_input(&mut self) {
        self.capture = None;
    }
}
const ROWS: [&str; 8] = [
    "KeyBindingAction",
    "KeyBindingAlternative",
    "KeyBindingCapture",
    "KeyBindingAdd",
    "KeyBindingDelete",
    "KeyBindingReset",
    "ScrollSettingsSave",
    "ScrollSettingsCancel",
];
impl State {
    pub(super) fn open_key_binding_settings(&mut self) {
        self.release_input();
        match Draft::capture(Arc::clone(&self.command_bindings.preferences)) {
            Ok(draft) => {
                self.key_binding_draft = Some(Settings {
                    draft,
                    capture: None,
                    conflicts: Vec::new(),
                    profile_report: None,
                    profile_page: 0,
                });
                self.key_binding_draft.as_mut().unwrap().refresh_conflicts();
                self.panel = Some(Panel::KeyBindingSettings(0));
                self.status = i18n::tr("Edition", "KeyBindingHelp");
            }
            Err(error) => self.key_binding_error(error),
        }
    }
    fn key_binding_error(&mut self, error: deadsync_editor::Error) {
        self.status = i18n::tr_fmt(
            "Edition",
            "Error",
            &[("message", &crate::i18n::editor_error(&error))],
        );
    }
    pub(super) fn sync_key_binding_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_key_binding_preferences_update() {
            self.configure_key_binding_preferences(update.preferences);
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
        }
    }
    /// Full raw events reach this before modifier/modal early returns. Capture
    /// finishes when its chord is released; Enter/Escape then act in Review.
    pub(super) fn key_binding_capture_event(&mut self, event: &RawKeyboardEvent) -> bool {
        if self.scroll_modifier_capture_event(event) {
            return true;
        }
        let Some(settings) = self.key_binding_draft.as_mut() else {
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
                    let keys = capture.keys().to_vec();
                    match settings.draft.replace(keys) {
                        Ok(()) => {
                            settings.capture = None;
                            settings.refresh_conflicts();
                        }
                        Err(error) => self.key_binding_error(error),
                    }
                }
            }
            return true;
        }
        let keys = self
            .command_bindings
            .capture_snapshot(self.control_keys, self.shift_keys);
        capture.observe(&keys);
        true
    }
    pub(super) fn key_profile_import_matches(
        &self,
        expected: &Arc<deadsync_editor::key_binding_preferences::Preferences>,
    ) -> bool {
        matches!(self.panel, Some(Panel::KeyBindingSettings(_)))
            && self.key_binding_draft.as_ref().is_some_and(|settings| {
                settings.capture.is_none() && Arc::ptr_eq(&settings.draft.staged(), expected)
            })
    }
    fn key_profile_review_status(&mut self) {
        let Some(settings) = self.key_binding_draft.as_ref() else {
            return;
        };
        let Some(report) = settings.profile_report.as_ref() else {
            return;
        };
        let mut details = Vec::new();
        for (key, ids) in [
            ("KeyProfileAvailable", &report.applied),
            ("KeyProfileAppliedModifiers", &report.applied_modifiers),
            ("KeyProfileUnavailable", &report.unavailable_commands),
            ("KeyProfileBlocked", &report.blocked_unavailable),
            ("KeyProfileNativeContext", &report.retained_native_contexts),
            ("KeyProfileModifierOnly", &report.retained_modifiers),
            ("KeyProfileUnknown", &report.retained_unknown),
            ("KeyProfileDisabled", &report.neutralized_native_extensions),
            ("KeyProfileRescue", &report.rescue_commands),
        ] {
            for id in ids {
                details.push(format!("{}: {}", i18n::tr("Edition", key), id));
            }
        }
        for rejected in &report.rejected {
            let reason = match rejected.reason {
                "Expected an alternatives array" => "KeyProfileReasonArray",
                "Too many alternatives" => "KeyProfileReasonAlternatives",
                "Expected an ordered key array" => "KeyProfileReasonChord",
                "Chord exceeds native key limit" => "KeyProfileReasonLength",
                "Source key has no supported native mapping" => "KeyProfileReasonKey",
                "Duplicate modifier/key after physical alias normalization" => {
                    "KeyProfileReasonDuplicate"
                }
                "Unsupported native key chord" => "KeyProfileReasonPrefix",
                _ => "KeyProfileReasonReserved",
            };
            let alternative = rejected
                .alternative
                .map(|i| format!(" #{}", i + 1))
                .unwrap_or_default();
            details.push(format!(
                "{}: {}{}: {}",
                i18n::tr("Edition", "KeyProfileRejected"),
                rejected.id,
                alternative,
                i18n::tr("Edition", reason)
            ));
        }
        let count = details.len().max(1);
        let detail = details
            .get(settings.profile_page % count)
            .map(String::as_str)
            .unwrap_or("");
        self.status = i18n::tr_fmt(
            "Edition",
            "KeyProfileReview",
            &[
                ("available", &report.applied.len().to_string()),
                (
                    "unavailable",
                    &report.unavailable_commands.len().to_string(),
                ),
                (
                    "disabled",
                    &report.neutralized_native_extensions.len().to_string(),
                ),
                ("rejected", &report.rejected.len().to_string()),
                ("item", detail),
            ],
        );
    }
    fn key_profile_stage(
        &mut self,
        prepared: deadsync_editor::key_binding_preferences::source_profile::Prepared,
    ) {
        let Some(settings) = self.key_binding_draft.as_mut() else {
            return;
        };
        match settings.draft.stage_profile(prepared.preferences) {
            Ok(()) => {
                settings.profile_report = Some(prepared.report);
                settings.profile_page = 0;
                settings.refresh_conflicts();
                self.key_profile_review_status();
            }
            Err(error) => self.key_binding_error(error),
        }
    }
    pub(super) fn key_profile_import_text(&mut self, text: &str) {
        let Some(settings) = self.key_binding_draft.as_ref() else {
            return;
        };
        let result = deadsync_editor::key_binding_preferences::source_profile::import(
            &settings.draft.staged(),
            text,
            deadsync_editor::key_binding_preferences::source_profile::Platform::current(),
        );
        match result {
            Ok(prepared) => self.key_profile_stage(prepared),
            Err(error) => self.key_binding_error(error),
        }
    }
    pub(super) fn key_binding_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 7 && enter) {
            self.key_binding_draft = None;
            self.panel = None;
            return;
        }
        if code == KeyCode::F10 {
            self.open_scroll_modifiers();
            return;
        }
        if matches!(
            code,
            KeyCode::F5 | KeyCode::F6 | KeyCode::F7 | KeyCode::F8 | KeyCode::F9
        ) {
            if code == KeyCode::F7 {
                self.queue_key_profile_import();
                return;
            }
            let Some(settings) = self.key_binding_draft.as_mut() else {
                return;
            };
            match code {
                KeyCode::F5 => {
                    let next =
                        deadsync_editor::key_binding_preferences::source_profile::native_defaults(
                            &settings.draft.staged(),
                        );
                    match settings.draft.stage_profile(next) {
                        Ok(()) => {
                            settings.profile_report = None;
                            settings.profile_page = 0;
                            settings.refresh_conflicts();
                            self.status = i18n::tr("Edition", "KeyProfileNativeStaged");
                        }
                        Err(error) => self.key_binding_error(error),
                    }
                }
                KeyCode::F6 => {
                    let result = deadsync_editor::key_binding_preferences::source_profile::preset(
                        &settings.draft.staged(),
                        deadsync_editor::key_binding_preferences::source_profile::Platform::current(
                        ),
                    );
                    match result {
                        Ok(prepared) => self.key_profile_stage(prepared),
                        Err(error) => self.key_binding_error(error),
                    }
                }
                KeyCode::F8 | KeyCode::F9 => {
                    if code == KeyCode::F8 {
                        settings.profile_page = settings.profile_page.saturating_sub(1);
                    } else {
                        settings.profile_page = settings.profile_page.saturating_add(1);
                    }
                    self.key_profile_review_status();
                }
                _ => {}
            }
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::KeyBindingSettings(
                (index + if backward { 7 } else { 1 }) % 8,
            ));
            return;
        }
        let Some(settings) = self.key_binding_draft.as_mut() else {
            return;
        };
        if matches!(code, KeyCode::PageUp | KeyCode::PageDown)
            || (index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight))
        {
            settings
                .draft
                .select_action(matches!(code, KeyCode::PageDown | KeyCode::ArrowRight));
            settings.refresh_conflicts();
            return;
        }
        if index == 1 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            let count = settings.draft.alternatives().len();
            settings.draft.alternative = (settings.draft.alternative
                + if code == KeyCode::ArrowRight {
                    1
                } else {
                    count - 1
                })
                % count;
            settings.refresh_conflicts();
            return;
        }
        if !enter {
            return;
        }
        let result = match index {
            2 => {
                settings.capture = Some(Capture::new());
                return;
            }
            3 => settings.draft.add(),
            4 => settings.draft.delete(),
            5 => settings.draft.reset_action(),
            6 => {
                let before = settings.draft.baseline();
                let after = settings.draft.staged();
                let result = (|| -> Result<(), deadsync_editor::Error> {
                    let file = self.capture_application_preferences_file()?;
                    self.workspace
                        .set_key_binding_preferences(before, after, Some(&file))?;
                    self.sync_key_binding_preferences_history();
                    Ok(())
                })();
                if result.is_ok() {
                    self.key_binding_draft = None;
                    self.panel = None;
                }
                result
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.key_binding_error(error);
        } else if let Some(settings) = self.key_binding_draft.as_mut() {
            settings.refresh_conflicts();
        }
    }
    pub(super) fn key_binding_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let Some(settings) = self.key_binding_draft.as_ref() else {
            return;
        };
        let x = screen_width() * 0.5;
        let top = screen_height() * 0.5 - 155.;
        actors
            .push(act!(quad:xy(x,top+145.):setsize(620.,330.):diffuse(0.04,0.06,0.1,0.98):z(8050)));
        let alternatives = settings.draft.alternatives();
        let chord = &alternatives[settings.draft.alternative];
        for (row, key) in ROWS.iter().enumerate() {
            let ui_row_start = actors.len();
            let label = match row {
                0 => i18n::tr_fmt(
                    "Edition",
                    key,
                    &[(
                        "action",
                        &i18n::tr(
                            "Edition",
                            &format!("KeyBindingAction{}", settings.draft.action().id()),
                        ),
                    )],
                ),
                1 => i18n::tr_fmt(
                    "Edition",
                    key,
                    &[
                        ("index", &(settings.draft.alternative + 1).to_string()),
                        ("total", &alternatives.len().to_string()),
                        (
                            "keys",
                            &if chord.is_empty() {
                                i18n::tr("Edition", "KeyBindingUnbound").to_string()
                            } else {
                                super::command_bindings::chord_label(chord)
                            },
                        ),
                    ],
                ),
                _ => i18n::tr("Edition", key),
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+row as f32*30.):zoom(0.62):maxwidth(580.):diffuse(1.,if row==index{0.8}else{1.},if row==index{0.3}else{1.},1.):z(8051)));

            self.ui_row(
                actors,
                ui_row_start,
                row == index && settings.capture.is_none(),
                30.,
            );
        }
        let conflicts = &settings.conflicts;
        if !conflicts.is_empty() {
            let warning = i18n::tr_fmt(
                "Edition",
                "KeyBindingConflicts",
                &[(
                    "commands",
                    &conflicts
                        .iter()
                        .map(|id| {
                            if let Some(source) = deadsync_editor::key_binding_preferences::source_profile::blocker_source(id) {
                                i18n::tr_fmt("Edition", "KeyProfileUnavailableActivation", &[("command", source)])
                            } else { i18n::tr("Edition", &format!("KeyBindingAction{id}")) }
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                )],
            );
            actors.push(act!(text:font("miso"):settext(warning):xy(x,top+240.):zoom(0.5):maxwidth(580.):diffuse(1.,0.7,0.2,1.):z(8052)));
        }
        if settings.capture.is_none() {
            let help = i18n::tr("Edition", "KeyProfileControls");
            actors.push(act!(text:font("miso"):settext(help):xy(x,top+285.):zoom(0.45):maxwidth(580.):maxheight(32.):z(8052)));
        }
        if let Some(capture) = &settings.capture {
            let ui_row_start = actors.len();
            let help = i18n::tr_fmt(
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
            );
            actors.push(act!(text:font("miso"):settext(help):xy(x,top+270.):zoom(0.55):maxwidth(580.):z(8052)));
            self.ui_row(actors, ui_row_start, true, 32.);
        }
    }
}

impl Settings {
    pub(super) fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }
}

#[cfg(test)]
mod source_reset_tests;
