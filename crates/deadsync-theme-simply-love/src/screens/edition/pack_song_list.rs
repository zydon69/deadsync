//! Modal, keyboard-only browsing. Header discovery never replaces the editor.
mod application_preferences;
use super::{Panel, State, pack_song_load};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::{
    chart_order::SessionBinding,
    generation::job::Job,
    pack::song_list::{Catalog, Limits, Orderer, Selection, Task},
    workspace::Command,
};
use std::path::PathBuf;

fn page_size() -> usize {
    ((((screen_height() - 28.0).min(480.0) - 170.0).max(28.0) / 28.0) as usize).clamp(1, 8)
}
#[derive(Clone, Copy)]
enum Mode {
    List,
    Unsaved(usize),
    Discard(bool),
}
pub(super) struct Settings {
    binding: SessionBinding,
    source: PathBuf,
    scan: Option<Job<Catalog>>,
    catalog: Option<Catalog>,
    order: Vec<usize>,
    selection: Selection,
    culture: Option<u64>,
    loading: Option<pack_song_load::Job>,
    candidate: Option<Box<State>>,
    prepared: Option<super::pack_song_preference::Prepared>,
    mode: Mode,
}
impl Settings {
    fn current(&self, state: &State) -> bool {
        self.source == state.workspace.file().path()
            && self.binding.is_current(&state.workspace.file().session)
    }
}
impl State {
    pub(super) fn open_pack_song_list(&mut self) {
        self.stop_audio();
        let source = self.workspace.file().path().to_path_buf();
        let selection = self
            .pack_song_list
            .take()
            .map(|settings| settings.selection)
            .unwrap_or_default();
        let task = Task::from_song_file(&source, Limits::default()).and_then(Task::spawn);
        match task {
            Ok(scan) => {
                self.pack_song_list = Some(Settings {
                    binding: SessionBinding::capture(&self.workspace.file().session),
                    source,
                    scan: Some(scan),
                    catalog: None,
                    order: Vec::new(),
                    selection,
                    culture: None,
                    loading: None,
                    candidate: None,
                    prepared: None,
                    mode: Mode::List,
                });
                self.panel = Some(Panel::PackSongList);
                self.status = i18n::tr("Edition", "PackSongsLoading");
            }
            Err(error) => {
                self.panel = None;
                self.pack_song_error(error);
            }
        }
    }
    fn pack_song_error(&mut self, error: impl std::fmt::Display) {
        self.status = i18n::tr_fmt(
            "Edition",
            "PackSongsFailed",
            &[("message", &error.to_string())],
        );
    }
    pub(super) fn poll_pack_song_list(&mut self) {
        let Some(settings) = self.pack_song_list.as_ref() else {
            return;
        };
        if !settings.current(self) {
            self.pack_song_list = None;
            if matches!(self.panel, Some(Panel::PackSongList)) {
                self.panel = None;
            }
            self.status = i18n::tr("Edition", "PackSongsSourceChanged");
            return;
        }
        let settings = self.pack_song_list.as_mut().unwrap();
        if let Some(result) = settings.scan.as_mut().and_then(Job::poll) {
            settings.scan = None;
            match result {
                Ok(catalog) => {
                    settings.catalog = Some(catalog);
                    settings.culture = None;
                }
                Err(error) => self.pack_song_error(error),
            }
        }
        let result = self
            .pack_song_list
            .as_mut()
            .and_then(|settings| settings.loading.as_mut())
            .and_then(pack_song_load::Job::poll);
        let Some(result) = result else {
            return;
        };
        self.pack_song_list.as_mut().unwrap().loading = None;
        match result {
            Ok(prepared) => self.pack_song_list.as_mut().unwrap().prepared = Some(prepared),
            Err(error) => self.pack_song_error(error),
        }
    }
    fn complete_pack_song_preference(&mut self, orderer: &mut impl deadsync_editor::chart_order::Orderer) -> bool {
        let revision = i18n::revision();
        let Some(settings) = self.pack_song_list.as_ref() else { return false; };
        if !settings.current(self) { return false; }
        let Some(prepared) = settings.prepared.as_ref() else { return false; };
        let selected = prepared.selected(orderer);
        let Some(settings) = self.pack_song_list.as_ref() else { return false; };
        if !settings.current(self) || revision != i18n::revision() || !matches!(self.panel, Some(Panel::PackSongList)) { return false; }
        let prepared = self.pack_song_list.as_mut().unwrap().prepared.take().unwrap();
        let result = selected.and_then(|chart| prepared.workspace(chart)).and_then(|(workspace, failures)| {
            let mut candidate = State::from_workspace(workspace)?;
            candidate.song_open_warnings = failures;
            application_preferences::Snapshot::capture(self).install(&mut candidate)?;
            Ok(candidate)
        });
        match result {
            Ok(candidate) => {
                self.pack_song_list.as_mut().unwrap().candidate = Some(Box::new(candidate));
                if self.workspace.file().session.is_dirty() {
                    self.pack_song_list.as_mut().unwrap().mode = Mode::Unsaved(0);
                    self.status = i18n::tr("Edition", "PackSongsUnsaved");
                } else { self.publish_pack_song(); }
            }
            Err(error) => self.pack_song_error(error),
        }
        true
    }
    /// Main-thread cultural boundary. Call on frames as well as raw-key dispatch:
    /// directory workers may finish without another keyboard event.
    pub fn process_pack_song_list(&mut self, orderer: &mut impl Orderer, chart_orderer: &mut impl deadsync_editor::chart_order::Orderer) -> bool {
        if !matches!(self.panel, Some(Panel::PackSongList)) {
            return false;
        }
        self.poll_pack_song_list();
        let completed = self.complete_pack_song_preference(chart_orderer);
        let revision = i18n::revision();
        let Some(settings) = self.pack_song_list.as_ref() else {
            return completed;
        };
        if !settings.current(self)
            || settings.culture == Some(revision)
            || settings.catalog.is_none()
        {
            return completed;
        }
        let result = settings.catalog.as_ref().unwrap().ordered(orderer);
        let Some(settings) = self.pack_song_list.as_ref() else {
            return completed;
        };
        if !settings.current(self) || revision != i18n::revision() {
            return completed;
        }
        let settings = self.pack_song_list.as_mut().unwrap();
        match result {
            Ok(order) => {
                settings.order = order;
                let initial = settings.selection.selected().is_none();
                let catalog = settings.catalog.as_ref().unwrap();
                settings.selection.reconcile(catalog, &settings.order);
                if initial {
                    if let Some(index) = settings
                        .order
                        .iter()
                        .position(|&index| catalog.rows[index].path == settings.source)
                    {
                        settings
                            .selection
                            .move_by(index as isize, catalog, &settings.order);
                    }
                }
                settings.culture = Some(revision);
                if settings.loading.is_none() && settings.candidate.is_none() {
                    self.status = i18n::tr("Edition", "PackSongsHelp");
                }
            }
            Err(error) => {
                settings.order.clear();
                // Avoid repeating a failing native boundary every frame. F5 retries.
                settings.culture = Some(revision);
                self.pack_song_error(error);
            }
        }
        true
    }
    pub(super) fn pack_song_list_key(&mut self, code: KeyCode) {
        let code = if code == KeyCode::NumpadEnter {
            KeyCode::Enter
        } else {
            code
        };
        if code == KeyCode::Escape {
            self.pack_song_list = None; // Drop cancels both workers; no publication.
            self.panel = None;
            self.status = i18n::tr("Edition", "PackSongsCancelled");
            return;
        }
        let Some(settings) = self.pack_song_list.as_ref() else {
            return;
        };
        if !settings.current(self) {
            self.poll_pack_song_list();
            return;
        }
        match settings.mode {
            Mode::Unsaved(index) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    let next = (index + if code == KeyCode::ArrowDown { 1 } else { 2 }) % 3;
                    self.pack_song_list.as_mut().unwrap().mode = Mode::Unsaved(next);
                } else if code == KeyCode::Enter {
                    match index {
                        0 => {
                            self.pack_song_list = None;
                            self.panel = None;
                        }
                        1 => {
                            self.command(Command::Save);
                            if !self.workspace.file().session.is_dirty() {
                                self.publish_pack_song();
                            }
                        }
                        _ => self.pack_song_list.as_mut().unwrap().mode = Mode::Discard(false),
                    }
                }
                return;
            }
            Mode::Discard(confirm) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.pack_song_list.as_mut().unwrap().mode = Mode::Discard(!confirm);
                } else if code == KeyCode::Enter {
                    if confirm {
                        self.publish_pack_song();
                    } else {
                        self.pack_song_list.as_mut().unwrap().mode = Mode::Unsaved(0);
                    }
                }
                return;
            }
            Mode::List => {}
        }
        if code == KeyCode::F5 && settings.scan.is_none() && settings.loading.is_none() && settings.prepared.is_none() {
            self.open_pack_song_list();
            return;
        }
        if settings.scan.is_some()
            || settings.loading.is_some()
            || settings.prepared.is_some()
            || settings.culture != Some(i18n::revision())
        {
            return;
        }
        let delta = match code {
            KeyCode::ArrowUp => -1,
            KeyCode::ArrowDown => 1,
            KeyCode::PageUp => -(page_size() as isize),
            KeyCode::PageDown => page_size() as isize,
            _ => 0,
        };
        if delta != 0 {
            let settings = self.pack_song_list.as_mut().unwrap();
            if let Some(catalog) = &settings.catalog {
                settings.selection.move_by(delta, catalog, &settings.order);
            }
            return;
        }
        if code != KeyCode::Enter {
            return;
        }
        if self.pack_has_unsaved_changes() || self.pack_has_pending_durability() {
            self.open_pack_properties();
            self.status = i18n::tr("Edition", "PackSongsPackGuard");
            return;
        }
        let settings = self.pack_song_list.as_ref().unwrap();
        let row = settings
            .catalog
            .as_ref()
            .and_then(|catalog| {
                settings
                    .order
                    .get(settings.selection.position())
                    .and_then(|&index| catalog.rows.get(index))
            })
            .cloned();
        let Some(row) = row else {
            return;
        };
        // Reopening the active path is explicit refresh, and still uses dirty guards.
        let preferences = match self.audio_preferences_file.as_ref().map(|file| file.opening()).transpose() {
            Ok(preferences) => preferences.unwrap_or_default(),
            Err(error) => { self.pack_song_error(error); return; },
        };
        match pack_song_load::Job::spawn(row, preferences) {
            Ok(job) => {
                self.pack_song_list.as_mut().unwrap().loading = Some(job);
                self.status = i18n::tr("Edition", "PackSongsOpening");
            }
            Err(error) => self.pack_song_error(error),
        }
    }
    fn publish_pack_song(&mut self) {
        if self.pack_has_unsaved_changes() || self.pack_has_pending_durability() {
            self.open_pack_properties();
            self.status = i18n::tr("Edition", "PackSongsPackGuard");
            return;
        }
        let Some(settings) = self.pack_song_list.as_ref() else {
            return;
        };
        if !settings.current(self) {
            self.poll_pack_song_list();
            return;
        }
        let Some(candidate) = settings.candidate.as_ref() else {
            return;
        };
        // Recheck the logical path too: a symlink or byte-identical file may
        // have been replaced while the confirmation was visible.
        let row = settings.catalog.as_ref().and_then(|catalog| {
            settings
                .order
                .get(settings.selection.position())
                .and_then(|&index| catalog.rows.get(index))
        });
        match row.map(|row| row.is_current()).transpose() {
            Ok(Some(true)) => {}
            Ok(_) => {
                self.pack_song_error(deadsync_editor::Error::ExternalChange);
                return;
            }
            Err(error) => {
                self.pack_song_error(error);
                return;
            }
        }
        // The target can change while the user reads a dirty-source confirmation.
        match candidate.workspace.file().has_external_change() {
            Ok(false) => {}
            Ok(true) => {
                self.pack_song_error(deadsync_editor::Error::ExternalChange);
                return;
            }
            Err(error) => {
                self.pack_song_error(error);
                return;
            }
        }
        let mut next = self
            .pack_song_list
            .as_mut()
            .unwrap()
            .candidate
            .take()
            .unwrap();
        self.stop_audio();
        // Keep the exact original preference persistence baseline, including
        // memory-only mode. No fresh read silently rebases external changes.
        next.audio_preferences_file = self.audio_preferences_file.take();
        next.audio_commands.extend(self.take_audio_commands());
        next.status = if let Some(first) = next.song_open_warnings.first() {
            i18n::tr_fmt("Edition", "PackSongsOpenedWarnings", &[("count", &next.song_open_warnings.len().to_string()), ("chart", &(first.source_index + 1).to_string()), ("message", &first.message)])
        } else { i18n::tr("Edition", "PackSongsOpened") };
        *self = *next;
    }
    pub(super) fn pack_song_list_actors(&self, actors: &mut Vec<Actor>) {
        let Some(settings) = &self.pack_song_list else {
            return;
        };
        let width = (screen_width() - 32.0).min(1000.0);
        let height = (screen_height() - 28.0).min(480.0);
        let left = (screen_width() - width) * 0.5;
        let top = (screen_height() - height) * 0.5;
        actors.push(act!(quad: xy(left+width*0.5,top+height*0.5): zoomto(width,height): diffuse(0.08,0.1,0.15,1.0): z(8020)));
        let label = |actors: &mut Vec<Actor>, value: String, y: f32, color: [f32; 4]| {
            actors.push(act!(text: font("miso"): settext(value): align(0.0,0.0): xy(left+14.0,y): zoom(0.53): maxwidth(width-28.0): diffuse(color[0],color[1],color[2],color[3]): z(8021)));
        };
        label(
            actors,
            i18n::tr("Edition", "PackSongsTitle").to_string(),
            top + 10.0,
            [1.0; 4],
        );
        let position = settings.selection.position();
        let first = position / page_size() * page_size();
        match settings.mode {
            Mode::List => {
                label(
                    actors,
                    i18n::tr("Edition", "PackSongsColumns").to_string(),
                    top + 42.0,
                    [0.6, 0.8, 0.9, 1.0],
                );
                for (offset, &index) in settings
                    .order
                    .iter()
                    .skip(first)
                    .take(page_size())
                    .enumerate()
                {
                    let row = &settings.catalog.as_ref().unwrap().rows[index];
                    let metadata = &row.metadata;
                    let value = format!(
                        "{} {} | {} | {} | {}",
                        if first + offset == position { ">" } else { " " },
                        display_text(metadata.title.as_deref().unwrap_or(&row.folder)),
                        display_text(metadata.artist.as_deref().unwrap_or("")),
                        display_text(metadata.credit.as_deref().unwrap_or("")),
                        display_text(&metadata.ratings)
                    );
                    label(
                        actors,
                        value,
                        top + 72.0 + offset as f32 * 28.0,
                        if first + offset == position {
                            [1.0, 0.85, 0.3, 1.0]
                        } else {
                            [0.85, 0.9, 1.0, 1.0]
                        },
                    );
                }
                if settings.catalog.is_some()
                    && settings.order.is_empty()
                    && settings.culture == Some(i18n::revision())
                {
                    label(
                        actors,
                        i18n::tr("Edition", "PackSongsEmpty").to_string(),
                        top + 75.0,
                        [0.8; 4],
                    );
                }
            }
            Mode::Unsaved(index) => {
                for (i, key) in ["PackSongsKeep", "PackSongsSaveOpen", "PackSongsDiscard"]
                    .iter()
                    .enumerate()
                {
                    label(
                        actors,
                        format!(
                            "{} {}",
                            if i == index { ">" } else { " " },
                            i18n::tr("Edition", key)
                        ),
                        top + 75.0 + i as f32 * 32.0,
                        [1.0; 4],
                    );
                }
            }
            Mode::Discard(confirm) => {
                label(
                    actors,
                    i18n::tr("Edition", "PackSongsDiscardConfirm").to_string(),
                    top + 65.0,
                    [1.0, 0.8, 0.3, 1.0],
                );
                for (i, key) in ["PackSongsKeep", "PackSongsDiscardOpen"].iter().enumerate() {
                    label(
                        actors,
                        format!(
                            "{} {}",
                            if (i == 1) == confirm { ">" } else { " " },
                            i18n::tr("Edition", key)
                        ),
                        top + 110.0 + i as f32 * 32.0,
                        [1.0; 4],
                    );
                }
            }
        }
        if let Some(catalog) = &settings.catalog {
            label(
                actors,
                i18n::tr_fmt(
                    "Edition",
                    "PackSongsCount",
                    &[
                        ("count", &catalog.rows.len().to_string()),
                        ("issues", &catalog.issues.len().to_string()),
                    ],
                )
                .to_string(),
                top + height - 100.0,
                [0.7, 0.8, 0.9, 1.0],
            );
        }
        label(
            actors,
            self.status.to_string(),
            top + height - 72.0,
            [1.0, 0.8, 0.3, 1.0],
        );
        label(
            actors,
            i18n::tr("Edition", "PackSongsHelp").to_string(),
            top + height - 36.0,
            [0.7, 0.8, 0.9, 1.0],
        );
    }
}

// Presentation work is bounded even when legal source text is megabytes long.
// Original metadata remains intact in Catalog; only the visible cell is clipped.
fn display_text(value: &str) -> String {
    let mut chars = value.chars();
    let mut text: String = chars.by_ref().take(256).collect();
    if chars.next().is_some() {
        text.push_str("...");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_cells_are_unicode_safe_and_bounded() {
        assert_eq!(display_text("abc"), "abc");
        let huge = "é".repeat(100_000);
        let visible = display_text(&huge);
        assert_eq!(visible.chars().count(), 259);
        assert!(visible.ends_with("..."));
    }
    #[test]
    fn stale_source_edit_rejects_background_publication() {
        i18n::init(deadsync_assets::language::load_for_tests("en"));
        let root = tempfile::tempdir().unwrap();
        let song = root.path().join("Pack/Song/song.ssc");
        std::fs::create_dir_all(song.parent().unwrap()).unwrap();
        std::fs::write(
            &song,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&song, 0).unwrap();
        state.open_pack_song_list();
        state.workspace.execute(Command::Place(1)).unwrap();
        state.poll_pack_song_list();
        assert!(state.pack_song_list.is_none());
        assert!(!matches!(state.panel, Some(Panel::PackSongList)));
        assert!(state.workspace.file().session.is_dirty());
    }
}
