//! Actions shared by the native screen and functional workflow tests. The UI
//! dispatches these actions instead of mutating notes or file state itself.
use crate::{
    Error,
    history::Clipboard,
    notes::{Grid, Kind, Note, Position},
    storage::FileSession,
};
use std::path::Path;

pub const SNAP_ROWS: [u64; 9] = [48, 24, 16, 12, 8, 6, 4, 3, 1];
pub const SNAP_NAMES: [&str; 9] = [
    "1/4", "1/8", "1/12", "1/16", "1/24", "1/32", "1/48", "1/64", "1/192",
];
pub const TOOLS: [Kind; 6] = [
    Kind::Tap,
    Kind::Hold,
    Kind::Roll,
    Kind::Mine,
    Kind::Lift,
    Kind::Fake,
];

#[derive(Clone, Copy, Debug)]
pub enum Command {
    Move(i64),
    Seek(u64),
    PreviousMeasure,
    NextMeasure,
    DuplicateChart,
    PreviousLabel,
    NextLabel,
    SetSnap(usize),
    SelectChart(usize),
    SelectPlayer(u32),
    SelectTool(Kind),
    ToggleAutoAdvance,
    ToggleSelection,
    Place(u16),
    Delete,
    Copy,
    Cut,
    Paste,
    CopyEvents,
    CutEvents,
    PasteEvents,
    Undo,
    Redo,
    Save,
    SaveRecovery,
    Recover,
    Transform(crate::transform::Transform),
}

struct StructuralFocus {
    before_revision: u64,
    before_chart: usize,
    after_chart: usize,
}

pub struct Workspace {
    file: FileSession,
    chart: usize,
    row: u64,
    snap: usize,
    player: u32,
    tool: Kind,
    anchor: Option<u64>,
    auto_advance: bool,
    clipboard: Option<Clipboard>,
    event_clipboard: Option<crate::event_clipboard::Clipboard>,
    structural_focus: std::collections::BTreeMap<u64, StructuralFocus>,
}

impl Workspace {
    pub fn open(path: &Path, chart: usize) -> Result<Self, Error> {
        Self::from_file(FileSession::open(path)?, chart)
    }

    pub fn from_file(mut file: FileSession, chart: usize) -> Result<Self, Error> {
        let chart = if chart == 0
            && file
                .session
                .document()
                .charts()
                .iter()
                .all(|c| crate::notes::layout_columns(&c.steps_type).is_none())
        {
            usize::MAX
        } else {
            file.session.grid(chart)?;
            chart
        };
        Ok(Self {
            file,
            chart,
            row: 0,
            snap: 0,
            player: 0,
            tool: Kind::Tap,
            anchor: None,
            auto_advance: false,
            clipboard: None,
            event_clipboard: None,
            structural_focus: Default::default(),
        })
    }

    pub fn change_note_owners(&mut self, count: u32, transform: crate::transform::Transform) -> Result<bool, Error> {
        let chart = self.chart;
        let (start, end) = self.selection().unwrap_or((self.row, self.row));
        let changed = self.file.session.edit_chart_atomic(chart, "Change note players", |staged| {
            staged.set_participants(chart, count)?;
            crate::transform::apply(staged, chart, start, end, None, transform)?;
            Ok(())
        })?;
        self.sync_active_chart()?;
        Ok(changed)
    }
    pub fn event_selection(&mut self) -> Result<crate::event_clipboard::Selection, Error> {
        let (start, end) = self.selection().unwrap_or((self.row, self.row));
        crate::event_clipboard::Selection::capture(&mut self.file.session, self.chart, start, end)
    }
    fn validate_event_selection(&self, selection: &crate::event_clipboard::Selection) -> Result<(), Error> {
        if selection.chart() == self.chart { Ok(()) } else { Err(Error::InvalidEdit("Event selection belongs to another chart".into())) }
    }
    pub fn copy_event_selection(&mut self, selection: &crate::event_clipboard::Selection) -> Result<(), Error> {
        self.validate_event_selection(selection)?;
        if let Some(clipboard) = selection.copy(&mut self.file.session)? { self.event_clipboard = Some(clipboard); }
        Ok(())
    }
    pub fn cut_event_selection(&mut self, selection: &crate::event_clipboard::Selection) -> Result<(), Error> {
        self.validate_event_selection(selection)?;
        if let Some(clipboard) = selection.cut(&mut self.file.session)? { self.event_clipboard = Some(clipboard); }
        self.sync_active_chart()
    }
    pub fn delete_event_selection(&mut self, selection: &crate::event_clipboard::Selection) -> Result<bool, Error> {
        self.validate_event_selection(selection)?;
        let changed = selection.delete(&mut self.file.session)?;
        self.sync_active_chart()?;
        Ok(changed)
    }

    pub fn set_participants(&mut self, count: u32) -> Result<bool, Error> {
        let changed = self.file.session.set_participants(self.chart, count)?;
        self.sync_active_chart()?;
        Ok(changed)
    }

    pub fn create_profile(&mut self, directory: &Path, profile: &crate::generation::profiles::Profile) -> Result<std::path::PathBuf, Error> {
        self.file.session.create_profile(directory, profile)
    }

    pub fn delete_profile(&mut self, file: &crate::generation::profiles::ProfileFile) -> Result<(), Error> {
        self.file.session.delete_profile(file)
    }

    pub fn file(&self) -> &FileSession {
        &self.file
    }
    pub fn active_chart(&self) -> Option<usize> {
        (self.chart < self.file.session.document().charts().len()).then_some(self.chart)
    }
    pub fn has_chart(&self) -> bool {
        self.active_chart().is_some()
    }
    pub fn try_grid(&self) -> Option<&Grid> {
        self.active_chart()
            .and_then(|chart| self.file.session.loaded_grid(chart))
    }
    /// Requires an active chart. Empty-workspace callers should use try_grid.
    pub fn grid(&self) -> &Grid {
        self.file
            .session
            .loaded_grid(self.chart)
            .expect("Active editor chart is loaded")
    }
    /// Legacy active index. Empty-aware callers must use active_chart instead.
    pub fn chart(&self) -> usize {
        self.chart
    }

    pub fn row(&self) -> u64 {
        self.row
    }
    pub fn snap(&self) -> usize {
        self.snap
    }
    pub fn player(&self) -> u32 {
        self.player
    }
    pub fn tool(&self) -> Kind {
        self.tool
    }
    pub fn auto_advance(&self) -> bool {
        self.auto_advance
    }
    pub fn selection(&self) -> Option<(u64, u64)> {
        self.anchor
            .map(|anchor| (anchor.min(self.row), anchor.max(self.row)))
    }

    pub fn execute(&mut self, command: Command) -> Result<(), Error> {
        if !self.has_chart() && !matches!(command, Command::Undo | Command::Redo | Command::Save | Command::SaveRecovery | Command::Recover | Command::SelectChart(_)) {
            return Err(Error::InvalidEdit("No active chart".into()));
        }
        match command {
            Command::Move(steps) => {
                let distance = SNAP_ROWS[self.snap]
                    .checked_mul(steps.unsigned_abs())
                    .ok_or_else(|| Error::InvalidEdit("Cursor movement overflow".into()))?;
                self.row = if steps >= 0 {
                    self.row.saturating_add(distance)
                } else {
                    self.row.saturating_sub(distance)
                };
            }
            Command::Seek(row) => self.row = row,
            Command::DuplicateChart => {
                self.duplicate_chart()?;
            }
            Command::PreviousMeasure | Command::NextMeasure => {
                let signatures = crate::timing_edit::signature::Signatures::from_document(
                    self.file.session.document(),
                    self.chart,
                )?;
                let destination = signatures.measure_destination(
                    self.row,
                    matches!(command, Command::PreviousMeasure),
                )?;
                // Workspace note positions are currently nonnegative reference rows.
                self.row = destination.clamp(0, i32::MAX as i64) as u64;
            }
            Command::PreviousLabel | Command::NextLabel => {
                let labels = crate::timing_edit::label::Labels::from_document(
                    self.file.session.document(),
                    self.chart,
                )?;
                let row = if matches!(command, Command::PreviousLabel) {
                    labels.previous_row(self.row)
                } else {
                    labels.next_row(self.row)
                };
                if let Some(row) = row {
                    self.row = row;
                }
            }
            Command::SetSnap(snap) => {
                if snap >= SNAP_ROWS.len() {
                    return Err(Error::InvalidEdit("Invalid quantization".into()));
                }
                self.snap = snap;
                self.row -= self.row % SNAP_ROWS[snap];
            }
            Command::SelectChart(chart) => {
                let grid = self.file.session.grid(chart)?;
                self.player = self.player.min(grid.players() - 1);
                self.chart = chart;
                self.anchor = None;
            }
            Command::SelectPlayer(player) => {
                if player >= self.grid().players() {
                    return Err(Error::InvalidEdit(
                        "Participant does not exist in this chart".into(),
                    ));
                }
                self.player = player;
            }
            Command::SelectTool(tool) => {
                if !TOOLS.contains(&tool) {
                    return Err(Error::Unsupported(
                        "This tool needs an attribute inspector which is not implemented yet"
                            .into(),
                    ));
                }
                self.tool = tool;
            }
            Command::ToggleAutoAdvance => self.auto_advance = !self.auto_advance,
            Command::ToggleSelection => {
                self.anchor = if self.anchor.is_some() {
                    None
                } else {
                    Some(self.row)
                }
            }
            Command::Place(lane) => {
                let head_row = if matches!(self.tool, Kind::Hold | Kind::Roll) {
                    self.selection().map_or(self.row, |(start, _)| start)
                } else {
                    self.row
                };
                let position = Position {
                    player: self.player,
                    lane,
                    row: head_row,
                };
                let note = if self.grid().get(position).is_some() {
                    None
                } else {
                    let end = if matches!(self.tool, Kind::Hold | Kind::Roll) {
                        Some(
                            self.selection()
                                .map(|(_, end)| end)
                                .filter(|&end| end > head_row)
                                .unwrap_or(head_row.checked_add(SNAP_ROWS[self.snap]).ok_or_else(
                                    || Error::InvalidEdit("Sustain exceeds row range".into()),
                                )?),
                        )
                    } else {
                        None
                    };
                    Some(Note {
                        kind: self.tool,
                        end,
                        ..Note::tap()
                    })
                };
                self.file
                    .session
                    .edit_notes(self.chart, "Place note", [(position, note)])?;
                if self.auto_advance {
                    self.row = self.row.saturating_add(SNAP_ROWS[self.snap]);
                }
            }
            Command::Delete => self.delete_selection()?,
            Command::Copy | Command::Cut => {
                let (start, end) = self.selection().unwrap_or((self.row, self.row));
                let clipboard = Clipboard::copy(self.grid(), start, end, None)?;
                if matches!(command, Command::Cut) {
                    self.delete_selection()?;
                }
                self.clipboard = Some(clipboard);
            }
            Command::Paste => {
                if let Some(clipboard) = &self.clipboard {
                    clipboard.paste_force(&mut self.file.session, self.chart, self.row, self.player)?;
                }
            }
            Command::CopyEvents | Command::CutEvents => {
                let (start, end) = self.selection().unwrap_or((self.row, self.row));
                let clipboard = if matches!(command, Command::CutEvents) {
                    crate::event_clipboard::Clipboard::cut(&mut self.file.session, self.chart, start, end)?
                } else {
                    crate::event_clipboard::Clipboard::copy(&mut self.file.session, self.chart, start, end)?
                };
                if let Some(clipboard) = clipboard { self.event_clipboard = Some(clipboard); }
            }
            Command::PasteEvents => {
                if let Some(clipboard) = &self.event_clipboard {
                    clipboard.paste(&mut self.file.session, self.chart, self.row, self.player)?;
                }
            }
            Command::Undo => {
                let revision = self.file.session.revision();
                if self.file.session.undo()? {
                    if let Some(focus) = self
                        .structural_focus
                        .get(&revision)
                        .filter(|focus| focus.before_revision == self.file.session.revision())
                    {
                        self.chart = focus.before_chart;
                    }
                }
                self.sync_active_chart()?;
            }
            Command::Redo => {
                let revision = self.file.session.revision();
                if self.file.session.redo()? {
                    if let Some(focus) = self
                        .structural_focus
                        .get(&self.file.session.revision())
                        .filter(|focus| focus.before_revision == revision)
                    {
                        self.chart = focus.after_chart;
                    }
                }
                self.sync_active_chart()?;
            }
            Command::Save => self.file.save()?,
            Command::SaveRecovery => self.file.save_recovery()?,
            Command::Recover => {
                self.file.recover()?;
                self.structural_focus.clear();
                self.sync_active_chart()?;
            }
            Command::Transform(transform) => {
                let (start, end) = self.selection().unwrap_or((self.row, self.row));
                crate::transform::apply(
                    &mut self.file.session,
                    self.chart,
                    start,
                    end,
                    None,
                    transform,
                )?;
            }
        }
        Ok(())
    }

    pub fn generation_task(
        &self,
        options: crate::generation::job::ChartOptions,
    ) -> Result<crate::generation::job::ChartTask, Error> {
        crate::generation::job::ChartTask::capture(&self.file.session, self.chart, options)
    }

    pub fn generation_task_using_chart_expression(
        &self,
        options: crate::generation::job::ChartOptions,
        directory: Option<&Path>,
    ) -> Result<crate::generation::job::ChartTask, Error> {
        self.generation_task(options)?
            .with_chart_expression(directory)
    }

    pub fn conversion_profile_task(
        &self,
        directory: Option<&Path>,
    ) -> Result<crate::generation::profile_catalog::Task, Error> {
        crate::generation::profile_catalog::Task::capture(&self.file.session, self.chart, directory)
    }
    pub fn select_conversion_profiles(
        &self,
        catalog: &crate::generation::profile_catalog::Catalog,
        selection: crate::generation::job::ProfileOverrides,
    ) -> Result<crate::generation::job::ProfileOverrides, Error> {
        catalog.conversion(&self.file.session, self.chart, selection)
    }
    pub fn generation_task_with_profiles(
        &self,
        options: crate::generation::job::ChartOptions,
        directory: Option<&Path>,
        profiles: crate::generation::job::ProfileOverrides,
    ) -> Result<crate::generation::job::ChartTask, Error> {
        self.generation_task(options)?
            .with_profile_overrides(directory, profiles)
    }

    pub fn pattern_task(
        &self,
        options: crate::generation::pattern_job::Options,
    ) -> Result<crate::generation::pattern_job::Task, Error> {
        crate::generation::pattern_job::Task::capture(&self.file.session, self.chart, options)
    }

    pub fn persistent_pattern_task(
        &self,
        options: crate::generation::pattern_job::Options,
        directory: Option<&Path>,
    ) -> Result<crate::generation::pattern_job::Task, Error> {
        crate::generation::pattern_job::Task::capture_persistent(
            &self.file.session,
            self.chart,
            options,
            directory,
        )
        .map(|task| task.with_chart_expression())
    }

    pub fn timing_migration(&self) -> Result<crate::timing_edit::migration::Draft, Error> {
        crate::timing_edit::migration::Draft::capture(&self.file.session)
    }
    pub fn apply_timing_migration(
        &mut self,
        draft: &crate::timing_edit::migration::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session)
    }

    pub fn region_draft(
        &self,
        scope: crate::timing_edit::Scope,
    ) -> Result<crate::timing_edit::region::Draft, Error> {
        crate::timing_edit::region::Draft::capture(&self.file.session, self.chart, scope)
    }
    pub fn apply_regions(
        &mut self,
        draft: &crate::timing_edit::region::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }

    pub fn pause_draft(
        &self,
        scope: crate::timing_edit::Scope,
    ) -> Result<crate::timing_edit::pause::Draft, Error> {
        crate::timing_edit::pause::Draft::capture(&self.file.session, self.chart, scope)
    }
    pub fn apply_pauses(
        &mut self,
        draft: &crate::timing_edit::pause::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }

    pub fn tempo_draft(
        &self,
        scope: crate::timing_edit::Scope,
    ) -> Result<crate::timing_edit::TempoDraft, Error> {
        crate::timing_edit::TempoDraft::capture(&self.file.session, self.chart, scope)
    }
    pub fn apply_tempo(&mut self, draft: &crate::timing_edit::TempoDraft) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }

    pub fn apply_labels(&mut self, draft: &crate::timing_edit::label::Draft) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn apply_speeds(
        &mut self,
        draft: &crate::timing_edit::speed::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn apply_combos(
        &mut self,
        draft: &crate::timing_edit::combo::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn apply_ticks(
        &mut self,
        draft: &crate::timing_edit::tick::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn apply_scrolls(
        &mut self,
        draft: &crate::timing_edit::scroll::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn apply_signatures(
        &mut self,
        draft: &crate::timing_edit::signature::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn chart_properties(&self) -> Result<crate::chart_properties::Draft, Error> {
        crate::chart_properties::Draft::capture(&self.file.session, self.chart)
    }
    pub fn attack_inspector(
        &self,
        scope: crate::timing_edit::Scope,
    ) -> Result<crate::attacks::inspector::Draft, Error> {
        crate::attacks::inspector::Draft::capture(&self.file.session, self.chart(), scope)
    }
    pub fn apply_attack_inspector(
        &mut self,
        draft: &crate::attacks::inspector::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn attacks(&self, scope: crate::timing_edit::Scope) -> Result<crate::attacks::Draft, Error> {
        crate::attacks::Draft::capture(&self.file.session, self.chart(), scope)
    }
    pub fn apply_attacks(&mut self, draft: &crate::attacks::Draft) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn note_attributes(&self, lane: u16) -> Result<crate::note_attributes::Draft, Error> {
        let position = crate::notes::Position {
            row: self.row(), lane, player: self.player(),
        };
        let note = self.grid().get(position)
            .ok_or_else(|| Error::InvalidEdit("No note head at this position".into()))?;
        Ok(crate::note_attributes::Draft::from_note(
            &self.file.session, self.chart(), position, note,
        ))
    }
    pub fn apply_note_attributes(&mut self, draft: &crate::note_attributes::Draft) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn song_properties(&self) -> Result<crate::song_properties::Draft, Error> {
        crate::song_properties::Draft::capture(&self.file.session)
    }
    pub fn apply_song_properties(
        &mut self,
        draft: &crate::song_properties::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session)
    }
    pub fn apply_chart_properties(
        &mut self,
        draft: &crate::chart_properties::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn chart_expression_task(
        &self,
        draft: &crate::chart_properties::Draft,
        directory: Option<&Path>,
    ) -> Result<crate::generation::profile_catalog::Task, Error> {
        draft.profile_task(&self.file.session, self.chart, directory)
    }
    pub fn select_chart_expression(
        &self,
        draft: &mut crate::chart_properties::Draft,
        catalog: &crate::generation::profile_catalog::Catalog,
        expression: &crate::generation::metadata::Guid,
    ) -> Result<(), Error> {
        draft.select_expression(&self.file.session, self.chart, catalog, expression)
    }

    pub fn edit_saved_pattern(&self) -> Result<crate::generation::marker_edit::Draft, Error> {
        crate::generation::marker_edit::Draft::at_cursor(&self.file.session, self.chart, self.row)
    }

    pub fn apply_marker_edit(
        &mut self,
        draft: &crate::generation::marker_edit::Draft,
        delete: bool,
    ) -> Result<bool, Error> {
        if delete {
            draft.delete(&mut self.file.session, self.chart)
        } else {
            draft.apply(&mut self.file.session, self.chart)
        }
    }

    pub fn marker_profile_task(
        &self,
        draft: &crate::generation::marker_edit::Draft,
        directory: Option<&Path>,
    ) -> Result<crate::generation::profile_catalog::Task, Error> {
        draft.profile_task(&self.file.session, self.chart, directory)
    }
    pub fn select_marker_profiles(
        &self,
        draft: &mut crate::generation::marker_edit::Draft,
        catalog: &crate::generation::profile_catalog::Catalog,
        pattern: &crate::generation::metadata::Guid,
        performed: &crate::generation::metadata::Guid,
    ) -> Result<(), Error> {
        draft.select_profiles(&self.file.session, self.chart, catalog, pattern, performed)
    }

    pub fn select_marker_generation_profiles(
        &self,
        draft: &mut crate::generation::marker_edit::Draft,
        catalog: &crate::generation::profile_catalog::Catalog,
        pattern: &crate::generation::metadata::Guid,
        performed: &crate::generation::metadata::Guid,
        expressed: &crate::generation::metadata::Guid,
    ) -> Result<(), Error> {
        draft.select_generation_profiles(
            &self.file.session,
            self.chart,
            catalog,
            pattern,
            performed,
            expressed,
        )
    }

    pub fn clear_pattern_task(
        &self,
        draft: &crate::generation::marker_edit::Draft,
        directory: Option<&Path>,
    ) -> Result<crate::generation::clear_patterns::Task, Error> {
        draft.clear_task(&self.file.session, self.chart, directory)
    }
    pub fn apply_cleared_pattern(
        &mut self,
        preview: &crate::generation::clear_patterns::Preview,
    ) -> Result<bool, Error> {
        let changed = preview.apply(&mut self.file.session, self.chart)?;
        self.anchor = None;
        Ok(changed)
    }
    pub fn navigate_saved_pattern(&mut self, forward: bool) -> Result<bool, Error> {
        let data =
            crate::generation::metadata::ChartData::read(self.file.session.document(), self.chart)?;
        if let Some(row) = data.and_then(|data| data.adjacent_marker(self.row, forward)) {
            self.row = row;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn saved_pattern_task(
        &self,
        registry: &crate::generation::metadata::Registry,
    ) -> Result<crate::generation::pattern_job::Task, Error> {
        let data =
            crate::generation::metadata::ChartData::read(self.file.session.document(), self.chart)?
                .ok_or_else(|| {
                    Error::InvalidEdit("No saved pattern markers in this chart".into())
                })?;
        let row = data
            .marker_at(self.row)
            .map(|(row, _)| row)
            .ok_or_else(|| Error::InvalidEdit("No saved pattern marker at the cursor".into()))?;
        crate::generation::pattern_job::Task::capture(
            &self.file.session,
            self.chart,
            data.options(row, registry)?,
        )
    }

    pub fn saved_pattern_task_from_directory(
        &self,
        directory: &Path,
    ) -> Result<crate::generation::pattern_job::SavedTask, Error> {
        let metadata =
            crate::generation::metadata::ChartData::read(self.file.session.document(), self.chart)?
                .ok_or_else(|| {
                    Error::InvalidEdit("No saved pattern markers in this chart".into())
                })?;
        let row = metadata
            .marker_at(self.row)
            .map(|(row, _)| row)
            .ok_or_else(|| Error::InvalidEdit("No saved pattern marker at the cursor".into()))?;
        crate::generation::pattern_job::SavedTask::capture(
            &self.file.session,
            self.chart,
            row,
            directory,
        )
    }

    pub fn apply_generated_pattern(
        &mut self,
        preview: &crate::generation::pattern_job::Preview,
    ) -> Result<bool, Error> {
        let changed = preview.apply(&mut self.file.session, self.chart)?;
        self.file.session.grid(self.chart)?;
        self.anchor = None;
        Ok(changed)
    }

    pub fn pattern_set_task(
        &self,
        rows: impl IntoIterator<Item = u64>,
        directory: Option<&Path>,
    ) -> Result<crate::generation::pattern_batch::Task, Error> {
        crate::generation::pattern_batch::Task::capture(
            &self.file.session,
            self.chart,
            rows,
            directory,
        )
    }
    pub fn all_patterns_task(
        &self,
        directory: Option<&Path>,
    ) -> Result<crate::generation::pattern_batch::Task, Error> {
        crate::generation::pattern_batch::Task::capture_all(
            &self.file.session,
            self.chart,
            directory,
        )
    }
    pub fn apply_pattern_set(
        &mut self,
        preview: &crate::generation::pattern_batch::Preview,
    ) -> Result<bool, Error> {
        let changed = preview.apply(&mut self.file.session, self.chart)?;
        self.file.session.grid(self.chart)?;
        self.anchor = None;
        Ok(changed)
    }

    pub fn batch_generation_task(
        &self,
        source_layout: &str,
        options: crate::generation::job::ChartOptions,
        directory: Option<&Path>,
        profiles: crate::generation::job::ProfileOverrides,
    ) -> Result<crate::generation::batch_job::Task, Error> {
        crate::generation::batch_job::Task::capture(
            &self.file.session,
            source_layout,
            options,
            directory,
            profiles,
        )
    }
    pub fn apply_batch_generated(
        &mut self,
        preview: &crate::generation::batch_job::Preview,
    ) -> Result<Vec<usize>, Error> {
        let before_revision = self.file.session.revision();
        let before_chart = self.chart;
        let indices = preview.apply(&mut self.file.session)?;
        let chart = indices[0];
        self.file.session.grid(chart)?;
        self.structural_focus.insert(
            self.file.session.revision(),
            StructuralFocus {
                before_revision,
                before_chart,
                after_chart: chart,
            },
        );
        self.chart = chart;
        self.player = 0;
        self.anchor = None;
        Ok(indices)
    }

    pub fn deletion_draft(&self) -> Result<crate::chart_deletion::Draft, Error> {
        crate::chart_deletion::Draft::capture(&self.file.session, self.chart)
    }
    pub fn delete_chart(&mut self, draft: &crate::chart_deletion::Draft) -> Result<(), Error> {
        draft.check(&self.file.session, self.chart)?;
        let before_revision = self.file.session.revision();
        let before_chart = self.chart;
        let current = crate::document::Document::parse(
            self.file.session.serialize()?,
            self.file.session.document().format,
        )?;
        let next = crate::chart_deletion::successor(&current, self.chart);
        if let Some(chart) = next {
            self.file.session.grid(chart)?;
        }
        draft.apply(&mut self.file.session)?;
        self.chart = next.map_or(usize::MAX, |chart| {
            if chart > before_chart {
                chart - 1
            } else {
                chart
            }
        });
        self.structural_focus.insert(
            self.file.session.revision(),
            StructuralFocus {
                before_revision,
                before_chart,
                after_chart: self.chart,
            },
        );
        self.player = 0;
        self.anchor = None;
        if self.has_chart() {
            self.file.session.grid(self.chart)?;
        }
        Ok(())
    }

    pub fn new_chart_draft(&self) -> Result<crate::chart_creation::Draft, Error> {
        crate::chart_creation::Draft::capture(&self.file.session)
    }
    pub fn create_chart(
        &mut self,
        draft: &crate::chart_creation::Draft,
        layout: &str,
    ) -> Result<usize, Error> {
        let before_revision = self.file.session.revision();
        let before_chart = self.chart;
        let chart = draft.apply(&mut self.file.session, layout)?;
        self.file.session.grid(chart)?;
        self.structural_focus.insert(
            self.file.session.revision(),
            StructuralFocus {
                before_revision,
                before_chart,
                after_chart: chart,
            },
        );
        self.chart = chart;
        self.row = 0;
        self.player = 0;
        self.anchor = None;
        Ok(chart)
    }

    pub fn duplicate_chart(&mut self) -> Result<usize, Error> {
        let before_revision = self.file.session.revision();
        let before_chart = self.chart;
        let chart = self
            .file
            .session
            .duplicate_chart(self.chart, before_revision)?;
        self.file.session.grid(chart)?;
        self.structural_focus.insert(
            self.file.session.revision(),
            StructuralFocus {
                before_revision,
                before_chart,
                after_chart: chart,
            },
        );
        self.chart = chart;
        self.player = 0;
        self.anchor = None;
        Ok(chart)
    }

    pub fn apply_generated(
        &mut self,
        preview: &crate::generation::job::ChartPreview,
    ) -> Result<usize, Error> {
        if self.chart != preview.source_chart() {
            return Err(Error::InvalidEdit(
                "Select the source chart before applying its generation preview".into(),
            ));
        }
        let before_revision = self.file.session.revision();
        let before_chart = self.chart;
        let chart = preview.apply(&mut self.file.session)?;
        self.file.session.grid(chart)?;
        self.structural_focus.insert(
            self.file.session.revision(),
            StructuralFocus {
                before_revision,
                before_chart,
                after_chart: chart,
            },
        );
        self.chart = chart;
        self.player = 0;
        self.anchor = None;
        Ok(chart)
    }

    fn sync_active_chart(&mut self) -> Result<(), Error> {
        if self.file.session.document().charts().is_empty() {
            self.chart = usize::MAX;
            self.player = 0;
            self.anchor = None;
            return Ok(());
        }
        if self.chart == usize::MAX {
            let candidate = self
                .file
                .session
                .document()
                .charts()
                .iter()
                .position(|c| crate::notes::layout_columns(&c.steps_type).is_some());
            let Some(chart) = candidate else {
                self.player = 0;
                self.anchor = None;
                return Ok(());
            };
            self.chart = chart;
        }
        let previous_chart = self.chart;
        self.chart = self.chart.min(
            self.file
                .session
                .document()
                .charts()
                .len()
                .saturating_sub(1),
        );
        let grid = self.file.session.grid(self.chart)?;
        self.player = self.player.min(grid.players() - 1);
        if self.chart != previous_chart {
            self.anchor = None;
        }
        Ok(())
    }

    fn delete_selection(&mut self) -> Result<(), Error> {
        let (start, end) = self.selection().unwrap_or((self.row, self.row));
        let edits: Vec<_> = self
            .grid()
            .iter()
            .filter(|(position, _)| position.row >= start && position.row <= end)
            .map(|(position, _)| (position, None))
            .collect();
        self.file
            .session
            .edit_notes(self.chart, "Delete selection", edits)?;
        Ok(())
    }
}
