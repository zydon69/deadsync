//! Display preferences are application state, shared by renderer and selection.
use super::*;
use deadsync_editor::options_preferences::{BackgroundPolicy, Display, RenderLimits, RenderPolicy};
impl State {
    pub fn render_policy(&self) -> RenderPolicy {
        self.display_options.render
    }
    pub fn render_limits(&self) -> RenderLimits {
        self.display_options.limits
    }
    pub fn misc_event_area_width(&self) -> f64 {
        f64::from(self.display_options.misc_width)
    }
    pub fn song_background_policy(&self) -> BackgroundPolicy {
        self.display_options.background
    }
    pub fn configure_display_options(&mut self, settings: Display) {
        self.display_options = settings;
    }
    pub fn configure_song_background_policy(&mut self, policy: BackgroundPolicy) {
        self.display_options.background = policy;
    }
}

use deadsync_editor::options_preferences::BackgroundSize;
pub(super) const DISPLAY_KEYS: [&str; 5] = [
    "OptionsField",
    "OptionsValue",
    "OptionsDisplayDefaults",
    "OptionsApply",
    "OptionsCancel",
];
impl State {
    pub(super) fn sync_display_options_history(&mut self) {
        if let Some(update) = self.workspace.take_display_options_update() {
            self.configure_display_options(update.preferences);
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
        }
    }
    pub(super) fn commit_display_options(
        &mut self,
        settings: Display,
    ) -> Result<bool, deadsync_editor::Error> {
        let file = self.capture_application_preferences_file()?;
        let changed =
            self.workspace
                .set_display_options(self.display_options, settings, Some(&file))?;
        self.sync_display_options_history();
        Ok(changed)
    }
    pub(super) fn open_display_options(&mut self) {
        self.lane_entry_release_input();
        self.options_draft = Some(self.display_options);
        self.options_field = 0;
        self.panel = Some(Panel::OptionsSettings(0));
        self.status = i18n::tr("Edition", "OptionsDisplayHelp");
    }
    pub(super) fn display_options_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 4 && enter) {
            self.options_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "OptionsDiscarded");
            return;
        }
        if matches!(code, KeyCode::Tab | KeyCode::ArrowUp | KeyCode::ArrowDown) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::OptionsSettings(
                (index + if back { 4 } else { 1 }) % 5,
            ));
            return;
        }
        if index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            self.options_field =
                (self.options_field + if code == KeyCode::ArrowLeft { 6 } else { 1 }) % 7;
            return;
        }
        if index == 2 && enter {
            if let Some(draft) = self.options_draft.as_mut() {
                *draft = draft.restore_defaults();
            }
            return;
        }
        if index == 3 && enter {
            if let Some(draft) = self.options_draft {
                match self.commit_display_options(draft) {
                    Ok(_) => {
                        self.options_draft = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "OptionsSaved");
                    }
                    Err(_) => self.status = i18n::tr("Edition", "OptionsSaveFailed"),
                }
            }
            return;
        }
        if index != 1
            || !matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Enter
                    | KeyCode::NumpadEnter
            )
        {
            return;
        }
        let Some(draft) = self.options_draft.as_mut() else {
            return;
        };
        let reset = code == KeyCode::Home;
        let defaults = Display::default();
        match self.options_field {
            0 => {
                draft.render.notes = if reset {
                    defaults.render.notes
                } else {
                    !draft.render.notes
                }
            }
            1 => {
                draft.render.markers = if reset {
                    defaults.render.markers
                } else {
                    !draft.render.markers
                }
            }
            2 => {
                draft.render.regions = if reset {
                    defaults.render.regions
                } else {
                    !draft.render.regions
                }
            }
            3 => {
                draft.render.misc_events = if reset {
                    defaults.render.misc_events
                } else {
                    !draft.render.misc_events
                }
            }
            4 => draft.background.hide = if reset { false } else { !draft.background.hide },
            5 => {
                draft.background.size = if reset {
                    BackgroundSize::ChartArea
                } else {
                    match draft.background.size {
                        BackgroundSize::ChartArea => BackgroundSize::Window,
                        _ => BackgroundSize::ChartArea,
                    }
                }
            }
            6 => {
                if !enter {
                    draft.misc_width = if reset {
                        100
                    } else {
                        draft
                            .misc_width
                            .saturating_add(if code == KeyCode::ArrowLeft { -1 } else { 1 })
                            .clamp(0, 200)
                    };
                }
            }
            _ => unreachable!("bounded Display field"),
        }
    }
    pub(super) fn display_options_label(&self, index: usize, key: &str) -> Arc<str> {
        let settings = self.options_draft.unwrap_or(self.display_options);
        let name = i18n::tr(
            "Edition",
            [
                "OptionsRenderNotes",
                "OptionsRenderMarkers",
                "OptionsRenderRegions",
                "OptionsRenderMisc",
                "OptionsHideBackground",
                "OptionsBackgroundSize",
                "OptionsMiscWidth",
            ][self.options_field],
        );
        if index == 0 {
            return i18n::tr_fmt("Edition", key, &[("value", &name)]);
        }
        if index != 1 {
            return i18n::tr("Edition", key);
        }
        let value = match self.options_field {
            0 => settings.render.notes.to_string(),
            1 => settings.render.markers.to_string(),
            2 => settings.render.regions.to_string(),
            3 => settings.render.misc_events.to_string(),
            4 => settings.background.hide.to_string(),
            5 => match settings.background.size {
                BackgroundSize::ChartArea => {
                    i18n::tr("Edition", "OptionsSizeChartArea").to_string()
                }
                BackgroundSize::Window => i18n::tr("Edition", "OptionsSizeWindow").to_string(),
                BackgroundSize::Unknown(value) => i18n::tr_fmt(
                    "Edition",
                    "OptionsEnumUnknown",
                    &[("value", &value.to_string())],
                )
                .to_string(),
            },
            6 => settings.misc_width.to_string(),
            _ => unreachable!(),
        };
        let value = if value == "true" {
            i18n::tr("Edition", "WaveformYes").to_string()
        } else if value == "false" {
            i18n::tr("Edition", "WaveformNo").to_string()
        } else {
            value
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
