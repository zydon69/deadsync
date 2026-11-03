//! Commands store only changed notes and tag values. One gesture is one history
//! entry; failed gestures roll back and never erase the redo branch.
#[path = "history/attack_selection.rs"]
mod attack_selection;
pub mod authoring;
pub(crate) mod chart_copy_live;
mod chart_properties;
mod event_properties;
mod marker_metadata;
mod pack_properties;
pub(crate) mod pattern_added;
pub mod profile_creation;
mod profile_load;
pub(crate) mod selection_effect;
pub(crate) mod song_live;
mod song_metadata;
pub(crate) mod song_presentation;
pub(crate) mod timing_live;
mod timing_migration;
pub use pattern_added::PatternAddedEffect;
pub use selection_effect::SelectionEffect;
mod chart_set;
pub mod file_intent;
pub(crate) mod media_relocation;
mod output_snapshot;
mod song_timing;

use crate::{
    Error,
    document::{Document, escape_text},
    notes::{Grid, Note, Position, layout_columns},
};
use std::{collections::BTreeMap, ops::Range, sync::Arc};

#[derive(Clone, Debug)]
struct NoteChange {
    position: Position,
    before: Option<Note>,
    after: Option<Note>,
}

#[derive(Debug)]
struct DocumentState {
    document: Document,
    grids: BTreeMap<usize, Grid>,
    touched_grids: std::collections::BTreeSet<usize>,
    chart_ids: Vec<crate::generation::metadata::Guid>,
    song_timing: Option<crate::song_timing_chart::Choice>,
}

#[derive(Debug)]
enum Change {
    PackProperties(Box<crate::pack::storage::history::Edit>),
    ProfileEdit(Box<crate::generation::profiles::edit::Edit>),
    PatternReadd {
        chart: usize,
    },
    MarkerSeedTouch {
        chart: usize,
    },
    AttackIdentity(crate::attacks::selection::Lifecycle),
    SourcePreferences(Box<crate::audio_preferences::source_import::transaction::Change>),
    SongTimingProperties(Box<Change>),
    TimingMigration(Box<Change>),
    ChartProperties {
        effect: chart_properties::Effect,
        change: Box<Change>,
    },
    TimingEdit(Box<Change>),
    SongPropertiesLive(Box<Change>),
    ChartEventCopy(Box<Change>),
    SongPropertiesMetadata(Box<Change>),
    MarkerMetadata(Box<Change>),
    SongTimingChoice {
        before: crate::song_timing_chart::Choice,
        after: crate::song_timing_chart::Choice,
    },
    KeyBindingPreferences(Box<crate::key_binding_preferences::history::Change>),
    NoteColorPreferences(Box<crate::note_color_preferences::history::Change>),
    AudioPreferences(Box<crate::audio_preferences::history::Change>),
    ReceptorPreferences(Box<crate::receptor_preferences::history::Change>),
    ScrollPreferences(Box<crate::scroll_preferences::history::Change>),
    RestoreOptions(Box<crate::options_preferences::restore::Change>),
    RestoreScroll(Box<crate::scroll_preferences::restore::Change>),
    PerformancePreferences(Box<crate::performance::history::Change>),
    GeneralOptions(Box<crate::options_preferences::general::history::Change>),
    DisplayOptions(Box<crate::options_preferences::history::Change>),
    VisualPreferences(Box<crate::visual_preferences::Change>),
    DarkPreferences(Box<crate::dark_preferences::history::Change>),
    StreamPreferences(Box<crate::stream_preferences::history::Change>),
    SelectionPreferences(Box<crate::selection_preferences::history::Change>),
    DensityPreferences(Box<crate::density_preferences::history::Change>),
    Group(Vec<Change>),
    PlayerCount {
        chart: usize,
        before: u32,
        after: u32,
    },
    ProfileDeletion {
        source: Box<crate::generation::profiles::ProfileFile>,
        deleted: Option<crate::generation::profiles::DeletedProfile>,
    },
    ProfileCreation {
        profile: Box<crate::generation::profiles::Profile>,
        directory: std::path::PathBuf,
        file: Option<Box<crate::generation::profiles::ProfileFile>>,
        removed: Option<crate::generation::profiles::DeletedProfile>,
    },
    DocumentSwap(Box<DocumentState>),
    Notes {
        chart: usize,
        changes: Vec<NoteChange>,
    },
    Value {
        range: Range<usize>,
        before: Option<Arc<[u8]>>,
        after: Option<Arc<[u8]>>,
    },
    AddedProperty {
        chart: Option<usize>,
        name: String,
        before: Option<Arc<[u8]>>,
        after: Option<Arc<[u8]>>,
    },
}

#[derive(Debug)]
struct Entry {
    authoring_policy: Option<(
        crate::output_policy::OutputPolicy,
        crate::output_policy::OutputPolicy,
    )>,
    selection: Option<selection_effect::SelectionHistory>,
    pattern_added: Option<pattern_added::History>,
    label: String,
    change: Change,
    before_revision: u64,
    after_revision: u64,
    before_content_revision: u64,
    after_content_revision: u64,
}

/// Actual history outcome, independent of user-visible labels or chart focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEffect {
    pub applied: bool,
    pub before_revision: u64,
    pub after_revision: u64,
    pub content_changed: bool,
    pub chart_structure: Option<ChartStructureDelta>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartStructureDelta {
    pub before: Vec<crate::generation::metadata::Guid>,
    pub after: Vec<crate::generation::metadata::Guid>,
}

/// A revision is only meaningful within one live session. Recovery/reload creates
/// a new identity even when bytes and revision numbers happen to be identical.
#[derive(Clone, Debug)]
pub(crate) struct RevisionToken {
    identity: Arc<()>,
    revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ContentToken {
    identity: Arc<()>,
    revision: u64,
}

/// Non-reusable review lease. Unlike a restorable revision number, a successful
/// edit, Undo or Redo replaces this identity even when bytes return unchanged.
#[derive(Clone, Debug)]
pub(crate) struct ApplicationReviewToken {
    session: Arc<()>,
    epoch: Arc<()>,
}
pub struct ChartAddition<'a> {
    pub source_chart: usize,
    pub layout: &'a str,
    pub grid: &'a Grid,
}

pub struct Session {
    profile_edit_update: Option<crate::generation::profiles::edit::Update>,
    profile_durability:
        BTreeMap<std::path::PathBuf, crate::generation::profiles::edit::DurabilityReceipt>,
    authoring_policy_update: Option<crate::output_policy::OutputPolicy>,
    application_review_epoch: Arc<()>,
    // Pending settings carry file snapshots; allocate only while an update exists.
    key_binding_preferences_update: Option<Box<crate::key_binding_preferences::Update>>,
    note_color_preferences_update: Option<Box<crate::note_color_preferences::Update>>,
    source_preferences_context: Option<
        Box<(
            ApplicationReviewToken,
            crate::audio_preferences::source_import::Snapshot,
        )>,
    >,
    source_preferences_update: Option<Box<crate::audio_preferences::source_import::Update>>,
    audio_preferences_update: Option<Box<crate::audio_preferences::Update>>,
    audio_preferences_current: Option<crate::audio_preferences::Audio>,
    receptor_preferences_update: Option<Box<crate::receptor_preferences::Update>>,
    scroll_preferences_update: Option<Box<crate::scroll_preferences::Update>>,
    restore_options_update: Option<Box<crate::options_preferences::restore::Update>>,
    restore_scroll_update: Option<Box<crate::scroll_preferences::restore::Update>>,
    performance_preferences_update: Option<Box<crate::performance::Update>>,
    general_options_update: Option<Box<crate::options_preferences::general::Update>>,
    display_options_update: Option<Box<crate::options_preferences::Update>>,
    visual_preferences_update: Option<Box<crate::visual_preferences::Update>>,
    dark_preferences_update: Option<Box<crate::dark_preferences::Update>>,
    stream_preferences_update: Option<Box<crate::stream_preferences::Update>>,
    selection_preferences_update: Option<Box<crate::selection_preferences::Update>>,
    density_preferences_update: Option<Box<crate::density_preferences::Update>>,
    event_selection_update: Option<SelectionEffect>,
    pattern_added_update: Vec<pattern_added::Target>,
    attack_selection: crate::attacks::selection::Memory,
    identity: Arc<()>,
    allows_external_edits: bool,
    document: Document,
    grids: BTreeMap<usize, Grid>,
    touched_grids: std::collections::BTreeSet<usize>,
    chart_ids: Vec<crate::generation::metadata::Guid>,
    song_timing: Option<crate::song_timing_chart::Choice>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    revision: u64,
    saved_revision: u64,
    content_revision: u64,
    next_revision: u64,
    history_capacity: usize,
}

impl Session {
    pub fn new(document: Document) -> Self {
        let chart_ids = (0..document.charts().len())
            .map(|_| crate::generation::metadata::Guid::random())
            .collect();
        Self {
            profile_edit_update: None,
            profile_durability: Default::default(),
            application_review_epoch: Arc::new(()),
            source_preferences_context: None,
            source_preferences_update: None,
            audio_preferences_update: None,
            audio_preferences_current: None,
            key_binding_preferences_update: None,
            note_color_preferences_update: None,
            receptor_preferences_update: None,
            scroll_preferences_update: None,
            restore_options_update: None,
            restore_scroll_update: None,
            performance_preferences_update: None,
            general_options_update: None,
            display_options_update: None,
            visual_preferences_update: None,
            dark_preferences_update: None,
            stream_preferences_update: None,
            selection_preferences_update: None,
            density_preferences_update: None,
            event_selection_update: None,
            pattern_added_update: Vec::new(),
            attack_selection: Default::default(),
            identity: Arc::new(()),
            authoring_policy_update: None,
            allows_external_edits: true,
            document,
            grids: BTreeMap::new(),
            touched_grids: Default::default(),
            chart_ids,
            song_timing: None,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            saved_revision: 0,
            content_revision: 0,
            next_revision: 1,
            history_capacity: 1024,
        }
    }

    fn staged(document: Document) -> Self {
        let mut session = Self::new(document);
        session.allows_external_edits = false;
        session
    }

    pub(crate) fn recovered(document: Document) -> Self {
        let mut session = Self::new(document);
        session.saved_revision = u64::MAX;
        session
    }

    pub(crate) fn application_review_token(&self) -> ApplicationReviewToken {
        ApplicationReviewToken {
            session: Arc::clone(&self.identity),
            epoch: Arc::clone(&self.application_review_epoch),
        }
    }
    pub(crate) fn accepts_application_review(&self, token: &ApplicationReviewToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.session)
            && Arc::ptr_eq(&self.application_review_epoch, &token.epoch)
    }
    pub(crate) fn revision_token(&self) -> RevisionToken {
        RevisionToken {
            identity: self.identity.clone(),
            revision: self.revision,
        }
    }
    pub(crate) fn content_token(&self) -> ContentToken {
        ContentToken {
            identity: self.identity.clone(),
            revision: self.content_revision,
        }
    }
    pub(crate) fn accepts_content_token(&self, token: &ContentToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.identity) && self.content_revision == token.revision
    }
    /// View registries survive content edits but never migrate to another song
    /// session, including when both sessions currently contain no charts.
    pub(crate) fn owns_content_token(&self, token: &ContentToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.identity)
    }

    pub(crate) fn after_profile_creation(
        &self,
        token: &RevisionToken,
    ) -> Result<RevisionToken, Error> {
        let valid = Arc::ptr_eq(&self.identity, &token.identity)
            && self.undo.last().is_some_and(|entry| {
                entry.before_revision == token.revision
                    && entry.after_revision == self.revision
                    && matches!(
                        entry.change,
                        Change::ProfileCreation { .. } | Change::ProfileEdit(_)
                    )
            });
        if !valid {
            return Err(Error::InvalidEdit(
                "Draft cannot follow this profile-library change".into(),
            ));
        }
        Ok(self.revision_token())
    }

    /// Rebind only across the immediately completed, disk-only profile action.
    /// Both directions authenticate the same history edge and session identity.
    pub(crate) fn after_profile_history(
        &self,
        token: &RevisionToken,
        redo: bool,
    ) -> Result<RevisionToken, Error> {
        let entry = if redo {
            self.undo.last()
        } else {
            self.redo.last()
        };
        let valid = Arc::ptr_eq(&self.identity, &token.identity)
            && entry.is_some_and(|entry| {
                let (from, to) = if redo {
                    (entry.before_revision, entry.after_revision)
                } else {
                    (entry.after_revision, entry.before_revision)
                };
                token.revision == from
                    && self.revision == to
                    && matches!(
                        entry.change,
                        Change::ProfileCreation { .. }
                            | Change::ProfileEdit(_)
                            | Change::ProfileDeletion { .. }
                    )
            });
        if !valid {
            return Err(Error::InvalidEdit(
                "Draft cannot follow this profile-library change".into(),
            ));
        }
        Ok(self.revision_token())
    }

    pub(crate) fn accepts_token(&self, token: &RevisionToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.identity) && self.revision == token.revision
    }

    pub(crate) fn redo_reaches(&self, token: &RevisionToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.identity)
            && self
                .redo
                .last()
                .is_some_and(|entry| entry.after_revision == token.revision)
    }

    pub fn document(&self) -> &Document {
        &self.document
    }
    /// Resident chart identities for presentation tie breaks. These are never
    /// written to the simfile and travel with structural undo/redo snapshots.
    pub fn chart_ids(&self) -> &[crate::generation::metadata::Guid] {
        &self.chart_ids
    }
    pub fn is_dirty(&self) -> bool {
        self.content_revision != self.saved_revision
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn content_revision(&self) -> u64 {
        self.content_revision
    }
    pub fn undo_changes_receptor_preferences(&self) -> bool {
        self.undo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::ReceptorPreferences(_)))
    }
    pub fn undo_changes_dark_preferences(&self) -> bool {
        self.undo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::DarkPreferences(_)))
    }
    pub fn undo_changes_stream_preferences(&self) -> bool {
        self.undo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::StreamPreferences(_)))
    }
    pub fn redo_changes_receptor_preferences(&self) -> bool {
        self.redo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::ReceptorPreferences(_)))
    }
    pub fn redo_changes_dark_preferences(&self) -> bool {
        self.redo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::DarkPreferences(_)))
    }
    pub fn redo_changes_stream_preferences(&self) -> bool {
        self.redo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::StreamPreferences(_)))
    }
    pub fn undo_changes_note_color_preferences(&self) -> bool {
        self.undo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::NoteColorPreferences(_)))
    }
    pub fn undo_changes_key_binding_preferences(&self) -> bool {
        self.undo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::KeyBindingPreferences(_)))
    }
    pub fn redo_changes_note_color_preferences(&self) -> bool {
        self.redo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::NoteColorPreferences(_)))
    }
    pub fn redo_changes_key_binding_preferences(&self) -> bool {
        self.redo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::KeyBindingPreferences(_)))
    }
    pub fn undo_changes_audio_preferences(&self) -> bool {
        self.undo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::AudioPreferences(_)))
    }
    pub fn redo_changes_audio_preferences(&self) -> bool {
        self.redo
            .last()
            .is_some_and(|entry| matches!(entry.change, Change::AudioPreferences(_)))
    }
    pub(crate) fn commit_marker_seed_touch(&mut self, chart: usize) -> Result<bool, Error> {
        if self.chart_ids.get(chart).is_none() {
            return Err(Error::InvalidEdit(
                "Pattern seed chart is unavailable".into(),
            ));
        }
        self.execute(
            "Set pattern random seed".into(),
            Change::MarkerSeedTouch { chart },
        )?;
        Ok(true)
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }
    pub(crate) fn invalidate_saved_position(&mut self) {
        self.saved_revision = u64::MAX;
    }
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.content_revision;
    }

    pub fn grid(&mut self, chart: usize) -> Result<&Grid, Error> {
        if !self.grids.contains_key(&chart) {
            let grid = Self::document_grid(&self.document, chart)?;
            self.grids.insert(chart, grid);
        }
        Ok(&self.grids[&chart])
    }

    /// The resident baseline includes declared empty participants, not just the
    /// owners encoded in NOTES. Loading and serialization must compare the same
    /// effective grid, otherwise Undo can rewrite an untouched empty note block.
    fn document_grid(document: &Document, chart: usize) -> Result<Grid, Error> {
        let descriptor = document
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?;
        let mut grid = Grid::parse_layout(
            document.value(descriptor.notes.clone()),
            &descriptor.steps_type,
        )?;
        if crate::notes::supports_variable_players(&descriptor.steps_type) {
            if let Ok(Some(metadata)) =
                crate::generation::metadata::ChartData::read(document, chart)
            {
                // Metadata never discards a participant who owns a note or tail.
                grid.set_players(grid.minimum_players().max(metadata.max_player_index + 1))?;
            }
        }
        Ok(grid)
    }

    /// Persist Fumen's effective Pump co-op layout as one reversible structural
    /// gesture. Merely opening a legacy Double chart stays byte-preserving.
    pub fn normalize_stepf2_chart(&mut self, chart: usize) -> Result<bool, Error> {
        let descriptor = self
            .document
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?;
        if descriptor.steps_type != "pump-double" || !self.grid(chart)?.uses_stepf2() {
            return Ok(false);
        }
        let mut expected = self.grid(chart)?.clone();
        let participants =
            match crate::generation::metadata::ChartData::read(&self.document, chart)? {
                Some(metadata) => expected
                    .minimum_players()
                    .max(metadata.max_player_index + 1),
                None => expected.players().max(2),
            };
        expected.set_players(participants)?;
        let current = self.document.reparse(self.serialize()?)?;
        let after = current.with_chart_layout(chart, "pump-routine")?;
        let mut validation = Session::staged(after.clone());
        if validation.grid(chart)? != &expected {
            return Err(Error::InvalidEdit(
                "Co-op normalization changed note data".into(),
            ));
        }
        let choice = self.song_timing_choice()?;
        let after = Self::materialize_song_timing_state(after, &self.chart_ids, &choice)?;
        self.execute(
            "Normalize StepF2 co-op layout".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
                touched_grids: Default::default(),
                chart_ids: self.chart_ids.clone(),
                song_timing: Some(self.song_timing_choice()?),
            })),
        )?;
        Ok(true)
    }

    pub fn edit_notes(
        &mut self,
        chart: usize,
        label: impl Into<String>,
        edits: impl IntoIterator<Item = (Position, Option<Note>)>,
    ) -> Result<bool, Error> {
        let edits: BTreeMap<_, _> = edits.into_iter().collect();
        let grid = self.grid(chart)?;
        if !grid.uses_stepf2() {
            for note in edits.values().flatten() {
                crate::stepf2::require_standard(note)?;
            }
        }
        if edits
            .keys()
            .any(|position| position.lane >= grid.columns() || position.player >= grid.players())
        {
            return Err(Error::InvalidEdit(
                "Lane or participant lies outside this chart".into(),
            ));
        }
        let changes: Vec<_> = edits
            .into_iter()
            .filter_map(|(position, after)| {
                let before = grid.get(position).cloned();
                (before != after).then_some(NoteChange {
                    position,
                    before,
                    after,
                })
            })
            .collect();
        if changes.is_empty() {
            return Ok(false);
        }
        self.execute(label.into(), Change::Notes { chart, changes })?;
        Ok(true)
    }

    pub(crate) fn edit_generated_notes(
        &mut self,
        chart: usize,
        label: &str,
        edits: impl IntoIterator<Item = (Position, Option<Note>)>,
    ) -> Result<bool, Error> {
        let before = Session::new(self.document.reparse(self.serialize()?)?);
        self.edit_chart_atomic(chart, label, |temporary| {
            if temporary.edit_notes(chart, label, edits)? {
                crate::attacks::anchoring::preserve(&before, temporary)?;
            }
            Ok(())
        })
    }

    /// Text values are escaped centrally; callers cannot inject another tag.
    pub fn set_text(
        &mut self,
        chart: Option<usize>,
        name: &str,
        text: &str,
    ) -> Result<bool, Error> {
        let after: Arc<[u8]> = escape_text(text).into();
        self.set_property_bytes(chart, name, after)
    }

    pub(crate) fn set_property_bytes(
        &mut self,
        chart: Option<usize>,
        name: &str,
        after: Arc<[u8]>,
    ) -> Result<bool, Error> {
        if matches!(
            name.to_ascii_uppercase().as_str(),
            "NOTES" | "NOTES2" | "NOTEDATA" | "STEPSTYPE"
        ) {
            return Err(Error::InvalidEdit(
                "Structural chart properties require a chart operation".into(),
            ));
        }
        let Some(tag) = self.document.tag(chart, name) else {
            let before = self.document.added_property(chart, name);
            if before.as_deref() == Some(after.as_ref()) {
                return Ok(false);
            }
            self.execute(
                format!("Change {name}"),
                Change::AddedProperty {
                    chart,
                    name: name.to_owned(),
                    before,
                    after: Some(after),
                },
            )?;
            return Ok(true);
        };
        let range = tag.value.clone();
        if self.document.value(range.clone()) == after.as_ref() {
            return Ok(false);
        }
        let before = self.document.replacement(&range);
        self.execute(
            format!("Change {name}"),
            Change::Value {
                range,
                before,
                after: Some(after),
            },
        )?;
        Ok(true)
    }

    /// Validate distinct tag edits on an isolated document, then record one
    /// reversible property gesture without serializing or copying note data.
    pub fn set_properties_atomic(
        &mut self,
        chart: Option<usize>,
        properties: &[(&str, &str)],
        label: impl Into<String>,
    ) -> Result<bool, Error> {
        let properties = properties
            .iter()
            .map(|(name, text)| {
                (
                    chart,
                    (*name).to_owned(),
                    Arc::<[u8]>::from(escape_text(text)),
                )
            })
            .collect::<Vec<_>>();
        self.set_property_bytes_atomic(&properties, label)
    }

    pub(crate) fn set_sm_header(
        &mut self,
        chart: usize,
        field: usize,
        text: &str,
    ) -> Result<bool, Error> {
        if !matches!(field, 1 | 2 | 3) {
            return Err(Error::InvalidEdit("Unsupported SM header edit".into()));
        }
        let range = self.document.sm_header_range(chart, field)?;
        let after: Arc<[u8]> = escape_text(text).into();
        if self.document.value(range.clone()) == after.as_ref() {
            return Ok(false);
        }
        let before = self.document.replacement(&range);
        self.execute(
            "Change chart header".into(),
            Change::Value {
                range,
                before,
                after: Some(after),
            },
        )?;
        Ok(true)
    }

    /// Preserve copied timing fields byte-for-byte while recording one gesture.
    pub(crate) fn set_property_bytes_atomic(
        &mut self,
        properties: &[(Option<usize>, String, Arc<[u8]>)],
        label: impl Into<String>,
    ) -> Result<bool, Error> {
        if properties.is_empty() {
            return Ok(false);
        }
        let mut names = std::collections::BTreeSet::new();
        let mut temporary = Session::staged(self.document.clone());
        temporary.chart_ids = self.chart_ids.clone();
        temporary.song_timing = Some(self.song_timing_choice()?);
        for (scope, name, bytes) in properties {
            if !names.insert((*scope, name.to_ascii_uppercase())) {
                return Err(Error::InvalidEdit(
                    "Atomic property edits must have distinct names".into(),
                ));
            }
            temporary.set_property_bytes(*scope, name, bytes.clone())?;
        }
        let changes_clock = properties.iter().any(|(_, name, _)| {
            let name = name.to_ascii_uppercase();
            crate::timing_edit::TIMING_TAGS.contains(&name.as_str())
                || name == "DISPLAYBPM"
                || name.starts_with("GACHARTDATA")
                || crate::timing_edit::resident::is_tag(&name)
        });
        // Explicit attack replacements already own their positions. Otherwise
        // timing and derived attack file times form a single reversible gesture.
        if changes_clock
            && !temporary.undo.is_empty()
            && !properties.iter().any(|(_, name, _)| {
                name.eq_ignore_ascii_case("ATTACKS") || crate::attacks::property::is_owner_tag(name)
            })
        {
            temporary.grids = self.grids.clone();
            crate::attacks::anchoring::preserve(self, &mut temporary)?;
        }
        let changes: Vec<_> = temporary
            .undo
            .into_iter()
            .map(|entry| entry.change)
            .collect();
        if changes.is_empty() {
            return Ok(false);
        }
        self.execute(label.into(), Change::Group(changes))?;
        Ok(true)
    }

    /// Stage one chart's note/property changes, then commit them as one
    /// gesture. Failure leaves both history branches and all document data intact.
    pub fn edit_chart_atomic(
        &mut self,
        chart: usize,
        label: impl Into<String>,
        operation: impl FnOnce(&mut Session) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        let grid = self.grid(chart)?.clone();
        let mut temporary = Session::staged(self.document.clone());
        temporary.chart_ids = self.chart_ids.clone();
        temporary.song_timing = Some(self.song_timing_choice()?);
        // Timing transactions may inspect other charts to preserve their attack
        // positions. They must see those charts' unsaved note edits as well.
        temporary.grids = self.grids.clone();
        temporary.grids.insert(chart, grid);
        operation(&mut temporary)?;
        let receipt = pattern_added::History {
            before: temporary
                .undo
                .iter()
                .rev()
                .filter_map(|entry| entry.pattern_added.as_ref())
                .flat_map(|receipt| receipt.before.iter().cloned())
                .collect(),
            after: std::mem::take(&mut temporary.pattern_added_update),
        };
        let changes: Vec<_> = temporary
            .undo
            .into_iter()
            .map(|entry| entry.change)
            .collect();
        fn allowed(change: &Change, chart: usize) -> bool {
            match change {
                Change::ChartProperties { .. }
                | Change::SongTimingProperties(_)
                | Change::TimingMigration(_)
                | Change::TimingEdit(_)
                | Change::MarkerMetadata(_)
                | Change::SongPropertiesLive(_)
                | Change::SongPropertiesMetadata(_)
                | Change::ChartEventCopy(_)
                | Change::SongTimingChoice { .. }
                | Change::DocumentSwap(_)
                | Change::PackProperties(_)
                | Change::ProfileEdit(_)
                | Change::ProfileDeletion { .. }
                | Change::ProfileCreation { .. }
                | Change::SourcePreferences(_)
                | Change::AudioPreferences(_)
                | Change::KeyBindingPreferences(_)
                | Change::NoteColorPreferences(_)
                | Change::ReceptorPreferences(_)
                | Change::DensityPreferences(_)
                | Change::ScrollPreferences(_)
                | Change::RestoreOptions(_)
                | Change::RestoreScroll(_)
                | Change::PerformancePreferences(_)
                | Change::GeneralOptions(_)
                | Change::DisplayOptions(_)
                | Change::VisualPreferences(_)
                | Change::DarkPreferences(_)
                | Change::StreamPreferences(_)
                | Change::SelectionPreferences(_) => false,
                Change::AttackIdentity(_) => true,
                Change::MarkerSeedTouch { chart: target }
                | Change::PatternReadd { chart: target }
                | Change::Notes { chart: target, .. }
                | Change::PlayerCount { chart: target, .. } => *target == chart,
                Change::Group(changes) => changes.iter().all(|change| allowed(change, chart)),
                _ => true,
            }
        }
        if changes.iter().any(|change| !allowed(change, chart)) {
            return Err(Error::InvalidEdit(
                "Chart gestures cannot change another chart's notes or structure".into(),
            ));
        }
        if changes.is_empty() {
            self.record_pattern_history(receipt, false);
            return Ok(false);
        }
        self.execute(label.into(), Change::Group(changes))?;
        self.record_pattern_history(receipt, true);
        Ok(true)
    }

    /// Create a profile as one disk-only gesture in global history.
    /// Existing library identities and destination files remain protected.
    pub fn set_participants(&mut self, chart: usize, requested: u32) -> Result<bool, Error> {
        let layout = self
            .document
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?
            .steps_type
            .clone();
        let grid = self.grid(chart)?;
        let before = grid.players();
        let after = requested.max(grid.minimum_players()).max(1);
        if before == after {
            return Ok(false);
        }
        if !crate::notes::supports_variable_players(&layout) {
            return Err(Error::Unsupported(
                "This chart type has a fixed participant count".into(),
            ));
        }
        grid.validate_players(after)?;
        let mut metadata =
            crate::generation::metadata::ChartData::existing_or_initial(&self.document, chart)?;
        metadata.max_player_index = after - 1;
        let mut staged = Session::staged(self.document.clone());
        staged.chart_ids = self.chart_ids.clone();
        staged.song_timing = Some(self.song_timing_choice()?);
        metadata.save(&mut staged, chart)?;
        let mut changes: Vec<_> = staged.undo.into_iter().map(|entry| entry.change).collect();
        changes.push(Change::PlayerCount {
            chart,
            before,
            after,
        });
        self.execute("Change chart participants".into(), Change::Group(changes))?;
        Ok(true)
    }

    pub fn create_profile(
        &mut self,
        directory: &std::path::Path,
        profile: &crate::generation::profiles::Profile,
    ) -> Result<std::path::PathBuf, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot create profile files".into(),
            ));
        }
        let loaded = crate::generation::profiles::Loaded::directory(directory)?;
        if loaded
            .profiles
            .contains_key(&(profile.config().kind(), profile.guid().clone()))
        {
            return Err(Error::InvalidEdit(
                "A profile with this identity already exists in the library".into(),
            ));
        }
        self.retry_profile_durability()?;
        let path = directory.join(profile.filename());
        self.execute(
            "Create generation profile".into(),
            Change::ProfileCreation {
                profile: Box::new(profile.clone()),
                directory: directory.to_owned(),
                file: None,
                removed: None,
            },
        )?;
        Ok(path)
    }

    /// Delete a custom profile and repair active-song references as one gesture.
    pub fn delete_profile(
        &mut self,
        file: &crate::generation::profiles::ProfileFile,
    ) -> Result<(), Error> {
        self.retry_profile_durability()?;
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot delete profile files".into(),
            ));
        }
        let reset = crate::generation::profile_references::Reset::capture(
            self,
            file.profile().config().kind(),
            file.profile().guid(),
        )?;
        let (path, bytes) = file.snapshot();
        let source = crate::generation::profiles::ProfileFile::capture_snapshot(
            &path,
            bytes,
            file.profile().clone(),
        )?;
        let mut temporary = Session::staged(self.document.clone());
        temporary.chart_ids = self.chart_ids.clone();
        temporary.song_timing = Some(self.song_timing_choice()?);
        temporary.set_property_bytes_atomic(reset.properties(), "Repair profile references")?;
        let mut changes: Vec<_> = temporary
            .undo
            .into_iter()
            .map(|entry| entry.change)
            .collect();
        // Disk deletion comes last forward and first backward. A disk conflict
        // leaves/restores every metadata edit and both history stacks intact.
        changes.push(Change::ProfileDeletion {
            source: Box::new(source),
            deleted: None,
        });
        self.execute("Delete generation profile".into(), Change::Group(changes))
    }

    pub(crate) fn apply_grid_delta(
        &mut self,
        chart: usize,
        staged: &Grid,
        label: &str,
    ) -> Result<bool, Error> {
        let original = self.grid(chart)?;
        original.require_same_opaque_cells(staged)?;
        if original.columns() != staged.columns() || original.players() != staged.players() {
            return Err(Error::InvalidEdit(
                "Grid replacement cannot change layout or participants".into(),
            ));
        }
        let positions: std::collections::BTreeSet<_> = original
            .iter()
            .map(|(pos, _)| pos)
            .chain(staged.iter().map(|(pos, _)| pos))
            .collect();
        let edits: Vec<_> = positions
            .into_iter()
            .filter_map(|pos| {
                let after = staged.get(pos);
                (original.get(pos) != after).then(|| (pos, after.cloned()))
            })
            .collect();
        self.edit_notes(chart, label, edits)
    }

    /// Authenticate live application settings immediately before a command or
    /// Undo/Redo. Consumed once; an intervening edit (including edit--Undo)
    /// invalidates the revision token even if document bytes are identical.
    pub fn capture_source_preferences_context(&mut self, file: &crate::audio_preferences::File) {
        self.source_preferences_context = Some(Box::new((
            self.application_review_token(),
            crate::audio_preferences::source_import::Snapshot::capture(file),
        )));
    }
    pub fn take_source_preferences_update(
        &mut self,
    ) -> Option<Box<crate::audio_preferences::source_import::Update>> {
        self.source_preferences_update.take()
    }
    pub fn import_source_preferences(
        &mut self,
        draft: crate::audio_preferences::source_import::Draft,
        current: &crate::audio_preferences::File,
        focus: Option<usize>,
        accept_adaptations: bool,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged edits cannot import application preferences".into(),
            ));
        }
        if !draft.is_current(self, current, focus) {
            return Err(Error::ExternalChange);
        }
        let change = draft.into_change(accept_adaptations)?;
        if change.is_empty() {
            return Ok(false);
        }
        self.capture_source_preferences_context(current);
        self.execute(
            "Import GrooveAuthor preferences".into(),
            Change::SourcePreferences(Box::new(change)),
        )?;
        Ok(true)
    }
    /// Source tick shortcuts change preferences without entering or clearing
    /// the editor action queue. Disk publication still uses the same field CAS.
    pub fn set_audio_preferences_untracked(
        &mut self,
        before: crate::audio_preferences::Audio,
        after: crate::audio_preferences::Audio,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change audio preferences".into(),
            ));
        }
        if file.is_some_and(|file| file.audio() != before) {
            return Err(Error::ExternalChange);
        }
        if before == after {
            return Ok(false);
        }
        let mut change = crate::audio_preferences::history::Change {
            before,
            after,
            file: file.cloned(),
        };
        let update = change.apply(
            true,
            Some(self.audio_preferences_current.unwrap_or(before)),
            file,
        )?;
        self.audio_preferences_current = Some(update.audio);
        self.audio_preferences_update = Some(Box::new(update));
        self.application_review_epoch = Arc::new(());
        self.source_preferences_context = None;
        Ok(true)
    }
    /// Application preferences share gesture history but do not dirty the song.
    pub fn set_audio_preferences(
        &mut self,
        before: crate::audio_preferences::Audio,
        after: crate::audio_preferences::Audio,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change audio preferences".into(),
            ));
        }
        if file.is_some_and(|file| file.audio() != before) {
            return Err(Error::InvalidEdit(
                "Audio preference snapshot differs from the active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        let previous_audio = self.audio_preferences_current;
        if previous_audio.is_none() {
            self.audio_preferences_current = Some(before);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        let result = self.execute(
            "Audio preferences".into(),
            Change::AudioPreferences(Box::new(crate::audio_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        );
        if let Err(error) = result {
            self.audio_preferences_current = previous_audio;
            return Err(error);
        }
        Ok(true)
    }
    pub fn set_density_preferences(
        &mut self,
        before: crate::density_preferences::Preferences,
        after: crate::density_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change density preferences".into(),
            ));
        }
        before.validate()?;
        after.validate()?;
        if file.is_some_and(|f| f.density() != before) {
            return Err(Error::InvalidEdit(
                "Density preference snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Density graph preferences".into(),
            Change::DensityPreferences(Box::new(crate::density_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_density_preferences_update(
        &mut self,
    ) -> Option<crate::density_preferences::Update> {
        self.density_preferences_update.take().map(|update| *update)
    }
    pub fn set_receptor_preferences(
        &mut self,
        before: crate::receptor_preferences::Preferences,
        after: crate::receptor_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change receptor preferences".into(),
            ));
        }
        if file.is_some_and(|f| f.receptors() != before) {
            return Err(Error::InvalidEdit(
                "Receptor preference snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Receptor preferences".into(),
            Change::ReceptorPreferences(Box::new(crate::receptor_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_receptor_preferences_update(
        &mut self,
    ) -> Option<crate::receptor_preferences::Update> {
        self.receptor_preferences_update
            .take()
            .map(|update| *update)
    }
    pub fn set_selection_preferences(
        &mut self,
        before: crate::selection_preferences::Preferences,
        after: crate::selection_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change selection preferences".into(),
            ));
        }
        before.validate()?;
        after.validate()?;
        if file.is_some_and(|file| file.selection() != before) {
            return Err(Error::InvalidEdit(
                "Selection preference snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Selection preferences".into(),
            Change::SelectionPreferences(Box::new(crate::selection_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_selection_preferences_update(
        &mut self,
    ) -> Option<crate::selection_preferences::Update> {
        self.selection_preferences_update
            .take()
            .map(|update| *update)
    }
    pub fn set_stream_preferences(
        &mut self,
        before: crate::stream_preferences::Preferences,
        after: crate::stream_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change stream preferences".into(),
            ));
        }
        before.validate()?;
        after.validate()?;
        if file.is_some_and(|f| f.streams() != before) {
            return Err(Error::InvalidEdit(
                "Stream snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Stream preferences".into(),
            Change::StreamPreferences(Box::new(crate::stream_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_stream_preferences_update(&mut self) -> Option<crate::stream_preferences::Update> {
        self.stream_preferences_update.take().map(|update| *update)
    }
    pub fn set_scroll_preferences(
        &mut self,
        before: crate::scroll_preferences::Preferences,
        after: crate::scroll_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change scroll preferences".into(),
            ));
        }
        if file.is_some_and(|f| f.scroll() != before) {
            return Err(Error::InvalidEdit(
                "Scroll preference snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Scroll preferences".into(),
            Change::ScrollPreferences(Box::new(crate::scroll_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_scroll_preferences_update(&mut self) -> Option<crate::scroll_preferences::Update> {
        self.scroll_preferences_update.take().map(|update| *update)
    }
    pub fn set_display_options(
        &mut self,
        before: crate::options_preferences::Display,
        after: crate::options_preferences::Display,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change display options".into(),
            ));
        }
        if file.is_some_and(|f| f.display_options() != before) {
            return Err(Error::InvalidEdit(
                "Display option snapshot differs from active settings".into(),
            ));
        }

        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Display options".into(),
            Change::DisplayOptions(Box::new(crate::options_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_display_options_update(&mut self) -> Option<crate::options_preferences::Update> {
        self.display_options_update.take().map(|update| *update)
    }
    pub fn set_visual_preferences(
        &mut self,
        before: crate::visual_preferences::Snapshot,
        after: crate::visual_preferences::Snapshot,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change visual preferences".into(),
            ));
        }
        if std::mem::discriminant(&before) != std::mem::discriminant(&after) {
            return Err(Error::InvalidEdit("Visual preference owners differ".into()));
        }
        before.validate()?;
        after.validate()?;
        if let Some(file) = file {
            if before.capture_like(file)? != before {
                return Err(Error::ExternalChange);
            }
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Visual preferences".into(),
            Change::VisualPreferences(Box::new(crate::visual_preferences::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_visual_preferences_update(&mut self) -> Option<crate::visual_preferences::Update> {
        self.visual_preferences_update.take().map(|update| *update)
    }
    pub fn set_dark_preferences(
        &mut self,
        before: crate::dark_preferences::Preferences,
        after: crate::dark_preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change dark preferences".into(),
            ));
        }
        if file.is_some_and(|f| f.dark() != before) {
            return Err(Error::InvalidEdit(
                "Dark preference snapshot differs from active settings".into(),
            ));
        }
        before.validate()?;
        after.validate()?;
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Dark preferences".into(),
            Change::DarkPreferences(Box::new(crate::dark_preferences::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn take_dark_preferences_update(&mut self) -> Option<crate::dark_preferences::Update> {
        self.dark_preferences_update.take().map(|update| *update)
    }
    pub fn set_note_color_preferences(
        &mut self,
        before: std::sync::Arc<crate::note_color_preferences::Preferences>,
        after: std::sync::Arc<crate::note_color_preferences::Preferences>,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change note color preferences".into(),
            ));
        }
        before.validate()?;
        after.validate()?;
        if file.is_some_and(|f| f.note_colors_arc().as_ref() != before.as_ref()) {
            return Err(Error::InvalidEdit(
                "Note color snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Note color preferences".into(),
            Change::NoteColorPreferences(Box::new(
                crate::note_color_preferences::history::Change {
                    before,
                    after,
                    file: file.cloned(),
                },
            )),
        )?;
        Ok(true)
    }
    pub fn take_note_color_preferences_update(
        &mut self,
    ) -> Option<crate::note_color_preferences::Update> {
        self.note_color_preferences_update
            .take()
            .map(|update| *update)
    }
    pub fn set_key_binding_preferences(
        &mut self,
        before: std::sync::Arc<crate::key_binding_preferences::Preferences>,
        after: std::sync::Arc<crate::key_binding_preferences::Preferences>,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change note color preferences".into(),
            ));
        }
        before.validate()?;
        after.validate()?;
        if file.is_some_and(|f| f.key_bindings().as_ref() != before.as_ref()) {
            return Err(Error::InvalidEdit(
                "Key binding snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Key binding preferences".into(),
            Change::KeyBindingPreferences(Box::new(
                crate::key_binding_preferences::history::Change {
                    before,
                    after,
                    file: file.cloned(),
                },
            )),
        )?;
        Ok(true)
    }
    pub fn take_key_binding_preferences_update(
        &mut self,
    ) -> Option<crate::key_binding_preferences::Update> {
        self.key_binding_preferences_update
            .take()
            .map(|update| *update)
    }
    pub fn take_audio_preferences_update(&mut self) -> Option<crate::audio_preferences::Update> {
        self.audio_preferences_update.take().map(|update| *update)
    }

    pub fn undo_with_effect(&mut self) -> Result<HistoryEffect, Error> {
        self.history_with_effect(false)
    }
    pub fn redo_with_effect(&mut self) -> Result<HistoryEffect, Error> {
        self.history_with_effect(true)
    }
    fn history_with_effect(&mut self, redo: bool) -> Result<HistoryEffect, Error> {
        let before_revision = self.revision;
        let before_content = self.content_revision;
        let before = self.chart_ids.clone();
        let applied = if redo { self.redo()? } else { self.undo()? };
        let chart_structure = (before != self.chart_ids).then(|| ChartStructureDelta {
            before,
            after: self.chart_ids.clone(),
        });
        Ok(HistoryEffect {
            applied,
            before_revision,
            after_revision: self.revision,
            content_changed: before_content != self.content_revision,
            chart_structure,
        })
    }
    pub fn undo(&mut self) -> Result<bool, Error> {
        self.profile_edit_update = None;
        let Some(mut entry) = self.undo.pop() else {
            return Ok(false);
        };
        let attack_selection = self.attack_selection.clone();
        if let Err(error) = self.apply(&mut entry.change, false) {
            self.attack_selection = attack_selection;
            self.undo.push(entry);
            return Err(error);
        }
        self.application_review_epoch = Arc::new(());
        self.authoring_policy_update = entry
            .authoring_policy
            .as_ref()
            .map(|(before, _)| before.clone())
            .or_else(|| self.authoring_policy_update.take());
        self.event_selection_update = entry
            .selection
            .as_ref()
            .map(|receipt| receipt.before.clone());
        self.revision = entry.before_revision;
        self.content_revision = entry.before_content_revision;
        if let Some(receipt) = &entry.pattern_added {
            self.pattern_added_update
                .extend(receipt.before.iter().cloned());
        }
        self.prune_attack_selection();
        self.redo.push(entry);
        self.trim_history();
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, Error> {
        self.profile_edit_update = None;
        let Some(mut entry) = self.redo.pop() else {
            return Ok(false);
        };
        let attack_selection = self.attack_selection.clone();
        if let Err(error) = self.apply(&mut entry.change, true) {
            self.attack_selection = attack_selection;
            self.redo.push(entry);
            return Err(error);
        }
        self.application_review_epoch = Arc::new(());
        self.authoring_policy_update = entry
            .authoring_policy
            .as_ref()
            .map(|(_, after)| after.clone())
            .or_else(|| self.authoring_policy_update.take());
        self.event_selection_update = entry
            .selection
            .as_ref()
            .map(|receipt| receipt.after.clone());
        self.revision = entry.after_revision;
        self.content_revision = entry.after_content_revision;
        if let Some(receipt) = &entry.pattern_added {
            self.pattern_added_update
                .extend(receipt.after.iter().cloned());
        }
        self.prune_attack_selection();
        self.undo.push(entry);
        self.trim_history();
        Ok(true)
    }

    /// Structural changes own the alternate document state. Swapping preserves
    /// earlier byte-span history and avoids cloning every loaded note grid.
    pub fn append_chart(
        &mut self,
        source_chart: usize,
        layout: &str,
        grid: &Grid,
        expected_revision: u64,
        label: impl Into<String>,
    ) -> Result<usize, Error> {
        let indices = self.append_charts(
            &[ChartAddition {
                source_chart,
                layout,
                grid,
            }],
            expected_revision,
            label,
        )?;
        Ok(indices[0])
    }

    /// Prepare every appended block against the same source, then publish one
    /// structural swap. Failed validation leaves both history branches intact.
    pub fn append_charts(
        &mut self,
        additions: &[ChartAddition<'_>],
        expected_revision: u64,
        label: impl Into<String>,
    ) -> Result<Vec<usize>, Error> {
        if self.revision != expected_revision {
            return Err(Error::InvalidEdit(
                "Document changed since chart preview".into(),
            ));
        }
        if additions.is_empty() {
            return Err(Error::InvalidEdit("No generated charts to apply".into()));
        }
        let mut blocks = Vec::with_capacity(additions.len());
        for addition in additions {
            let columns = layout_columns(addition.layout).ok_or_else(|| {
                Error::Unsupported(format!("Unknown chart layout {}", addition.layout))
            })?;
            if columns != addition.grid.columns() {
                return Err(Error::InvalidEdit(
                    "Generated grid does not match its chart layout".into(),
                ));
            }
            let notes: Arc<[u8]> = addition.grid.serialize()?.into();
            if Grid::parse_layout(&notes, addition.layout)? != *addition.grid {
                return Err(Error::InvalidEdit(
                    "Generated notes do not round-trip".into(),
                ));
            }
            blocks.push(self.document.cloned_chart_bytes(
                addition.source_chart,
                addition.layout,
                notes,
            )?);
        }
        let start = self.document.charts().len();
        let mut bytes = self.serialize()?;
        for block in blocks {
            bytes.extend_from_slice(b"\n\n");
            bytes.extend_from_slice(&block);
        }
        let mut after = self.document.reparse(Arc::<[u8]>::from(bytes))?;
        if self.document.format == crate::document::Format::Sm {
            for (offset, addition) in additions.iter().enumerate() {
                crate::generation::metadata::clone_chart_owner(
                    &self.document,
                    addition.source_chart,
                    &mut after,
                    start + offset,
                )?;
                crate::timing_edit::resident::clone_chart_owner(
                    &self.document,
                    addition.source_chart,
                    &mut after,
                    start + offset,
                )?;
                crate::attacks::property::clone_chart(
                    &self.document,
                    addition.source_chart,
                    &mut after,
                    start + offset,
                )?;
            }
            after = after.reparse(after.serialize())?;
        }
        if after.charts().len() != start + additions.len() {
            return Err(Error::InvalidEdit(
                "Generated chart structure is ambiguous".into(),
            ));
        }
        for (index, addition) in additions.iter().enumerate() {
            let descriptor = &after.charts()[start + index];
            if descriptor.steps_type != addition.layout
                || Grid::parse_layout(
                    after.value(descriptor.notes.clone()),
                    &descriptor.steps_type,
                )? != *addition.grid
            {
                return Err(Error::InvalidEdit(
                    "Generated notes changed inside the simfile".into(),
                ));
            }
        }
        let snapshot = Session::new(self.document.reparse(self.serialize()?)?);
        let mut staged = Session::new(after);
        let copies: Vec<_> = additions
            .iter()
            .enumerate()
            .map(|(index, addition)| (addition.source_chart, start + index))
            .collect();
        crate::attacks::anchoring::preserve_generated(&snapshot, &mut staged, &copies)?;
        let after = self.document.reparse(staged.serialize()?)?;
        let mut chart_ids = self.chart_ids.clone();
        chart_ids.extend((0..additions.len()).map(|_| crate::generation::metadata::Guid::random()));
        self.execute(
            label.into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
                touched_grids: Default::default(),
                chart_ids,
                song_timing: Some(self.song_timing_choice()?),
            })),
        )?;
        Ok((start..start + additions.len()).collect())
    }

    pub(crate) fn remove_chart(
        &mut self,
        chart: usize,
        timing_after: Option<&crate::song_timing_chart::Choice>,
    ) -> Result<(), Error> {
        crate::song_properties::validate_structural_deletion(&self.document)?;
        let choice = self.song_timing_choice()?;
        let selected = choice.identity() == self.chart_ids.get(chart);
        let choice = if selected {
            match timing_after {
                Some(choice) => choice.clone(),
                None => {
                    let snapshot = self.song_timing_snapshot()?.without_source(chart);
                    if snapshot.keys().len() > 1 {
                        return Err(Error::InvalidEdit("Deleting the song timing chart requires the current cultural chart order".into()));
                    }
                    crate::song_timing_chart::Choice::load(
                        &snapshot,
                        None,
                        &crate::open_preferences::Preferences::from_load_context(
                            self.document.load_context(),
                        ),
                    )?
                }
            }
        } else {
            choice
        };
        let snapshot = self.document.reparse(self.serialize()?)?;
        let bytes = crate::chart_deletion::prepare(&snapshot, chart)?;
        let after = self.document.reparse(bytes)?;
        if after.charts().len() + 1 != snapshot.charts().len() {
            return Err(Error::InvalidEdit(
                "Deleted chart structure is ambiguous".into(),
            ));
        }
        let mut chart_ids = self.chart_ids.clone();
        chart_ids.remove(chart);
        let after = Self::materialize_song_timing_state(after, &chart_ids, &choice)?;
        self.execute(
            "Delete chart".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
                touched_grids: Default::default(),
                chart_ids,
                song_timing: Some(choice),
            })),
        )
    }

    pub(crate) fn insert_empty_chart(
        &mut self,
        block: Vec<u8>,
        grid: &Grid,
        metadata: crate::generation::metadata::ChartData,
        resident_timing: Option<Arc<[u8]>>,
    ) -> Result<usize, Error> {
        let index = self.document.charts().len();
        if self.document.format == crate::document::Format::Sm && index > 9999 {
            return Err(Error::InvalidEdit(
                "SM chart metadata exceeds its four-digit index limit".into(),
            ));
        }
        let mut bytes = self.serialize()?;
        bytes.extend_from_slice(b"\n\n");
        bytes.extend_from_slice(&block);
        let mut after = self.document.reparse(Arc::<[u8]>::from(bytes))?;
        if after.charts().len() != index + 1 {
            return Err(Error::InvalidEdit(
                "New chart structure is ambiguous".into(),
            ));
        }
        if index == 0
            && self.document.format == crate::document::Format::Ssc
            && crate::timing_edit::migration::needed(&after)
        {
            if let Some(tag) = after.tag(None, "VERSION") {
                let range = tag.value.clone();
                after.replace(range, Some(Arc::from(b"0.83".as_slice())))?;
            } else {
                after.set_added_property(None, "VERSION", Some(Arc::from(b"0.83".as_slice())))?;
            }
        }
        if let Some(raw) = resident_timing {
            let tag = crate::timing_edit::resident::tag(Some(index))?;
            if after.raw_property(None, &tag).is_some() {
                return Err(Error::InvalidEdit(
                    "Resident timing occupies the new chart index".into(),
                ));
            }
            after.set_added_property(None, &tag, Some(raw))?;
        }
        if after.format == crate::document::Format::Sm {
            crate::attacks::property::empty(&mut after, index)?;
        }
        let descriptor = &after.charts()[index];
        if layout_columns(&descriptor.steps_type) != Some(grid.columns())
            || Grid::parse_layout(
                after.value(descriptor.notes.clone()),
                &descriptor.steps_type,
            )? != *grid
        {
            return Err(Error::InvalidEdit(
                "Empty chart notes changed during insertion".into(),
            ));
        }
        let (scope, suffix) = if self.document.format == crate::document::Format::Ssc {
            (Some(index), String::new())
        } else {
            (None, format!("CHART{index:04}"))
        };
        if after
            .raw_property(scope, &format!("DSAttackRows{suffix}"))
            .is_some()
        {
            return Err(Error::InvalidEdit(
                "Resident attack metadata occupies the new chart index".into(),
            ));
        }
        for prefix in ["GAChartDataVersion", "GAChartData"] {
            let name = format!("{prefix}{suffix}");
            if after.raw_property(scope, &name).is_some() {
                return Err(Error::InvalidEdit(
                    "Existing metadata occupies the new chart index".into(),
                ));
            }
            let value = if prefix == "GAChartDataVersion" {
                b"1".to_vec()
            } else {
                escape_text(
                    &serde_json::to_string(&metadata)
                        .map_err(|e| Error::InvalidEdit(e.to_string()))?,
                )
            };
            after.set_added_property(scope, &name, Some(value.into()))?;
        }
        let mut chart_ids = self.chart_ids.clone();
        chart_ids.push(crate::generation::metadata::Guid::random());
        let mut timing_choice = self.song_timing_choice()?;
        if timing_choice.identity().is_none() {
            timing_choice = crate::song_timing_chart::Choice::restored(chart_ids.last().cloned());
        }
        let after = Self::materialize_song_timing_state(after, &chart_ids, &timing_choice)?;
        self.execute(
            "Create chart".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
                touched_grids: Default::default(),
                chart_ids,
                song_timing: Some(timing_choice),
            })),
        )?;
        Ok(index)
    }

    /// Duplicate the complete working chart in one structural history gesture.
    /// Reparse the serialized snapshot so pending notes/properties are included.
    pub fn duplicate_chart(
        &mut self,
        chart: usize,
        expected_revision: u64,
    ) -> Result<usize, Error> {
        if self.revision != expected_revision {
            return Err(Error::InvalidEdit(
                "Document changed before chart duplication".into(),
            ));
        }
        let mut bytes = self.serialize()?;
        let current = self.document.reparse(Arc::<[u8]>::from(bytes.as_slice()))?;
        let range = current.chart_block_range(chart)?;
        let index = current.charts().len();
        let block = bytes[range].to_vec();
        bytes.extend_from_slice(b"\n\n");
        bytes.extend_from_slice(&block);
        let mut after = self.document.reparse(Arc::<[u8]>::from(bytes))?;
        if after.charts().len() != index + 1 {
            return Err(Error::InvalidEdit(
                "Duplicated chart structure is ambiguous".into(),
            ));
        }
        if self.document.format == crate::document::Format::Sm {
            crate::generation::metadata::clone_chart_owner(&current, chart, &mut after, index)?;
            for prefix in [
                "DSAttackRows",
                "DSAttackRowsLocal",
                "DSChartAttacks",
                "DSTiming",
            ] {
                let source = format!("{prefix}CHART{chart:04}");
                let target = format!("{prefix}CHART{index:04}");
                if after.raw_property(None, &target).is_some() {
                    return Err(Error::InvalidEdit(
                        "Existing SM metadata occupies the duplicated chart index".into(),
                    ));
                }
                if let Some(value) = current.raw_property(None, &source) {
                    if index > 9999 {
                        return Err(Error::InvalidEdit(
                            "SM chart metadata exceeds its four-digit index limit".into(),
                        ));
                    }
                    after.set_added_property(None, &target, Some(Arc::from(value)))?;
                }
            }
        }
        if let Some(grid) = self.grids.get(&chart) {
            let descriptor = &after.charts()[index];
            let columns = layout_columns(&descriptor.steps_type).ok_or_else(|| {
                Error::Unsupported("Duplicated chart has an unknown layout".into())
            })?;
            if columns != grid.columns()
                || Grid::parse_layout(
                    after.value(descriptor.notes.clone()),
                    &descriptor.steps_type,
                )? != *grid
            {
                return Err(Error::InvalidEdit(
                    "Duplicated chart notes do not match the working grid".into(),
                ));
            }
        }
        let mut chart_ids = self.chart_ids.clone();
        chart_ids.push(crate::generation::metadata::Guid::random());
        self.execute(
            "Duplicate chart".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
                touched_grids: Default::default(),
                chart_ids,
                song_timing: Some(self.song_timing_choice()?),
            })),
        )?;
        Ok(index)
    }

    fn execute(&mut self, label: String, mut change: Change) -> Result<(), Error> {
        self.profile_edit_update = None;
        self.ensure_song_timing_choice()?;
        let next = self
            .next_revision
            .checked_add(1)
            .ok_or_else(|| Error::InvalidEdit("Editor revision overflow".into()))?;
        let attack_selection = self.attack_selection.clone();
        if let Err(error) = self.apply(&mut change, true) {
            self.attack_selection = attack_selection;
            return Err(error);
        }
        self.application_review_epoch = Arc::new(());
        fn affects_song(change: &Change) -> bool {
            match change {
                Change::PackProperties(_)
                | Change::ProfileEdit(_)
                | Change::ProfileDeletion { .. }
                | Change::ProfileCreation { .. }
                | Change::SourcePreferences(_)
                | Change::AudioPreferences(_)
                | Change::KeyBindingPreferences(_)
                | Change::NoteColorPreferences(_)
                | Change::ReceptorPreferences(_)
                | Change::DensityPreferences(_)
                | Change::ScrollPreferences(_)
                | Change::RestoreOptions(_)
                | Change::RestoreScroll(_)
                | Change::PerformancePreferences(_)
                | Change::GeneralOptions(_)
                | Change::DisplayOptions(_)
                | Change::VisualPreferences(_)
                | Change::DarkPreferences(_)
                | Change::StreamPreferences(_)
                | Change::SelectionPreferences(_) => false,
                Change::Group(changes) => changes.iter().any(affects_song),
                _ => true,
            }
        }
        let content_revision = if affects_song(&change) {
            self.next_revision
        } else {
            self.content_revision
        };
        self.event_selection_update = None;
        self.undo.push(Entry {
            authoring_policy: None,
            selection: None,
            pattern_added: None,
            label,
            change,
            before_revision: self.revision,
            after_revision: self.next_revision,
            before_content_revision: self.content_revision,
            after_content_revision: content_revision,
        });
        self.revision = self.next_revision;
        self.content_revision = content_revision;
        self.next_revision = next;
        self.redo.clear();
        self.prune_attack_selection();
        self.trim_history();
        Ok(())
    }

    fn apply(&mut self, change: &mut Change, forward: bool) -> Result<(), Error> {
        match change {
            Change::AttackIdentity(effect) => {
                self.apply_attack_lifecycle(effect, forward);
                Ok(())
            }
            Change::PatternReadd { .. } | Change::MarkerSeedTouch { .. } => Ok(()),
            Change::SourcePreferences(change) => {
                let context = self.source_preferences_context.take().ok_or_else(|| {
                    Error::InvalidEdit("Fresh application preference snapshot required".into())
                })?;
                if !self.accepts_application_review(&context.0) {
                    return Err(Error::ExternalChange);
                }
                let update = change.apply(&context.1, forward)?;
                self.set_history_capacity(
                    update
                        .snapshot()
                        .file()
                        .general_options()
                        .effective_history_size(),
                )?;
                self.audio_preferences_current = Some(update.snapshot().file().audio());
                self.source_preferences_update = Some(Box::new(update));
                Ok(())
            }

            Change::ChartProperties { change, .. }
            | Change::SongTimingProperties(change)
            | Change::TimingMigration(change)
            | Change::TimingEdit(change)
            | Change::MarkerMetadata(change)
            | Change::SongPropertiesLive(change)
            | Change::SongPropertiesMetadata(change)
            | Change::ChartEventCopy(change) => self.apply(change, forward),
            Change::SongTimingChoice { before, after } => {
                self.song_timing = Some(if forward {
                    after.clone()
                } else {
                    before.clone()
                });
                Ok(())
            }
            Change::DensityPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|context| !self.accepts_application_review(&context.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|context| context.1.file());
                self.density_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::ReceptorPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.receptor_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::SelectionPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.selection_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::StreamPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.stream_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::ScrollPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.scroll_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::RestoreScroll(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                let update = change.apply(forward, live)?;
                self.restore_scroll_update = Some(Box::new(update));
                Ok(())
            }
            Change::PerformancePreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.performance_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::RestoreOptions(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                let update = change.apply(forward, live)?;
                self.set_history_capacity(update.preferences.general.effective_history_size())?;
                self.restore_options_update = Some(Box::new(update));
                Ok(())
            }
            Change::GeneralOptions(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                let update = change.apply(forward, live)?;
                self.set_history_capacity(update.preferences.effective_history_size())?;
                self.general_options_update = Some(Box::new(update));
                Ok(())
            }
            Change::DisplayOptions(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.display_options_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::VisualPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|context| !self.accepts_application_review(&context.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|context| context.1.file());
                self.visual_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::DarkPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|context| !self.accepts_application_review(&context.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|context| context.1.file());
                self.dark_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::NoteColorPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.note_color_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::KeyBindingPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let live = context.as_ref().map(|c| c.1.file());
                self.key_binding_preferences_update = Some(Box::new(change.apply(forward, live)?));
                Ok(())
            }
            Change::AudioPreferences(change) => {
                let context = self.source_preferences_context.take();
                if context
                    .as_ref()
                    .is_some_and(|c| !self.accepts_application_review(&c.0))
                {
                    return Err(Error::ExternalChange);
                }
                let fresh = context.as_ref().map(|c| c.1.file());
                let update = change.apply(forward, self.audio_preferences_current, fresh)?;
                self.audio_preferences_current = Some(update.audio);
                self.audio_preferences_update = Some(Box::new(update));
                Ok(())
            }
            Change::Group(changes) => {
                let length = changes.len();
                let index = |offset| if forward { offset } else { length - 1 - offset };
                for offset in 0..changes.len() {
                    let at = index(offset);
                    if let Err(error) = self.apply(&mut changes[at], forward) {
                        for previous in (0..offset).rev() {
                            let at = index(previous);
                            self.apply(&mut changes[at], !forward)?;
                        }
                        return Err(error);
                    }
                }
                Ok(())
            }
            Change::PackProperties(edit) => edit.apply(forward),
            Change::ProfileEdit(edit) => {
                let (update, receipt) = edit.apply(forward)?;
                if let Some(receipt) = receipt {
                    self.profile_durability.insert(update.path.clone(), receipt);
                } else {
                    self.profile_durability.remove(&update.path);
                }
                self.profile_edit_update = Some(update);
                Ok(())
            }
            Change::ProfileCreation {
                profile,
                directory,
                file,
                removed,
            } => {
                self.retry_profile_durability()?;
                if forward {
                    if let Some(removed) = removed {
                        removed.restore()?;
                    } else {
                        let mut created = profile.export_new_committed(directory)?.into_file();
                        self.adopt_profile_publication(&mut created);
                        *file = Some(Box::new(created));
                    }
                } else if let Some(removed) = removed {
                    removed.delete_again()?;
                } else {
                    let source = file.as_ref().ok_or_else(|| {
                        Error::InvalidEdit("Missing created profile snapshot".into())
                    })?;
                    *removed = Some(source.delete()?);
                }
                Ok(())
            }
            Change::ProfileDeletion { source, deleted } => {
                self.retry_profile_durability()?;
                if forward {
                    match deleted {
                        Some(file) => {
                            file.delete_again()?;
                        }
                        None => {
                            *deleted = Some(source.delete()?);
                        }
                    }
                } else if let Some(file) = deleted {
                    file.restore()?;
                }
                Ok(())
            }
            Change::PlayerCount {
                chart,
                before,
                after,
            } => {
                self.grid(*chart)?;
                self.grids.get_mut(chart).unwrap().set_players(if forward {
                    *after
                } else {
                    *before
                })?;
                self.touched_grids.insert(*chart);
                Ok(())
            }
            Change::DocumentSwap(state) => {
                std::mem::swap(&mut self.document, &mut state.document);
                std::mem::swap(&mut self.grids, &mut state.grids);
                std::mem::swap(&mut self.touched_grids, &mut state.touched_grids);
                std::mem::swap(&mut self.chart_ids, &mut state.chart_ids);
                std::mem::swap(&mut self.song_timing, &mut state.song_timing);
                Ok(())
            }
            Change::AddedProperty {
                chart,
                name,
                before,
                after,
            } => self.document.set_added_property(
                *chart,
                name,
                if forward { after } else { before }.clone(),
            ),
            Change::Value {
                range,
                before,
                after,
            } => self
                .document
                .replace(range.clone(), if forward { after } else { before }.clone()),
            Change::Notes { chart, changes } => {
                self.grid(*chart)?;
                let grid = self.grids.get_mut(chart).unwrap();
                // Validate every address before removing any note, including
                // deletion-only edits which otherwise could fail mid-gesture.
                if changes.iter().any(|change| {
                    change.position.lane >= grid.columns()
                        || change.position.player >= grid.players()
                }) {
                    return Err(Error::InvalidEdit(
                        "Lane or participant lies outside this chart".into(),
                    ));
                }
                for change in changes.iter() {
                    grid.set(change.position, None)?;
                }
                for change in changes.iter() {
                    let value = if forward {
                        &change.after
                    } else {
                        &change.before
                    };
                    if let Err(error) = grid.set(change.position, value.clone()) {
                        for change in changes.iter() {
                            grid.set(change.position, None)?;
                        }
                        for change in changes.iter() {
                            let original = if forward {
                                &change.before
                            } else {
                                &change.after
                            };
                            grid.set(change.position, original.clone())?;
                        }
                        return Err(error);
                    }
                }
                self.touched_grids.insert(*chart);
                Ok(())
            }
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        self.serialize_document(self.document.clone())
    }
    fn serialize_document(&self, mut document: Document) -> Result<Vec<u8>, Error> {
        // Editing a grid is independent of how many Undo entries remain.
        // Untouched lazy views keep their original note formatting byte-for-byte.
        for &chart in &self.touched_grids {
            let descriptor = &document.charts()[chart];
            let grid = &self.grids[&chart];
            let original = Self::document_grid(&document, chart)?;
            if grid != &original {
                document.replace(descriptor.notes.clone(), Some(grid.serialize()?.into()))?;
            }
        }
        Ok(document.serialize())
    }

    pub fn loaded_grid(&self, chart: usize) -> Option<&Grid> {
        self.grids.get(&chart)
    }
}

#[derive(Clone, Debug)]
pub struct Clipboard {
    columns: u16,
    single_player: bool,
    notes: Vec<(Position, Note)>,
}

impl Clipboard {
    pub fn copy(grid: &Grid, start: u64, end: u64, player: Option<u32>) -> Result<Self, Error> {
        if start > end {
            return Err(Error::InvalidEdit("Selection ends before it starts".into()));
        }
        Self::copy_matching(grid, start, |pos| {
            pos.row >= start && pos.row <= end && player.is_none_or(|player| pos.player == player)
        })
    }
    pub(crate) fn copy_positions(
        grid: &Grid,
        start: u64,
        positions: &std::collections::BTreeSet<Position>,
    ) -> Result<Self, Error> {
        if positions
            .iter()
            .any(|p| p.row < start || grid.get(*p).is_none())
        {
            return Err(Error::InvalidEdit("Invalid clipboard note position".into()));
        }
        Self::copy_matching(grid, start, |pos| positions.contains(&pos))
    }
    fn copy_matching(
        grid: &Grid,
        start: u64,
        include: impl Fn(Position) -> bool,
    ) -> Result<Self, Error> {
        let mut owner = None;
        let mut single_player = true;
        let notes = grid
            .iter()
            .filter(|(pos, _)| include(*pos))
            .map(|(pos, note)| {
                if let Some(owner) = owner {
                    single_player &= owner == pos.player;
                } else {
                    owner = Some(pos.player);
                }
                let mut note = note.clone();
                note.end = note.end.map(|end| end - start);
                (
                    Position {
                        row: pos.row - start,
                        ..pos
                    },
                    note,
                )
            })
            .collect();
        Ok(Self {
            columns: grid.columns(),
            single_player,
            notes,
        })
    }

    /// Paste with original GrooveAuthor lane-conflict replacement semantics.
    /// Plan on a private grid, then publish every side effect as one gesture.
    pub fn paste_force(
        &self,
        session: &mut Session,
        chart: usize,
        row: u64,
        active_player: u32,
    ) -> Result<bool, Error> {
        self.paste_force_with_report(session, chart, row, active_player)
            .map(|(changed, _)| changed)
    }

    pub(crate) fn paste_force_with_report(
        &self,
        session: &mut Session,
        chart: usize,
        row: u64,
        active_player: u32,
    ) -> Result<(bool, usize), Error> {
        let original = session.grid(chart)?;
        let mut staged = original.clone();
        let mut skipped = 0;
        let mut pasted = Vec::new();
        let mut notes = self.notes.clone();
        notes.sort_by_key(|(pos, _)| (pos.row, pos.lane, pos.player));
        for (pos, mut note) in notes {
            // Original CanEventExistAtRow skips an out-of-range lane without
            // preventing compatible notes/miscellaneous events from pasting.
            if pos.lane >= staged.columns() {
                skipped += 1;
                continue;
            }
            let player = if self.single_player {
                active_player
            } else if pos.player < staged.players() {
                pos.player
            } else {
                0
            };
            let position = Position {
                player,
                row: pos
                    .row
                    .checked_add(row)
                    .ok_or_else(|| Error::InvalidEdit("Paste exceeds the row range".into()))?,
                ..pos
            };
            note.end = note
                .end
                .map(|end| {
                    end.checked_add(row)
                        .ok_or_else(|| Error::InvalidEdit("Paste exceeds the row range".into()))
                })
                .transpose()?;
            staged.force_insert(position, note)?;
            pasted.push(position);
        }
        pasted.retain(|position| staged.get(*position).is_some());
        pasted.sort_by_key(|position| {
            (
                position.row,
                staged.get(*position).unwrap().kind.editor_phase(),
                position.lane,
                position.player,
            )
        });
        pasted.dedup();
        let before = SelectionEffect::capture(session, chart, Vec::new());
        let changed = session.apply_grid_delta(chart, &staged, "Paste notes")?;
        let after = SelectionEffect::capture(
            session,
            chart,
            pasted
                .into_iter()
                .map(crate::event_clipboard::Event::Note)
                .collect(),
        );
        session.record_event_selection(
            selection_effect::SelectionHistory { before, after },
            changed,
        );
        Ok((changed, skipped))
    }

    /// GrooveAuthor pastes a single-owner selection as the active participant;
    /// a mixed-owner selection preserves its participant assignments.
    pub fn paste(
        &self,
        session: &mut Session,
        chart: usize,
        row: u64,
        active_player: u32,
    ) -> Result<bool, Error> {
        let grid = session.grid(chart)?;
        if grid.columns() != self.columns {
            return Err(Error::InvalidEdit(
                "Clipboard layout has a different number of lanes".into(),
            ));
        }
        let mut edits = Vec::with_capacity(self.notes.len());
        for (pos, note) in &self.notes {
            let mut note = note.clone();
            note.end = note
                .end
                .map(|end| {
                    end.checked_add(row)
                        .ok_or_else(|| Error::InvalidEdit("Paste exceeds the row range".into()))
                })
                .transpose()?;
            let position = Position {
                player: if self.single_player {
                    active_player
                } else {
                    pos.player
                },
                row: pos
                    .row
                    .checked_add(row)
                    .ok_or_else(|| Error::InvalidEdit("Paste exceeds the row range".into()))?,
                ..*pos
            };
            edits.push((position, Some(note)));
        }
        session.edit_notes(chart, "Paste notes", edits)
    }
}

impl Session {
    pub fn history_capacity(&self) -> usize {
        self.history_capacity
    }
    /// Source UndoStack.Resize prioritizes most recent past, then closest future.
    pub fn set_history_capacity(&mut self, size: usize) -> Result<(), Error> {
        if !(1..=32768).contains(&size) {
            return Err(Error::InvalidEdit("Invalid undo history capacity".into()));
        }
        self.history_capacity = size;
        self.trim_history();
        Ok(())
    }
    fn trim_history(&mut self) {
        let discard = self.undo.len().saturating_sub(self.history_capacity);
        if discard > 0 {
            self.undo.drain(..discard);
        }
        let future = self.history_capacity - self.undo.len();
        let discard = self.redo.len().saturating_sub(future);
        if discard > 0 {
            self.redo.drain(..discard);
        }
    }
}

impl Session {
    /// Capture editable state without formatting or reparsing the song on the
    /// UI thread. Only modified grids are copied; immutable source bytes are
    /// shared. Materialization belongs to the requesting assistant worker.
    pub(crate) fn deferred_snapshot(
        &self,
    ) -> impl FnOnce() -> Result<Document, Error> + Send + 'static {
        let document = self.document.clone();
        let grids: Vec<_> = self
            .touched_grids
            .iter()
            .map(|&chart| (chart, self.grids[&chart].clone()))
            .collect();
        move || {
            let mut session = Self::new(document);
            for (chart, grid) in grids {
                session.grids.insert(chart, grid);
                session.touched_grids.insert(chart);
            }
            session.snapshot_document()
        }
    }
    pub fn snapshot_document(&self) -> Result<Document, Error> {
        self.document.reparse(self.serialize()?)
    }
}

impl Session {
    pub fn serialize_for_output(&self) -> Result<Vec<u8>, Error> {
        self.serialize_output_context(crate::song_properties::OutputContext::NativeLossless)
    }
}

impl Session {
    pub fn set_general_options(
        &mut self,
        before: crate::options_preferences::General,
        after: crate::options_preferences::General,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change General Options".into(),
            ));
        }
        before.validate()?;
        after.validate()?;
        if file.is_some_and(|file| file.general_options() != &before) {
            return Err(Error::InvalidEdit(
                "General Options snapshot differs from active settings".into(),
            ));
        }
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "General options".into(),
            Change::GeneralOptions(Box::new(
                crate::options_preferences::general::history::Change {
                    before,
                    after,
                    file: file.cloned(),
                },
            )),
        )?;
        Ok(true)
    }
    pub fn take_general_options_update(
        &mut self,
    ) -> Option<crate::options_preferences::GeneralUpdate> {
        self.general_options_update.take().map(|update| *update)
    }
}

impl Session {
    /// Content stamps are the shared transaction contract, independent of labels,
    /// currently loaded charts, preference variants and media-relocation retokens.
    pub fn can_undo_application_change(&self) -> bool {
        self.undo
            .last()
            .is_some_and(|entry| entry.before_content_revision == entry.after_content_revision)
    }
    pub fn can_redo_application_change(&self) -> bool {
        self.redo
            .last()
            .is_some_and(|entry| entry.before_content_revision == entry.after_content_revision)
    }
}

impl Session {
    pub fn restore_options(
        &mut self,
        before: crate::options_preferences::restore::Snapshot,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot restore Options".into(),
            ));
        }
        if file.is_some_and(|file| {
            crate::options_preferences::restore::Snapshot::capture(file) != before
        }) {
            return Err(Error::InvalidEdit(
                "Options snapshot differs from active settings".into(),
            ));
        }
        let after = before.restored();
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Restore Options defaults".into(),
            Change::RestoreOptions(Box::new(crate::options_preferences::restore::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    /// Change only the MRU limit. Source Options is a plain field: existing
    /// records are retained until the next real record operation applies the limit.
    pub fn set_recent_files_history_size(
        &mut self,
        before: usize,
        after: usize,
        file: &crate::audio_preferences::File,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change Options".into(),
            ));
        }
        if file.recent_songs().maximum != before {
            return Err(Error::ExternalChange);
        }
        i32::try_from(after)
            .map_err(|_| Error::InvalidEdit("Recent history size overflows source int".into()))?;
        if before == after {
            return Ok(false);
        }
        let before = crate::options_preferences::restore::Snapshot::capture(file);
        let mut after_snapshot = before.clone();
        after_snapshot.recent_maximum = after;
        self.capture_source_preferences_context(file);
        self.execute(
            "Recent files history size".into(),
            Change::RestoreOptions(Box::new(crate::options_preferences::restore::Change {
                before,
                after: after_snapshot,
                file: Some(file.clone()),
            })),
        )?;
        Ok(true)
    }
    pub fn set_suppress_external_song_notifications(
        &mut self,
        before: bool,
        after: bool,
        file: &crate::audio_preferences::File,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change Options".into(),
            ));
        }
        if file.suppress_external_song_notifications() != before {
            return Err(Error::ExternalChange);
        }
        if before == after {
            return Ok(false);
        }
        let before = crate::options_preferences::restore::Snapshot::capture(file);
        let mut after_snapshot = before.clone();
        after_snapshot.suppress_external_song = after;
        self.capture_source_preferences_context(file);
        self.execute(
            "Song modification notifications".into(),
            Change::RestoreOptions(Box::new(crate::options_preferences::restore::Change {
                before,
                after: after_snapshot,
                file: Some(file.clone()),
            })),
        )?;
        Ok(true)
    }
    pub fn take_restore_options_update(
        &mut self,
    ) -> Option<crate::options_preferences::restore::Update> {
        self.restore_options_update.take().map(|v| *v)
    }
}

impl Session {
    pub fn serialize_for_source_compatible(&self) -> Result<Vec<u8>, Error> {
        self.serialize_output_context(crate::song_properties::OutputContext::SourceCompatible)
    }
    fn serialize_output_context(
        &self,
        mode: crate::song_properties::OutputContext,
    ) -> Result<Vec<u8>, Error> {
        let snapshot = self.song_timing_snapshot()?;
        self.serialize_output_context_ordered(mode, snapshot.source_indices())
    }
    pub(crate) fn serialize_source_compatible_ordered(
        &self,
        order: &[usize],
    ) -> Result<Vec<u8>, Error> {
        self.serialize_output_context_ordered(
            crate::song_properties::OutputContext::SourceCompatible,
            order,
        )
    }
    fn serialize_output_context_ordered(
        &self,
        mode: crate::song_properties::OutputContext,
        order: &[usize],
    ) -> Result<Vec<u8>, Error> {
        let mut document = self.document.reparse(self.serialize()?)?;
        let snapshot = self.song_timing_snapshot()?;
        let fields = self.song_timing_choice()?.output_fields(&snapshot, order)?;
        crate::song_properties::materialize_output_context(&mut document, fields, mode)?;
        Ok(document.serialize())
    }
}

impl Session {
    pub fn restore_scroll(
        &mut self,
        before: crate::scroll_preferences::restore::Snapshot,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot restore Scroll preferences".into(),
            ));
        }
        if file.is_some_and(|file| {
            crate::scroll_preferences::restore::Snapshot::capture(file) != before
        }) {
            return Err(Error::InvalidEdit(
                "Scroll snapshot differs from active settings".into(),
            ));
        }
        if !before.restoration_needed() {
            return Ok(false);
        }
        let after = before.restored();
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            "Restore Scroll defaults".into(),
            Change::RestoreScroll(Box::new(crate::scroll_preferences::restore::Change {
                before,
                after,
                file: file.cloned(),
                initial: None,
            })),
        )?;
        Ok(true)
    }
    pub fn take_restore_scroll_update(
        &mut self,
    ) -> Option<crate::scroll_preferences::restore::Update> {
        self.restore_scroll_update.take().map(|value| *value)
    }
}

impl Session {
    fn change_performance_preferences(
        &mut self,
        label: &str,
        before: crate::performance::preferences::Preferences,
        after: crate::performance::preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot change performance preferences".into(),
            ));
        }
        if file.is_some_and(|file| file.performance() != before) {
            return Err(Error::InvalidEdit(
                "Performance snapshot differs from active settings".into(),
            ));
        }
        before.write(&mut serde_json::Map::new())?;
        after.write(&mut serde_json::Map::new())?;
        if before == after {
            return Ok(false);
        }
        if let Some(file) = file {
            self.capture_source_preferences_context(file);
        }
        self.execute(
            label.into(),
            Change::PerformancePreferences(Box::new(crate::performance::history::Change {
                before,
                after,
                file: file.cloned(),
            })),
        )?;
        Ok(true)
    }
    pub fn set_performance_preferences(
        &mut self,
        before: crate::performance::preferences::Preferences,
        after: crate::performance::preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        self.change_performance_preferences("Performance preferences", before, after, file)
    }
    pub fn restore_performance_preferences(
        &mut self,
        before: crate::performance::preferences::Preferences,
        file: Option<&crate::audio_preferences::File>,
    ) -> Result<bool, Error> {
        if !before.restoration_needed() {
            return Ok(false);
        }
        self.change_performance_preferences(
            "Restore Performance defaults",
            before,
            before.restored(),
            file,
        )
    }
    pub fn take_performance_preferences_update(&mut self) -> Option<crate::performance::Update> {
        self.performance_preferences_update
            .take()
            .map(|update| *update)
    }
}
#[cfg(test)]
mod source_opaque_reconstruction_tests {
    use super::*;
    #[test]
    fn reconstructed_grid_cannot_discard_opaque_cells_at_session_owner() {
        let raw = b"#BPMS:0=120;#NOTES:dance-single::Easy:1:0:\n0[7]100\n0000\n0000\n0000;";
        let mut session =
            Session::new(Document::parse(raw.as_slice(), crate::document::Format::Sm).unwrap());
        let original = session.grid(0).unwrap().clone();
        let before = session.serialize().unwrap();
        let revision = session.revision();
        let reconstructed = Grid::new(4, 1).unwrap();
        assert!(
            session
                .apply_grid_delta(0, &reconstructed, "Reconstruct")
                .is_err()
        );
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(session.revision(), revision);
        assert_eq!(session.grid(0).unwrap(), &original);
        assert!(!session.undo().unwrap());
    }
}

impl Session {
    pub fn history_changes_sm_authoring(&self, redo: bool) -> bool {
        self.next_authoring_policy(redo).is_some()
    }
}

impl Session {
    pub fn edit_profile(
        &mut self,
        file: &crate::generation::profiles::ProfileFile,
        profile: &crate::generation::profiles::Profile,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit(
                "Staged chart edits cannot edit profile files".into(),
            ));
        }
        self.retry_profile_durability()?;
        let Some(edit) = crate::generation::profiles::edit::Edit::capture(file, profile)? else {
            return Ok(false);
        };
        self.execute(
            "Edit generation profile".into(),
            Change::ProfileEdit(Box::new(edit)),
        )?;
        Ok(true)
    }
    pub fn take_profile_edit_update(
        &mut self,
    ) -> Option<crate::generation::profiles::edit::Update> {
        self.profile_edit_update.take()
    }
    pub fn pending_profile_durability_update(
        &self,
    ) -> Option<crate::generation::profiles::edit::Update> {
        self.profile_durability
            .values()
            .next()
            .map(|receipt| receipt.update())
    }
    pub fn has_profile_durability(&self) -> bool {
        !self.profile_durability.is_empty()
    }
    pub fn retry_profile_durability(&mut self) -> Result<(), Error> {
        let paths: Vec<_> = self.profile_durability.keys().cloned().collect();
        for path in paths {
            self.profile_durability[&path].retry()?;
            self.profile_durability.remove(&path);
        }
        Ok(())
    }
}

impl Session {
    fn adopt_profile_publication(&mut self, file: &mut crate::generation::profiles::ProfileFile) {
        if let Some(receipt) = file.take_durability_receipt() {
            let update = receipt.update();
            self.profile_durability.insert(update.path.clone(), receipt);
            self.profile_edit_update = Some(update);
        }
    }
    /// Export is an explicit external publication, not a song/history edit.
    /// Retain its pending CAS receipt across bounded-history eviction and close.
    pub fn retain_profile_publication(
        &mut self,
        mut file: crate::generation::profiles::ProfileFile,
    ) {
        self.adopt_profile_publication(&mut file);
    }
}

impl Session {
    /// Publish one Zoom draft atomically. Source duration/multiplier/limit changes
    /// enter history; SizeCap remains direct even when submitted in the same draft.
    pub fn apply_zoom_preferences(
        &mut self,
        before: crate::zoom_preferences::Preferences,
        target: crate::zoom_preferences::Preferences,
        file: &crate::audio_preferences::File,
    ) -> Result<bool, Error> {
        if !self.allows_external_edits {
            return Err(Error::ExternalChange);
        }
        let merged = file.zoom().merge_changes(before, target)?;
        let baseline = crate::scroll_preferences::restore::Snapshot::capture(file);
        if baseline.zoom == merged {
            return Ok(false);
        }
        let mut initial = baseline;
        initial.zoom = merged;
        let mut after = baseline;
        after.zoom = baseline.zoom.undoable_fields_from(merged);
        let mut change = crate::scroll_preferences::restore::Change {
            before: baseline,
            after,
            file: Some(file.clone()),
            initial: Some(initial),
        };
        if after == baseline {
            let update = change.apply(true, Some(file))?;
            self.restore_scroll_update = Some(Box::new(update));
            self.application_review_epoch = Arc::new(());
            self.source_preferences_context = None;
        } else {
            self.capture_source_preferences_context(file);
            self.execute(
                "Change Zoom preferences".into(),
                Change::RestoreScroll(Box::new(change)),
            )?;
        }
        Ok(true)
    }
}

impl Session {
    /// Publish an isolated assistant proposal as one undoable transaction.
    pub(crate) fn commit_assistant_document(
        &mut self,
        token: &RevisionToken,
        bytes: &[u8],
    ) -> Result<(), Error> {
        if !self.accepts_token(token) {
            return Err(Error::InvalidEdit(
                "The song changed while the assistant was working; request a new proposal".into(),
            ));
        }
        if self.serialize()? == bytes {
            return Ok(());
        }
        let after = self.document.reparse(Arc::<[u8]>::from(bytes))?;
        if after.charts().len() < self.document.charts().len() {
            return Err(Error::InvalidEdit(
                "Assistant proposals cannot remove charts".into(),
            ));
        }
        for (before, next) in self.document.charts().iter().zip(after.charts()) {
            if before.steps_type != next.steps_type {
                return Err(Error::InvalidEdit(
                    "Assistant proposals cannot replace existing chart layouts".into(),
                ));
            }
        }
        for (index, chart) in after.charts().iter().enumerate() {
            let unchanged = self.document.charts().get(index).is_some_and(|old| {
                self.document.value(old.notes.clone()) == after.value(chart.notes.clone())
            });
            if !unchanged {
                Grid::parse_layout(after.value(chart.notes.clone()), &chart.steps_type)?;
            }
        }
        let mut chart_ids = self.chart_ids.clone();
        while chart_ids.len() < after.charts().len() {
            chart_ids.push(crate::generation::metadata::Guid::random());
        }
        let mut choice = self.song_timing_choice()?;
        if choice.identity().is_none() {
            choice = crate::song_timing_chart::Choice::restored(chart_ids.first().cloned());
        }
        let after = Self::materialize_song_timing_state(after, &chart_ids, &choice)?;
        self.execute(
            "Assistant chart proposal".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
                touched_grids: Default::default(),
                chart_ids,
                song_timing: Some(choice),
            })),
        )
    }
}

#[cfg(test)]
mod profile_history_rebinding_tests {
    use super::*;
    fn fixture_session() -> Session {
        Session::new(
            Document::parse(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
                    .as_slice(),
                crate::document::Format::Ssc,
            )
            .unwrap(),
        )
    }
    #[test]
    fn profile_history_rebinds_pending_chart_fields_only_across_the_authenticated_edge() {
        let root = tempfile::tempdir().unwrap();
        let mut session = fixture_session();
        let mut draft = crate::chart_properties::Draft::capture(&session, 0).unwrap();
        draft.set_music_offset(0.125).unwrap();
        draft.set_use_chart_offset(true);
        let profile = crate::generation::profiles::Profile::parse(
            crate::generation::profiles::Kind::Expressed,
            br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"History","Config":{}}"#,
        )
        .unwrap();
        session.create_profile(root.path(), &profile).unwrap();
        draft.after_profile_creation(&session).unwrap();
        let created = session.revision_token();
        session.undo().unwrap();
        assert!(session.after_profile_history(&created, true).is_err());
        assert!(
            fixture_session()
                .after_profile_history(&created, false)
                .is_err()
        );
        draft.after_profile_history(&session, false).unwrap();
        assert!(draft.has_changes());
        assert_eq!(draft.music_offset(), 0.125);
        session.redo().unwrap();
        draft.after_profile_history(&session, true).unwrap();
        assert_eq!(draft.music_offset(), 0.125);
        session
            .edit_notes(
                0,
                "Note",
                [(
                    crate::notes::Position {
                        player: 0,
                        lane: 0,
                        row: 0,
                    },
                    Some(crate::notes::Note::tap()),
                )],
            )
            .unwrap();
        let note = session.revision_token();
        session.undo().unwrap();
        assert!(session.after_profile_history(&note, false).is_err());
    }
}
