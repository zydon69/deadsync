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
    structural_focus: std::collections::BTreeMap<u64, StructuralFocus>,
}

impl Workspace {
    pub fn open(path: &Path, chart: usize) -> Result<Self, Error> {
        Self::from_file(FileSession::open(path)?, chart)
    }

    pub fn from_file(mut file: FileSession, chart: usize) -> Result<Self, Error> {
        file.session.grid(chart)?;
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
            structural_focus: Default::default(),
        })
    }

    pub fn file(&self) -> &FileSession {
        &self.file
    }
    pub fn grid(&self) -> &Grid {
        self.file
            .session
            .loaded_grid(self.chart)
            .expect("Active editor chart is loaded")
    }
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
                let chart = self.chart.min(
                    self.file
                        .session
                        .document()
                        .charts()
                        .len()
                        .saturating_sub(1),
                );
                self.file.session.grid(chart)?;
                self.chart = chart;
                self.player = self.player.min(self.grid().players() - 1);
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
            self.anchor = None;
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
