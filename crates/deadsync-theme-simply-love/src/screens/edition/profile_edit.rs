use super::{Panel, ProfileTarget, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::generation::{
    metadata::Guid,
    profile_controls::{Control, Editor},
    profiles::{Config, Kind, Profile, TravelSpeed},
};
#[derive(Default)]
pub(super) struct Choices {
    pub pattern: Option<Guid>,
    pub performed: Option<Guid>,
    pub expressed: Option<Guid>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TextField {
    Name,
    Description,
}
impl TextField {
    fn key(self, name: &'static str, description: &'static str) -> &'static str {
        match self {
            Self::Name => name,
            Self::Description => description,
        }
    }
    fn row(self) -> usize {
        match self {
            Self::Name => 2,
            Self::Description => 4,
        }
    }
}
pub(super) struct Draft {
    source: Guid,
    kind: Kind,
    profile: Profile,
    pub editor: Editor,
    input: Option<(usize, String)>,
    back: Panel,
    custom_source: bool,
    name: Option<String>,
    description: Option<String>,
    pub(super) text_field: TextField,
    pub(super) text_buffer: Option<deadsync_editor::text_buffer::Buffer>,
    travel_speed: Option<TravelSpeed>,
    travel_editor: Option<TravelSpeed>,
    travel_input: Option<(usize, String)>,
    reset_return: usize,
    reset_defaults: bool,
    weight_layout: usize,
}
impl Draft {
    pub(super) fn action_count(&self) -> usize {
        if self.supports_travel_speed() { 11 } else { 9 }
    }
    pub(super) fn supports_travel_speed(&self) -> bool {
        self.kind == Kind::Performed
    }
}
impl State {
    pub(super) fn open_profile_edit(&mut self, kind: Kind, back: Panel) {
        let result: Result<Draft, String> = (|| {
            let settings = self
                .profile_selection
                .as_ref()
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            let selected = match kind {
                Kind::Pattern => settings.pattern,
                Kind::Performed => settings.performed,
                Kind::Expressed => settings.expressed,
            };
            let selected = if matches!(back, Panel::ConversionProfiles(_)) {
                selected.and_then(|i| i.checked_sub(1))
            } else {
                selected
            };
            let entry = selected
                .and_then(|i| settings.catalog.entries(kind).get(i))
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            let source = settings
                .catalog
                .profile(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    kind,
                    &entry.guid,
                )
                .map_err(profile_error)?;
            let name = i18n::tr_fmt(
                "Edition",
                "ExpressionEditCopyName",
                &[("name", &super::profile_entry_name(entry))],
            );
            let profile = source.duplicate(&name).map_err(profile_error)?;
            let editor = Editor::new(profile.config()).map_err(profile_error)?;
            Ok(Draft {
                source: entry.guid.clone(),
                kind,
                profile,
                editor,
                input: None,
                back,
                custom_source: entry.builtin.is_none(),
                name: None,
                description: None,
                text_field: TextField::Name,
                text_buffer: None,
                travel_speed: None,
                travel_editor: None,
                travel_input: None,
                reset_return: 0,
                reset_defaults: false,
                weight_layout: 0,
            })
        })();
        match result {
            Ok(draft) => {
                self.profile_edit = Some(draft);
                self.panel = Some(Panel::ProfileEdit(0));
                self.status = i18n::tr("Edition", "ProfileEditHelp");
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error)]),
        }
    }
    fn open_profile_delete(&mut self) {
        let result: Result<(usize, usize), String> = (|| {
            let draft = self
                .profile_edit
                .as_ref()
                .ok_or_else(|| "Missing draft".to_string())?;
            if !draft.custom_source {
                return Err(i18n::tr("Edition", "ProfileEditProtected").to_string());
            }
            if draft.input.is_some()
                || draft.name.is_some()
                || draft.description.is_some()
                || draft.travel_speed.is_some()
                || draft.reset_defaults
                || draft.editor.finish().map_err(profile_error)? != *draft.profile.config()
                || self
                    .chart_properties
                    .as_ref()
                    .is_some_and(|d| d.has_changes())
                || self.marker.as_ref().is_some_and(|d| d.has_changes())
            {
                return Err(i18n::tr("Edition", "ProfileDeletePending").to_string());
            }
            let plan = deadsync_editor::generation::profile_references::Reset::capture(
                &self.workspace.file().session,
                draft.kind,
                &draft.source,
            )
            .map_err(profile_error)?;
            Ok((plan.affected_charts(), plan.affected_references()))
        })();
        match result {
            Ok((charts, references)) => {
                self.panel = Some(Panel::ProfileDelete(0));
                self.status = i18n::tr_fmt(
                    "Edition",
                    "ProfileDeleteHelp",
                    &[
                        ("charts", &charts.to_string()),
                        ("references", &references.to_string()),
                    ],
                );
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error)]),
        }
    }
    pub(super) fn profile_delete_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.panel = Some(Panel::ProfileEdit(0)),
            KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::ProfileDelete(1 - index))
            }
            KeyCode::Enter if index == 0 => self.panel = Some(Panel::ProfileEdit(0)),
            KeyCode::Enter => {
                let result: Result<(), String> = (|| {
                    let draft = self
                        .profile_edit
                        .as_ref()
                        .ok_or_else(|| "Missing draft".to_string())?;
                    let settings = self
                        .profile_selection
                        .as_ref()
                        .ok_or_else(|| "Missing profiles".to_string())?;
                    let file = settings
                        .catalog
                        .profile_file(
                            &self.workspace.file().session,
                            self.workspace.chart(),
                            draft.kind,
                            &draft.source,
                        )
                        .map_err(profile_error)?;
                    self.workspace
                        .delete_profile(&file)
                        .map_err(profile_error)?;
                    if self.generation.profiles.performed.as_ref() == Some(&draft.source) {
                        self.generation.profiles.performed = None;
                        self.generation.performed_name = None;
                    }
                    if self.generation.profiles.expressed.as_ref() == Some(&draft.source) {
                        self.generation.profiles.expressed = None;
                        self.generation.expressed_name = None;
                    }
                    Ok(())
                })();
                match result {
                    Ok(()) => {
                        self.profile_edit = None;
                        self.profile_selection = None;
                        self.chart_properties = None;
                        self.marker = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "ProfileDeleted");
                        self.refresh();
                    }
                    Err(error) => {
                        self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error)])
                    }
                }
            }
            _ => {}
        }
    }
    fn profile_saved_history(&mut self, redo: bool) {
        let result: Result<bool, String> = (|| {
            let draft = self
                .profile_edit
                .as_ref()
                .ok_or_else(|| "Missing draft".to_string())?;
            if draft.input.is_some()
                || draft.name.is_some()
                || draft.description.is_some()
                || draft.travel_speed.is_some()
                || draft.reset_defaults
                || draft.editor.finish().map_err(profile_error)? != *draft.profile.config()
            {
                return Err(i18n::tr("Edition", "ProfileHistoryPending").to_string());
            }
            let settings = self
                .profile_selection
                .as_ref()
                .ok_or_else(|| "Missing profiles".to_string())?;
            let captured = settings
                .catalog
                .profile_file(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    draft.kind,
                    &draft.source,
                )
                .map_err(profile_error)?;
            let Some(file) = self.profile_history.get_mut(captured.path()) else {
                return Ok(false);
            };
            let changed = if redo { file.redo() } else { file.undo() }.map_err(profile_error)?;
            if !changed {
                return Ok(false);
            }
            let offset = usize::from(matches!(draft.back, Panel::ConversionProfiles(_)));
            let selected = |kind, index: Option<usize>| {
                index
                    .and_then(|i| i.checked_sub(offset))
                    .and_then(|i| settings.catalog.entries(kind).get(i))
                    .map(|entry| entry.guid.clone())
            };
            self.profile_saved_choices = Some(Choices {
                pattern: selected(Kind::Pattern, settings.pattern),
                performed: selected(Kind::Performed, settings.performed),
                expressed: selected(Kind::Expressed, settings.expressed),
            });
            Ok(true)
        })();
        match result {
            Ok(true) => {
                self.profile_edit = None;
                match self.catalog_target {
                    ProfileTarget::Chart => self.open_chart_expression(),
                    ProfileTarget::Marker => self.open_marker_profiles(),
                    target => self.open_conversion_profiles_for(target),
                }
            }
            Ok(false) => self.status = i18n::tr("Edition", "ProfileHistoryEmpty"),
            Err(error) => self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error)]),
        }
    }
    fn stage_profile_input(&mut self) -> bool {
        let Some(draft) = self.profile_edit.as_mut() else {
            return false;
        };
        let Some((index, text)) = &draft.input else {
            return true;
        };
        if draft.editor.set_text(*index, text).is_err() {
            self.status = i18n::tr(
                "Edition",
                if draft.kind == Kind::Expressed {
                    "ExpressionEditInvalid"
                } else {
                    "ProfileEditInvalid"
                },
            );
            return false;
        }
        draft.input = None;
        true
    }
    pub(super) fn profile_edit_key(&mut self, index: usize, code: KeyCode) {
        let Some(draft) = self.profile_edit.as_ref() else {
            return;
        };
        let count = draft.editor.fields().len();
        let actions = draft.action_count();
        if code == KeyCode::F9 && !self.control
            || index == count + actions - 1 && code == KeyCode::Enter
        {
            self.open_profile_delete();
            return;
        }
        let undo = code == KeyCode::F7 && !self.control
            || index == count + actions - 3 && code == KeyCode::Enter;
        let redo = code == KeyCode::F8 && !self.control
            || index == count + actions - 2 && code == KeyCode::Enter;
        if undo || redo {
            self.profile_saved_history(redo);
            return;
        }
        if (index == count + actions - 4 && code == KeyCode::Enter)
            || (code == KeyCode::F5 && !self.control)
        {
            if let Some(draft) = self.profile_edit.as_mut() {
                draft.reset_return = index;
            }
            self.panel = Some(Panel::ProfileDefaults(0));
            self.status = i18n::tr("Edition", "ProfileDefaultsHelp");
            return;
        }
        if draft.supports_travel_speed()
            && ((index == count + 6 && code == KeyCode::Enter)
                || (code == KeyCode::F6 && !self.control))
        {
            if !self.stage_profile_input() {
                return;
            }
            self.panel = Some(Panel::ProfileWeights(0));
            self.status = i18n::tr("Edition", "ProfileWeightsHelp");
            return;
        }
        if code == KeyCode::Escape || (index == count + 1 && code == KeyCode::Enter) {
            let back = draft.back;
            self.profile_edit = None;
            self.panel = Some(back);
            self.status = i18n::tr("Edition", "ExpressionEditDiscarded");
            return;
        }
        if (index == count + 2 && code == KeyCode::Enter) || (code == KeyCode::F2 && !self.control)
        {
            if !self.stage_profile_input() {
                return;
            }
            self.open_profile_text(TextField::Name);
            return;
        }
        if (index == count + 4 && code == KeyCode::Enter) || (code == KeyCode::F3 && !self.control)
        {
            if !self.stage_profile_input() {
                return;
            }
            self.open_profile_text(TextField::Description);
            return;
        }
        if draft.supports_travel_speed()
            && ((index == count + 5 && code == KeyCode::Enter)
                || (code == KeyCode::F4 && !self.control))
        {
            if !self.stage_profile_input() {
                return;
            }
            self.open_profile_speed();
            return;
        }
        if index == count + 3 && code == KeyCode::Enter {
            if self.stage_profile_input() {
                self.save_profile(false);
            }
            return;
        }
        let numeric = draft
            .editor
            .fields()
            .get(index)
            .is_some_and(|f| matches!(f.control, Control::Integer | Control::Number));
        if !self.control && numeric {
            let character = numeric_key(code, self.shift);

            if let Some(draft) = self.profile_edit.as_mut() {
                if let Some(c) = character {
                    let (_, text) = draft.input.get_or_insert_with(|| (index, String::new()));
                    if text.len() < 64 {
                        text.push(c);
                    }
                    return;
                }
                if code == KeyCode::Delete {
                    draft.input = Some((index, String::new()));
                    return;
                }
                if code == KeyCode::Backspace {
                    let value = draft
                        .editor
                        .value(index)
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    draft.input.get_or_insert((index, value)).1.pop();
                    return;
                }
            }
        }
        if !self.stage_profile_input() {
            return;
        }
        let next = match code {
            KeyCode::ArrowUp => Some((index + count + actions - 1) % (count + actions)),
            KeyCode::ArrowDown => Some((index + 1) % (count + actions)),
            KeyCode::Tab => {
                Some((index + if self.shift { count + actions - 1 } else { 1 }) % (count + actions))
            }
            KeyCode::PageDown => Some((index + 8).min(count + actions - 1)),
            KeyCode::PageUp => Some(index.saturating_sub(8)),
            KeyCode::Home => Some(0),
            KeyCode::End => Some(count),
            _ => None,
        };
        if let Some(next) = next {
            self.panel = Some(Panel::ProfileEdit(next));
            return;
        }
        if index == count && code == KeyCode::Enter {
            self.save_profile(true);
            return;
        }
        if index >= count
            || !matches!(
                code,
                KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter
            )
        {
            return;
        }
        if code == KeyCode::Enter && numeric {
            return;
        }
        if let Some(draft) = self.profile_edit.as_mut() {
            if let Err(error) = draft
                .editor
                .step(index, code != KeyCode::ArrowLeft, self.shift)
            {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
            }
        }
    }
    fn save_profile(&mut self, copy: bool) {
        let result: Result<Choices, String> = (|| {
            let draft = self
                .profile_edit
                .as_mut()
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            let settings = self
                .profile_selection
                .as_ref()
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            settings
                .catalog
                .profile(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    draft.kind,
                    &draft.source,
                )
                .map_err(profile_error)?;
            let config = draft.editor.finish().map_err(|error| {
                if error.to_string().contains("Aggressive bracket threshold") {
                    i18n::tr("Edition", "ExpressionEditThresholdOrder").to_string()
                } else {
                    error.to_string()
                }
            })?;
            let saved_id = if copy {
                let directory = self
                    .profile_directory
                    .as_deref()
                    .ok_or_else(|| i18n::tr("Edition", "ExpressionEditUnavailable").to_string())?;
                let mut profile = draft.profile.clone();
                if draft.reset_defaults {
                    profile.restore_defaults().map_err(profile_error)?;
                }
                if let Some(speed) = draft.travel_speed {
                    profile.set_travel_speed(speed).map_err(profile_error)?;
                }
                profile.set_config(config).map_err(profile_error)?;
                if let Some(name) = &draft.name {
                    profile.rename(name).map_err(profile_error)?;
                }
                if let Some(description) = &draft.description {
                    profile
                        .set_description(Some(description))
                        .map_err(profile_error)?;
                }
                profile.export_new(directory).map_err(profile_error)?;
                profile.guid().clone()
            } else {
                if !draft.custom_source {
                    return Err(i18n::tr("Edition", "ProfileEditProtected").to_string());
                }
                let captured = settings
                    .catalog
                    .profile_file(
                        &self.workspace.file().session,
                        self.workspace.chart(),
                        draft.kind,
                        &draft.source,
                    )
                    .map_err(profile_error)?;
                let file = self
                    .profile_history
                    .entry(captured.path().to_owned())
                    .or_insert(captured);
                let mut profile = file.profile().clone();
                if draft.reset_defaults {
                    profile.restore_defaults().map_err(profile_error)?;
                }
                if let Some(speed) = draft.travel_speed {
                    profile.set_travel_speed(speed).map_err(profile_error)?;
                }
                profile.set_config(config).map_err(profile_error)?;
                if let Some(name) = &draft.name {
                    profile.rename(name).map_err(profile_error)?;
                }
                if let Some(description) = &draft.description {
                    profile
                        .set_description(Some(description))
                        .map_err(profile_error)?;
                }
                file.save(&profile).map_err(profile_error)?;
                profile.guid().clone()
            };
            let offset = usize::from(matches!(draft.back, Panel::ConversionProfiles(_)));
            let selected = |kind, index: Option<usize>| {
                index
                    .and_then(|i| i.checked_sub(offset))
                    .and_then(|i| settings.catalog.entries(kind).get(i))
                    .map(|entry| entry.guid.clone())
            };
            let mut choices = Choices {
                pattern: selected(Kind::Pattern, settings.pattern),
                performed: selected(Kind::Performed, settings.performed),
                expressed: selected(Kind::Expressed, settings.expressed),
            };
            let id = Some(saved_id);
            match draft.kind {
                Kind::Pattern => choices.pattern = id,
                Kind::Performed => choices.performed = id,
                Kind::Expressed => choices.expressed = id,
            };
            Ok(choices)
        })();
        match result {
            Ok(choices) => {
                self.profile_edit = None;
                match self.catalog_target {
                    ProfileTarget::Chart => self.open_chart_expression(),
                    ProfileTarget::Marker => self.open_marker_profiles(),
                    target => self.open_conversion_profiles_for(target),
                };
                if self.catalog_job.is_some() {
                    self.profile_saved_choices = Some(choices);
                }
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error)]),
        }
    }
    pub(super) fn profile_edit_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let Some(draft) = &self.profile_edit else {
            return i18n::tr("Edition", key);
        };
        let count = draft.editor.fields().len();
        if index == count + 2 {
            let fallback = i18n::tr("Edition", "ProfileNameUnchanged");
            let name = draft.name.as_deref().unwrap_or(&fallback);
            return i18n::tr_fmt("Edition", "ProfileEditName", &[("value", name)]);
        }
        if index == count + 4 {
            let description = draft
                .description
                .as_deref()
                .or_else(|| draft.profile.description())
                .unwrap_or_default();
            let preview: String = description
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(44)
                .collect();
            return i18n::tr_fmt("Edition", "ProfileEditDescription", &[("value", &preview)]);
        }
        if index == count + 3 {
            return i18n::tr(
                "Edition",
                if draft.custom_source {
                    "ProfileEditSaveOriginal"
                } else {
                    "ProfileEditProtected"
                },
            );
        }
        let Some(field) = draft.editor.fields().get(index) else {
            return i18n::tr("Edition", key);
        };
        let Some(value) = draft.editor.value(index) else {
            return i18n::tr("Edition", "ProfileMissing");
        };
        let value = if let Some((_, text)) = draft.input.as_ref().filter(|(i, _)| *i == index) {
            text.clone()
        } else {
            match field.control {
                Control::Boolean | Control::OptionalBoolean => i18n::tr(
                    "Edition",
                    match value.as_bool() {
                        Some(true) => "PatternYes",
                        Some(false) => "PatternNo",
                        None => "ProfileEditInherit",
                    },
                )
                .to_string(),
                Control::Choice(_) => {
                    let text = value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string());
                    if field.name() == "BeatSubDivision" {
                        format!("1/{}", value.as_u64().unwrap_or(1) * 4)
                    } else if field.name() == "StartingFootSpecified" {
                        i18n::tr(
                            "Edition",
                            if text == "0" {
                                "ProfileChoiceLeft"
                            } else {
                                "ProfileChoiceRight"
                            },
                        )
                        .to_string()
                    } else {
                        i18n::tr("Edition", &format!("ProfileChoice{text}")).to_string()
                    }
                }
                Control::Number
                    if draft.kind == Kind::Performed
                        && value.as_f64() == Some(-1.0)
                        && !matches!(
                            field.name(),
                            "LateralMinPanelDistance" | "LongitudinalMinPanelDistance"
                        ) =>
                {
                    i18n::tr("Edition", "ProfileEditInheritNumeric").to_string()
                }
                Control::Number => format!("{:.6}", value.as_f64().unwrap_or_default()),
                Control::Integer => value.to_string(),
            }
        };
        let key = match field.name() {
            "DefaultBracketParsingMethod" => "ExpressionEditMethod".to_owned(),
            "BracketParsingDetermination" => "ExpressionEditDetermination".to_owned(),
            "MinLevelForBrackets" => "ExpressionEditLevel".to_owned(),
            "UseAggressiveBracketsWhenMoreSimultaneousNotesThanCanBeCoveredWithoutBrackets" => {
                "ExpressionEditAggressive".to_owned()
            }
            "BalancedBracketsPerMinuteForAggressiveBrackets" => "ExpressionEditHigh".to_owned(),
            "BalancedBracketsPerMinuteForNoBrackets" => "ExpressionEditLow".to_owned(),
            _ => format!("ProfileField{}", field.name()),
        };
        if field.path().first().is_some_and(|p| p == "ArrowWeights") {
            return i18n::tr_fmt(
                "Edition",
                "ProfileFieldArrowWeight",
                &[
                    ("layout", &field.path()[1]),
                    (
                        "lane",
                        &(field.name().parse::<usize>().unwrap_or(0) + 1).to_string(),
                    ),
                    ("value", &value),
                ],
            );
        }
        let key = if field.name() == "Enabled" {
            format!("ProfileField{}Enabled", field.path()[0])
        } else {
            key
        };
        i18n::tr_fmt("Edition", &key, &[("value", &value)])
    }
}

impl State {
    fn open_profile_text(&mut self, field: TextField) {
        let Some(draft) = self.profile_edit.as_mut() else {
            return;
        };
        draft.text_field = field;
        let text = match field {
            TextField::Name => draft.name.as_deref().or_else(|| draft.profile.name()),
            TextField::Description => draft
                .description
                .as_deref()
                .or_else(|| draft.profile.description()),
        }
        .unwrap_or_default();
        draft.text_buffer = Some(deadsync_editor::text_buffer::Buffer::new(text, true));
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        self.label_preedit.clear();
        self.panel = Some(Panel::ProfileText);
        self.status = i18n::tr(
            "Edition",
            field.key("ProfileNameHelp", "ProfileDescriptionHelp"),
        );
    }
    pub(super) fn profile_text_input(&mut self, text: &str) -> bool {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        let changed = self
            .profile_edit
            .as_mut()
            .and_then(|d| d.text_buffer.as_mut())
            .is_some_and(|buffer| buffer.insert(text));
        self.label_preedit.clear();
        changed
    }
    pub(super) fn profile_text_key(&mut self, code: KeyCode) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        if self.control && matches!(code, KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV) {
            self.queue_text_clipboard(code);
            return;
        }
        use deadsync_editor::text_buffer::Movement;
        let Some(draft) = self.profile_edit.as_mut() else {
            return;
        };
        let Some(buffer) = draft.text_buffer.as_mut() else {
            return;
        };
        if code == KeyCode::Escape {
            draft.text_buffer = None;
            self.label_preedit.clear();
            self.panel = Some(Panel::ProfileEdit(
                draft.editor.fields().len() + draft.text_field.row(),
            ));
            self.status = i18n::tr(
                "Edition",
                draft
                    .text_field
                    .key("ProfileNameDiscarded", "ProfileDescriptionDiscarded"),
            );
            return;
        }
        if code == KeyCode::Enter {
            if !self.label_preedit.is_empty() {
                return;
            }
            match draft.text_field {
                TextField::Name => draft.name = Some(buffer.value().to_owned()),
                TextField::Description => {
                    draft.description = (buffer.value()
                        != draft.profile.description().unwrap_or_default())
                    .then(|| buffer.value().to_owned());
                }
            }
            draft.text_buffer = None;
            self.panel = Some(Panel::ProfileEdit(draft.editor.fields().len()));
            self.status = i18n::tr(
                "Edition",
                draft
                    .text_field
                    .key("ProfileNameStaged", "ProfileDescriptionStaged"),
            );
            return;
        }
        if self.control {
            match code {
                KeyCode::KeyA => buffer.select_all(),
                KeyCode::KeyZ if self.shift => {
                    buffer.redo();
                }
                KeyCode::KeyZ => {
                    buffer.undo();
                }
                KeyCode::KeyY => {
                    buffer.redo();
                }
                _ => {}
            }
        }
        let movement = match code {
            KeyCode::ArrowLeft => Some(if self.control {
                Movement::WordLeft
            } else {
                Movement::Left
            }),
            KeyCode::ArrowRight => Some(if self.control {
                Movement::WordRight
            } else {
                Movement::Right
            }),
            KeyCode::Home => Some(Movement::Start),
            KeyCode::End => Some(Movement::End),
            _ => None,
        };
        if let Some(movement) = movement {
            buffer.move_cursor(movement, self.shift);
            self.label_preedit.clear();
        }
        if code == KeyCode::Backspace {
            buffer.backspace(self.control);
            self.label_preedit.clear();
        }
        if code == KeyCode::Delete {
            buffer.delete(self.control);
            self.label_preedit.clear();
        }
    }
    pub(super) fn profile_text_keys(&self) -> &'static [&'static str] {
        match self.profile_edit.as_ref().map(|draft| draft.text_field) {
            Some(TextField::Description) => &[
                "ProfileDescriptionInput",
                "ProfileDescriptionAccept",
                "ProfileDescriptionCancel",
            ],
            _ => &["ProfileNameInput", "ProfileNameAccept", "ProfileNameCancel"],
        }
    }
    pub(super) fn profile_text_label(&self) -> std::sync::Arc<str> {
        let value = self
            .profile_edit
            .as_ref()
            .and_then(|draft| draft.text_buffer.as_ref())
            .map(|buffer| buffer.display(&self.label_preedit, 44))
            .unwrap_or_default();
        let key = self
            .profile_edit
            .as_ref()
            .map(|draft| {
                draft
                    .text_field
                    .key("ProfileNameInput", "ProfileDescriptionInput")
            })
            .unwrap_or("ProfileNameInput");
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}

fn profile_error(error: deadsync_editor::Error) -> String {
    if matches!(error, deadsync_editor::Error::ExternalChange) {
        i18n::tr("Edition", "ProfileEditExternalChange").to_string()
    } else {
        error.to_string()
    }
}

fn numeric_key(code: KeyCode, shift: bool) -> Option<char> {
    match code {
        KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
        KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
        KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
        KeyCode::KeyE => Some('e'),
        KeyCode::NumpadAdd => Some('+'),
        KeyCode::Equal if shift => Some('+'),
        _ => None,
    }
}

impl State {
    fn open_profile_speed(&mut self) {
        let Some(draft) = self.profile_edit.as_mut() else {
            return;
        };
        draft.travel_editor = draft.travel_speed.or_else(|| draft.profile.travel_speed());
        if draft.travel_editor.is_none() {
            return;
        }
        draft.travel_input = None;
        self.panel = Some(Panel::ProfileSpeed(0));
        self.status = i18n::tr("Edition", "ProfileSpeedHelp");
    }
    fn stage_speed_input(&mut self) -> bool {
        let Some(draft) = self.profile_edit.as_mut() else {
            return false;
        };
        let Some((index, text)) = &draft.travel_input else {
            return true;
        };
        let Some(value) = text
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|v| (1..=1000).contains(v))
        else {
            self.status = i18n::tr("Edition", "ProfileSpeedInvalid");
            return false;
        };
        let Some(speed) = draft.travel_editor.as_mut() else {
            return false;
        };
        if *index == 1 {
            speed.min_bpm = value;
        } else {
            speed.max_bpm = value;
        }
        draft.travel_input = None;
        true
    }
    pub(super) fn profile_speed_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape || (index == 6 && code == KeyCode::Enter) {
            if let Some(draft) = self.profile_edit.as_mut() {
                draft.travel_editor = None;
                draft.travel_input = None;
                self.panel = Some(Panel::ProfileEdit(draft.editor.fields().len() + 5));
            }
            self.status = i18n::tr("Edition", "ProfileSpeedDiscarded");
            return;
        }
        if !self.control && matches!(index, 1 | 2) {
            if let Some(draft) = self.profile_edit.as_mut() {
                if let Some(c) = numeric_key(code, self.shift) {
                    let (_, text) = draft
                        .travel_input
                        .get_or_insert_with(|| (index, String::new()));
                    if text.len() < 32 {
                        text.push(c);
                    }
                    return;
                }
                if code == KeyCode::Delete {
                    draft.travel_input = Some((index, String::new()));
                    return;
                }
                if code == KeyCode::Backspace {
                    let Some(speed) = draft.travel_editor else {
                        return;
                    };
                    let value = if index == 1 {
                        speed.min_bpm
                    } else {
                        speed.max_bpm
                    };
                    draft
                        .travel_input
                        .get_or_insert((index, value.to_string()))
                        .1
                        .pop();
                    return;
                }
            }
        }
        if !self.stage_speed_input() {
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            self.panel = Some(Panel::ProfileSpeed(
                (index
                    + if code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift) {
                        6
                    } else {
                        1
                    })
                    % 7,
            ));
            return;
        }
        let Some(draft) = self.profile_edit.as_mut() else {
            return;
        };
        let Some(speed) = draft.travel_editor.as_mut() else {
            return;
        };
        if index == 5 && code == KeyCode::Enter {
            if draft.editor.apply_travel_speed(*speed).is_err() {
                self.status = i18n::tr("Edition", "ProfileSpeedInvalid");
                return;
            }
            draft.travel_speed = Some(*speed);
            draft.travel_editor = None;
            self.panel = Some(Panel::ProfileEdit(draft.editor.fields().len()));
            self.status = i18n::tr("Edition", "ProfileSpeedStaged");
            return;
        }
        if !matches!(
            code,
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter
        ) {
            return;
        }
        let forward = code != KeyCode::ArrowLeft;
        if index == 0 {
            speed.subdivision_index = (speed.subdivision_index + if forward { 1 } else { 8 }) % 9;
        }
        if matches!(index, 1 | 2) && code != KeyCode::Enter {
            let value = if index == 1 {
                &mut speed.min_bpm
            } else {
                &mut speed.max_bpm
            };
            *value = value
                .saturating_add(if forward {
                    if self.shift { 10 } else { 1 }
                } else if self.shift {
                    -10
                } else {
                    -1
                })
                .clamp(1, 1000);
        }
    }
    pub(super) fn profile_speed_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let Some(draft) = self.profile_edit.as_ref() else {
            return i18n::tr("Edition", key);
        };
        let Some(speed) = draft.travel_editor else {
            return i18n::tr("Edition", key);
        };
        let value = match index {
            0 => format!(
                "1/{}",
                TravelSpeed::SUBDIVISIONS[speed.subdivision_index] * 4
            ),
            1 | 2 => draft
                .travel_input
                .as_ref()
                .filter(|(i, _)| *i == index)
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| {
                    if index == 1 {
                        speed.min_bpm
                    } else {
                        speed.max_bpm
                    }
                    .to_string()
                }),
            3 | 4 => speed
                .intervals()
                .map(|(min, max)| format!("{:.6}", if index == 3 { min } else { max }))
                .unwrap_or_else(|_| "-".into()),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}

impl State {
    pub(super) fn profile_defaults_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape || (index == 1 && code == KeyCode::Enter) {
            if let Some(draft) = &self.profile_edit {
                self.panel = Some(Panel::ProfileEdit(draft.reset_return));
            }
            self.status = i18n::tr("Edition", "ProfileDefaultsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            self.panel = Some(Panel::ProfileDefaults(1 - index));
            return;
        }
        if index != 0 || code != KeyCode::Enter {
            return;
        }
        let Some(draft) = self.profile_edit.as_mut() else {
            return;
        };
        match Editor::new(&Config::editor_defaults(draft.kind)) {
            Ok(editor) => {
                draft.editor = editor;
                draft.reset_defaults = true;
                draft.input = None;
                draft.travel_speed =
                    (draft.kind == Kind::Performed).then(TravelSpeed::editor_defaults);
                draft.travel_editor = None;
                draft.travel_input = None;
                self.panel = Some(Panel::ProfileEdit(draft.editor.fields().len()));
                self.status = i18n::tr("Edition", "ProfileDefaultsStaged");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
}

impl State {
    pub(super) fn profile_weights_key(&mut self, index: usize, code: KeyCode) {
        use deadsync_editor::generation::job::TARGET_LAYOUTS;
        if code == KeyCode::Escape || (index == 4 && code == KeyCode::Enter) {
            if let Some(draft) = &self.profile_edit {
                self.panel = Some(Panel::ProfileEdit(draft.editor.fields().len()));
            }
            self.status = i18n::tr("Edition", "ProfileWeightsBack");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            self.panel = Some(Panel::ProfileWeights(
                (index
                    + if code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift) {
                        4
                    } else {
                        1
                    })
                    % 5,
            ));
            return;
        }
        let Some(draft) = self.profile_edit.as_mut() else {
            return;
        };
        if index == 0
            && matches!(
                code,
                KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter
            )
        {
            draft.weight_layout = (draft.weight_layout
                + if code == KeyCode::ArrowLeft {
                    TARGET_LAYOUTS.len() - 1
                } else {
                    1
                })
                % TARGET_LAYOUTS.len();
        }
        if matches!(index, 2 | 3) && code == KeyCode::Enter {
            match draft
                .editor
                .set_arrow_weights_presence(TARGET_LAYOUTS[draft.weight_layout], index == 2)
            {
                Ok(changed) => {
                    self.status = i18n::tr(
                        "Edition",
                        if !changed {
                            "ProfileWeightsNoChange"
                        } else if index == 2 {
                            "ProfileWeightsAdded"
                        } else {
                            "ProfileWeightsRemoved"
                        },
                    )
                }
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
    }
    pub(super) fn profile_weights_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        use deadsync_editor::generation::job::TARGET_LAYOUTS;
        let Some(draft) = &self.profile_edit else {
            return i18n::tr("Edition", key);
        };
        let layout = TARGET_LAYOUTS[draft.weight_layout];
        let value = match index {
            0 => layout.to_owned(),
            1 => i18n::tr(
                "Edition",
                if draft.editor.has_arrow_weights(layout).unwrap_or(false) {
                    "ProfileWeightsExplicit"
                } else {
                    "ProfileEditInherit"
                },
            )
            .to_string(),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
