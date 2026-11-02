//! Actions shared by the native screen and functional workflow tests. The UI
//! dispatches these actions instead of mutating notes or file state itself.
mod save_as;
use crate::{
    Error,
    history::Clipboard,
    notes::{Grid, Kind, Note, Position},
    storage::FileSession,
};
pub use save_as::WorkspaceSaveAsReview;
use std::path::Path;

pub const SNAP_ROWS: [u64; 10] = [48, 24, 16, 12, 8, 6, 4, 3, 1, 0];
/// Increment for discrete edit tools, distinct from Off measure navigation.
pub fn snap_edit_rows(index: usize) -> u64 {
    SNAP_ROWS[index].max(1)
}
/// Source Editor.GetShiftNotesRows: Off shifts one beat; insertion keeps its own minimum.
pub fn snap_shift_rows(index: usize) -> u64 {
    if SNAP_ROWS[index] == 0 {
        crate::notes::ROWS_PER_BEAT
    } else {
        SNAP_ROWS[index]
    }
}
pub const SNAP_NAMES: [&str; 10] = [
    "1/4", "1/8", "1/12", "1/16", "1/24", "1/32", "1/48", "1/64", "1/192", "Off",
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
    NormalizeStepF2,
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

/// Detached application clipboard values. No document/session identity or
/// mutable source reference survives a File/Open, Close, or pack switch.
#[derive(Clone, Default)]
pub struct Clipboards {
    notes: Option<Clipboard>,
    events: Option<crate::event_clipboard::Clipboard>,
}
mod player_memory;
mod row_selection;
pub struct Workspace {
    file: FileSession,
    chart: usize,
    row: u64,
    snap: usize,
    player: u32,
    player_memory: player_memory::Memory,
    tool: Kind,
    anchor: Option<u64>,
    source_row_anchor: Option<i32>,
    selection_end: Option<u64>,
    auto_advance: bool,
    clipboard: Option<Clipboard>,
    event_clipboard: Option<crate::event_clipboard::Clipboard>,
    structural_focus: std::collections::BTreeMap<u64, StructuralFocus>,
}

impl Workspace {
    /// Host-owned assistant tools use the same session and history as the UI.
    pub(crate) fn assistant_session(&mut self) -> &mut crate::history::Session {
        &mut self.file.session
    }
    pub fn capture_clipboards(&self) -> Clipboards {
        Clipboards {
            notes: self.clipboard.clone(),
            events: self.event_clipboard.clone(),
        }
    }
    pub fn install_clipboards(&mut self, values: Clipboards) {
        self.clipboard = values.notes;
        self.event_clipboard = values.events;
    }

    /// Commit a prevalidated musical focus transition. Load the target before
    /// changing any cursor state; subsequent assignments cannot fail.
    pub(crate) fn commit_focus_position(&mut self, chart: usize, row: u64) -> Result<(), Error> {
        let player = self.remembered_chart_player(chart)?;
        self.chart = chart;
        self.row = row;
        self.player = player;
        self.anchor = None;
        self.source_row_anchor = None;
        self.selection_end = None;
        Ok(())
    }

    /// Live presentation order as source chart indices. The caller supplies
    /// culture-aware collation; no source sections or active focus are moved.
    pub fn ordered_charts(
        &mut self,
        collate: impl FnMut(&str, &str) -> std::cmp::Ordering,
    ) -> Result<Vec<usize>, Error> {
        crate::chart_order::Snapshot::resident(&mut self.file.session)?.indices_with(collate)
    }
    /// Immutable live inputs for a native collator that precomputes sort keys.
    pub fn chart_order_snapshot(&mut self) -> Result<crate::chart_order::Snapshot, Error> {
        crate::chart_order::Snapshot::loadable(&mut self.file.session).map(|report| report.snapshot)
    }

    pub fn new_song(resource_base: &Path, sync_offset: f64) -> Result<Self, Error> {
        let candidate = crate::file_origin::NewSong::prepare(resource_base, sync_offset)?;
        Ok(Self::from_file_unfocused(FileSession::from_new(candidate)?))
    }
    pub fn open(path: &Path, chart: usize) -> Result<Self, Error> {
        Self::open_with_context(path, chart, Default::default())
    }
    pub fn open_with_context(
        path: &Path,
        chart: usize,
        context: crate::document::LoadContext,
    ) -> Result<Self, Error> {
        Self::from_file(FileSession::open_with_context(path, context)?, chart)
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
        Ok(Self::from_file_ready(file, chart))
    }
    /// Open a source document without forcing an unavailable chart grid. Source
    /// blocks and resident identities remain intact; New Chart stays available.
    pub fn from_file_unfocused(file: FileSession) -> Self {
        Self::from_file_ready(file, usize::MAX)
    }
    fn from_file_ready(file: FileSession, chart: usize) -> Self {
        let player_memory = player_memory::Memory::new(&file.session, chart, 0);
        Self {
            file,
            chart,
            row: 0,
            snap: 0,
            player: 0,
            player_memory,
            tool: Kind::Tap,
            anchor: None,
            source_row_anchor: None,
            selection_end: None,
            auto_advance: false,
            clipboard: None,
            event_clipboard: None,
            structural_focus: Default::default(),
        }
    }

    pub fn can_normalize_stepf2_chart(&self) -> bool {
        self.has_chart()
            && self.file.session.document().charts()[self.chart].steps_type == "pump-double"
            && self.grid().uses_stepf2()
    }

    pub fn change_note_owners(
        &mut self,
        count: u32,
        transform: crate::transform::Transform,
    ) -> Result<bool, Error> {
        let chart = self.chart;
        let (start, end) = self.selection().unwrap_or((self.row, self.row));
        let changed =
            self.file
                .session
                .edit_chart_atomic(chart, "Change note players", |staged| {
                    staged.set_participants(chart, count)?;
                    crate::transform::apply(staged, chart, start, end, None, transform)?;
                    Ok(())
                })?;
        self.sync_active_chart()?;
        Ok(changed)
    }
    /// Simultaneously swaps both kinds over the selected heads, for all players.
    pub fn swap_note_types(&mut self, pair: crate::note_swap::Pair) -> Result<bool, Error> {
        self.swap_note_types_detailed(pair).map_err(Into::into)
    }
    pub fn swap_note_types_detailed(
        &mut self,
        pair: crate::note_swap::Pair,
    ) -> Result<bool, crate::note_swap::Failure> {
        let chart = self
            .active_chart()
            .ok_or_else(|| Error::InvalidEdit("No active chart".into()))?;
        let (start, end) = self.selection().unwrap_or((self.row, self.row));
        let selected = self
            .file
            .session
            .grid(chart)?
            .iter()
            .filter(|(position, _)| position.row >= start && position.row <= end)
            .map(|(position, _)| position)
            .collect();
        crate::note_swap::apply_detailed(&mut self.file.session, chart, &selected, pair)
    }
    /// Prepares against the effective clock; None selection means all events.
    pub fn prepare_timing_conversion(
        &self,
        direction: crate::timing_edit::conversion::Direction,
        all: bool,
    ) -> Result<crate::timing_edit::conversion::Plan, Error> {
        self.prepare_timing_conversion_detailed(direction, all)
            .map_err(Into::into)
    }
    pub fn prepare_timing_conversion_detailed(
        &self,
        direction: crate::timing_edit::conversion::Direction,
        all: bool,
    ) -> Result<crate::timing_edit::conversion::Plan, crate::timing_edit::conversion::Failure> {
        use crate::generation::timing::{EventKind, GenerationTiming};
        use crate::timing_edit::conversion::{Direction, Plan};
        let chart = self
            .active_chart()
            .ok_or_else(|| Error::InvalidEdit("No active chart".into()))?;
        let timing = GenerationTiming::from_document(self.file.session.document(), chart)?;
        let (start, end) = self.selection().unwrap_or((self.row, self.row));
        let selected: Vec<_> = timing
            .editor_events()
            .iter()
            .filter(|event| {
                event.row >= 0
                    && (start..=end).contains(&(event.row as u64))
                    && match direction {
                        Direction::WarpsToNegativeStops => matches!(event.kind, EventKind::Warp(_)),
                        Direction::NegativeStopsToWarps => {
                            matches!(event.kind, EventKind::Stop(value) if value < 0.0)
                        }
                    }
            })
            .map(|event| event.row as u64)
            .collect();
        Plan::prepare_detailed(
            &self.file.session,
            chart,
            direction,
            if all { None } else { Some(&selected) },
        )
    }
    pub fn apply_timing_conversion(
        &mut self,
        plan: &crate::timing_edit::conversion::Plan,
        acknowledged: &std::collections::BTreeSet<u64>,
    ) -> Result<bool, Error> {
        self.apply_timing_conversion_detailed(plan, acknowledged)
            .map_err(Into::into)
    }
    pub fn apply_timing_conversion_detailed(
        &mut self,
        plan: &crate::timing_edit::conversion::Plan,
        acknowledged: &std::collections::BTreeSet<u64>,
    ) -> Result<bool, crate::timing_edit::conversion::Failure> {
        if self.active_chart() != Some(plan.chart()) {
            return Err(crate::timing_edit::conversion::Failure::stale());
        }
        let changed = plan.apply_detailed(&mut self.file.session, acknowledged)?;
        self.sync_active_chart()?;
        Ok(changed)
    }
    pub fn expand_event_selection(
        &mut self,
        selection: &crate::event_clipboard::Selection,
    ) -> Result<crate::event_clipboard::Selection, Error> {
        self.validate_event_selection(selection)?;
        selection.expand_to_chart(&mut self.file.session)
    }
    pub fn prepare_note_operation(
        &mut self,
        scope: crate::note_operation::Scope,
        operation: crate::note_operation::Operation,
    ) -> Result<crate::note_operation::Plan, crate::note_operation::Failure> {
        let chart = self
            .active_chart()
            .ok_or_else(|| Error::InvalidEdit("No chart for note operation".into()))?;
        crate::note_operation::Plan::prepare(&mut self.file.session, chart, scope, operation)
    }
    pub fn prepare_selected_note_operation(
        &mut self,
        selection: &crate::event_clipboard::Selection,
        operation: crate::note_operation::Operation,
    ) -> Result<crate::note_operation::Plan, crate::note_operation::Failure> {
        self.validate_event_selection(selection)?;
        selection.prepare_note_operation(&mut self.file.session, operation)
    }
    pub fn apply_note_operation(
        &mut self,
        plan: &crate::note_operation::Plan,
    ) -> Result<bool, crate::note_operation::Failure> {
        let chart = self
            .active_chart()
            .ok_or_else(|| Error::InvalidEdit("No chart for note operation".into()))?;
        plan.apply(&mut self.file.session, chart)
    }
    pub fn take_pattern_added_update(&mut self) -> Option<crate::history::PatternAddedEffect> {
        self.file.session.take_pattern_added_update()
    }
    pub fn take_event_selection_update(&mut self) -> Option<crate::history::SelectionEffect> {
        self.file.session.take_event_selection_update()
    }
    pub fn prepare_selected_timing_conversion(
        &self,
        selection: &crate::event_clipboard::Selection,
        direction: crate::timing_edit::conversion::Direction,
    ) -> Result<crate::timing_edit::conversion::Plan, crate::timing_edit::conversion::Failure> {
        self.validate_event_selection(selection)
            .map_err(|_| crate::timing_edit::conversion::Failure::stale())?;
        selection.prepare_timing_conversion(&self.file.session, direction)
    }
    pub fn prepare_selected_transform(
        &mut self,
        selection: &crate::event_clipboard::Selection,
        transform: crate::transform::Transform,
    ) -> Result<crate::selected_transform::Plan, crate::selected_transform::Failure> {
        self.validate_event_selection(selection)
            .map_err(|_| crate::selected_transform::Failure::stale())?;
        selection.prepare_transform(&mut self.file.session, transform)
    }
    pub fn prepare_transform_scope(
        &mut self,
        scope: crate::note_operation::Scope,
        transform: crate::transform::Transform,
    ) -> Result<crate::selected_transform::Plan, crate::selected_transform::Failure> {
        let chart = self
            .active_chart()
            .ok_or_else(|| Error::InvalidEdit("No chart to transform".into()))?;
        crate::selected_transform::Plan::prepare(&mut self.file.session, chart, scope, transform)
    }
    pub fn apply_selected_transform(
        &mut self,
        plan: &crate::selected_transform::Plan,
        acknowledged: &std::collections::BTreeSet<crate::selected_transform::Impact>,
    ) -> Result<bool, crate::selected_transform::Failure> {
        let chart = self
            .active_chart()
            .ok_or_else(|| Error::InvalidEdit("No chart to transform".into()))?;
        plan.apply(&mut self.file.session, chart, acknowledged)
    }
    pub fn event_selection(&mut self) -> Result<crate::event_clipboard::Selection, Error> {
        let (start, end) = self.selection().unwrap_or((self.row, self.row));
        self.event_selection_between(start, end)
    }
    pub fn event_selection_between(
        &mut self,
        start: u64,
        end: u64,
    ) -> Result<crate::event_clipboard::Selection, Error> {
        crate::event_clipboard::Selection::capture(&mut self.file.session, self.chart, start, end)
    }
    pub fn shift_event_selection(
        &mut self,
        selection: &crate::event_clipboard::Selection,
        rows: i64,
    ) -> Result<crate::event_shift::Report, Error> {
        self.validate_event_selection(selection)?;
        crate::event_shift::Plan::capture(&mut self.file.session, selection, rows)?
            .apply(&mut self.file.session)
    }
    fn validate_event_selection(
        &self,
        selection: &crate::event_clipboard::Selection,
    ) -> Result<(), Error> {
        if selection.chart() == self.chart {
            Ok(())
        } else {
            Err(Error::InvalidEdit(
                "Event selection belongs to another chart".into(),
            ))
        }
    }
    /// Bridge the native keyboard row-range into the shared global identities.
    /// Notes are the only native range-selected objects; timing/attacks/patterns
    /// inside that row interval must not become selected implicitly.
    pub fn note_range_event_selection(
        &mut self,
    ) -> Result<crate::event_clipboard::Selection, Error> {
        let mut selection = self.event_selection()?;
        let choices = selection
            .events()
            .iter()
            .map(|e| matches!(e, crate::event_clipboard::Event::Note(_)))
            .collect::<Vec<_>>();
        for (i, chosen) in choices.into_iter().enumerate() {
            selection.set_selected(i, chosen)?;
        }
        Ok(selection)
    }
    pub fn copy_event_selection(
        &mut self,
        selection: &crate::event_clipboard::Selection,
    ) -> Result<(), Error> {
        self.validate_event_selection(selection)?;
        if let Some(clipboard) = selection.copy(&mut self.file.session)? {
            self.event_clipboard = Some(clipboard);
        }
        Ok(())
    }
    pub fn copy_chart_events(
        &mut self,
        destinations: impl IntoIterator<Item = usize>,
        types: impl IntoIterator<Item = crate::chart_event_copy::EventType>,
    ) -> Result<crate::chart_event_copy::Report, Error> {
        crate::chart_event_copy::Plan::capture(
            &mut self.file.session,
            self.chart,
            destinations,
            types,
        )?
        .apply(&mut self.file.session)
    }
    pub fn paste_events_at_fractional_row_with_presentation_receipt(
        &mut self,
        row: f64,
    ) -> Result<
        Option<(
            crate::event_clipboard::PasteReport,
            crate::song_properties::Application,
        )>,
        Error,
    > {
        let result = if let Some(clipboard) = &self.event_clipboard {
            Some(clipboard.paste_at_fractional_row_with_presentation_receipt(
                &mut self.file.session,
                self.chart,
                row,
                self.player,
            )?)
        } else {
            None
        };
        self.sync_active_chart()?;
        Ok(result)
    }
    pub fn paste_events_at_fractional_row(
        &mut self,
        row: f64,
    ) -> Result<crate::event_clipboard::PasteReport, Error> {
        let result = if let Some(clipboard) = &self.event_clipboard {
            clipboard.paste_at_fractional_row(&mut self.file.session, self.chart, row, self.player)
        } else {
            Ok(Default::default())
        };
        if result.is_ok() {
            self.sync_active_chart()?;
        }
        result
    }
    pub fn paste_events_with_report(
        &mut self,
    ) -> Result<crate::event_clipboard::PasteReport, Error> {
        if let Some(clipboard) = &self.event_clipboard {
            clipboard.paste_with_report(&mut self.file.session, self.chart, self.row, self.player)
        } else {
            Ok(Default::default())
        }
    }
    pub fn cut_event_selection(
        &mut self,
        selection: &crate::event_clipboard::Selection,
    ) -> Result<(), Error> {
        self.validate_event_selection(selection)?;
        if let Some(clipboard) = selection.cut(&mut self.file.session)? {
            self.event_clipboard = Some(clipboard);
        }
        self.sync_active_chart()
    }
    pub fn delete_event_selection(
        &mut self,
        selection: &crate::event_clipboard::Selection,
    ) -> Result<bool, Error> {
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

    pub fn create_bound_profile(
        &mut self,
        plan: &crate::history::profile_creation::Plan,
        directory: &Path,
        profile: &crate::generation::profiles::Profile,
    ) -> Result<crate::history::profile_creation::Committed, Error> {
        plan.publish(&mut self.file.session, directory, profile)
    }
    pub fn create_profile(
        &mut self,
        directory: &Path,
        profile: &crate::generation::profiles::Profile,
    ) -> Result<std::path::PathBuf, Error> {
        self.file.session.create_profile(directory, profile)
    }

    pub fn delete_profile(
        &mut self,
        file: &crate::generation::profiles::ProfileFile,
    ) -> Result<(), Error> {
        self.file.session.delete_profile(file)
    }

    pub fn capture_lane_input(&mut self) -> Result<crate::lane_entry::Inputs, Error> {
        let chart = self
            .active_chart()
            .ok_or_else(|| Error::InvalidEdit("No active lane input chart".into()))?;
        crate::lane_entry::Inputs::capture(&mut self.file.session, chart)
    }
    pub fn press_lane_input(
        &mut self,
        input: &mut crate::lane_entry::Inputs,
        press: crate::lane_entry::Press,
    ) -> Result<crate::lane_entry::Outcome, Error> {
        if self.active_chart() != Some(input.chart()) {
            return Err(Error::InvalidEdit("Lane input chart lost focus".into()));
        }
        input.press(&mut self.file.session, press)
    }
    pub fn release_lane_input(
        &mut self,
        input: &mut crate::lane_entry::Inputs,
        lane: u16,
    ) -> Result<bool, Error> {
        if self.active_chart() != Some(input.chart()) {
            return Err(Error::InvalidEdit("Lane input chart lost focus".into()));
        }
        input.release(&mut self.file.session, lane)
    }
    pub fn prepare_quick_add(
        &self,
        kind: crate::quick_add::Kind,
    ) -> Result<crate::quick_add::Plan, crate::quick_add::Failure> {
        crate::quick_add::Plan::prepare(&self.file.session, self.chart(), kind, self.row() as f64)
    }
    pub fn apply_quick_add(
        &mut self,
        plan: &crate::quick_add::Plan,
    ) -> Result<bool, crate::quick_add::Failure> {
        let chart = self.chart();
        plan.apply(&mut self.file.session, chart)
    }
    pub fn prepare_checked_shift(
        &mut self,
        selection: &crate::event_clipboard::Selection,
        later: bool,
    ) -> Result<crate::shift_all::Plan, crate::shift_all::Failure> {
        self.validate_event_selection(selection)?;
        let chart = self.chart();
        let rows = snap_shift_rows(self.snap()) as i64 * if later { 1 } else { -1 };
        crate::shift_all::Plan::checked(&mut self.file.session, chart, selection, rows)
    }
    pub fn prepare_shift_all(
        &mut self,
        later: bool,
    ) -> Result<crate::shift_all::Plan, crate::shift_all::Failure> {
        let rows = snap_shift_rows(self.snap()) as i64 * if later { 1 } else { -1 };
        let chart = self.chart();
        crate::shift_all::Plan::prepare(&mut self.file.session, chart, rows)
    }
    pub fn apply_shift_all(
        &mut self,
        plan: crate::shift_all::Plan,
        review: &crate::event_shift::Report,
    ) -> Result<crate::event_shift::Report, crate::shift_all::Failure> {
        let chart = self.chart();
        plan.apply(&mut self.file.session, chart, review)
    }
    pub fn prepare_all_pattern_action(
        &self,
        action: crate::pattern_actions::Action,
    ) -> Result<crate::pattern_actions::Plan, crate::pattern_actions::Failure> {
        crate::pattern_actions::Plan::all(&self.file.session, self.chart(), action)
    }
    pub fn prepare_checked_pattern_action(
        &self,
        selection: &crate::event_clipboard::Selection,
        action: crate::pattern_actions::Action,
    ) -> Result<crate::pattern_actions::Plan, crate::pattern_actions::Failure> {
        crate::pattern_actions::Plan::checked(&self.file.session, self.chart(), selection, action)
    }
    pub fn begin_pattern_action(
        &mut self,
        plan: crate::pattern_actions::Plan,
        review: &crate::pattern_actions::Review,
        directory: Option<&Path>,
    ) -> Result<crate::pattern_actions::Started, crate::pattern_actions::Failure> {
        let chart = self.chart();
        plan.begin(&mut self.file.session, chart, review, directory)
    }
    pub fn begin_pattern_action_with_presentation_receipt(
        &mut self,
        plan: crate::pattern_actions::Plan,
        review: &crate::pattern_actions::Review,
        directory: Option<&Path>,
    ) -> crate::pattern_actions::Admission {
        let chart = self.chart();
        plan.begin_with_presentation_receipt(&mut self.file.session, chart, review, directory)
    }
    pub fn misc_inventory(
        &mut self,
        chart: usize,
    ) -> Result<crate::region_selection::inventory::Inventory, Error> {
        crate::region_selection::inventory::Inventory::capture_misc(&mut self.file.session, chart)
    }
    pub fn source_chart_end_position(&mut self) -> Result<f64, Error> {
        self.source_chart_end_position_at(self.row as f64)
    }
    pub fn region_inventory(
        &mut self,
        chart: usize,
    ) -> Result<crate::region_selection::inventory::Inventory, Error> {
        crate::region_selection::inventory::Inventory::capture(&mut self.file.session, chart)
    }
    pub fn file(&self) -> &FileSession {
        &self.file
    }
    pub fn set_audio_preferences_untracked(
        &mut self,
        before: crate::audio_preferences::Audio,
        after: crate::audio_preferences::Audio,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_audio_preferences_untracked(before, after, file)
    }
    pub fn set_audio_preferences(
        &mut self,
        before: crate::audio_preferences::Audio,
        after: crate::audio_preferences::Audio,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file.session.set_audio_preferences(before, after, file)
    }
    pub fn set_density_preferences(
        &mut self,
        before: crate::density_preferences::Preferences,
        after: crate::density_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_density_preferences(before, after, file)
    }
    pub fn take_density_preferences_update(
        &mut self,
    ) -> Option<crate::density_preferences::Update> {
        self.file.session.take_density_preferences_update()
    }
    pub fn set_receptor_preferences(
        &mut self,
        before: crate::receptor_preferences::Preferences,
        after: crate::receptor_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_receptor_preferences(before, after, file)
    }
    pub fn set_stream_preferences(
        &mut self,
        before: crate::stream_preferences::Preferences,
        after: crate::stream_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_stream_preferences(before, after, file)
    }
    pub fn set_selection_preferences(
        &mut self,
        before: crate::selection_preferences::Preferences,
        after: crate::selection_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_selection_preferences(before, after, file)
    }
    pub fn take_selection_preferences_update(
        &mut self,
    ) -> Option<crate::selection_preferences::Update> {
        self.file.session.take_selection_preferences_update()
    }
    pub fn take_stream_preferences_update(&mut self) -> Option<crate::stream_preferences::Update> {
        self.file.session.take_stream_preferences_update()
    }
    pub fn set_scroll_preferences(
        &mut self,
        before: crate::scroll_preferences::Preferences,
        after: crate::scroll_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_scroll_preferences(before, after, file)
    }
    pub fn take_receptor_preferences_update(
        &mut self,
    ) -> Option<crate::receptor_preferences::Update> {
        self.file.session.take_receptor_preferences_update()
    }
    pub fn take_scroll_preferences_update(&mut self) -> Option<crate::scroll_preferences::Update> {
        self.file.session.take_scroll_preferences_update()
    }
    pub fn set_display_options(
        &mut self,
        before: crate::options_preferences::Display,
        after: crate::options_preferences::Display,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file.session.set_display_options(before, after, file)
    }
    pub fn take_display_options_update(&mut self) -> Option<crate::options_preferences::Update> {
        self.file.session.take_display_options_update()
    }
    pub fn set_visual_preferences(
        &mut self,
        before: crate::visual_preferences::Snapshot,
        after: crate::visual_preferences::Snapshot,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_visual_preferences(before, after, file)
    }
    pub fn take_visual_preferences_update(&mut self) -> Option<crate::visual_preferences::Update> {
        self.file.session.take_visual_preferences_update()
    }
    pub fn set_dark_preferences(
        &mut self,
        before: crate::dark_preferences::Preferences,
        after: crate::dark_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file.session.set_dark_preferences(before, after, file)
    }
    pub fn take_dark_preferences_update(&mut self) -> Option<crate::dark_preferences::Update> {
        self.file.session.take_dark_preferences_update()
    }
    pub fn set_note_color_preferences(
        &mut self,
        before: std::sync::Arc<crate::note_color_preferences::Preferences>,
        after: std::sync::Arc<crate::note_color_preferences::Preferences>,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_note_color_preferences(before, after, file)
    }
    pub fn set_key_binding_preferences(
        &mut self,
        before: std::sync::Arc<crate::key_binding_preferences::Preferences>,
        after: std::sync::Arc<crate::key_binding_preferences::Preferences>,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_key_binding_preferences(before, after, file)
    }
    pub fn take_note_color_preferences_update(
        &mut self,
    ) -> Option<crate::note_color_preferences::Update> {
        self.file.session.take_note_color_preferences_update()
    }
    pub fn take_key_binding_preferences_update(
        &mut self,
    ) -> Option<crate::key_binding_preferences::Update> {
        self.file.session.take_key_binding_preferences_update()
    }
    pub fn take_audio_preferences_update(&mut self) -> Option<crate::audio_preferences::Update> {
        self.file.session.take_audio_preferences_update()
    }
    pub fn active_chart(&self) -> Option<usize> {
        (self.chart < self.file.session.document().charts().len()).then_some(self.chart)
    }
    pub fn has_chart(&self) -> bool {
        self.active_chart().is_some()
    }
    /// Close the final editor view without deleting or unloading its song.
    pub fn clear_active_chart(&mut self) {
        self.chart = usize::MAX;
        self.anchor = None;
        self.source_row_anchor = None;
        self.selection_end = None;
    }
    /// Prepare immutable view access to resident charts. Loading note caches
    /// neither selects a chart nor changes document content or undo history.
    pub fn prepare_view_grids(&mut self, charts: &[usize]) -> Result<(), Error> {
        for &chart in charts {
            self.file.session.grid(chart)?;
        }
        Ok(())
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
    /// The native auto-advance control and source NoteEntryMode share one state.
    /// This application preference never creates a chart Undo entry.
    pub fn set_auto_advance(&mut self, advance: bool) {
        self.auto_advance = advance;
    }
    pub fn toggle_note_entry_mode(&mut self) {
        self.set_auto_advance(!self.auto_advance);
    }
    pub fn auto_advance(&self) -> bool {
        self.auto_advance
    }
    pub fn selection(&self) -> Option<(u64, u64)> {
        self.anchor.map(|anchor| {
            let end = self.selection_end.unwrap_or(self.row);
            (anchor.min(end), anchor.max(end))
        })
    }

    /// Move presentation focus without stretching an already selected note range.
    /// Selection-consuming commands still read its exact original endpoints.
    pub fn seek_preserving_selection(&mut self, row: u64) -> Result<(), Error> {
        if !self.has_chart() {
            return Err(Error::InvalidEdit("No active chart".into()));
        }
        if self.source_row_anchor.is_some() {
            self.selection_end = None;
        } else if self.anchor.is_some() && self.selection_end.is_none() {
            self.selection_end = Some(self.row);
        }
        self.row = row;
        Ok(())
    }

    pub fn execute(&mut self, command: Command) -> Result<(), Error> {
        if !self.has_chart()
            && !matches!(
                command,
                Command::Undo
                    | Command::Redo
                    | Command::Save
                    | Command::SaveRecovery
                    | Command::Recover
                    | Command::SelectChart(_)
                    | Command::SetSnap(_)
            )
        {
            return Err(Error::InvalidEdit("No active chart".into()));
        }
        match command {
            Command::Move(steps) => {
                let spacing = if SNAP_ROWS[self.snap] == 0 {
                    crate::timing_edit::signature::Signatures::from_document(
                        self.file.session.document(),
                        self.chart,
                    )?
                    .measure_position(self.row)?
                    .rows_per_measure
                } else {
                    SNAP_ROWS[self.snap]
                };
                let distance = spacing
                    .checked_mul(steps.unsigned_abs())
                    .ok_or_else(|| Error::InvalidEdit("Cursor movement overflow".into()))?;
                self.selection_end = None;
                self.row = if steps >= 0 {
                    self.row.saturating_add(distance)
                } else {
                    self.row.saturating_sub(distance)
                };
            }
            Command::Seek(row) => {
                self.selection_end = None;
                self.row = row;
            }
            Command::DuplicateChart => {
                self.duplicate_chart()?;
            }
            Command::NormalizeStepF2 => {
                self.file.session.normalize_stepf2_chart(self.chart)?;
                self.sync_active_chart()?;
            }
            Command::PreviousMeasure | Command::NextMeasure => {
                let signatures = crate::timing_edit::signature::Signatures::from_document(
                    self.file.session.document(),
                    self.chart,
                )?;
                let destination = signatures
                    .measure_destination(self.row, matches!(command, Command::PreviousMeasure))?;
                // Workspace note positions are currently nonnegative reference rows.
                self.selection_end = None;
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
                    self.selection_end = None;
                    self.row = row;
                }
            }
            Command::SetSnap(snap) => {
                if snap >= SNAP_ROWS.len() {
                    return Err(Error::InvalidEdit("Invalid quantization".into()));
                }
                self.selection_end = None;
                self.snap = snap;
                // Choosing a subdivision never seeks the chart (source SnapManager).
            }
            Command::SelectChart(chart) => {
                self.player = self.remembered_chart_player(chart)?;
                self.chart = chart;
                self.anchor = None;
                self.source_row_anchor = None;
                self.selection_end = None;
            }
            Command::SelectPlayer(player) => {
                if player >= self.grid().players() {
                    return Err(Error::InvalidEdit(
                        "Participant does not exist in this chart".into(),
                    ));
                }
                self.player = player;
                self.player_memory
                    .explicit(&self.file.session, self.chart, player);
            }
            Command::SelectTool(tool) => {
                if !TOOLS.contains(&tool) {
                    return Err(Error::Unsupported(
                        "This note kind is not available as a lane entry tool".into(),
                    ));
                }
                self.tool = tool;
            }
            Command::ToggleAutoAdvance => self.toggle_note_entry_mode(),
            Command::ToggleSelection => {
                self.source_row_anchor = None;
                self.selection_end = None;
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
                                .unwrap_or(
                                    head_row.checked_add(snap_edit_rows(self.snap)).ok_or_else(
                                        || Error::InvalidEdit("Sustain exceeds row range".into()),
                                    )?,
                                ),
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
                    self.selection_end = None;
                    self.row = self.row.saturating_add(snap_edit_rows(self.snap));
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
                    clipboard.paste_force(
                        &mut self.file.session,
                        self.chart,
                        self.row,
                        self.player,
                    )?;
                }
            }
            Command::CopyEvents | Command::CutEvents => {
                let (start, end) = self.selection().unwrap_or((self.row, self.row));
                let clipboard = if matches!(command, Command::CutEvents) {
                    crate::event_clipboard::Clipboard::cut(
                        &mut self.file.session,
                        self.chart,
                        start,
                        end,
                    )?
                } else {
                    crate::event_clipboard::Clipboard::copy(
                        &mut self.file.session,
                        self.chart,
                        start,
                        end,
                    )?
                };
                if let Some(clipboard) = clipboard {
                    self.event_clipboard = Some(clipboard);
                }
            }
            Command::PasteEvents => {
                self.paste_events_with_report()?;
            }
            Command::Undo => {
                let focused = self
                    .active_chart()
                    .and_then(|chart| self.file.session.chart_ids().get(chart))
                    .cloned();
                let effect = self.file.authoring_history_with_effect(false)?;
                let explicit = effect
                    .applied
                    .then(|| self.structural_focus.get(&effect.before_revision))
                    .flatten()
                    .filter(|focus| focus.before_revision == effect.after_revision)
                    .map(|focus| focus.before_chart);
                self.sync_history_focus(&effect, focused.as_ref(), explicit)?;
            }
            Command::Redo => {
                let focused = self
                    .active_chart()
                    .and_then(|chart| self.file.session.chart_ids().get(chart))
                    .cloned();
                let effect = self.file.authoring_history_with_effect(true)?;
                let explicit = effect
                    .applied
                    .then(|| self.structural_focus.get(&effect.after_revision))
                    .flatten()
                    .filter(|focus| focus.before_revision == effect.before_revision)
                    .map(|focus| focus.after_chart);
                self.sync_history_focus(&effect, focused.as_ref(), explicit)?;
            }
            Command::Save => self.file.save()?,
            Command::SaveRecovery => {
                let _publication = self.file.save_recovery()?;
            }
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
        self.generation_task_at_with_profiles(self.chart, options, directory, profiles)
    }
    pub fn generation_task_at_with_profiles(
        &self,
        source_chart: usize,
        options: crate::generation::job::ChartOptions,
        directory: Option<&Path>,
        profiles: crate::generation::job::ProfileOverrides,
    ) -> Result<crate::generation::job::ChartTask, Error> {
        crate::generation::job::ChartTask::capture(&self.file.session, source_chart, options)?
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

    pub fn apply_sm_authoring_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        if !self.file.session.history_changes_sm_authoring(redo) {
            return Err(Error::InvalidEdit(
                "History does not contain an authoring promotion".into(),
            ));
        }
        let before = self.file.session.content_token();
        let ids = self.file.session.chart_ids().to_vec();
        self.execute(if redo { Command::Redo } else { Command::Undo })?;
        Ok(crate::history::song_presentation::preserved_authoring(
            before,
            ids,
            &self.file.session,
        ))
    }
    pub fn prepare_sm_authoring(&mut self) -> Result<crate::history::authoring::Plan, Error> {
        self.file.prepare_sm_authoring()
    }
    pub fn apply_sm_authoring(
        &mut self,
        plan: crate::history::authoring::Plan,
        options: crate::advanced_export::Options,
    ) -> Result<crate::song_properties::Application, Error> {
        self.file.apply_sm_authoring(plan, options)
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

    pub fn apply_timing_migration_with_presentation_receipt(
        &mut self,
        draft: &crate::timing_edit::migration::Draft,
    ) -> Result<crate::song_properties::Application, Error> {
        draft.apply_with_presentation_receipt(&mut self.file.session)
    }
    pub fn apply_timing_migration_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        self.file.session.apply_timing_migration_history(redo)
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

    pub fn apply_labels(
        &mut self,
        draft: &crate::timing_edit::label::Draft,
    ) -> Result<bool, Error> {
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
    pub fn apply_ticks(&mut self, draft: &crate::timing_edit::tick::Draft) -> Result<bool, Error> {
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
    pub fn select_attack(
        &mut self,
        scope: crate::timing_edit::Scope,
        row: u64,
    ) -> Result<(), Error> {
        self.file.session.select_attack(self.chart, scope, row)
    }
    pub fn forget_chart_attack(&mut self, chart: usize) {
        self.file.session.forget_chart_attack(chart);
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
    pub fn attacks(
        &self,
        scope: crate::timing_edit::Scope,
    ) -> Result<crate::attacks::Draft, Error> {
        crate::attacks::Draft::capture(&self.file.session, self.chart(), scope)
    }
    pub fn apply_attacks(&mut self, draft: &crate::attacks::Draft) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn note_attributes(&self, lane: u16) -> Result<crate::note_attributes::Draft, Error> {
        let position = crate::notes::Position {
            row: self.row(),
            lane,
            player: self.player(),
        };
        let note = self
            .grid()
            .get(position)
            .ok_or_else(|| Error::InvalidEdit("No note head at this position".into()))?;
        Ok(crate::note_attributes::Draft::from_note(
            &self.file.session,
            self.chart(),
            position,
            note,
        ))
    }
    pub fn apply_note_attributes(
        &mut self,
        draft: &crate::note_attributes::Draft,
    ) -> Result<bool, Error> {
        draft.apply(&mut self.file.session, self.chart)
    }
    pub fn apply_song_assets(
        &mut self,
        assets: &crate::song_assets::Assets,
        only_unset: bool,
    ) -> Result<usize, Error> {
        assets.apply(&mut self.file.session, only_unset)
    }
    pub fn song_properties(&self) -> Result<crate::song_properties::Draft, Error> {
        crate::song_properties::Draft::capture(&self.file.session)
    }
    pub fn apply_song_properties_with_presentation_receipt(
        &mut self,
        draft: &crate::song_properties::Draft,
    ) -> Result<crate::song_properties::Application, Error> {
        draft.apply_with_presentation_receipt(&mut self.file.session)
    }
    pub fn apply_song_properties(
        &mut self,
        draft: &crate::song_properties::Draft,
    ) -> Result<bool, Error> {
        Ok(draft
            .apply_with_presentation_receipt(&mut self.file.session)?
            .changed())
    }
    pub fn apply_chart_properties_with_presentation_receipt(
        &mut self,
        draft: &crate::chart_properties::Draft,
    ) -> Result<crate::chart_properties::Application, Error> {
        self.apply_chart_properties_at_with_presentation_receipt(self.chart, draft)
    }
    pub fn apply_chart_properties_at_with_presentation_receipt(
        &mut self,
        chart: usize,
        draft: &crate::chart_properties::Draft,
    ) -> Result<crate::chart_properties::Application, Error> {
        draft.apply_with_presentation_receipt(&mut self.file.session, chart)
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

    pub fn reseed_inspected_marker(
        &mut self,
        draft: &crate::generation::marker_edit::Draft,
        seed: i32,
    ) -> Result<crate::song_properties::Application, Error> {
        crate::history::song_presentation::reseed_marker(
            draft,
            &mut self.file.session,
            self.chart,
            seed,
        )
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
        self.selection_end = None;
        Ok(changed)
    }
    pub fn navigate_saved_pattern_at(&mut self, cursor: f64, forward: bool) -> Result<bool, Error> {
        if !cursor.is_finite() {
            return Err(Error::InvalidEdit(
                "Pattern navigation position must be finite".into(),
            ));
        }
        let data =
            crate::generation::metadata::ChartData::read(self.file.session.document(), self.chart)?;
        if let Some(row) = data.and_then(|data| data.adjacent_marker_at(cursor, forward)) {
            self.seek_preserving_selection(row)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn navigate_saved_pattern(&mut self, forward: bool) -> Result<bool, Error> {
        let data =
            crate::generation::metadata::ChartData::read(self.file.session.document(), self.chart)?;
        if let Some(row) = data.and_then(|data| data.adjacent_marker(self.row, forward)) {
            self.selection_end = None;
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
        self.selection_end = None;
        Ok(changed)
    }

    /// Same validated worker application, with a typed clock/audio proof for
    /// callers which own a fractional presentation position.
    pub fn apply_generated_pattern_with_presentation_receipt(
        &mut self,
        preview: &crate::generation::pattern_job::Preview,
    ) -> Result<crate::song_properties::Application, Error> {
        let applied = crate::history::song_presentation::apply_pattern(
            preview,
            &mut self.file.session,
            self.chart,
        )?;
        self.file.session.grid(self.chart)?;
        self.anchor = None;
        self.selection_end = None;
        Ok(applied)
    }

    pub fn apply_pattern_set_with_presentation_receipt(
        &mut self,
        preview: &crate::generation::pattern_batch::Preview,
    ) -> Result<crate::song_properties::Application, Error> {
        let applied = crate::history::song_presentation::apply_pattern_set(
            preview,
            &mut self.file.session,
            self.chart,
        )?;
        self.file.session.grid(self.chart)?;
        self.anchor = None;
        self.selection_end = None;
        Ok(applied)
    }
    pub fn apply_cleared_pattern_with_presentation_receipt(
        &mut self,
        preview: &crate::generation::clear_patterns::Preview,
    ) -> Result<crate::song_properties::Application, Error> {
        let applied = crate::history::song_presentation::apply_cleared_pattern(
            preview,
            &mut self.file.session,
            self.chart,
        )?;
        self.file.session.grid(self.chart)?;
        self.anchor = None;
        self.selection_end = None;
        Ok(applied)
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
        self.selection_end = None;
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
        let before_ids = self.file.session.chart_ids().to_vec();
        let indices = preview.apply(&mut self.file.session)?;
        self.initialize_added_chart_players(&before_ids)?;
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
        self.player = self.remembered_chart_player(chart)?;
        self.anchor = None;
        self.selection_end = None;
        Ok(indices)
    }

    pub fn deletion_draft(&self) -> Result<crate::chart_deletion::Draft, Error> {
        crate::chart_deletion::Draft::capture(&self.file.session, self.chart)
    }
    pub fn delete_chart(&mut self, draft: &crate::chart_deletion::Draft) -> Result<(), Error> {
        draft.check(&self.file.session, self.chart)?;
        let before_revision = self.file.session.revision();
        let before_chart = self.chart;
        let current = self.file.session.snapshot_document()?;
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
        self.player_memory.prune(&self.file.session);
        if self.has_chart() {
            self.player = self.remembered_chart_player(self.chart)?;
        }
        self.anchor = None;
        self.selection_end = None;
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
        self.player = self.remembered_chart_player(chart)?;
        self.anchor = None;
        self.selection_end = None;
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
        self.player = self.remembered_chart_player(chart)?;
        self.anchor = None;
        self.selection_end = None;
        Ok(chart)
    }

    pub(crate) fn apply_assistant_draft(
        &mut self,
        token: &crate::history::RevisionToken,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let before_revision = self.file.session.revision();
        let before_chart = self.chart;
        let before_ids = self.file.session.chart_ids().to_vec();
        let count = self.file.session.document().charts().len();
        self.file.session.commit_assistant_document(token, bytes)?;
        let after = self.file.session.document().charts().len();
        if after > count {
            self.initialize_added_chart_players(&before_ids)?;
            let chart = after - 1;
            self.structural_focus.insert(
                self.file.session.revision(),
                StructuralFocus {
                    before_revision,
                    before_chart,
                    after_chart: chart,
                },
            );
            self.execute(Command::SelectChart(chart))?;
        }
        Ok(())
    }
    pub fn apply_generated(
        &mut self,
        preview: &crate::generation::job::ChartPreview,
    ) -> Result<usize, Error> {
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
        self.player = self.remembered_chart_player(chart)?;
        self.anchor = None;
        self.selection_end = None;
        Ok(chart)
    }

    /// Ordinary history updates its owning chart in the background. Only an
    /// explicit structural focus directive may reopen a closed view.
    fn sync_history_focus(
        &mut self,
        effect: &crate::history::HistoryEffect,
        focused: Option<&crate::generation::metadata::Guid>,
        explicit: Option<usize>,
    ) -> Result<(), Error> {
        if !effect.applied {
            return Ok(());
        }
        self.player_memory.prune(&self.file.session);
        if let Some(delta) = &effect.chart_structure {
            self.initialize_added_chart_players(&delta.before)?;
        }
        let identities = effect
            .chart_structure
            .as_ref()
            .map_or(self.file.session.chart_ids(), |delta| {
                delta.after.as_slice()
            });
        self.chart = explicit.unwrap_or_else(|| {
            focused
                .and_then(|id| identities.iter().position(|candidate| candidate == id))
                .unwrap_or(usize::MAX)
        });
        if self.chart != usize::MAX {
            self.sync_active_chart()?;
        } else {
            self.anchor = None;
            self.selection_end = None;
        }
        Ok(())
    }

    fn sync_active_chart(&mut self) -> Result<(), Error> {
        if self.file.session.document().charts().is_empty() {
            self.chart = usize::MAX;
            self.anchor = None;
            self.selection_end = None;
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
                self.anchor = None;
                self.selection_end = None;
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
        self.player = self.remembered_chart_player(self.chart)?;
        if self.chart != previous_chart {
            self.anchor = None;
            self.selection_end = None;
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

impl Workspace {
    pub fn set_history_capacity(&mut self, size: usize) -> Result<(), Error> {
        self.file.session.set_history_capacity(size)
    }
    pub fn set_general_options(
        &mut self,
        before: crate::options_preferences::General,
        after: crate::options_preferences::General,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file.session.set_general_options(before, after, file)
    }
    pub fn take_general_options_update(
        &mut self,
    ) -> Option<crate::options_preferences::GeneralUpdate> {
        self.file.session.take_general_options_update()
    }
}

impl Workspace {
    pub fn restore_options(
        &mut self,
        before: crate::options_preferences::restore::Snapshot,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file.session.restore_options(before, file)
    }
    pub fn set_recent_files_history_size(
        &mut self,
        before: usize,
        after: usize,
        file: &crate::audio_preferences::File,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_recent_files_history_size(before, after, file)
    }
    pub fn set_suppress_external_song_notifications(
        &mut self,
        before: bool,
        after: bool,
        file: &crate::audio_preferences::File,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_suppress_external_song_notifications(before, after, file)
    }
    pub fn take_restore_options_update(
        &mut self,
    ) -> Option<crate::options_preferences::restore::Update> {
        self.file.session.take_restore_options_update()
    }
}

impl Workspace {
    pub fn apply_song_timing_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        self.file.session.apply_song_timing_history(redo)
    }
}

impl Workspace {
    pub fn restore_scroll(
        &mut self,
        before: crate::scroll_preferences::restore::Snapshot,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file.session.restore_scroll(before, file)
    }
    pub fn take_restore_scroll_update(
        &mut self,
    ) -> Option<crate::scroll_preferences::restore::Update> {
        self.file.session.take_restore_scroll_update()
    }
}

impl Workspace {
    pub fn set_performance_preferences(
        &mut self,
        before: crate::performance::preferences::Preferences,
        after: crate::performance::preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .set_performance_preferences(before, after, file)
    }
    pub fn restore_performance_preferences(
        &mut self,
        before: crate::performance::preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.file
            .session
            .restore_performance_preferences(before, file)
    }
    pub fn take_performance_preferences_update(&mut self) -> Option<crate::performance::Update> {
        self.file.session.take_performance_preferences_update()
    }
}

impl Workspace {
    pub fn prepare_source_preferences(
        &mut self,
        current: &crate::audio_preferences::File,
        raw: &[u8],
        platform: crate::key_binding_preferences::source_profile::Platform,
    ) -> Result<crate::audio_preferences::source_import::Draft, Error> {
        let focus = self.has_chart().then_some(self.chart);
        crate::audio_preferences::source_import::Draft::prepare(
            &mut self.file.session,
            focus,
            current,
            raw,
            platform,
        )
    }
    pub fn import_source_preferences(
        &mut self,
        draft: crate::audio_preferences::source_import::Draft,
        current: &crate::audio_preferences::File,
        accept_adaptations: bool,
    ) -> Result<bool, Error> {
        let focus = self.has_chart().then_some(self.chart);
        self.file
            .session
            .import_source_preferences(draft, current, focus, accept_adaptations)
    }
    pub fn capture_source_preferences_context(&mut self, current: &crate::audio_preferences::File) {
        self.file
            .session
            .capture_source_preferences_context(current);
    }
    pub fn take_source_preferences_update(
        &mut self,
    ) -> Option<Box<crate::audio_preferences::source_import::Update>> {
        self.file.session.take_source_preferences_update()
    }
    pub fn source_preference_player(&mut self, raw: i32) -> Result<(), Error> {
        if !self.has_chart() {
            return Ok(());
        }
        let players = self.file.session.grid(self.chart)?.players();
        self.execute(Command::SelectPlayer(
            raw.clamp(0, players.saturating_sub(1) as i32) as u32,
        ))
    }
    pub fn install_source_snap(&mut self, level: crate::snap::Level) {
        self.snap = level.native_index();
    }
}

impl Workspace {
    pub fn edit_profile(
        &mut self,
        file: &crate::generation::profiles::ProfileFile,
        profile: &crate::generation::profiles::Profile,
    ) -> Result<bool, Error> {
        self.file.session.edit_profile(file, profile)
    }
    pub fn take_profile_edit_update(
        &mut self,
    ) -> Option<crate::generation::profiles::edit::Update> {
        self.file.session.take_profile_edit_update()
    }
    pub fn pending_profile_durability_update(
        &self,
    ) -> Option<crate::generation::profiles::edit::Update> {
        self.file.session.pending_profile_durability_update()
    }
    pub fn has_profile_durability(&self) -> bool {
        self.file.session.has_profile_durability()
    }
    pub fn retry_profile_durability(&mut self) -> Result<(), Error> {
        self.file.session.retry_profile_durability()
    }
}

impl Workspace {
    pub fn retain_profile_publication(&mut self, file: crate::generation::profiles::ProfileFile) {
        self.file.session.retain_profile_publication(file);
    }
}

impl Workspace {
    pub fn apply_chart_properties_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        self.file.session.apply_chart_properties_history(redo)
    }
    pub fn apply_timing_live_plan(
        &mut self,
        plan: crate::timing_edit::live::Plan,
    ) -> Result<crate::timing_edit::live::Application, Error> {
        plan.apply(&mut self.file.session)
    }
    pub fn apply_unfocused_timing_history(&mut self, redo: bool) -> Result<bool, Error> {
        self.file.session.apply_unfocused_timing_history(redo)
    }
}

impl Workspace {
    pub fn apply_marker_metadata_with_presentation_receipt(
        &mut self,
        draft: &crate::generation::marker_edit::Draft,
        delete: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        crate::history::song_presentation::apply_marker_metadata(
            draft,
            &mut self.file.session,
            self.chart,
            delete,
        )
    }
    pub fn apply_marker_metadata_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        crate::history::song_presentation::apply_marker_metadata_history(
            &mut self.file.session,
            redo,
        )
    }
}

impl Workspace {
    pub fn apply_song_properties_live_plan(
        &mut self,
        plan: crate::song_properties::live::Plan,
    ) -> Result<crate::song_properties::live::Application, Error> {
        if plan.chart() != self.active_chart() {
            return Err(Error::ExternalChange);
        }
        plan.apply(&mut self.file.session)
    }
}

impl Workspace {
    pub fn apply_song_properties_metadata_history(
        &mut self,
        redo: bool,
    ) -> Result<crate::song_properties::Application, Error> {
        self.file
            .session
            .apply_song_properties_metadata_history(redo)
    }
}

impl Workspace {
    pub fn apply_zoom_preferences(
        &mut self,
        before: crate::zoom_preferences::Preferences,
        target: crate::zoom_preferences::Preferences,
        file: &crate::audio_preferences::File,
    ) -> Result<bool, Error> {
        self.file
            .session
            .apply_zoom_preferences(before, target, file)
    }
}

impl Workspace {
    pub fn prepare_chart_event_copy(
        &mut self,
        source: usize,
        destinations: impl IntoIterator<Item = usize>,
        types: impl IntoIterator<Item = crate::chart_event_copy::EventType>,
        focus: Option<(
            crate::chart_focus::Position,
            crate::timing_edit::live::Anchor,
        )>,
    ) -> Result<crate::chart_event_copy::live::Plan, Error> {
        crate::chart_event_copy::live::Plan::prepare(
            &mut self.file.session,
            source,
            destinations,
            types,
            focus,
        )
    }
    pub fn apply_chart_event_copy_live(
        &mut self,
        plan: crate::chart_event_copy::live::Plan,
    ) -> Result<crate::chart_event_copy::live::Application, Error> {
        if plan.chart() != self.active_chart() {
            return Err(Error::ExternalChange);
        }
        plan.apply(&mut self.file.session)
    }
}

impl Workspace {
    pub fn edit_pack_properties_reviewed(
        &mut self,
        review: &crate::pack::storage::history::Review,
        after: crate::pack::Data,
    ) -> Result<bool, Error> {
        self.file
            .session
            .edit_pack_properties_reviewed(review, after)
    }
}
