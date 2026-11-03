//! Modal, keyboard-only browsing. Header discovery never replaces the editor.
pub(super) mod application_preferences;
pub(super) mod file_open;
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
};
use std::path::PathBuf;

fn page_size() -> usize {
    ((((screen_height() - 28.0).min(480.0) - 170.0).max(28.0) / 28.0) as usize).clamp(1, 8)
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
    prepared: Option<super::pack_song_preference::Prepared>,
}
impl Settings {
    fn current(&self, state: &State) -> bool {
        Some(self.source.as_path()) == state.workspace.file().existing_path()
            && self.binding.is_current(&state.workspace.file().session)
    }
}
impl State {
    pub(super) fn pack_file_io_active(&self) -> bool {
        self.pack_song_list.as_ref().is_some_and(|settings| {
            settings.scan.is_some() || settings.loading.is_some() || settings.prepared.is_some()
        })
    }
    pub(super) fn open_pack_song_list(&mut self) {
        if self.save_as_pending_guard() {
            return;
        }
        self.stop_audio();
        let Some(source) = self
            .workspace
            .file()
            .existing_path()
            .map(std::path::Path::to_path_buf)
        else {
            self.status = i18n::tr("Edition", "FileRequiresSaveAs");
            return;
        };
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
                    prepared: None,
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
    fn pack_song_error(&mut self, error: impl crate::i18n::EditorDiagnostic) {
        self.status = i18n::tr_fmt(
            "Edition",
            "PackSongsFailed",
            &[("message", &crate::i18n::editor_error(&error))],
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
    fn complete_pack_song_preference(
        &mut self,
        orderer: &mut impl deadsync_editor::chart_order::Orderer,
    ) -> bool {
        let revision = i18n::revision();
        let Some(settings) = self.pack_song_list.as_ref() else {
            return false;
        };
        if !settings.current(self) {
            return false;
        }
        let Some(prepared) = settings.prepared.as_ref() else {
            return false;
        };
        let selected = prepared.selected(orderer);
        let Some(settings) = self.pack_song_list.as_ref() else {
            return false;
        };
        if !settings.current(self)
            || revision != i18n::revision()
            || !matches!(self.panel, Some(Panel::PackSongList))
        {
            return false;
        }
        let prepared = self
            .pack_song_list
            .as_mut()
            .unwrap()
            .prepared
            .take()
            .unwrap();
        let result = selected
            .and_then(|chart| prepared.workspace(chart))
            .and_then(|(workspace, failures)| {
                let mut candidate = State::from_workspace(workspace)?;
                candidate.song_open_warnings = failures;
                Ok(candidate)
            });
        match result {
            Ok(candidate) => {
                let row = self
                    .pack_song_list
                    .as_ref()
                    .and_then(|settings| {
                        settings.catalog.as_ref().and_then(|catalog| {
                            settings
                                .order
                                .get(settings.selection.position())
                                .and_then(|&i| catalog.rows.get(i))
                        })
                    })
                    .cloned();
                match row {
                    Some(row) => self.command_open_prepared_pack_song(candidate, row),
                    None => self.pack_song_error(deadsync_editor::Error::ExternalChange),
                }
            }
            Err(error) => self.pack_song_error(error),
        }
        true
    }
    /// Main-thread cultural boundary. Call on frames as well as raw-key dispatch:
    /// directory workers may finish without another keyboard event.
    pub fn process_pack_song_list(
        &mut self,
        orderer: &mut impl Orderer,
        chart_orderer: &mut impl deadsync_editor::chart_order::Orderer,
    ) -> bool {
        let file_completed = self.process_file_menu(chart_orderer);
        if !matches!(self.panel, Some(Panel::PackSongList)) {
            return file_completed;
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
                if settings.loading.is_none() {
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
        if code == KeyCode::F5
            && settings.scan.is_none()
            && settings.loading.is_none()
            && settings.prepared.is_none()
        {
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
        let preferences = match self.application_opening_preferences() {
            Ok(preferences) => preferences,
            Err(error) => {
                self.pack_song_error(error);
                return;
            }
        };
        match pack_song_load::Job::spawn_with_profiles(
            row,
            preferences,
            self.profile_directory.clone(),
        ) {
            Ok(job) => {
                self.pack_song_list.as_mut().unwrap().loading = Some(job);
                self.status = i18n::tr("Edition", "PackSongsOpening");
            }
            Err(error) => self.pack_song_error(error),
        }
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
            let ui_row_start = actors.len();
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

            self.ui_row(actors, ui_row_start, first + offset == position, 28.);
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
    use deadsync_editor::workspace::Command;
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
    const STRICT: &str = "#VERSION:0.83;#TITLE:Candidate;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fn row(path: &std::path::Path) -> deadsync_editor::pack::song_list::Row {
        use deadsync_editor::pack::song_list::HeaderReader;
        let catalog = Task::from_song_file(path, Limits::default())
            .unwrap()
            .run(
                &HeaderReader,
                &std::sync::atomic::AtomicBool::new(false),
                &mut |_| {},
            )
            .unwrap();
        let canonical = path.canonicalize().unwrap();
        catalog
            .rows
            .into_iter()
            .find(|r| r.path == canonical)
            .unwrap()
    }
    fn transition_fixture(other_pack: bool) -> (tempfile::TempDir, State, PathBuf) {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("Pack/A/song.ssc");
        let target = root.path().join(if other_pack {
            "Other/B/song.ssc"
        } else {
            "Pack/B/song.ssc"
        });
        for path in [&source, &target] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, STRICT).unwrap();
        }
        std::fs::write(
            root.path().join("Pack/Pack.ini"),
            b"[Group]\nVersion=1\nDisplayTitle=Original\n",
        )
        .unwrap();
        let mut state = State::open(&source, 0).unwrap();
        state.open_pack_properties();
        state
            .pack_properties
            .as_mut()
            .unwrap()
            .set_text(super::super::pack_properties::Field::Title, "Unsaved pack")
            .unwrap();
        state.panel = None;
        (root, state, target)
    }
    #[test]
    fn common_prepared_open_retains_same_pack_dirty_metadata_and_exact_identity() {
        let (root, mut state, target) = transition_fixture(false);
        let guard = state.capture_pack_coupling();
        let before = std::fs::read(root.path().join("Pack/Pack.ini")).unwrap();
        let candidate = State::open(&target, 0).unwrap();
        state.command_open_prepared_pack_song(candidate, row(&target));
        assert_eq!(
            state.workspace.file().existing_path(),
            Some(target.canonicalize().unwrap().as_path())
        );
        assert!(state.pack_has_unsaved_changes());
        assert!(state.pack_coupling_current(&guard));
        assert!(state.pack_text_current());
        assert_eq!(
            std::fs::read(root.path().join("Pack/Pack.ini")).unwrap(),
            before
        );
    }
    #[test]
    fn common_prepared_other_pack_open_uses_save_discard_cancel_once() {
        for choice in [0, 1, 2] {
            let (root, mut state, target) = transition_fixture(true);
            let source = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            let before = std::fs::read(root.path().join("Pack/Pack.ini")).unwrap();
            state.command_open_prepared_pack_song(State::open(&target, 0).unwrap(), row(&target));
            assert!(matches!(state.panel, Some(Panel::FileUnsaved(0))));
            assert!(!state.file_menu_key(choice, KeyCode::Enter));
            if choice == 0 {
                assert_eq!(
                    state.workspace.file().existing_path(),
                    Some(source.as_path())
                );
                assert!(state.pack_has_unsaved_changes());
            } else {
                if choice == 2 {
                    assert!(matches!(state.panel, Some(Panel::FileDiscard)));
                    state.file_menu_key(0, KeyCode::Enter);
                }
                assert_eq!(
                    state.workspace.file().existing_path(),
                    Some(target.canonicalize().unwrap().as_path())
                );
                assert!(state.pack_properties.is_none());
            }
            assert_eq!(
                std::fs::read(root.path().join("Pack/Pack.ini")).unwrap() == before,
                choice != 1
            );
        }
    }
    #[test]
    fn common_prepared_target_change_and_pack_edit_return_revoke_publication() {
        for mutate_pack in [false, true] {
            let (_root, mut state, target) = transition_fixture(true);
            let source = state
                .workspace
                .file()
                .require_existing_path()
                .unwrap()
                .to_owned();
            state.command_open_prepared_pack_song(State::open(&target, 0).unwrap(), row(&target));
            if mutate_pack {
                let settings = state.pack_properties.as_mut().unwrap();
                settings
                    .set_text(super::super::pack_properties::Field::Title, "Changed again")
                    .unwrap();
                settings
                    .set_text(super::super::pack_properties::Field::Title, "Unsaved pack")
                    .unwrap();
            } else {
                std::fs::write(&target, STRICT.replace("Candidate", "External target")).unwrap();
            }
            state.file_menu_key(2, KeyCode::Enter);
            state.file_menu_key(0, KeyCode::Enter);
            assert_eq!(
                state.workspace.file().existing_path(),
                Some(source.as_path())
            );
            assert!(state.pack_has_unsaved_changes());
        }
    }

    #[test]
    fn prepared_pack_candidate_must_match_the_exact_catalog_row() {
        let (_root, mut state, target) = transition_fixture(false);
        let source = state
            .workspace
            .file()
            .require_existing_path()
            .unwrap()
            .to_owned();
        let wrong = row(&source);
        state.command_open_prepared_pack_song(State::open(&target, 0).unwrap(), wrong);
        assert_eq!(
            state.workspace.file().existing_path(),
            Some(source.as_path())
        );
        assert!(state.pack_has_unsaved_changes());
    }
}
