//! Pack metadata is staged independently from the active song document.
use super::{Panel, State};
use crate::{act, i18n};
use chrono::Datelike;
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::{
    chart_order::SessionBinding,
    pack::{
        Data,
        storage::{DeleteOutcome, DeleteSnapshot, Durability, Error, FileSession},
    },
    text_buffer::Movement,
};
use std::{borrow::Cow, path::PathBuf, sync::Arc};
// Small native I/O boundary: domain/disk behavior stays in pack::storage;
// presentation tests can inject a published-but-not-yet-durable receipt.
trait PackFile: Send {
    fn history_owner(&self) -> Option<deadsync_editor::pack::storage::history::Owner> {
        None
    }
    fn synchronize(&mut self) {}
    fn retire(&self) {}
    fn prepare_delete(&self) -> Result<DeleteSnapshot, Error> {
        Err(Error::StaleSnapshot)
    }
    fn delete(&mut self, _review: DeleteSnapshot) -> Result<DeleteOutcome, Error> {
        Err(Error::StaleSnapshot)
    }
    fn deletion_pending(&self) -> bool {
        false
    }
    fn retry_delete(&mut self) -> Result<DeleteOutcome, Error> {
        Err(Error::StaleSnapshot)
    }
    fn external_snapshot(&self) -> Option<deadsync_editor::pack::storage::ExternalSnapshot> {
        None
    }
    fn data(&self) -> &Data;
    fn dirty(&self) -> bool;
    fn pending(&self) -> bool;
    fn save(&mut self) -> Result<Durability, Error>;
    fn retry(&mut self) -> Result<Durability, Error>;
    fn external(&self) -> Result<bool, Error>;
}
mod history_owner;
use history_owner::boxed;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Title,
    TranslitTitle,
    SortTitle,
    Series,
    Year,
    Banner,
}
impl Field {
    pub(super) const ALL: [Self; 6] = [
        Self::Title,
        Self::TranslitTitle,
        Self::SortTitle,
        Self::Series,
        Self::Year,
        Self::Banner,
    ];
    fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap()
    }
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Title => "PackTitle",
            Self::TranslitTitle => "PackTranslitTitle",
            Self::SortTitle => "PackSortTitle",
            Self::Series => "PackSeries",
            Self::Year => "PackYear",
            Self::Banner => "PackBanner",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    DeleteConfirm,
    DeletePending,
    Edit,
    Missing,
    ReadFailed,
    Conflict,
    ReloadConfirm,
    Pending,
    SourceChanged,
    ReleaseConfirm,
}
#[derive(Clone)]
pub(super) struct CouplingGuard {
    identity: Arc<()>,
    revision: u64,
    root: PathBuf,
}
pub(super) struct Settings {
    identity: Arc<()>,
    revision: u64,
    text_reviews: [Option<deadsync_editor::pack::storage::history::Review>; 6],
    delete_review: Option<(u64, DeleteSnapshot)>,
    binding: SessionBinding,
    source: PathBuf,
    root: PathBuf,
    defaults: Data,
    file: Option<Box<dyn PackFile>>,
    mode: Mode,
    index: usize,
    // Cached field text stays local until deactivation. Invalid text survives
    // closing/reopening without becoming a saved value or a history action.
    invalid: [Option<String>; 6],
}
impl Settings {
    fn is_dirty(&self) -> bool {
        let pending = self.invalid.iter().enumerate().any(|(index, value)| {
            value.as_ref().is_some_and(|value| {
                self.file
                    .as_ref()
                    .is_none_or(|file| !Self::field_matches(file.data(), Field::ALL[index], value))
            })
        });
        pending || self.file.as_ref().is_some_and(|file| file.dirty())
    }
    fn pending_mode(&self) -> Mode {
        if self
            .file
            .as_ref()
            .is_some_and(|file| file.deletion_pending())
        {
            Mode::DeletePending
        } else {
            Mode::Pending
        }
    }
    fn pending(&self) -> bool {
        self.file.as_ref().is_some_and(|file| file.pending())
    }
    pub(super) fn can_edit(&self) -> bool {
        self.mode == Mode::Edit && !self.pending()
    }
    pub(super) fn field(&self) -> Option<Field> {
        if self.mode == Mode::Edit {
            Field::ALL.get(self.index).copied()
        } else {
            None
        }
    }
    pub(super) fn text(&self, field: Field) -> Cow<'_, str> {
        if let Some(value) = &self.invalid[field.index()] {
            return value.as_str().into();
        }
        let Some(file) = &self.file else {
            return "".into();
        };
        let data = file.data();
        match field {
            Field::Title => data.title.as_deref().unwrap_or("").into(),
            Field::TranslitTitle => data.title_transliteration.as_deref().unwrap_or("").into(),
            Field::SortTitle => data.title_sort.as_deref().unwrap_or("").into(),
            Field::Series => data.series.as_deref().unwrap_or("").into(),
            Field::Banner => data.banner.as_deref().unwrap_or("").into(),
            Field::Year => data.year.to_string().into(),
        }
    }
    fn field_matches(data: &Data, field: Field, value: &str) -> bool {
        if value.contains(['\r', '\n', '\0']) {
            return false;
        }
        match field {
            Field::Title => data.title.as_deref() == Some(value),
            Field::TranslitTitle => data.title_transliteration.as_deref() == Some(value),
            Field::SortTitle => data.title_sort.as_deref() == Some(value),
            Field::Series => data.series.as_deref() == Some(value),
            Field::Banner => data.banner.as_deref() == Some(value),
            Field::Year => value
                .trim()
                .parse::<i32>()
                .is_ok_and(|year| year >= 0 && year == data.year),
        }
    }
    fn changed_field(
        mut data: Data,
        field: Field,
        value: &str,
    ) -> Result<Data, deadsync_editor::Error> {
        let invalid = || deadsync_editor::Error::InvalidEdit("Invalid pack property value".into());
        if value.contains(['\r', '\n', '\0']) {
            return Err(invalid());
        }
        match field {
            Field::Title => data.title = Some(value.into()),
            Field::TranslitTitle => data.title_transliteration = Some(value.into()),
            Field::SortTitle => data.title_sort = Some(value.into()),
            Field::Series => data.series = Some(value.into()),
            Field::Banner => data.banner = Some(value.into()),
            Field::Year => {
                let invalid_year =
                    || deadsync_editor::Error::InvalidEdit("Invalid pack year".into());
                let year = value.trim().parse::<i32>().map_err(|_| invalid_year())?;
                if year < 0 {
                    return Err(invalid_year());
                }
                data.year = year;
            }
        }
        Ok(data)
    }
    fn begin_field(&mut self, field: Field) -> Result<(), deadsync_editor::Error> {
        if self.text_reviews[field.index()].is_some() {
            return Ok(());
        }
        if !self.can_edit() {
            return Err(deadsync_editor::Error::ExternalChange);
        }
        let file = self
            .file
            .as_ref()
            .ok_or(deadsync_editor::Error::ExternalChange)?;
        let owner = file
            .history_owner()
            .ok_or(deadsync_editor::Error::ExternalChange)?;
        let review = owner.review().map_err(pack_edit_error)?;
        if review.data() != file.data() {
            return Err(pack_edit_error(deadsync_editor::Error::ExternalChange));
        }
        self.text_reviews[field.index()] = Some(review);
        Ok(())
    }
    /// Source cached InputText: staging is local, never a provisional global action.
    pub(super) fn set_text(
        &mut self,
        field: Field,
        value: &str,
    ) -> Result<(), deadsync_editor::Error> {
        self.begin_field(field)?;
        self.revision = self.revision.checked_add(1).ok_or_else(|| {
            deadsync_editor::Error::InvalidEdit("Invalid pack property value".into())
        })?;
        self.invalid[field.index()] = Some(value.into());
        let file = self
            .file
            .as_ref()
            .ok_or(deadsync_editor::Error::ExternalChange)?;
        Self::changed_field(file.data().clone(), field, value)?;
        Ok(())
    }
    /// Deactivation can be followed by a file command whose panel index is
    /// unrelated to the Pack field. Validate every retained field against its
    /// captured baseline before publishing one atomic property action.
    fn commit_pending_fields(
        &mut self,
        workspace: &mut deadsync_editor::workspace::Workspace,
    ) -> Result<bool, deadsync_editor::Error> {
        if !self.invalid.iter().any(Option::is_some) {
            return Ok(false);
        }
        if !self.can_edit() {
            return Err(deadsync_editor::Error::ExternalChange);
        }
        let file = self
            .file
            .as_mut()
            .ok_or(deadsync_editor::Error::ExternalChange)?;
        let mut after = file.data().clone();
        let mut captured: Option<&deadsync_editor::pack::storage::history::Review> = None;
        for field in Field::ALL {
            let Some(value) = self.invalid[field.index()].as_ref() else {
                continue;
            };
            let review = self.text_reviews[field.index()]
                .as_ref()
                .ok_or_else(|| pack_edit_error(deadsync_editor::Error::ExternalChange))?;
            if review.data() != file.data() {
                return Err(pack_edit_error(deadsync_editor::Error::ExternalChange));
            }
            after = Self::changed_field(after, field, value)?;
            if captured.is_some_and(|previous| !previous.same_capture(review)) {
                return Err(pack_edit_error(deadsync_editor::Error::ExternalChange));
            }
            captured.get_or_insert(review);
        }
        let review = captured.ok_or(deadsync_editor::Error::ExternalChange)?;
        let changed = workspace
            .edit_pack_properties_reviewed(review, after)
            .map_err(pack_edit_error)?;
        self.invalid.fill(None);
        self.text_reviews.fill(None);
        file.synchronize();
        Ok(changed)
    }
    fn commit_field(
        &mut self,
        workspace: &mut deadsync_editor::workspace::Workspace,
        field: Field,
    ) -> Result<bool, deadsync_editor::Error> {
        let Some(value) = self.invalid[field.index()].clone() else {
            self.text_reviews[field.index()] = None;
            return Ok(false);
        };
        if !self.can_edit() {
            return Err(deadsync_editor::Error::ExternalChange);
        }
        let file = self
            .file
            .as_mut()
            .ok_or(deadsync_editor::Error::ExternalChange)?;
        // Invalid raw text is retained without publishing a history action.
        let after = Self::changed_field(file.data().clone(), field, &value)?;
        let review = self.text_reviews[field.index()]
            .as_ref()
            .ok_or_else(|| pack_edit_error(deadsync_editor::Error::ExternalChange))?;
        if review.data() != file.data() {
            return Err(pack_edit_error(deadsync_editor::Error::ExternalChange));
        }
        let changed = workspace
            .edit_pack_properties_reviewed(review, after)
            .map_err(pack_edit_error)?;
        self.invalid[field.index()] = None;
        self.text_reviews[field.index()] = None;
        file.synchronize();
        Ok(changed)
    }
    fn count(&self) -> usize {
        match self.mode {
            Mode::Edit => 10,
            Mode::Conflict => 3,
            Mode::Pending => 3,
            _ => 2,
        }
    }
    fn row_key(&self, index: usize) -> &'static str {
        match self.mode {
            Mode::DeleteConfirm => {
                if index == 0 {
                    "PackKeepEditing"
                } else {
                    "PackDeleteConfirm"
                }
            }
            Mode::DeletePending => {
                if index == 0 {
                    "PackRetry"
                } else {
                    "PackClosePending"
                }
            }
            Mode::Edit => match index {
                0..=5 => Field::ALL[index].key(),
                6 => "PackSync",
                7 => "PackSave",
                9 => "PackDelete",
                _ => "PackCancel",
            },
            Mode::Missing => {
                if index == 0 {
                    "PackCreate"
                } else {
                    "PackClose"
                }
            }
            Mode::ReadFailed => {
                if index == 0 {
                    "PackReadAgain"
                } else {
                    "PackClose"
                }
            }
            Mode::Conflict => match index {
                0 => "PackKeepEditing",
                1 => "PackReload",
                _ => "PackClose",
            },
            Mode::ReloadConfirm => {
                if index == 0 {
                    "PackReloadConfirm"
                } else {
                    "PackKeepEditing"
                }
            }
            Mode::SourceChanged => {
                if index == 0 {
                    if self.pending() {
                        "PackReleaseOpen"
                    } else {
                        "PackCancel"
                    }
                } else {
                    "PackClose"
                }
            }
            Mode::ReleaseConfirm => {
                if index == 0 {
                    "PackKeepPending"
                } else {
                    "PackReleaseConfirm"
                }
            }
            Mode::Pending => match index {
                0 => "PackRetry",
                1 => "PackClosePending",
                _ => "PackReleaseOpen",
            },
        }
    }
    fn notice_key(&self) -> &'static str {
        match self.mode {
            Mode::DeleteConfirm => "PackDeleteWarning",
            Mode::DeletePending => "PackDeletePending",
            Mode::Missing => "PackMissing",
            Mode::ReadFailed => "PackReadFailed",
            Mode::Conflict => "PackConflict",
            Mode::ReloadConfirm => "PackReloadWarning",
            Mode::Pending => "PackPending",
            Mode::SourceChanged => "PackSourceChanged",
            Mode::ReleaseConfirm => "PackReleaseWarning",
            Mode::Edit => {
                if self.is_dirty() {
                    "PackDirty"
                } else {
                    "PackClean"
                }
            }
        }
    }
}
impl State {
    pub(super) fn pack_system_alert_modal(&self) -> Option<super::system_alert::Modal> {
        match self.pack_properties.as_ref()?.mode {
            Mode::Conflict => Some(super::system_alert::Modal::ExternalPack),
            Mode::DeleteConfirm => Some(super::system_alert::Modal::DeletePack),
            _ => None,
        }
    }
    // Source chooses a recognized active-song offset first, then the CURRENT
    // global preference; Data::new owns the common reference-equality rules.
    fn pack_delete_result(
        settings: &mut Settings,
        result: Result<DeleteOutcome, Error>,
        status: &mut Arc<str>,
    ) {
        match result {
            Ok(_) => {
                settings.file = None;
                settings.mode = Mode::Missing;
                settings.index = 0;
                settings.invalid = std::array::from_fn(|_| None);
                *status = i18n::tr("Edition", "PackDeleted");
            }
            Err(_) => {
                settings.mode = if settings
                    .file
                    .as_ref()
                    .is_some_and(|file| file.deletion_pending())
                {
                    Mode::DeletePending
                } else {
                    Mode::Edit
                };
                settings.index = if settings.mode == Mode::DeletePending {
                    0
                } else {
                    9
                };
                *status = i18n::tr(
                    "Edition",
                    if settings.mode == Mode::DeletePending {
                        "PackDeletePending"
                    } else {
                        "PackDeleteFailed"
                    },
                );
            }
        }
    }
    pub(super) fn pack_banner_context(&self) -> Option<(PathBuf, Option<String>)> {
        if !matches!(self.panel, Some(Panel::PackProperties)) {
            return None;
        }
        let settings = self.pack_properties.as_ref()?;
        if Some(settings.source.as_path()) != self.workspace.file().existing_path()
            || !settings.binding.belongs_to(&self.workspace.file().session)
        {
            return None;
        }
        // In-progress Banner text has priority, including an explicit clear.
        let explicit = settings.invalid[Field::Banner.index()].clone().or_else(|| {
            settings
                .file
                .as_ref()
                .and_then(|file| file.data().banner.clone())
        });
        Some((settings.root.clone(), explicit))
    }
    #[cfg(test)]
    pub(super) fn pack_banner_set_explicit_for_test(&mut self, value: &str) {
        self.pack_properties
            .as_mut()
            .unwrap()
            .set_text(Field::Banner, value)
            .unwrap();
        self.pack_properties
            .as_mut()
            .unwrap()
            .commit_field(&mut self.workspace, Field::Banner)
            .unwrap();
    }
    fn pack_creation_defaults(&self, root: &std::path::Path) -> Data {
        let title = root.file_name().unwrap_or_default().to_string_lossy();
        let active =
            deadsync_editor::song_properties::sync_offset(self.workspace.file().session.document())
                .ok();
        Data::new(
            &title,
            self.pack_banner.inferred_name(root).unwrap_or(""),
            chrono::Utc::now().year(),
            active,
            self.general_options.new_sync,
        )
    }

    pub(super) fn command_save_pack_file(&mut self) {
        if self.file_io_active() || self.pack_file_io_active() || self.save_as_pending() {
            self.status = i18n::tr("Edition", "FileBusy");
            return;
        }
        self.open_pack_properties();
        if self
            .pack_properties
            .as_ref()
            .is_some_and(|p| p.mode == Mode::Edit)
        {
            self.save_pack_properties();
        }
    }
    pub(super) fn capture_pack_coupling(&self) -> Option<CouplingGuard> {
        self.pack_properties.as_ref().map(|p| CouplingGuard {
            identity: Arc::clone(&p.identity),
            revision: p.revision,
            root: p.root.clone(),
        })
    }
    pub(super) fn pack_coupling_current(&self, guard: &Option<CouplingGuard>) -> bool {
        match (guard, self.pack_properties.as_ref()) {
            (None, None) => true,
            (Some(g), Some(p)) => {
                Arc::ptr_eq(&g.identity, &p.identity)
                    && g.revision == p.revision
                    && g.root == p.root
            }
            _ => false,
        }
    }
    pub(super) fn pack_source_current(&self) -> bool {
        self.pack_properties.as_ref().is_none_or(|p| {
            Some(p.source.as_path()) == self.workspace.file().existing_path()
                && p.binding.belongs_to(&self.workspace.file().session)
        })
    }
    pub(super) fn target_is_same_pack(&self, target: &std::path::Path) -> bool {
        let canonical = target.canonicalize().ok();
        let target = canonical.as_deref().unwrap_or(target);
        self.pack_properties
            .as_ref()
            .is_some_and(|p| target.parent().and_then(|p| p.parent()) == Some(p.root.as_path()))
    }
    pub(super) fn retain_pack_for(&mut self, next: &mut State) {
        if next
            .workspace
            .file()
            .existing_path()
            .is_some_and(|p| self.target_is_same_pack(p))
        {
            next.pack_properties = self.pack_properties.take();
            if let Some(p) = next.pack_properties.as_mut() {
                p.source = next.workspace.file().existing_path().unwrap().to_owned();
                p.binding = SessionBinding::capture(&next.workspace.file().session);
            }
        }
    }
    /// A Save command authorizes saving its active pack, as in the source.
    /// The song is published only after this exact pack is durably saved.
    pub(super) fn save_coupled_pack(&mut self) -> bool {
        if self.pack_has_pending_durability() {
            self.open_pack_properties();
            self.status = i18n::tr("Edition", "PackCoupledPending");
            return false;
        }
        if !self.pack_has_unsaved_changes() {
            return true;
        }
        let valid = self.pack_properties.as_ref().is_some_and(|p| {
            p.source.as_path()
                == self
                    .workspace
                    .file()
                    .existing_path()
                    .unwrap_or(std::path::Path::new(""))
                && p.binding.belongs_to(&self.workspace.file().session)
        });
        if !valid {
            self.open_pack_properties();
            self.status = i18n::tr("Edition", "PackSourceChanged");
            return false;
        }
        self.save_pack_properties();
        if self.pack_has_unsaved_changes() || self.pack_has_pending_durability() {
            self.panel = Some(Panel::PackProperties);
            return false;
        }
        true
    }
    pub(super) fn pack_has_unsaved_changes(&self) -> bool {
        self.pack_properties
            .as_ref()
            .is_some_and(Settings::is_dirty)
    }
    pub(super) fn pack_has_pending_durability(&self) -> bool {
        self.pack_properties.as_ref().is_some_and(Settings::pending)
    }
    pub(super) fn begin_pack_field(&mut self) {
        let Some(settings) = self.pack_properties.as_mut() else {
            return;
        };
        let Some(field) = settings.field() else {
            return;
        };
        if let Err(error) = settings.begin_field(field) {
            self.status = pack_history_error_status(&error);
        }
    }
    /// File intentions bind monotonically to history. Finish the source cached
    /// field before capturing that intention, never rebind an existing guard
    /// over an ordinary application-history mutation. No disk Save here.
    pub(super) fn deactivate_pack_before_file_intent(&mut self) -> bool {
        if matches!(self.panel, Some(Panel::PackProperties)) && !self.sync_text() {
            return false;
        }
        if let Some(settings) = self.pack_properties.as_mut() {
            if let Err(error) = settings.commit_pending_fields(&mut self.workspace) {
                self.status = pack_history_error_status(&error);
                return false;
            }
        }
        true
    }
    pub(super) fn commit_pack_field(&mut self) -> bool {
        let Some(settings) = self.pack_properties.as_mut() else {
            return true;
        };
        let Some(field) = settings.field() else {
            return true;
        };
        match settings.commit_field(&mut self.workspace, field) {
            Ok(_) => true,
            Err(error) => {
                self.status = match &error {
                    deadsync_editor::Error::InvalidEdit(message)
                        if message == "Invalid pack property value" =>
                    {
                        i18n::tr(
                            "Edition",
                            if field == Field::Year {
                                "PackInvalidYear"
                            } else {
                                "PackInvalidText"
                            },
                        )
                    }
                    _ => pack_history_error_status(&error),
                };
                false
            }
        }
    }
    pub(super) fn pack_media_directory(&self) -> Option<PathBuf> {
        self.pack_text_current()
            .then(|| self.pack_properties.as_ref().unwrap().root.clone())
    }
    pub(super) fn pack_text_current(&self) -> bool {
        self.pack_properties.as_ref().is_some_and(|settings| {
            settings.can_edit()
                && Some(settings.source.as_path()) == self.workspace.file().existing_path()
                && settings.binding.belongs_to(&self.workspace.file().session)
        })
    }
    pub(super) fn open_pack_properties(&mut self) {
        if self.is_generating() {
            return;
        }
        let Some(source) = self
            .workspace
            .file()
            .existing_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.status = i18n::tr("Edition", "FileRequiresSaveAs");
            return;
        };
        if let Some(settings) = &self.pack_properties {
            if settings.source == source
                && settings.binding.belongs_to(&self.workspace.file().session)
            {
                self.panel = Some(Panel::PackProperties);
                self.focus_text();
                self.status = i18n::tr("Edition", "PackPropertiesTitle");
                return;
            }
            if settings.is_dirty() || settings.pending() {
                let settings = self.pack_properties.as_mut().unwrap();
                settings.mode = Mode::SourceChanged;
                settings.index = settings.count() - 1;
                self.panel = Some(Panel::PackProperties);
                self.focus_text();
                self.status = i18n::tr("Edition", "PackSourceChanged");
                return;
            }
        }
        let Some(root) = source
            .parent()
            .and_then(|song| song.parent())
            .map(|p| p.to_path_buf())
        else {
            self.status = i18n::tr("Edition", "PackReadFailed");
            return;
        };
        let defaults = self.pack_creation_defaults(&root);
        let (file, mode) = match FileSession::open(&root) {
            Ok(Some(file)) => (Some(boxed(file)), Mode::Edit),
            Ok(None) => (None, Mode::Missing),
            Err(_) => (None, Mode::ReadFailed),
        };
        self.pack_properties = Some(Settings {
            identity: Arc::new(()),
            revision: 0,
            text_reviews: std::array::from_fn(|_| None),
            delete_review: None,
            binding: SessionBinding::capture(&self.workspace.file().session),
            source,
            root,
            defaults,
            file,
            mode,
            index: 0,
            invalid: std::array::from_fn(|_| None),
        });
        self.panel = Some(Panel::PackProperties);
        self.focus_text();
        self.status = i18n::tr("Edition", "PackPropertiesTitle");
    }
    pub(super) fn pack_properties_key(&mut self, code: KeyCode) {
        if code == KeyCode::F5 {
            self.pack_banner.reset();
        }
        let Some(settings) = &self.pack_properties else {
            return;
        };
        if code == KeyCode::Escape && settings.mode == Mode::DeleteConfirm {
            let settings = self.pack_properties.as_mut().unwrap();
            settings.delete_review = None;
            settings.mode = Mode::Edit;
            settings.index = 9;
            self.focus_text();
            return;
        }
        if code == KeyCode::Escape && settings.mode == Mode::ReleaseConfirm {
            let settings = self.pack_properties.as_mut().unwrap();
            settings.mode = settings.pending_mode();
            settings.index = if settings.mode == Mode::DeletePending {
                1
            } else {
                2
            };
            self.status = i18n::tr("Edition", "PackPending");
            self.focus_text();
            return;
        }
        if code == KeyCode::Escape {
            let accepted = self.sync_text() && self.commit_pack_field();
            let refusal = (!accepted).then(|| self.status.clone());
            self.close_pack_panel();
            if let Some(refusal) = refusal {
                self.status = refusal;
            }
            return;
        }
        if Some(settings.source.as_path()) != self.workspace.file().existing_path()
            || !settings.binding.belongs_to(&self.workspace.file().session)
        {
            let settings = self.pack_properties.as_mut().unwrap();
            if settings.mode != Mode::ReleaseConfirm {
                settings.mode = Mode::SourceChanged;
                settings.index = settings.index.min(settings.count() - 1);
            }
            self.status = i18n::tr("Edition", "PackSourceChanged");
            if (self.control && code == KeyCode::KeyS) || code == KeyCode::F5 {
                return;
            }
        }
        if self.text_target().is_some()
            && self.control
            && matches!(code, KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV)
        {
            self.queue_text_clipboard(code);
            return;
        }
        if self
            .text_target()
            .is_some_and(|target| target == super::text_fields::Target::Pack(Field::Banner))
            && matches!(code, KeyCode::F2 | KeyCode::F3)
        {
            if !self.sync_text() {
                return;
            }
            if !self.commit_pack_field() {
                return;
            }
            if code == KeyCode::F2 {
                self.queue_music_picker();
            } else {
                self.clear_music();
                self.commit_pack_field();
            }
            return;
        }
        let action = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if action
            && self
                .pack_properties
                .as_ref()
                .and_then(Settings::field)
                .is_some()
        {
            if self.sync_text() && self.commit_pack_field() {
                self.focus_text();
            }
            return;
        }
        if self.control && code == KeyCode::KeyS {
            self.sync_text();
            self.save_pack_properties();
            return;
        }
        if code == KeyCode::F5 {
            self.check_pack_external();
            return;
        }
        if self.text_target().is_none() && matches!(code, KeyCode::Home | KeyCode::End) {
            let settings = self.pack_properties.as_mut().unwrap();
            settings.index = if code == KeyCode::Home {
                0
            } else {
                settings.count() - 1
            };
            self.focus_text();
            return;
        }
        if matches!(
            code,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Tab
                | KeyCode::PageUp
                | KeyCode::PageDown
        ) {
            if !self.sync_text() || !self.commit_pack_field() {
                return;
            }
            let settings = self.pack_properties.as_mut().unwrap();
            let count = settings.count();
            settings.index = match code {
                KeyCode::PageUp => 0,
                KeyCode::PageDown => count - 1,
                KeyCode::ArrowUp => (settings.index + count - 1) % count,
                KeyCode::Tab if self.shift => (settings.index + count - 1) % count,
                _ => (settings.index + 1) % count,
            };
            self.focus_text();
            return;
        }
        let Some(mut settings) = self.pack_properties.take() else {
            return;
        };
        let save = action && settings.mode == Mode::Edit && settings.index == 7;
        let mut close = false;
        let mut discard = false;
        let mut released_status = None;
        let mut durability_completed = false;
        if settings.mode == Mode::Edit
            && settings.index == 6
            && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight)
        {
            if let Some(file) = &mut settings.file {
                let current = file.data().sync_offset;
                let next = match current {
                    0 => 1,
                    1 => 0,
                    _ => {
                        if code == KeyCode::ArrowLeft {
                            0
                        } else {
                            1
                        }
                    }
                };
                let mut data = file.data().clone();
                data.sync_offset = next;
                match settings.revision.checked_add(1) {
                    Some(next) => {
                        let result = file
                            .history_owner()
                            .ok_or(deadsync_editor::Error::ExternalChange)
                            .and_then(|owner| owner.review())
                            .and_then(|review| {
                                if review.data() != file.data() {
                                    return Err(deadsync_editor::Error::ExternalChange);
                                }
                                self.workspace.edit_pack_properties_reviewed(&review, data)
                            });
                        match result {
                            Ok(true) => {
                                settings.revision = next;
                                file.synchronize();
                            }
                            Ok(false) => {}
                            Err(error) => self.status = pack_history_error_status(&error),
                        }
                    }
                    None => self.status = i18n::tr("Edition", "PackFailed"),
                }
            }
        } else if action {
            match settings.mode {
                Mode::SourceChanged => {
                    if settings.index == 0 && settings.pending() {
                        settings.mode = Mode::ReleaseConfirm;
                        settings.index = 0;
                    } else {
                        discard = settings.index == 0;
                        close = true;
                    }
                }
                Mode::ReleaseConfirm => {
                    if settings.index == 0 {
                        settings.mode = settings.pending_mode();
                        settings.index = if settings.mode == Mode::DeletePending {
                            1
                        } else {
                            2
                        };
                    } else {
                        // Releasing this local receipt does not acknowledge it
                        // and never writes, rolls back or removes the disk file.
                        released_status =
                            Some(match settings.file.as_ref().map(|file| file.external()) {
                                Some(Ok(true)) => "PackReleasedExternal",
                                Some(Ok(false)) => "PackReleasedPublished",
                                _ => "PackReleasedUnknown",
                            });
                        discard = true;
                        close = true;
                    }
                }
                Mode::DeleteConfirm => {
                    if settings.index == 0 {
                        settings.delete_review = None;
                        settings.mode = Mode::Edit;
                        settings.index = 9;
                    } else {
                        let review = settings.delete_review.take();
                        // Revoke older coupled intents before touching namespace.
                        settings.identity = Arc::new(());
                        let result = match review {
                            Some((revision, review)) if revision == settings.revision => settings
                                .file
                                .as_mut()
                                .ok_or(Error::StaleSnapshot)
                                .and_then(|file| file.delete(review)),
                            _ => Err(Error::StaleSnapshot),
                        };
                        let removed = matches!(&result, Ok(DeleteOutcome::Deleted(_)))
                            || settings
                                .file
                                .as_ref()
                                .is_some_and(|file| file.deletion_pending());
                        if removed {
                            self.pack_observer_own_delete();
                        }
                        Self::pack_delete_result(&mut settings, result, &mut self.status);
                    }
                }
                Mode::DeletePending => {
                    if settings.index == 0 {
                        let result = settings
                            .file
                            .as_mut()
                            .ok_or(Error::StaleSnapshot)
                            .and_then(|file| file.retry_delete());
                        let removed = matches!(&result, Ok(DeleteOutcome::Deleted(_)))
                            || settings
                                .file
                                .as_ref()
                                .is_some_and(|file| file.deletion_pending());
                        if removed {
                            self.pack_observer_own_delete();
                        }
                        Self::pack_delete_result(&mut settings, result, &mut self.status);
                    } else {
                        close = true;
                    }
                }
                Mode::Edit => match settings.index {
                    7 => {}
                    9 => {
                        match settings
                            .file
                            .as_ref()
                            .ok_or(Error::StaleSnapshot)
                            .and_then(|file| file.prepare_delete())
                        {
                            Ok(review) => {
                                settings.delete_review = Some((settings.revision, review));
                                settings.mode = Mode::DeleteConfirm;
                                settings.index = 0;
                                self.status = i18n::tr("Edition", "PackDeleteWarning");
                            }
                            Err(_) => self.status = i18n::tr("Edition", "PackDeleteFailed"),
                        }
                    }
                    8 => {
                        discard = true;
                        close = true
                    }
                    _ => {}
                },
                Mode::Missing => {
                    if settings.index == 0 {
                        // A panel may survive changes to global options. Re-evaluate
                        // at Create, rather than using its original display snapshot.
                        if !self.pack_banner_ready(&settings.root)
                            && !self.pack_banner.failed(&settings.root)
                        {
                            self.status = i18n::tr("Edition", "PackBannerWait");
                            self.pack_properties = Some(settings);
                            return;
                        }
                        settings.defaults = self.pack_creation_defaults(&settings.root);
                        match FileSession::create(&settings.root, settings.defaults.clone()) {
                            Ok(file) => {
                                settings.file = Some(boxed(file));
                                settings.mode = Mode::Edit;
                                settings.index = 0;
                                self.status = i18n::tr("Edition", "PackCreationStaged");
                            }
                            Err(Error::ExternalChange) => {
                                settings.mode = Mode::Conflict;
                                settings.index = 1;
                                self.status = i18n::tr("Edition", "PackConflict");
                            }
                            Err(_) => self.status = i18n::tr("Edition", "PackFailed"),
                        }
                    } else {
                        close = true
                    }
                }
                Mode::ReadFailed => {
                    if settings.index == 0 {
                        Self::reload_pack(&mut settings, &mut self.status)
                    } else {
                        close = true
                    }
                }
                Mode::Conflict => match settings.index {
                    0 => {
                        settings.mode = if settings.file.is_some() {
                            Mode::Edit
                        } else {
                            Mode::Missing
                        };
                        settings.index = 0;
                    }
                    1 => {
                        settings.mode = Mode::ReloadConfirm;
                        settings.index = 1;
                    }
                    _ => close = true,
                },
                Mode::ReloadConfirm => {
                    if settings.index == 0 {
                        Self::reload_pack(&mut settings, &mut self.status)
                    } else {
                        settings.mode = Mode::Conflict;
                        settings.index = 0;
                    }
                }
                Mode::Pending => {
                    if settings.index == 0 {
                        if let Some(file) = &mut settings.file {
                            match file.retry() {
                                Ok(guarantee) => {
                                    settings.mode = Mode::Edit;
                                    settings.index = 7;
                                    durability_completed = true;
                                    self.status = save_status(guarantee)
                                }
                                Err(Error::ExternalChange) => {
                                    self.status = i18n::tr("Edition", "PackPendingConflict")
                                }
                                Err(_) => self.status = i18n::tr("Edition", "PackPending"),
                            }
                        }
                    } else if settings.index == 1 {
                        close = true
                    } else {
                        settings.mode = Mode::ReleaseConfirm;
                        settings.index = 0;
                    }
                }
            }
        }
        self.pack_properties = Some(settings);
        if durability_completed {
            self.file_pack_durability_completed();
            if !matches!(self.panel, Some(Panel::PackProperties)) {
                return;
            }
        }
        if discard {
            if let Some(file) = self
                .pack_properties
                .as_ref()
                .and_then(|settings| settings.file.as_ref())
            {
                file.retire();
            }
            self.pack_properties = None;
        }
        if close {
            self.close_pack_panel();
            if let Some(key) = released_status {
                self.status = i18n::tr("Edition", key);
            }
            return;
        }
        if save {
            self.sync_text();
            self.save_pack_properties();
            return;
        }
        if action {
            self.focus_text();
        } else if self.text_target().is_some() {
            self.pack_buffer_key(code);
        } else {
            self.focus_text();
        }
    }
    fn close_pack_panel(&mut self) {
        self.panel = None;
        self.text_buffer = None;
        self.label_preedit.clear();
        self.status = i18n::tr(
            "Edition",
            if self.pack_has_pending_durability() {
                "PackPending"
            } else if self.pack_has_unsaved_changes() {
                "PackEditsKept"
            } else {
                "Ready"
            },
        );
    }
    fn reload_pack(settings: &mut Settings, status: &mut Arc<str>) {
        match FileSession::open(&settings.root) {
            Ok(file) => {
                settings.identity = Arc::new(());
                settings.revision = 0;
                settings.text_reviews = std::array::from_fn(|_| None);
                if let Some(previous) = &settings.file {
                    previous.retire();
                }
                settings.file = file.map(boxed);
                settings.mode = if settings.file.is_some() {
                    Mode::Edit
                } else {
                    Mode::Missing
                };
                settings.index = 0;
                settings.invalid = std::array::from_fn(|_| None);
                *status = i18n::tr("Edition", "PackPropertiesTitle");
            }
            Err(_) => *status = i18n::tr("Edition", "PackReadFailed"),
        }
    }
    fn check_pack_external(&mut self) {
        let accepted = self.sync_text() && self.commit_pack_field();
        let refusal = (!accepted).then(|| self.status.clone());
        let Some(settings) = self.pack_properties.as_mut() else {
            return;
        };
        if settings.pending() {
            self.status = i18n::tr("Edition", "PackPending");
            return;
        }
        if let Some(file) = &settings.file {
            match file.external() {
                Ok(true) => {
                    settings.mode = Mode::Conflict;
                    settings.index = 0;
                    self.status = i18n::tr("Edition", "PackConflict")
                }
                Ok(false) => {
                    self.status = refusal.unwrap_or_else(|| i18n::tr("Edition", "PackUnchanged"))
                }
                Err(_) => self.status = i18n::tr("Edition", "PackReadFailed"),
            }
        } else {
            Self::reload_pack(settings, &mut self.status);
        }
        self.focus_text();
    }
    fn save_pack_properties(&mut self) {
        // Save/Close/Open may have replaced the Pack panel; its old row is
        // presentation state, never the authority for the pending field.
        if let Some(settings) = self.pack_properties.as_mut() {
            if let Err(error) = settings.commit_pending_fields(&mut self.workspace) {
                // A stale field review must expose the reload decision rather
                // than leave navigation trapped in an uncommittable text field.
                if pack_history_conflict(&error) {
                    settings.mode = Mode::Conflict;
                    settings.index = 0;
                    self.focus_text();
                }
                self.status = pack_history_error_status(&error);
                return;
            }
        }
        let Some(settings) = self.pack_properties.as_mut() else {
            return;
        };
        if settings.mode != Mode::Edit {
            return;
        }
        if let Some(index) = settings.invalid.iter().position(Option::is_some) {
            settings.index = index;
            self.status = i18n::tr(
                "Edition",
                if index == 4 {
                    "PackInvalidYear"
                } else {
                    "PackInvalidText"
                },
            );
            self.focus_text();
            return;
        }
        let Some(file) = &mut settings.file else {
            return;
        };
        match file.save() {
            Ok(guarantee) => self.status = save_status(guarantee),
            Err(Error::ExternalChange) => {
                settings.mode = Mode::Conflict;
                settings.index = 0;
                self.status = i18n::tr("Edition", "PackConflict")
            }
            Err(Error::DurabilityPending(_)) => {
                settings.mode = Mode::Pending;
                settings.index = 0;
                self.status = i18n::tr("Edition", "PackPending")
            }
            Err(_) => self.status = i18n::tr("Edition", "PackFailed"),
        }
        self.focus_text();
    }
    fn pack_buffer_key(&mut self, code: KeyCode) {
        let Some(buffer) = self.text_buffer.as_mut() else {
            return;
        };
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        match code {
            KeyCode::ArrowLeft => buffer.move_cursor(
                if self.control {
                    Movement::WordLeft
                } else {
                    Movement::Left
                },
                self.shift,
            ),
            KeyCode::ArrowRight => buffer.move_cursor(
                if self.control {
                    Movement::WordRight
                } else {
                    Movement::Right
                },
                self.shift,
            ),
            KeyCode::Home => buffer.move_cursor(Movement::Start, self.shift),
            KeyCode::End => buffer.move_cursor(Movement::End, self.shift),
            KeyCode::KeyA if self.control => buffer.select_all(),
            KeyCode::Backspace => {
                buffer.backspace(self.control);
            }
            KeyCode::Delete => {
                buffer.delete(self.control);
            }
            KeyCode::KeyZ if self.control => {
                if self.shift {
                    buffer.redo();
                } else {
                    buffer.undo();
                }
            }
            KeyCode::KeyY if self.control => {
                buffer.redo();
            }
            _ => return,
        }
        self.sync_text();
    }
    pub(super) fn pack_properties_actors(&self, actors: &mut Vec<Actor>) {
        let Some(settings) = &self.pack_properties else {
            return;
        };
        let width = (screen_width() - 40.0).min(780.0);
        let left = (screen_width() - width) * 0.5;
        let height = (screen_height() - 30.0).min(500.0);
        let top = (screen_height() - height) * 0.5;
        // Keep every editable row above the status and help on the native
        // minimum 480px canvas; larger DPI pages retain their shared paging.
        let row_pitch =
            ((height - 272.0) / settings.count().saturating_sub(1).max(1) as f32).clamp(18.0, 25.0);
        actors.push(act!(quad: xy(left+width*0.5,top+height*0.5): zoomto(width,height): diffuse(0.08,0.1,0.15,1.0): z(8020)));
        let label = |actors: &mut Vec<Actor>, text: String, y: f32, color: [f32; 4]| {
            actors.push(act!(text: font("miso"): settext(text): align(0.0,0.0): xy(left+16.0,y): zoom(0.58): maxwidth(width-32.0): diffuse(color[0],color[1],color[2],color[3]): z(8021)));
        };
        label(
            actors,
            i18n::tr("Edition", "PackPropertiesTitle").to_string(),
            top + 12.0,
            [1.0; 4],
        );
        label(
            actors,
            settings.root.display().to_string(),
            top + 37.0,
            [0.7, 0.8, 0.9, 1.0],
        );
        label(
            actors,
            i18n::tr("Edition", settings.notice_key()).to_string(),
            top + 62.0,
            [1.0, 0.8, 0.3, 1.0],
        );
        self.pack_banner_actors(
            actors,
            left + width * 0.5,
            top + 118.,
            (width - 32.).min(300.),
        );
        for index in 0..settings.count() {
            let ui_row_start = actors.len();
            let text = if settings.mode == Mode::Missing
                && index == 0
                && self.pack_banner.failed(&settings.root)
            {
                i18n::tr("Edition", "PackCreateWithoutBanner").to_string()
            } else if settings.mode == Mode::Edit && index < 6 {
                self.text_field_label(index).to_string()
            } else if settings.mode == Mode::Edit && index == 6 {
                let sync = settings
                    .file
                    .as_ref()
                    .map(|file| match file.data().sync_offset {
                        0 => "NULL".into(),
                        1 => "ITG".into(),
                        v => v.to_string(),
                    })
                    .unwrap_or_default();
                i18n::tr_fmt("Edition", "PackSync", &[("value", &sync)]).to_string()
            } else {
                i18n::tr("Edition", settings.row_key(index)).to_string()
            };
            label(
                actors,
                format!("{} {text}", if index == settings.index { ">" } else { " " }),
                top + 166.0 + index as f32 * row_pitch,
                if index == settings.index {
                    [1.0, 0.85, 0.35, 1.0]
                } else {
                    [0.85, 0.9, 1.0, 1.0]
                },
            );

            self.ui_row(actors, ui_row_start, index == settings.index, row_pitch);
        }
        actors.push(act!(text: font("miso"): settext(self.status.clone()): align(0.0,0.0): xy(left+16.0,top+height-88.0): zoom(0.50): wrapwidthpixels((width-32.0)/0.50): maxwidth(width-32.0): maxheight(42.0): diffuse(0.9,0.9,0.9,1.0): z(8021)));
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition", "PackHelp")): align(0.0,0.0): xy(left+16.0,top+height-38.0): zoom(0.45): wrapwidthpixels((width-32.0)/0.45): maxwidth(width-32.0): maxheight(30.0): diffuse(0.7,0.8,0.9,1.0): z(8021)));
    }
}
fn save_status(guarantee: Durability) -> Arc<str> {
    i18n::tr(
        "Edition",
        match guarantee {
            Durability::FileAndDirectory => "PackSaved",
            Durability::FileOnly => "PackSavedFileOnly",
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io, sync::Mutex};
    #[test]
    fn pack_help_is_unique_and_edit_rows_fit_small_and_large_panels_en_fr() {
        for language in ["en", "fr"] {
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let directory = root.path().join("Song");
            fs::create_dir(&directory).unwrap();
            let path = directory.join("song.ssc");
            fs::write(
                &path,
                "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            )
            .unwrap();
            fs::write(
                root.path().join("Pack.ini"),
                Data::new("Fixture", "", 2026, None, 0.009)
                    .canonical()
                    .unwrap(),
            )
            .unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.open_pack_properties();
            assert_eq!(state.status, i18n::tr("Edition", "PackPropertiesTitle"));
            assert_eq!(state.pack_properties.as_ref().unwrap().count(), 10);
            for height in [480.0, 720.0] {
                deadlib_present::space::with_metrics(
                    deadlib_present::space::Metrics::centered(854.0, height),
                    || {
                        let mut actors = Vec::new();
                        state.pack_properties_actors(&mut actors);
                        assert_eq!(actors.iter().filter(|actor| matches!(actor,
                            Actor::Text { content, .. } if content.as_str() == i18n::tr("Edition", "PackHelp").as_ref()
                        )).count(), 1);
                        let panel_height = (height - 30.0).min(500.0);
                        let top = (height - panel_height) * 0.5;
                        let status_y = top + panel_height - 88.0;
                        let last_row = i18n::tr("Edition", "PackDelete");
                        let last_row_y = actors
                            .iter()
                            .find_map(|actor| match actor {
                                Actor::Text {
                                    content, offset, ..
                                } if content.as_str().trim_start() == last_row.as_ref() => {
                                    Some(offset[1])
                                }
                                _ => None,
                            })
                            .unwrap();
                        assert!(last_row_y + 18.0 <= status_y);
                        assert!(top >= 0.0 && top + panel_height <= height);
                    },
                );
            }
            state.open_pack_properties();
            assert_eq!(state.status, i18n::tr("Edition", "PackPropertiesTitle"));
        }
    }
    #[test]
    fn create_pack_uses_current_general_sync_preference_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let song = root.path().join("Song");
            fs::create_dir(&song).unwrap();
            let path = song.join("song.ssc");
            fs::write(&path,r#"#VERSION:0.83;#TITLE:Fixture;#GASongDataVersion:1;#GASongData:{"SyncOffset":0.002};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;"#).unwrap();
            let mut state = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
            state.general_options.new_sync = 0.009;
            state.open_pack_properties();
            assert_eq!(
                state.pack_properties.as_ref().unwrap().defaults.sync_offset,
                1
            );
            // Change the preference after the panel snapshot: Create must use
            // current General, while custom song sync falls through to it.
            state.general_options.new_sync = 0.0;
            state.pack_banner = super::super::pack_banner::Runtime::settled(
                root.path().canonicalize().unwrap(),
                None,
            );
            state.pack_properties_key(KeyCode::Enter);
            assert_eq!(
                state
                    .pack_properties
                    .as_ref()
                    .unwrap()
                    .file
                    .as_ref()
                    .unwrap()
                    .data()
                    .sync_offset,
                0
            );
            assert!(!root.path().join("Pack.ini").exists());
        }
    }
    #[test]
    fn pack_delete_cancel_confirm_and_external_refusal_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let song = root.path().join("Song");
            fs::create_dir(&song).unwrap();
            let path = song.join("song.ssc");
            fs::write(&path,"#VERSION:0.83;#TITLE:Fixture;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
            let pack = root.path().join("Pack.ini");
            let bytes = Data::new("Original", "", 2026, None, 0.009)
                .canonical()
                .unwrap();
            fs::write(&pack, &bytes).unwrap();
            let mut state = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
            state.open_pack_properties();
            state.pack_properties.as_mut().unwrap().index = 9;
            state.pack_properties_key(KeyCode::Enter);
            assert!(state.pack_properties.as_ref().unwrap().mode == Mode::DeleteConfirm);
            assert_eq!(state.pack_properties.as_ref().unwrap().index, 0);
            assert_eq!(fs::read(&pack).unwrap(), bytes);
            state.pack_properties_key(KeyCode::Escape);
            assert!(state.pack_properties.as_ref().unwrap().mode == Mode::Edit);
            assert!(pack.exists());
            state.pack_properties_key(KeyCode::Enter);
            state.pack_properties.as_mut().unwrap().index = 1;
            fs::write(&pack, b"external replacement").unwrap();
            state.pack_properties_key(KeyCode::Enter);
            assert_eq!(fs::read(&pack).unwrap(), b"external replacement");
            assert!(state.pack_properties.as_ref().unwrap().file.is_some());
            fs::write(&pack, &bytes).unwrap();
            state.pack_properties.as_mut().unwrap().index = 9;
            state.pack_properties_key(KeyCode::Enter);
            state.pack_properties.as_mut().unwrap().index = 1;
            state.pack_properties_key(KeyCode::Enter);
            assert!(!pack.exists());
            assert!(state.pack_properties.as_ref().unwrap().mode == Mode::Missing);
            assert_eq!(state.status, i18n::tr("Edition", "PackDeleted"));
        }
    }
    #[derive(Default)]
    struct Calls {
        saves: usize,
        retries: usize,
        pending: bool,
        external: bool,
        dirty: bool,
    }
    struct Published {
        data: Data,
        calls: Arc<Mutex<Calls>>,
    }
    impl PackFile for Published {
        fn data(&self) -> &Data {
            &self.data
        }
        fn dirty(&self) -> bool {
            self.calls.lock().unwrap().dirty
        }
        fn pending(&self) -> bool {
            self.calls.lock().unwrap().pending
        }
        fn save(&mut self) -> Result<Durability, Error> {
            let mut calls = self.calls.lock().unwrap();
            calls.saves += 1;
            calls.pending = true;
            Err(Error::DurabilityPending(io::Error::other(
                "injected native boundary",
            )))
        }
        fn retry(&mut self) -> Result<Durability, Error> {
            let mut calls = self.calls.lock().unwrap();
            calls.retries += 1;
            if calls.external {
                return Err(Error::ExternalChange);
            }
            calls.pending = false;
            calls.dirty = false;
            Ok(Durability::FileAndDirectory)
        }
        fn external(&self) -> Result<bool, Error> {
            Ok(self.calls.lock().unwrap().external)
        }
    }
    #[test]
    fn published_pending_receipt_survives_close_guard_and_reopens_without_republication_en_fr() {
        release_pending_after_confirmation_keeps_real_disk_state_en_fr();
        for language in ["en", "fr"] {
            for external in [false, true] {
                i18n::init(deadsync_assets::language::load_for_tests(language));
                i18n::set_locale(deadsync_assets::language::load_for_tests(language));
                let root = tempfile::tempdir().unwrap();
                let song = root.path().join("Song");
                fs::create_dir(&song).unwrap();
                let path = song.join("song.ssc");
                fs::write(&path,"#VERSION:0.83;#TITLE:Fixture;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
                let mut state = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
                state.open_pack_properties();
                let calls = Arc::new(Mutex::new(Calls {
                    external,
                    dirty: true,
                    ..Calls::default()
                }));
                let settings = state.pack_properties.as_mut().unwrap();
                settings.file = Some(Box::new(Published {
                    data: settings.defaults.clone(),
                    calls: calls.clone(),
                }));
                settings.mode = Mode::Edit;
                settings.index = 7;
                state.focus_text();
                state.pack_properties_key(KeyCode::Enter);
                assert!(state.pack_has_pending_durability());
                assert!(state.pack_has_unsaved_changes());
                let settings = state.pack_properties.as_ref().unwrap();
                assert!(settings.mode == Mode::Pending);
                assert_eq!(settings.row_key(0), "PackRetry");
                assert_eq!(settings.row_key(1), "PackClosePending");
                assert!(!state.wants_text_input());
                state.pack_properties_key(KeyCode::Escape);
                assert!(state.panel.is_none());
                assert!(state.pack_has_pending_durability());
                state.workspace.file().save_recovery().unwrap();
                let source_binding = SessionBinding::capture(&state.workspace.file().session);
                for (code, pressed) in [
                    (KeyCode::ControlLeft, true),
                    (KeyCode::KeyR, true),
                    (KeyCode::KeyR, false),
                    (KeyCode::ControlLeft, false),
                ] {
                    state.raw_key(&deadlib_platform::input::RawKeyboardEvent {
                        code,
                        pressed,
                        repeat: false,
                        timestamp: std::time::Instant::now(),
                        host_nanos: 0,
                    });
                }
                assert_eq!(state.status, i18n::tr("Edition", "PackRecoveryBlocked"));
                assert!(source_binding.belongs_to(&state.workspace.file().session));
                assert!(state.pack_has_pending_durability());
                assert_eq!(calls.lock().unwrap().saves, 1);
                state.pack_properties_key(KeyCode::Escape);
                assert!(!state.request_close());
                assert!(matches!(state.panel, Some(Panel::PackProperties)));
                state.pack_properties_key(KeyCode::Enter);
                let actual = calls.lock().unwrap();
                assert_eq!(actual.saves, 1, "retry must not republish");
                assert_eq!(actual.retries, 1);
                drop(actual);
                if external {
                    assert!(state.pack_has_pending_durability());
                    assert!(state.pack_has_unsaved_changes());
                    assert_eq!(state.status, i18n::tr("Edition", "PackPendingConflict"));
                } else {
                    assert!(!state.pack_has_pending_durability());
                    assert!(!state.pack_has_unsaved_changes());
                    assert_eq!(state.status, i18n::tr("Edition", "PackSaved"));
                }
                assert!(
                    !root.path().join("Pack.ini").exists(),
                    "native fault adapter never writes a user or fixture Pack.ini"
                );
                // Another song session invalidates text, clipboard targets and
                // save, while Escape still preserves the old draft.
                let mut stale = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
                stale.open_pack_properties();
                // Finish the independent banner scan before creating the
                // unpublished draft exercised by the stale-session guard.
                stale.pack_banner = super::super::pack_banner::Runtime::settled(
                    root.path().canonicalize().unwrap(),
                    None,
                );
                stale.pack_properties_key(KeyCode::Enter);
                let foreign = deadsync_editor::history::Session::new(
                    deadsync_editor::document::Document::parse(
                        b"#TITLE:Other;#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;".as_slice(),
                        deadsync_editor::document::Format::Sm,
                    )
                    .unwrap(),
                );
                stale.pack_properties.as_mut().unwrap().binding = SessionBinding::capture(&foreign);
                assert!(!stale.text_input("Must not edit another session"));
                stale.control = true;
                stale.pack_properties_key(KeyCode::KeyS);
                stale.control = false;
                assert_eq!(stale.status, i18n::tr("Edition", "PackSourceChanged"));
                assert!(!root.path().join("Pack.ini").exists());
                stale.pack_properties_key(KeyCode::Escape);
                assert!(stale.panel.is_none());
                assert!(stale.pack_has_unsaved_changes());
                assert!(!stale.request_close());
                stale.pack_properties_key(KeyCode::ArrowUp);
                stale.pack_properties_key(KeyCode::Enter);
                assert!(
                    stale.pack_properties.is_none(),
                    "an unpublished stale draft can be explicitly discarded"
                );
            }
        }
    }
    fn release_pending_after_confirmation_keeps_real_disk_state_en_fr() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // Real disk/baseline checks, injected pending-only native boundary. The
        // core's published receipt mechanics have their own fault tests.
        struct PendingFile {
            file: FileSession,
            writes: Arc<AtomicUsize>,
        }
        impl PackFile for PendingFile {
            fn data(&self) -> &Data {
                self.file.draft().data()
            }
            fn dirty(&self) -> bool {
                self.file.draft().has_unsaved_changes()
            }
            fn pending(&self) -> bool {
                true
            }
            fn save(&mut self) -> Result<Durability, Error> {
                self.writes.fetch_add(1, Ordering::SeqCst);
                Err(Error::PendingSave)
            }
            fn retry(&mut self) -> Result<Durability, Error> {
                self.writes.fetch_add(1, Ordering::SeqCst);
                Err(Error::PendingSave)
            }
            fn external(&self) -> Result<bool, Error> {
                self.file.has_external_change()
            }
        }
        fn press(state: &mut State, code: KeyCode) {
            for pressed in [true, false] {
                state.raw_key(&deadlib_platform::input::RawKeyboardEvent {
                    code,
                    pressed,
                    repeat: false,
                    timestamp: std::time::Instant::now(),
                    host_nanos: 0,
                });
            }
        }
        for language in ["en", "fr"] {
            for external in [false, true] {
                i18n::init(deadsync_assets::language::load_for_tests(language));
                i18n::set_locale(deadsync_assets::language::load_for_tests(language));
                let root = tempfile::tempdir().unwrap();
                let song = root.path().join("Song");
                fs::create_dir(&song).unwrap();
                let path = song.join("song.ssc");
                fs::write(&path, "#VERSION:0.83;#TITLE:Fixture;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
                let pack_path = root.path().join("Pack.ini");
                fs::write(
                    &pack_path,
                    Data::new("Published", "", 2026, None, 0.009)
                        .canonical()
                        .unwrap(),
                )
                .unwrap();
                let mut state = State::open_selected(&path, "dance-single", "Hard", "", 9).unwrap();
                state.open_pack_properties();
                let mut file = FileSession::open(root.path()).unwrap().unwrap();
                let mut edits = file.draft().data().clone();
                edits.title = Some("Local edits".into());
                file.replace(edits).unwrap();
                let writes = Arc::new(AtomicUsize::new(0));
                let settings = state.pack_properties.as_mut().unwrap();
                settings.file = Some(Box::new(PendingFile {
                    file,
                    writes: writes.clone(),
                }));
                settings.mode = Mode::Pending;
                settings.index = 0;
                if external {
                    fs::write(&pack_path, b"external bytes must remain exactly intact").unwrap();
                }
                let before = fs::read(&pack_path).unwrap();
                let identity = fs::metadata(&pack_path).unwrap().modified().unwrap();
                press(&mut state, KeyCode::ArrowDown);
                press(&mut state, KeyCode::ArrowDown);
                press(&mut state, KeyCode::Enter);
                assert!(state.pack_properties.as_ref().unwrap().mode == Mode::ReleaseConfirm);
                assert_eq!(
                    state.pack_properties.as_ref().unwrap().index,
                    0,
                    "confirmation defaults to keeping edits"
                );
                press(&mut state, KeyCode::Enter); // Cancel using the default row.
                assert!(state.pack_has_pending_durability());
                assert!(state.pack_has_unsaved_changes());
                assert!(state.pack_properties.as_ref().unwrap().mode == Mode::Pending);
                press(&mut state, KeyCode::Enter); // Open it again, then cancel with Escape.
                press(&mut state, KeyCode::Escape);
                assert!(matches!(state.panel, Some(Panel::PackProperties)));
                assert!(state.pack_properties.as_ref().unwrap().mode == Mode::Pending);
                assert_eq!(fs::read(&pack_path).unwrap(), before);
                press(&mut state, KeyCode::Enter);
                press(&mut state, KeyCode::ArrowDown);
                press(&mut state, KeyCode::Enter);
                assert!(state.panel.is_none());
                assert!(state.pack_properties.is_none());
                assert!(!state.pack_has_pending_durability());
                assert!(!state.pack_has_unsaved_changes());
                assert_eq!(
                    state.status,
                    i18n::tr(
                        "Edition",
                        if external {
                            "PackReleasedExternal"
                        } else {
                            "PackReleasedPublished"
                        }
                    )
                );
                assert_eq!(
                    writes.load(Ordering::SeqCst),
                    0,
                    "release is neither save nor retry"
                );
                assert_eq!(fs::read(&pack_path).unwrap(), before);
                assert_eq!(
                    fs::metadata(&pack_path).unwrap().modified().unwrap(),
                    identity
                );
                assert!(
                    state.request_close(),
                    "explicit release must permit closing after permanent pending failure"
                );
            }
        }
    }
    #[test]
    fn coupled_pending_pack_does_not_publish_song_or_complete_close_en_fr() {
        for language in ["en", "fr"] {
            i18n::init_for_tests();
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let song = root.path().join("Song");
            fs::create_dir(&song).unwrap();
            let path = song.join("song.ssc");
            fs::write(&path,b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
            let original = fs::read(&path).unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.open_pack_properties();
            let calls = Arc::new(Mutex::new(Calls {
                dirty: true,
                ..Default::default()
            }));
            let settings = state.pack_properties.as_mut().unwrap();
            settings.file = Some(Box::new(Published {
                data: settings.defaults.clone(),
                calls: calls.clone(),
            }));
            settings.mode = Mode::Edit;
            state
                .workspace
                .execute(deadsync_editor::workspace::Command::Place(1))
                .unwrap();
            state.workspace.file().save_recovery().unwrap();
            let recovery = state.workspace.file().recovery_path().unwrap();
            let recovery_before = fs::read(&recovery).unwrap();
            assert!(!state.request_close());
            assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
            assert!(!state.file_menu_key(1, KeyCode::Enter));
            assert_eq!(calls.lock().unwrap().saves, 1);
            assert!(state.pack_has_pending_durability());
            assert!(state.workspace.file().session.is_dirty());
            assert_eq!(fs::read(&path).unwrap(), original);
            assert_eq!(fs::read(&recovery).unwrap(), recovery_before);
            assert!(!state.take_file_close());
        }
    }

    #[test]
    fn coupled_pack_only_retry_completes_exact_exit_intent_and_edit_return_revokes_it() {
        for mutate in [false, true] {
            i18n::init_for_tests();
            let root = tempfile::tempdir().unwrap();
            let song = root.path().join("Song");
            fs::create_dir(&song).unwrap();
            let path = song.join("song.ssc");
            fs::write(&path,b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.open_pack_properties();
            let calls = Arc::new(Mutex::new(Calls {
                dirty: true,
                ..Default::default()
            }));
            let settings = state.pack_properties.as_mut().unwrap();
            settings.file = Some(Box::new(Published {
                data: settings.defaults.clone(),
                calls: calls.clone(),
            }));
            settings.mode = Mode::Edit;
            assert_eq!(
                state.request_application_exit(),
                super::super::ApplicationExit::Pending
            );
            assert!(!state.file_menu_key(1, KeyCode::Enter));
            assert!(state.pack_has_pending_durability());
            if mutate {
                state
                    .workspace
                    .execute(deadsync_editor::workspace::Command::Place(1))
                    .unwrap();
                state
                    .workspace
                    .execute(deadsync_editor::workspace::Command::Undo)
                    .unwrap();
            }
            state.pack_properties_key(KeyCode::Enter);
            assert_eq!(calls.lock().unwrap().saves, 1);
            assert_eq!(calls.lock().unwrap().retries, 1);
            assert_eq!(
                state.take_application_exit(),
                Some(if mutate {
                    super::super::ApplicationExit::Cancelled
                } else {
                    super::super::ApplicationExit::Ready
                })
            );
            assert!(!state.take_file_close());
        }
    }

    #[test]
    fn source_close_waits_for_pack_durability_before_entering_no_song_then_os_exit_is_ready() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("Song");
        fs::create_dir(&song).unwrap();
        let path = song.join("song.ssc");
        fs::write(&path,b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.open_pack_properties();
        let calls = Arc::new(Mutex::new(Calls {
            dirty: true,
            ..Default::default()
        }));
        let settings = state.pack_properties.as_mut().unwrap();
        settings.file = Some(Box::new(Published {
            data: settings.defaults.clone(),
            calls: calls.clone(),
        }));
        settings.mode = Mode::Edit;
        state.command_file_close();
        assert!(!state.file_menu_key(1, KeyCode::Enter));
        assert!(state.has_song());
        assert!(state.pack_has_pending_durability());
        assert!(!state.take_file_close());
        state.pack_properties_key(KeyCode::Enter);
        assert_eq!(calls.lock().unwrap().saves, 1);
        assert_eq!(calls.lock().unwrap().retries, 1);
        assert!(!state.has_song());
        assert!(state.pack_properties.is_none());
        assert!(!state.take_file_close());
        assert_eq!(
            state.request_application_exit(),
            super::super::ApplicationExit::Ready
        );
        assert_eq!(
            state.take_application_exit(),
            Some(super::super::ApplicationExit::Ready)
        );
    }

    #[test]
    fn prepared_other_pack_waits_for_exact_pack_retry_and_rechecks_target_and_source() {
        for changed in [0, 1, 2] {
            i18n::init_for_tests();
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("Pack/A/song.ssc");
            let target = root.path().join("Other/B/song.ssc");
            let text=b"#VERSION:0.83;#TITLE:Native;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
            for p in [&path, &target] {
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(p, text).unwrap();
            }
            let row =
                deadsync_editor::pack::song_list::Task::from_song_file(&target, Default::default())
                    .unwrap()
                    .run(
                        &deadsync_editor::pack::song_list::HeaderReader,
                        &std::sync::atomic::AtomicBool::new(false),
                        &mut |_| {},
                    )
                    .unwrap()
                    .rows
                    .into_iter()
                    .find(|r| r.path == target.canonicalize().unwrap())
                    .unwrap();
            let mut state = State::open(&path, 0).unwrap();
            state.open_pack_properties();
            let calls = Arc::new(Mutex::new(Calls {
                dirty: true,
                ..Default::default()
            }));
            let settings = state.pack_properties.as_mut().unwrap();
            settings.file = Some(Box::new(Published {
                data: settings.defaults.clone(),
                calls: calls.clone(),
            }));
            settings.mode = Mode::Edit;
            state.command_open_prepared_pack_song(State::open(&target, 0).unwrap(), row);
            assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
            assert!(!state.file_menu_key(1, KeyCode::Enter));
            assert!(state.pack_has_pending_durability());
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(path.canonicalize().unwrap().as_path())
            );
            if changed == 1 {
                fs::write(&target, b"#TITLE:Changed externally;").unwrap();
            }
            if changed == 2 {
                state
                    .workspace
                    .execute(deadsync_editor::workspace::Command::Place(1))
                    .unwrap();
                state
                    .workspace
                    .execute(deadsync_editor::workspace::Command::Undo)
                    .unwrap();
            }
            state.pack_properties_key(KeyCode::Enter);
            assert_eq!(calls.lock().unwrap().saves, 1);
            assert_eq!(calls.lock().unwrap().retries, 1);
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(
                    if changed == 0 {
                        target.canonicalize().unwrap()
                    } else {
                        path.canonicalize().unwrap()
                    }
                    .as_path()
                )
            );
            assert!(!state.take_file_close());
        }
    }
}

impl State {
    pub(super) fn pack_watch_snapshot(
        &self,
        root: Option<&std::path::Path>,
    ) -> Option<deadsync_editor::pack::storage::ExternalSnapshot> {
        self.pack_properties
            .as_ref()
            .filter(|s| Some(s.root.as_path()) == root)
            .and_then(|s| s.file.as_ref())
            .and_then(|file| file.external_snapshot())
    }
    /// Adopt the worker's exact old baseline, never re-read the externally changed file.
    pub(super) fn open_passive_pack_notice(&mut self, baseline: &mut Option<FileSession>) -> bool {
        let Some(source) = self.workspace.file().existing_path().map(PathBuf::from) else {
            return false;
        };
        let Some(root) = source.parent().and_then(|p| p.parent()).map(PathBuf::from) else {
            return false;
        };
        if let Some(settings) = &self.pack_properties {
            if settings.source != source
                || settings.root != root
                || !settings.binding.belongs_to(&self.workspace.file().session)
                || settings.pending()
            {
                return false;
            }
        } else {
            let Some(file) = baseline.as_ref() else {
                return false;
            };
            if file.path().parent() != Some(root.as_path()) {
                return false;
            }
            let defaults = self.pack_creation_defaults(&root);
            let file = baseline.take().expect("validated worker baseline");
            self.pack_properties = Some(Settings {
                identity: Arc::new(()),
                revision: 0,
                text_reviews: std::array::from_fn(|_| None),
                delete_review: None,
                binding: SessionBinding::capture(&self.workspace.file().session),
                source,
                root,
                defaults,
                file: Some(boxed(file)),
                mode: Mode::Conflict,
                index: 0,
                invalid: std::array::from_fn(|_| None),
            });
        }
        let settings = self
            .pack_properties
            .as_mut()
            .expect("existing or worker baseline");
        settings.mode = Mode::Conflict;
        settings.index = 0;
        self.stop_audio();
        self.panel = Some(Panel::PackProperties);
        self.focus_text();
        self.status = i18n::tr("Edition", "PackPassiveChanged");
        true
    }
}

fn pack_edit_error(error: deadsync_editor::Error) -> deadsync_editor::Error {
    match error {
        deadsync_editor::Error::ExternalChange => {
            deadsync_editor::Error::InvalidEdit("Pack.ini changed outside this editor".into())
        }
        other => other,
    }
}
fn pack_history_conflict(error: &deadsync_editor::Error) -> bool {
    matches!(error, deadsync_editor::Error::ExternalChange)
        || matches!(error, deadsync_editor::Error::InvalidEdit(message)
            if message == "Pack.ini changed outside this editor")
}
fn pack_history_error_status(error: &deadsync_editor::Error) -> Arc<str> {
    match error {
        deadsync_editor::Error::ExternalChange => i18n::tr("Edition", "PackConflict"),
        _ => i18n::editor_error(error).into(),
    }
}
impl State {
    pub(super) fn apply_pack_properties_history(&mut self, redo: bool) {
        let result = self.workspace.execute(if redo {
            deadsync_editor::workspace::Command::Redo
        } else {
            deadsync_editor::workspace::Command::Undo
        });
        match result {
            Ok(()) => {
                if let Some(settings) = &mut self.pack_properties {
                    if let Some(file) = &mut settings.file {
                        file.synchronize();
                    }
                    settings.identity = Arc::new(());
                    settings.revision = 0;
                    settings.delete_review = None;
                }
                self.refresh();
                self.status = i18n::tr("Edition", if redo { "Redo" } else { "Undo" });
            }
            Err(error) => self.status = pack_history_error_status(&error),
        }
    }
}

#[cfg(test)]
impl State {
    pub(super) fn pack_history_index_for_test(&self) -> Option<usize> {
        self.pack_properties.as_ref().map(|settings| settings.index)
    }
    pub(super) fn pack_history_data_for_test(&self) -> Option<Data> {
        self.pack_properties
            .as_ref()?
            .file
            .as_ref()
            .map(|file| file.data().clone())
    }
}

#[cfg(test)]
mod staged_departure_tests;

#[cfg(test)]
mod file_intent_deactivation_tests;
