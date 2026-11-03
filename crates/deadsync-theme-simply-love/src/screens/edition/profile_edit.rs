pub(crate) mod save_as;
use super::{Panel, ProfileTarget, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::generation::{
    metadata::Guid,
    profile_controls::{Availability, Control, Editor},
    profiles::{Config, Kind, Profile, TravelSpeed},
};
#[derive(Clone)]
pub(super) struct PreferenceSnapshot {
    profiles: deadsync_editor::generation::job::ProfileOverrides,
    performed_name: Option<std::sync::Arc<str>>,
    expressed_name: Option<std::sync::Arc<str>>,
    revision: u64,
    preset:usize,
}
pub(super) struct PreferenceChange {
    before: PreferenceSnapshot,
    after: PreferenceSnapshot,
}
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
    ImportPath,
    ExportPath,
}
impl TextField {
    fn key(
        self,
        name: &'static str,
        description: &'static str,
        import: &'static str,
        export: &'static str,
    ) -> &'static str {
        match self {
            Self::Name => name,
            Self::Description => description,
            Self::ImportPath => import,
            Self::ExportPath => export,
        }
    }
    fn row(self) -> usize {
        match self {
            Self::Name => 2,
            Self::Description => 4,
            Self::ImportPath | Self::ExportPath => 0,
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
    fn prepared_copy(&self, config: Config) -> Result<Profile, String> {
        let draft = self;
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
        Ok(profile)
    }
    pub(super) fn action_count(&self) -> usize {
        if self.supports_travel_speed() { 13 } else { 11 }
    }
    pub(super) fn visible_rows(&self)->Vec<usize>{
        let count=self.editor.fields().len();
        (0..count+self.action_count()).filter(|index|self.editor.availability(*index)!=Some(Availability::Hidden)).collect()
    }
    pub(super) fn row_enabled(&self,index:usize)->bool{
        if index==self.editor.fields().len()+5&&self.supports_travel_speed(){return self.editor.source_travel_speed_enabled();}
        self.editor.availability(index).is_none_or(|state|state==Availability::Active)
    }
    pub(super) fn supports_travel_speed(&self) -> bool {
        self.kind == Kind::Performed
    }
}
impl State {
    pub(super) fn create_application_profile(&mut self, kind:Kind) {
        let result:Result<(Choices,Guid),String>=(||{
            if !matches!(self.catalog_target,ProfileTarget::Application){return Err("Global profile manager is not open".into());}
            let settings=self.profile_selection.as_ref().ok_or_else(||"Missing profiles".to_string())?;
            if !self.profile_catalog_current(&settings.catalog){return Err(i18n::tr("Edition","ProfileStale").to_string());}
            let selected=|kind,index:Option<usize>| {
                index.and_then(|i|settings.catalog.entries(kind).get(i)).map(|entry|entry.guid.clone())
            };
            let mut choices=Choices{pattern:selected(Kind::Pattern,settings.pattern),performed:selected(Kind::Performed,settings.performed),expressed:selected(Kind::Expressed,settings.expressed),
            };
            // A new profile starts from editor defaults, never the selected
            // preset or an unrelated custom profile's unknown metadata.
            let mut profile=Profile::new_editor_default(kind).map_err(profile_error)?;
            profile.rename(&i18n::tr("Edition","ProfileManagerNewName")).map_err(profile_error)?;
            let directory=self.profile_directory.as_deref().ok_or_else(||i18n::tr("Edition","ExpressionEditUnavailable").to_string())?;
            self.workspace.create_profile(directory,&profile).map_err(profile_error)?;
            let id=profile.guid().clone();match kind{Kind::Pattern=>choices.pattern=Some(id.clone()),Kind::Performed=>choices.performed=Some(id.clone()),Kind::Expressed=>choices.expressed=Some(id.clone()),
            };
            Ok((choices,id))
        })();
        match result {
            Ok((choices,id))=>{
                self.profile_creations.insert(self.workspace.file().session.revision(),(kind,id));
                self.open_profile_manager();
                if self.catalog_job.is_some(){self.profile_saved_choices=Some(choices);self.application_profile_created=Some(kind);}
                self.sync_profile_edit_notice();
            }
            Err(error)=>self.status=i18n::tr_fmt("Edition","Error",&[("message",&error)]),
        }
    }
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
                .profile_in(
                    &self.workspace.file().session,
                    self.profile_catalog_context().map_err(profile_error)?,
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
    fn profile_preference_snapshot(&self) -> PreferenceSnapshot {
        PreferenceSnapshot {
            profiles: self.generation.profiles.clone(),
            performed_name: self.generation.performed_name.clone(),
            expressed_name: self.generation.expressed_name.clone(),
            revision: self.profile_preference_revision,
            preset:self.generation.preset,
        }
    }
    pub(super) fn profile_preferences_changed(&mut self) {
        self.profile_preference_serial += 1;
        self.profile_preference_revision = self.profile_preference_serial;
    }
    pub(super) fn restore_profile_preferences(
        &mut self,
        command: super::Command,
        before_revision: u64,
    ) {
        let revision = self.workspace.file().session.revision();
        if revision == before_revision {
            return;
        }
        if matches!(command, super::Command::Undo) {
            if let Some((kind, guid)) = self.profile_creations.get(&before_revision).cloned() {
                let after = self.profile_preference_snapshot();
                if kind == Kind::Performed
                    && self.generation.profiles.performed.as_ref() == Some(&guid)
                {
                    self.generation.profiles.performed = None;
                    self.generation.performed_name = None;
                    self.generation.preset=0;
                }
                if kind == Kind::Expressed
                    && self.generation.profiles.expressed.as_ref() == Some(&guid)
                {
                    self.generation.profiles.expressed = None;
                    self.generation.expressed_name = None;
                }
                if self.generation.profiles != after.profiles {
                    self.profile_preferences_changed();
                    let before = self.profile_preference_snapshot();
                    self.profile_preference_history
                        .insert(before_revision, PreferenceChange { before, after });
                } else {
                    self.profile_preference_history.remove(&before_revision);
                }
                self.publish_autogen_profile_preferences();
                return;
            }
        }
        let change = match command {
            super::Command::Undo => self
                .profile_preference_history
                .get(&before_revision)
                .map(|change| (&change.after, &change.before)),
            super::Command::Redo => self
                .profile_preference_history
                .get(&revision)
                .map(|change| (&change.before, &change.after)),
            _ => None,
        };
        let Some((expected, restore)) = change else {
            return;
        };
        if self.profile_preference_revision != expected.revision {
            return;
        }
        self.generation.profiles = restore.profiles.clone();
        self.generation.performed_name = restore.performed_name.clone();
        self.generation.expressed_name = restore.expressed_name.clone();
        self.profile_preference_revision = restore.revision;
        self.generation.preset=restore.preset;
        self.publish_autogen_profile_preferences();
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
                        .profile_file_in(
                            &self.workspace.file().session,
                            self.profile_catalog_context().map_err(profile_error)?,
                            draft.kind,
                            &draft.source,
                        )
                        .map_err(profile_error)?;
                    let before = self.profile_preference_snapshot();
                    self.workspace
                        .delete_profile(&file)
                        .map_err(profile_error)?;
                    if draft.kind == Kind::Performed
                        && self.generation.profiles.performed.as_ref() == Some(&draft.source)
                    {
                        self.generation.profiles.performed = None;
                        self.generation.performed_name = None;
                        self.generation.preset=0;
                    }
                    if draft.kind == Kind::Expressed
                        && self.generation.profiles.expressed.as_ref() == Some(&draft.source)
                    {
                        self.generation.profiles.expressed = None;
                        self.generation.expressed_name = None;
                    }
                    if self.generation.profiles != before.profiles {
                        self.profile_preferences_changed();
                        let after = self.profile_preference_snapshot();
                        self.profile_preference_history.insert(
                            self.workspace.file().session.revision(),
                            PreferenceChange { before, after },
                        );
                    }
                    self.publish_autogen_profile_preferences();
                    Ok(())
                })();
                match result {
                    Ok(()) => {
                        self.profile_edit = None;
                        self.profile_selection = None;
                        self.chart_properties = None;
                        self.marker = None;
                        self.panel = None;
                        self.refresh();
                        self.status = i18n::tr("Edition", "ProfileDeleted");
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
        let prepared:Result<Choices,String>=(||{
            let draft=self.profile_edit.as_ref().ok_or_else(||"Missing draft".to_string())?;
            if draft.input.is_some()||draft.name.is_some()||draft.description.is_some()||draft.travel_speed.is_some()||draft.reset_defaults||draft.editor.finish().map_err(profile_error)?!=*draft.profile.config(){return Err(i18n::tr("Edition","ProfileHistoryPending").to_string());}
            let settings=self.profile_selection.as_ref().ok_or_else(||"Missing profiles".to_string())?;
            let offset=usize::from(matches!(draft.back,Panel::ConversionProfiles(_)));
            let selected=|kind,index:Option<usize>| {
                index.and_then(|i|i.checked_sub(offset)).and_then(|i|settings.catalog.entries(kind).get(i)).map(|entry|entry.guid.clone())
            };
            Ok(Choices{pattern:selected(Kind::Pattern,settings.pattern),performed:selected(Kind::Performed,settings.performed),expressed:selected(Kind::Expressed,settings.expressed),
            })
        })();
        let choices=match prepared{Ok(value)=>value,Err(error)=>{self.status=i18n::tr_fmt("Edition","Error",&[("message",&error)]);return;}};
        let session=&self.workspace.file().session;
        if if redo{session.redo_label().is_none()}else{session.undo_label().is_none()}{self.status=i18n::tr("Edition","ProfileHistoryEmpty");return;}
        let before=session.revision();
        self.command(if redo{deadsync_editor::workspace::Command::Redo}else{deadsync_editor::workspace::Command::Undo});
        if self.workspace.file().session.revision()==before{return;}
        let rebound = match self.catalog_target{ProfileTarget::Chart=>self.chart_properties
                .as_mut()
                .map(|draft| draft.after_profile_history(&self.workspace.file().session, redo)),ProfileTarget::Marker=>self.marker
                .as_mut()
                .map(|draft| draft.after_profile_history(&self.workspace.file().session, redo)),
            _ => None,
        };
        if let Some(Err(error)) = rebound {
            self.status = i18n::editor_error(&error).into();
            return;
        }
        let command_status = self.status.clone();
        self.profile_saved_choices = Some(choices);
        self.profile_edit = None;
        match self.catalog_target {
            ProfileTarget::Chart => self.open_chart_expression(),
            ProfileTarget::Marker => self.open_marker_profiles(),target=>self.open_conversion_profiles_for(target),
        };
        if self.workspace.has_profile_durability(){self.status=command_status;}
        self.sync_profile_edit_notice();
    }
    pub(super) fn sync_profile_edit_notice(&mut self){
        if let Some(update)=self.workspace.take_profile_edit_update().or_else(||self.workspace.pending_profile_durability_update()){
            if let Some(error)=update.durability_warning{self.status=i18n::tr_fmt("Edition","ProfileDurabilityWarning",&[("path",&update.path.to_string_lossy()),("error",&error)],
                );}
        }
    }
    fn stage_profile_input(&mut self) -> bool {
        let Some(draft) = self.profile_edit.as_mut() else {
            return false;
        };
        let Some((index, text)) = &draft.input else {
            return true;
        };
        if draft.editor.availability(*index)!=Some(Availability::Active){
            draft.input=None;self.status=i18n::tr("Edition","ProfileControlUnavailable");return true;
        }
        if draft.editor.set_source_text(*index, text).is_err() {
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
        if code == KeyCode::F10 && !self.control
            || index == count + actions - 2 && code == KeyCode::Enter
        {
            if self.stage_profile_input() {
                self.open_profile_text(TextField::ImportPath);
            }
            return;
        }
        if code == KeyCode::F11 && !self.control
            || index == count + actions - 1 && code == KeyCode::Enter
        {
            if self.stage_profile_input() {
                self.open_profile_text(TextField::ExportPath);
            }
            return;
        }
        if code == KeyCode::F9 && !self.control
            || index == count + actions - 3 && code == KeyCode::Enter
        {
            self.open_profile_delete();
            return;
        }
        let undo = code == KeyCode::F7 && !self.control
            || index == count + actions - 5 && code == KeyCode::Enter;
        let redo = code == KeyCode::F8 && !self.control
            || index == count + actions - 4 && code == KeyCode::Enter;
        if undo || redo {
            self.profile_saved_history(redo);
            return;
        }
        if (index == count + actions - 6 && code == KeyCode::Enter)
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
            if self.profile_edit.as_ref().is_some_and(|draft|draft.editor.source_travel_speed_enabled()){self.open_profile_speed();}
            else{self.status=i18n::tr("Edition","ProfileControlUnavailable");}
            return;
        }
        if index == count + 3 && code == KeyCode::Enter {
            if self.stage_profile_input() {
                self.save_profile(false);
            }
            return;
        }
        if !draft.row_enabled(index)&&!matches!(code,KeyCode::ArrowUp|KeyCode::ArrowDown|KeyCode::Tab|KeyCode::PageUp|KeyCode::PageDown|KeyCode::Home|KeyCode::End){
            self.status=i18n::tr("Edition","ProfileControlUnavailable");return;
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
        let rows=self.profile_edit.as_ref().map(Draft::visible_rows).unwrap_or_default();
        let current=rows.iter().position(|row|*row==index).unwrap_or_else(||rows.partition_point(|row|*row<index).min(rows.len()-1));
        let next=match code{
            KeyCode::ArrowUp=>Some(rows[(current+rows.len()-1)%rows.len()]),
            KeyCode::ArrowDown=>Some(rows[(current+1)%rows.len()]),
            KeyCode::Tab=> {
                Some(rows[(current+if self.shift{rows.len()-1}else{1})%rows.len()])
            }
            KeyCode::PageDown=>Some(rows[(current+8).min(rows.len()-1)]),
            KeyCode::PageUp=>Some(rows[current.saturating_sub(8)]),
            KeyCode::Home=>Some(rows[0]),KeyCode::End=>Some(count),_=>None,
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
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))],
                );
            }
        }
    }
    fn save_profile(&mut self, copy: bool) {
        let result: Result<Choices, String> = (|| {
            let chart=self.profile_catalog_context().map_err(profile_error)?;
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
                .profile_in(
                    &self.workspace.file().session,
                    chart,
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
                let profile = draft.prepared_copy(config)?;
                self.workspace
                    .create_profile(directory, &profile)
                    .map_err(profile_error)?;
                self.profile_creations.insert(
                    self.workspace.file().session.revision(),
                    (draft.kind, profile.guid().clone()),
                );
                match self.catalog_target {
                    ProfileTarget::Chart => {
                        if let Some(parent) = self.chart_properties.as_mut() {
                            parent
                                .after_profile_creation(&self.workspace.file().session)
                                .map_err(profile_error)?;
                        }
                    }
                    ProfileTarget::Marker => {
                        if let Some(parent) = self.marker.as_mut() {
                            parent
                                .after_profile_creation(&self.workspace.file().session)
                                .map_err(profile_error)?;
                        }
                    }
                    _ => {}
                }

                profile.guid().clone()
            } else {
                if !draft.custom_source {
                    return Err(i18n::tr("Edition", "ProfileEditProtected").to_string());
                }
                let captured = settings
                    .catalog
                    .profile_file_in(
                        &self.workspace.file().session,
                        chart,
                        draft.kind,
                        &draft.source,
                    )
                    .map_err(profile_error)?;
                let mut profile = captured.profile().clone();
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
                let changed=self.workspace.edit_profile(&captured,&profile).map_err(profile_error)?;
                if changed {
                    match self.catalog_target {
                        ProfileTarget::Chart=> {
                            if let Some(parent)=self.chart_properties.as_mut(){parent.after_profile_creation(&self.workspace.file().session).map_err(profile_error)?;}
                        }
                        ProfileTarget::Marker=> {
                            if let Some(parent)=self.marker.as_mut(){parent.after_profile_creation(&self.workspace.file().session).map_err(profile_error)?;}
                        }
                        _=>{}
                    }
                }
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
                self.sync_profile_edit_notice();
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
        if index==count+5&&draft.supports_travel_speed()&&!draft.row_enabled(index){
            return i18n::tr_fmt("Edition","ProfileControlInactive",&[("value",&i18n::tr("Edition",key))],
            );
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
        let label=i18n::tr_fmt("Edition", &key, &[("value", &value)]);
        if draft.row_enabled(index){label}else{i18n::tr_fmt("Edition","ProfileControlInactive",&[("value",&label)])}
    }
}

impl State {
    fn finish_profile_transfer(&mut self, field: TextField, text: &str) {
        let result: Result<(), String> = (|| {
            let draft = self
                .profile_edit
                .as_mut()
                .ok_or_else(|| "Missing draft".to_string())?;
            let path = std::path::Path::new(text);
            if !path.is_absolute()
                || deadsync_editor::generation::profiles::Kind::from_path(path) != Some(draft.kind)
            {
                return Err(i18n::tr("Edition", "ProfileTransferInvalidPath").to_string());
            }
            if field == TextField::ExportPath {
                let profile = draft.prepared_copy(draft.editor.finish().map_err(profile_error)?)?;
                self.workspace.retry_profile_durability().map_err(profile_error)?;
                let committed=profile.export_to_committed(path).map_err(profile_error)?;
                self.workspace.retain_profile_publication(committed.into_file());
            } else {
                let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                let source = Profile::parse(draft.kind, &bytes).map_err(profile_error)?;
                let profile = source
                    .duplicate(source.name().unwrap_or("Imported"))
                    .map_err(profile_error)?;
                let editor = Editor::new(profile.config()).map_err(profile_error)?;
                draft.profile = profile;
                draft.editor = editor;
                draft.custom_source = false;
                draft.input = None;
                draft.name = None;
                draft.description = None;
                draft.travel_speed = None;
                draft.reset_defaults = false;
            }
            draft.text_buffer = None;
            self.panel = Some(Panel::ProfileEdit(draft.editor.fields().len()));
            Ok(())
        })();
        self.status = match result {
            Ok(()) => i18n::tr(
                "Edition",
                if field == TextField::ImportPath {
                    "ProfileImported"
                } else {
                    "ProfileExported"
                },
            ),
            Err(error) => i18n::tr_fmt("Edition", "Error", &[("message", &error)]),
        };
        self.sync_profile_edit_notice();
    }
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
            TextField::ImportPath | TextField::ExportPath => Some(""),
        }
        .unwrap_or_default();
        draft.text_buffer = Some(deadsync_editor::text_buffer::Buffer::new(text, true));
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        self.label_preedit.clear();
        self.panel = Some(Panel::ProfileText);
        self.status = i18n::tr(
            "Edition",
            field.key(
                "ProfileNameHelp",
                "ProfileDescriptionHelp",
                "ProfileImportHelp",
                "ProfileExportHelp",
            ),
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
        if code == KeyCode::Enter && self.label_preedit.is_empty() {
            let transfer = self.profile_edit.as_ref().and_then(|draft| {
                matches!(
                    draft.text_field,
                    TextField::ImportPath | TextField::ExportPath
                )
                .then(|| {
                    (
                        draft.text_field,
                        draft
                            .text_buffer
                            .as_ref()
                            .map(|b| b.value().to_owned())
                            .unwrap_or_default(),
                    )
                })
            });
            if let Some((field, path)) = transfer {
                self.finish_profile_transfer(field, &path);
                return;
            }
        }

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
                draft.text_field.key(
                    "ProfileNameDiscarded",
                    "ProfileDescriptionDiscarded",
                    "ProfileTransferCancelled",
                    "ProfileTransferCancelled",
                ),
            );
            return;
        }
        if code == KeyCode::Enter {
            if !self.label_preedit.is_empty() {
                return;
            }
            match draft.text_field {
                TextField::ImportPath | TextField::ExportPath => {
                    unreachable!("transfer handled above")
                }
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
                draft.text_field.key(
                    "ProfileNameStaged",
                    "ProfileDescriptionStaged",
                    "ProfileImported",
                    "ProfileExported",
                ),
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
            Some(TextField::ImportPath) => &[
                "ProfileImportInput",
                "ProfileImportAccept",
                "ProfileTransferCancel",
            ],
            Some(TextField::ExportPath) => &[
                "ProfileExportInput",
                "ProfileExportAccept",
                "ProfileTransferCancel",
            ],
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
                draft.text_field.key(
                    "ProfileNameInput",
                    "ProfileDescriptionInput",
                    "ProfileImportInput",
                    "ProfileExportInput",
                )
            })
            .unwrap_or("ProfileNameInput");
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}

fn profile_error(error: deadsync_editor::Error) -> String {
    if matches!(error, deadsync_editor::Error::ExternalChange) {
        i18n::tr("Edition", "ProfileEditExternalChange").to_string()
    } else {
        i18n::editor_error(&error)
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
        if !draft.editor.source_travel_speed_enabled(){self.status=i18n::tr("Edition","ProfileControlUnavailable");return;}
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
        if !draft.editor.source_travel_speed_enabled(){
            draft.travel_input=None;self.status=i18n::tr("Edition","ProfileControlUnavailable");return false;
        }
        let Some((index, text)) = &draft.travel_input else {
            return true;
        };
        let Some(value) = text
            .trim()
            .parse::<i32>()
            .ok()
        else {
            self.status = i18n::tr("Edition", "ProfileSpeedInvalid");
            return false;
        };
        let Some(speed) = draft.travel_editor.as_mut() else {
            return false;
        };
        if *index == 1 {
            speed.min_bpm = value.clamp(1,speed.max_bpm.clamp(1,1000));
        } else {
            speed.max_bpm = value.clamp(speed.min_bpm.clamp(1,1000),1000);
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
        if !self.profile_edit.as_ref().is_some_and(|draft|draft.editor.source_travel_speed_enabled()){
            self.status=i18n::tr("Edition","ProfileControlUnavailable");return;
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
            let (minimum,maximum)=if index==1{(1,speed.max_bpm.clamp(1,1000))}else{(speed.min_bpm.clamp(1,1000),1000)};
            let value = if index == 1 {&mut speed.min_bpm} else {&mut speed.max_bpm};
            *value = value
                .saturating_add(if forward {
                    if self.shift { 10 } else { 1 }
                } else if self.shift {
                    -10
                } else {
                    -1
                })
                .clamp(minimum,maximum);
        }
    }
    pub(super) fn profile_speed_label(&self,index:usize,key:&str)->std::sync::Arc<str>{
        let label=self.profile_speed_label_value(index,key);
        if matches!(index,0|1|2|5)&&self.profile_edit.as_ref().is_some_and(|draft|!draft.editor.source_travel_speed_enabled()){
            i18n::tr_fmt("Edition","ProfileControlInactive",&[("value",&label)])
        }else{label}
    }
    fn profile_speed_label_value(&self, index: usize, key: &str) -> std::sync::Arc<str> {
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
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))],
                )
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
                        i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))],
                    )
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

#[cfg(test)]
mod global_profile_history_tests {
    use super::*;
    use deadlib_platform::input::RawKeyboardEvent;
    use deadsync_editor::workspace::Command;
    fn poll(state:&mut State){let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);while state.catalog_job.is_some(){assert!(std::time::Instant::now()<deadline,"profile catalog did not complete");state.poll_profile_catalog();std::thread::yield_now();}assert!(state.profile_selection.is_some(),"{}",state.status);}
    fn edit_custom(state:&mut State){state.open_conversion_profiles();poll(state);let settings=state.profile_selection.as_mut().unwrap();let index=settings.catalog.entries(Kind::Expressed).iter().position(|entry|entry.guid.to_string()=="12345678-9abc-4def-8123-456789abcdef").unwrap();settings.expressed=Some(index+1);state.open_profile_edit(Kind::Expressed,Panel::ConversionProfiles(0));assert!(state.profile_edit.is_some());}
    fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0,
            });}}
    #[test]
    fn original_profile_save_and_f7_f8_share_the_global_note_history_cursor_en_fr(){
        for language in ["en","fr"]{
            i18n::init(deadsync_assets::language::load_for_tests(language));i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root=tempfile::tempdir().unwrap();let song=root.path().join("song.ssc");let profile=root.path().join("ecc-global.json");let original=br#"{ "Guid":"12345678-9abc-4def-8123-456789abcdef", "Name":"Original", "Future":1e999, "Config":{} }
"#;std::fs::write(&profile,original).unwrap();std::fs::write(&song,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
            ).unwrap();
            let mut state=State::open_with_profiles(&song,0,root.path()).unwrap();let clean=state.workspace.file().session.serialize().unwrap();state.command(Command::Place(0));let note=state.workspace.file().session.serialize().unwrap();assert_ne!(note,clean);let content=state.workspace.file().session.content_revision();
            edit_custom(&mut state);state.profile_edit.as_mut().unwrap().name=Some("Changed".into());state.save_profile(false);poll(&mut state);let changed=std::fs::read(&profile).unwrap();assert_ne!(changed,original);assert_eq!(state.workspace.file().session.content_revision(),content);assert_eq!(state.workspace.file().session.serialize().unwrap(),note);
            state.panel=None;state.command(Command::Undo);assert_eq!(std::fs::read(&profile).unwrap(),original);assert_eq!(state.workspace.file().session.serialize().unwrap(),note);state.command(Command::Undo);assert_eq!(state.workspace.file().session.serialize().unwrap(),clean);assert!(!state.workspace.file().session.is_dirty());state.command(Command::Redo);state.command(Command::Redo);assert_eq!(std::fs::read(&profile).unwrap(),changed);
            edit_custom(&mut state);press(&mut state,KeyCode::F7);poll(&mut state);assert_eq!(std::fs::read(&profile).unwrap(),original);assert_eq!(state.workspace.file().session.serialize().unwrap(),note);let revision=state.workspace.file().session.revision();edit_custom(&mut state);press(&mut state,KeyCode::F8);poll(&mut state);assert_ne!(state.workspace.file().session.revision(),revision);assert_eq!(std::fs::read(&profile).unwrap(),changed);
            // Pending native inspector edits are not silently discarded by F7.
            edit_custom(&mut state);state.profile_edit.as_mut().unwrap().name=Some("Pending".into());let revision=state.workspace.file().session.revision();press(&mut state,KeyCode::F7);assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(std::fs::read(&profile).unwrap(),changed);press(&mut state,KeyCode::Escape);assert!(state.profile_edit.is_none());assert_eq!(std::fs::read(&profile).unwrap(),changed);
        }
    }
    #[test]
    fn stale_save_original_keeps_pending_draft_global_history_and_external_profile(){
        i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let song=root.path().join("song.ssc");let profile=root.path().join("ecc-global.json");std::fs::write(&profile,br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","Config":{}}"#,
        ).unwrap();std::fs::write(&song,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
        ).unwrap();let mut state=State::open_with_profiles(&song,0,root.path()).unwrap();edit_custom(&mut state);state.profile_edit.as_mut().unwrap().name=Some("Pending".into());let before=state.workspace.file().session.serialize().unwrap();let revision=state.workspace.file().session.revision();std::fs::write(&profile,b"external writer").unwrap();state.save_profile(false);assert!(state.profile_edit.is_some());assert_eq!(state.profile_edit.as_ref().unwrap().name.as_deref(),Some("Pending"));assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert_eq!(std::fs::read(&profile).unwrap(),b"external writer");assert!(state.workspace.file().session.undo_label().is_none());
    }
}

#[cfg(test)]
mod explicit_profile_export_tests {
    use super::*;
    #[test]
    fn explicit_export_uses_no_clobber_and_keeps_inspector_and_song_history_en_fr(){
        for language in ["en","fr"]{
            i18n::init(deadsync_assets::language::load_for_tests(language));i18n::set_locale(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let song=root.path().join("song.ssc");std::fs::write(&song,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            ).unwrap();let original=root.path().join("ecc-original.json");std::fs::write(&original,br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","Config":{}}"#,
            ).unwrap();let mut state=State::open_with_profiles(&song,0,root.path()).unwrap();state.open_conversion_profiles();let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);while state.catalog_job.is_some(){assert!(std::time::Instant::now()<deadline);state.poll_profile_catalog();std::thread::yield_now();}let settings=state.profile_selection.as_mut().unwrap();let index=settings.catalog.entries(Kind::Expressed).iter().position(|entry|entry.guid.to_string()=="12345678-9abc-4def-8123-456789abcdef").unwrap();settings.expressed=Some(index+1);state.open_profile_edit(Kind::Expressed,Panel::ConversionProfiles(0));state.profile_edit.as_mut().unwrap().name=Some("Exported copy".into());let before=state.workspace.file().session.serialize().unwrap();let revision=state.workspace.file().session.revision();let output=root.path().join("ecc-export.json");std::fs::write(&output,b"existing export").unwrap();state.finish_profile_transfer(TextField::ExportPath,output.to_str().unwrap());assert_eq!(std::fs::read(&output).unwrap(),b"existing export");assert!(state.profile_edit.is_some());assert_eq!(state.workspace.file().session.revision(),revision);std::fs::remove_file(&output).unwrap();state.finish_profile_transfer(TextField::ExportPath,output.to_str().unwrap());let exported=Profile::parse(Kind::Expressed,&std::fs::read(&output).unwrap()).unwrap();assert_eq!(exported.name(),Some("Exported copy"));assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert!(state.workspace.file().session.undo_label().is_none());assert!(state.profile_edit.is_some());assert!(matches!(state.panel,Some(Panel::ProfileEdit(_))));
        }
    }
}

#[cfg(test)]
mod source_bounded_input_tests{
    use super::*;
    fn poll(state:&mut State){let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);while state.catalog_job.is_some(){assert!(std::time::Instant::now()<deadline);state.poll_profile_catalog();std::thread::yield_now();}assert!(state.profile_selection.is_some(),"{}",state.status);}
    #[test]
    fn actual_inspector_stage_clamps_source_text_and_travel_range_before_save_en_fr(){
        for language in ["en","fr"]{
            i18n::init(deadsync_assets::language::load_for_tests(language));i18n::set_locale(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let song=root.path().join("song.ssc");let path=root.path().join("pcc-source-input.json");std::fs::write(&song,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            ).unwrap();let json=format!(r#"{{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","TravelSpeedNoteTypeDenominatorIndex":3,"TravelSpeedMinBPM":125,"TravelSpeedMaxBPM":170,"Config":{}}}"#,Config::editor_defaults(Kind::Performed).json().unwrap());std::fs::write(&path,json).unwrap();let mut state=State::open_with_profiles(&song,0,root.path()).unwrap();let before=state.workspace.file().session.serialize().unwrap();state.open_conversion_profiles();poll(&mut state);let settings=state.profile_selection.as_mut().unwrap();let i=settings.catalog.entries(Kind::Performed).iter().position(|entry|entry.guid.to_string()=="12345678-9abc-4def-8123-456789abcdef").unwrap();settings.performed=Some(i+1);state.open_profile_edit(Kind::Performed,Panel::ConversionProfiles(0));
            let draft=state.profile_edit.as_mut().unwrap();let index=draft.editor.fields().iter().position(|f| {
                    f.path()==["Facing".to_owned(),"InwardPercentageCutoff".to_owned()]
                }).unwrap();draft.input=Some((index,"1.34".into()));state.profile_edit_key(index,KeyCode::ArrowDown);assert_eq!(state.profile_edit.as_ref().unwrap().editor.value(index).unwrap().as_f64(),Some(1.0));
            state.open_profile_speed();state.profile_edit.as_mut().unwrap().travel_input=Some((1,"1001".into()));assert!(state.stage_speed_input());assert_eq!(state.profile_edit.as_ref().unwrap().travel_editor.unwrap().min_bpm,170);
            let draft=state.profile_edit.as_mut().unwrap();draft.travel_editor=Some(TravelSpeed::editor_defaults());draft.travel_input=Some((2,"0".into()));assert!(state.stage_speed_input());assert_eq!(state.profile_edit.as_ref().unwrap().travel_editor.unwrap().max_bpm,125);state.profile_speed_key(5,KeyCode::Enter);state.save_profile(false);poll(&mut state);
            let profile=deadsync_editor::generation::profiles::ProfileFile::open(&path).unwrap();let Config::Performed(config)=profile.profile().config()else{panic!()};assert_eq!(config.facing.inward_percentage_cutoff,1.0);let speed=profile.profile().travel_speed().unwrap();assert_eq!((speed.min_bpm,speed.max_bpm),(125,125));assert_eq!(config.step_tightening.speed_min_time_seconds,120.0/(4.0*125.0));assert_eq!(config.step_tightening.speed_max_time_seconds,120.0/(4.0*125.0));assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert!(!state.workspace.file().session.is_dirty());
        }
    }
}

#[cfg(test)]
mod application_manager_tests {
    use super::super::*;
    use super::*;
    use deadsync_editor::generation::profiles::Kind;
    fn wait(state:&mut State){
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        while state.catalog_job.is_some(){assert!(std::time::Instant::now()<deadline);state.poll_profile_catalog();std::thread::yield_now();}
        assert!(state.profile_selection.is_some(),"{}",state.status);
    }
    fn selected(state:&State,kind:Kind)->Guid{
        let s=state.profile_selection.as_ref().unwrap();
        let index=match kind{Kind::Pattern=>s.pattern,Kind::Performed=>s.performed,Kind::Expressed=>s.expressed,
        }.unwrap();
        s.catalog.entries(kind)[index].guid.clone()
    }
    fn edit(state:&mut State,kind:Kind){
        let row=match kind{Kind::Pattern=>0,Kind::Performed=>1,Kind::Expressed=>2,
        };
        state.panel=Some(Panel::Profiles(row));state.panel_key(KeyCode::KeyE);assert!(state.profile_edit.is_some());
    }
    #[test]
    fn global_manager_all_kinds_without_or_with_song_uses_real_crud_and_global_history(){
        for song in [false,true] {
            crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");
            std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
            ).unwrap();
            let mut state=if song{State::open(&path,0).unwrap()}else{State::without_song(root.path()).unwrap()};
            state.profile_directory=Some(root.path().to_owned());
            let bytes=state.workspace.file().session.serialize().unwrap();let chart=state.workspace.has_chart().then(||state.workspace.chart());let position=state.playback_position();
            let profiles=state.generation.profiles.clone();
            // The actual two entry menus, with no fabricated chart prerequisite.
            state.panel=Some(if song{Panel::Menu(34)}else{Panel::FileMenu(17)});state.panel_key(KeyCode::Enter);wait(&mut state);
            assert!(matches!(state.catalog_target,ProfileTarget::Application));assert!(matches!(state.panel,Some(Panel::Profiles(0))));
            for kind in [Kind::Pattern,Kind::Performed,Kind::Expressed] {
                let row=match kind{Kind::Pattern=>0,Kind::Performed=>1,Kind::Expressed=>2,
                };state.panel=Some(Panel::Profiles(row));state.panel_key(KeyCode::KeyN);wait(&mut state);
                assert!(state.profile_edit.is_some(),"{kind:?}, song={song}: {}",state.status);assert_eq!(state.profile_edit.as_ref().unwrap().profile.config(),&Config::editor_defaults(kind));
                state.profile_edit.as_mut().unwrap().name=Some(format!("Global {kind:?}"));state.save_profile(true);wait(&mut state);
                let id=selected(&state,kind);let captured=state.profile_selection.as_ref().unwrap().catalog.profile_file_in(&state.workspace.file().session,profile_catalog::Context::Application,kind,&id,
                    ).unwrap();
                let file=captured.path().to_owned();let original=std::fs::read(&file).unwrap();
                edit(&mut state,kind);state.profile_edit.as_mut().unwrap().name=Some("Edited globally".into());state.save_profile(false);wait(&mut state);
                let changed=std::fs::read(&file).unwrap();assert_ne!(changed,original);
                edit(&mut state,kind);state.panel_key(KeyCode::F7);wait(&mut state);assert_eq!(std::fs::read(&file).unwrap(),original);
                edit(&mut state,kind);state.panel_key(KeyCode::F8);wait(&mut state);assert_eq!(std::fs::read(&file).unwrap(),changed);
                edit(&mut state,kind);state.profile_delete_key(1,KeyCode::Enter);assert!(!file.exists());
                state.command(Command::Undo);assert_eq!(std::fs::read(&file).unwrap(),changed);
                state.command(Command::Redo);assert!(!file.exists());state.command(Command::Undo);assert_eq!(std::fs::read(&file).unwrap(),changed);
                state.open_profile_manager();wait(&mut state);
                assert_eq!(state.workspace.file().session.serialize().unwrap(),bytes);assert!(!state.workspace.file().session.is_dirty());assert_eq!(state.generation.profiles,profiles);
            }
            assert_eq!(state.workspace.has_chart().then(||state.workspace.chart()),chart);assert_eq!(state.playback_position(),position);
            if song {
                state.profile_edit=None;state.profile_selection=None;state.panel=None;state.command_file_close();
                assert!(!state.has_song());assert!(state.workspace.file().session.undo_label().is_none(),"CloseSong resets source ActionQueue");
                state.panel=Some(Panel::FileMenu(17));state.panel_key(KeyCode::Enter);wait(&mut state);
                for kind in [Kind::Pattern,Kind::Performed,Kind::Expressed]{assert!(state.profile_selection.as_ref().unwrap().catalog.entries(kind).iter().any(|entry|entry.name.as_deref()==Some("Edited globally")));}
            } else {assert!(state.workspace.file().session.document().charts().is_empty());}
        }
    }
    #[test]
    fn application_catalog_stale_and_external_profile_change_preserve_pending_draft(){
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();state.profile_directory=Some(root.path().to_owned());state.open_profile_manager();wait(&mut state);
        edit(&mut state,Kind::Expressed);state.save_profile(true);wait(&mut state);let id=selected(&state,Kind::Expressed);
        let file=state.profile_selection.as_ref().unwrap().catalog.profile_file_in(&state.workspace.file().session,profile_catalog::Context::Application,Kind::Expressed,&id,
            ).unwrap().path().to_owned();
        edit(&mut state,Kind::Expressed);state.profile_edit.as_mut().unwrap().name=Some("Pending".into());let revision=state.workspace.file().session.revision();std::fs::write(&file,b"external writer").unwrap();state.save_profile(false);
        assert!(state.profile_edit.is_some());assert_eq!(state.workspace.file().session.revision(),revision);assert_eq!(std::fs::read(&file).unwrap(),b"external writer");
        let old=state.profile_selection.take().unwrap().catalog;state.workspace=Workspace::new_song(root.path(),0.009).unwrap();assert!(!state.profile_catalog_current(&old));
    }
}

#[cfg(test)]
mod source_gates;
