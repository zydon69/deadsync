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
        storage::{Durability, Error, FileSession},
    },
    text_buffer::Movement,
};
use std::{borrow::Cow, path::PathBuf, sync::Arc};
// Small native I/O boundary: domain/disk behavior stays in pack::storage;
// presentation tests can inject a published-but-not-yet-durable receipt.
trait PackFile: Send {
    fn data(&self) -> &Data;
    fn dirty(&self) -> bool;
    fn pending(&self) -> bool;
    fn replace(&mut self, data: Data) -> Result<bool, Error>;
    fn save(&mut self) -> Result<Durability, Error>;
    fn retry(&mut self) -> Result<Durability, Error>;
    fn external(&self) -> Result<bool, Error>;
}
impl PackFile for FileSession {
    fn data(&self) -> &Data {
        self.draft().data()
    }
    fn dirty(&self) -> bool {
        self.draft().has_unsaved_changes()
    }
    fn pending(&self) -> bool {
        self.durability_pending()
    }
    fn replace(&mut self, data: Data) -> Result<bool, Error> {
        FileSession::replace(self, data)
    }
    fn save(&mut self) -> Result<Durability, Error> {
        FileSession::save(self)
    }
    fn retry(&mut self) -> Result<Durability, Error> {
        self.retry_durability()
    }
    fn external(&self) -> Result<bool, Error> {
        self.has_external_change()
    }
}
fn boxed(file: FileSession) -> Box<dyn PackFile> {
    Box::new(file)
}
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
    Edit,
    Missing,
    ReadFailed,
    Conflict,
    ReloadConfirm,
    Pending,
    SourceChanged,
    ReleaseConfirm,
}
pub(super) struct Settings {
    binding: SessionBinding,
    source: PathBuf,
    root: PathBuf,
    defaults: Data,
    file: Option<Box<dyn PackFile>>,
    mode: Mode,
    index: usize,
    // Invalid in-progress text survives closing/reopening without making it a
    // valid saved value or silently discarding what the user typed.
    invalid: [Option<String>; 6],
}
impl Settings {
    fn is_dirty(&self) -> bool {
        self.invalid.iter().any(Option::is_some) || self.file.as_ref().is_some_and(|f| f.dirty())
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
    pub(super) fn set_text(
        &mut self,
        field: Field,
        value: &str,
    ) -> Result<(), deadsync_editor::Error> {
        let invalid = || deadsync_editor::Error::InvalidEdit("Invalid pack property value".into());
        if self.mode != Mode::Edit {
            return Err(invalid());
        }
        let Some(file) = &mut self.file else {
            return Err(invalid());
        };
        self.invalid[field.index()] = Some(value.into());
        if value.contains(['\r', '\n', '\0']) {
            return Err(invalid());
        }
        let mut data = file.data().clone();
        match field {
            Field::Title => data.title = Some(value.into()),
            Field::TranslitTitle => data.title_transliteration = Some(value.into()),
            Field::SortTitle => data.title_sort = Some(value.into()),
            Field::Series => data.series = Some(value.into()),
            Field::Banner => data.banner = Some(value.into()),
            Field::Year => {
                let year = value.trim().parse::<i32>().map_err(|_| invalid())?;
                if year < 0 {
                    return Err(invalid());
                }
                data.year = year;
            }
        }
        file.replace(data).map_err(|_| invalid())?;
        self.invalid[field.index()] = None;
        Ok(())
    }
    fn count(&self) -> usize {
        match self.mode {
            Mode::Edit => 9,
            Mode::Conflict => 3,
            Mode::Pending => 3,
            _ => 2,
        }
    }
    fn row_key(&self, index: usize) -> &'static str {
        match self.mode {
            Mode::Edit => match index {
                0..=5 => Field::ALL[index].key(),
                6 => "PackSync",
                7 => "PackSave",
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
    pub(super) fn pack_has_unsaved_changes(&self) -> bool {
        self.pack_properties
            .as_ref()
            .is_some_and(Settings::is_dirty)
    }
    pub(super) fn pack_has_pending_durability(&self) -> bool {
        self.pack_properties.as_ref().is_some_and(Settings::pending)
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
        let Some(source) = self.workspace.file().existing_path().map(std::path::Path::to_path_buf) else {
            self.status = i18n::tr("Edition", "FileRequiresSaveAs");
            return;
        };
        if let Some(settings) = &self.pack_properties {
            if settings.source == source
                && settings.binding.belongs_to(&self.workspace.file().session)
            {
                self.stop_audio();
                self.panel = Some(Panel::PackProperties);
                self.focus_text();
                self.status = i18n::tr("Edition", "PackHelp");
                return;
            }
            if settings.is_dirty() || settings.pending() {
                let settings = self.pack_properties.as_mut().unwrap();
                settings.mode = Mode::SourceChanged;
                settings.index = settings.count() - 1;
                self.stop_audio();
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
        let title = root.file_name().unwrap_or_default().to_string_lossy();
        let active =
            deadsync_editor::song_properties::sync_offset(self.workspace.file().session.document())
                .ok();
        let defaults = Data::new(&title, "", chrono::Utc::now().year(), active, 0.009);
        let (file, mode) = match FileSession::open(&root) {
            Ok(Some(file)) => (Some(boxed(file)), Mode::Edit),
            Ok(None) => (None, Mode::Missing),
            Err(_) => (None, Mode::ReadFailed),
        };
        self.stop_audio();
        self.pack_properties = Some(Settings {
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
        self.status = i18n::tr("Edition", "PackHelp");
    }
    pub(super) fn pack_properties_key(&mut self, code: KeyCode) {
        let Some(settings) = &self.pack_properties else {
            return;
        };
        if code == KeyCode::Escape && settings.mode == Mode::ReleaseConfirm {
            let settings = self.pack_properties.as_mut().unwrap();
            settings.mode = Mode::Pending;
            settings.index = 2;
            self.status = i18n::tr("Edition", "PackPending");
            self.focus_text();
            return;
        }
        if code == KeyCode::Escape {
            self.sync_text();
            self.close_pack_panel();
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
        let action = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
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
            self.sync_text();
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
                if file.replace(data).is_err() {
                    self.status = i18n::tr("Edition", "PackFailed");
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
                        settings.mode = Mode::Pending;
                        settings.index = 2;
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
                Mode::Edit => match settings.index {
                    7 => {}
                    8 => {
                        discard = true;
                        close = true
                    }
                    _ => {}
                },
                Mode::Missing => {
                    if settings.index == 0 {
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
        if discard {
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
                settings.file = file.map(boxed);
                settings.mode = if settings.file.is_some() {
                    Mode::Edit
                } else {
                    Mode::Missing
                };
                settings.index = 0;
                settings.invalid = std::array::from_fn(|_| None);
                *status = i18n::tr("Edition", "PackHelp");
            }
            Err(_) => *status = i18n::tr("Edition", "PackReadFailed"),
        }
    }
    fn check_pack_external(&mut self) {
        self.sync_text();
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
                Ok(false) => self.status = i18n::tr("Edition", "PackUnchanged"),
                Err(_) => self.status = i18n::tr("Edition", "PackReadFailed"),
            }
        } else {
            Self::reload_pack(settings, &mut self.status);
        }
        self.focus_text();
    }
    fn save_pack_properties(&mut self) {
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
        let height = 430.0;
        let top = (screen_height() - height).max(10.0) * 0.5;
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
        for index in 0..settings.count() {
            let text = if settings.mode == Mode::Edit && index < 6 {
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
                top + 96.0 + index as f32 * 25.0,
                if index == settings.index {
                    [1.0, 0.85, 0.35, 1.0]
                } else {
                    [0.85, 0.9, 1.0, 1.0]
                },
            );
        }
        actors.push(act!(text: font("miso"): settext(self.status.clone()): align(0.0,0.0): xy(left+16.0,top+height-88.0): zoom(0.50): wrapwidthpixels((width-32.0)/0.50): maxwidth(width-32.0): maxheight(42.0): diffuse(0.9,0.9,0.9,1.0): z(8021)));
        label(
            actors,
            i18n::tr("Edition", "PackHelp").to_string(),
            top + height - 38.0,
            [0.7, 0.8, 0.9, 1.0],
        );
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
        fn replace(&mut self, data: Data) -> Result<bool, Error> {
            let changed = self.data != data;
            self.data = data;
            self.calls.lock().unwrap().dirty |= changed;
            Ok(changed)
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
            fn replace(&mut self, data: Data) -> Result<bool, Error> {
                self.file.replace(data)
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
}
