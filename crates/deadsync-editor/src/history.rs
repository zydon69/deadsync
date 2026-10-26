//! Commands store only changed notes and tag values. One gesture is one history
//! entry; failed gestures roll back and never erase the redo branch.
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
}

#[derive(Debug)]
enum Change {
    Group(Vec<Change>),
    PlayerCount { chart: usize, before: u32, after: u32 },
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
    label: String,
    change: Change,
    before_revision: u64,
    after_revision: u64,
    before_content_revision: u64,
    after_content_revision: u64,
}

/// A revision is only meaningful within one live session. Recovery/reload creates
/// a new identity even when bytes and revision numbers happen to be identical.
#[derive(Clone, Debug)]
pub(crate) struct RevisionToken {
    identity: Arc<()>,
    revision: u64,
}

pub struct ChartAddition<'a> {
    pub source_chart: usize,
    pub layout: &'a str,
    pub grid: &'a Grid,
}

pub struct Session {
    identity: Arc<()>,
    allows_external_edits: bool,
    document: Document,
    grids: BTreeMap<usize, Grid>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    revision: u64,
    saved_revision: u64,
    content_revision: u64,
    next_revision: u64,
}

impl Session {
    pub fn new(document: Document) -> Self {
        Self {
            identity: Arc::new(()),
            allows_external_edits: true,
            document,
            grids: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            saved_revision: 0,
            content_revision: 0,
            next_revision: 1,
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

    pub(crate) fn revision_token(&self) -> RevisionToken {
        RevisionToken {
            identity: self.identity.clone(),
            revision: self.revision,
        }
    }

    pub(crate) fn after_profile_creation(&self, token: &RevisionToken) -> Result<RevisionToken, Error> {
        let valid = Arc::ptr_eq(&self.identity, &token.identity)
            && self.undo.last().is_some_and(|entry|
                entry.before_revision == token.revision && entry.after_revision == self.revision
                && matches!(entry.change, Change::ProfileCreation { .. }));
        if !valid { return Err(Error::InvalidEdit("Draft cannot follow this profile-library change".into())); }
        Ok(self.revision_token())
    }

    pub(crate) fn accepts_token(&self, token: &RevisionToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.identity) && self.revision == token.revision
    }

    pub(crate) fn redo_reaches(&self, token: &RevisionToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.identity)
            && self.redo.last().is_some_and(|entry| entry.after_revision == token.revision)
    }

    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn is_dirty(&self) -> bool {
        self.content_revision != self.saved_revision
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.content_revision;
    }

    pub fn grid(&mut self, chart: usize) -> Result<&Grid, Error> {
        if !self.grids.contains_key(&chart) {
            let descriptor = self
                .document
                .charts()
                .get(chart)
                .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?;
            let columns = layout_columns(&descriptor.steps_type).ok_or_else(|| {
                Error::Unsupported(format!(
                    "Layout {} cannot yet be edited",
                    descriptor.steps_type
                ))
            })?;
            let mut grid = Grid::parse(self.document.value(descriptor.notes.clone()), columns)?;
            if crate::notes::supports_variable_players(&descriptor.steps_type) {
                if let Ok(Some(metadata)) = crate::generation::metadata::ChartData::read(&self.document, chart) {
                    grid.set_players(grid.players().max(metadata.max_player_index + 1))?;
                }
            }
            self.grids.insert(chart, grid);
        }
        Ok(&self.grids[&chart])
    }

    pub fn edit_notes(
        &mut self,
        chart: usize,
        label: impl Into<String>,
        edits: impl IntoIterator<Item = (Position, Option<Note>)>,
    ) -> Result<bool, Error> {
        let edits: BTreeMap<_, _> = edits.into_iter().collect();
        let grid = self.grid(chart)?;
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
            matches!(
                name.as_str(),
                "BPMS" | "STOPS" | "DELAYS" | "WARPS" | "OFFSET" | "TIMESIGNATURES"
            ) || name.starts_with("GACHARTDATA")
        });
        // Explicit attack replacements already own their positions. Otherwise
        // timing and derived attack file times form a single reversible gesture.
        if changes_clock
            && !temporary.undo.is_empty()
            && !properties.iter().any(|(_, name, _)| name.eq_ignore_ascii_case("ATTACKS"))
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
        temporary.grids.insert(chart, grid);
        operation(&mut temporary)?;
        let changes: Vec<_> = temporary
            .undo
            .into_iter()
            .map(|entry| entry.change)
            .collect();
        fn allowed(change: &Change, chart: usize) -> bool {
            match change {
                Change::DocumentSwap(_) | Change::ProfileDeletion { .. } | Change::ProfileCreation { .. } => false,
                Change::Notes { chart: target, .. } | Change::PlayerCount { chart: target, .. } => *target == chart,
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
            return Ok(false);
        }
        self.execute(label.into(), Change::Group(changes))?;
        Ok(true)
    }

    /// Create a profile as one disk-only gesture in global history.
    /// Existing library identities and destination files remain protected.
    pub fn set_participants(&mut self, chart: usize, requested: u32) -> Result<bool, Error> {
        let layout = self.document.charts().get(chart)
            .ok_or_else(|| Error::InvalidEdit("Chart does not exist".into()))?.steps_type.clone();
        let grid = self.grid(chart)?;
        let before = grid.players();
        let after = requested.max(grid.minimum_players()).max(1);
        if before == after { return Ok(false); }
        if !crate::notes::supports_variable_players(&layout) {
            return Err(Error::Unsupported("This chart type has a fixed participant count".into()));
        }
        grid.validate_players(after)?;
        let mut metadata = crate::generation::metadata::ChartData::existing_or_initial(&self.document, chart)?;
        metadata.max_player_index = after - 1;
        let mut staged = Session::staged(self.document.clone());
        metadata.save(&mut staged, chart)?;
        let mut changes: Vec<_> = staged.undo.into_iter().map(|entry| entry.change).collect();
        changes.push(Change::PlayerCount { chart, before, after });
        self.execute("Change chart participants".into(), Change::Group(changes))?;
        Ok(true)
    }

    pub fn create_profile(&mut self, directory: &std::path::Path, profile: &crate::generation::profiles::Profile) -> Result<std::path::PathBuf, Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit("Staged chart edits cannot create profile files".into()));
        }
        let loaded = crate::generation::profiles::Loaded::directory(directory)?;
        if loaded.profiles.contains_key(&(profile.config().kind(), profile.guid().clone())) {
            return Err(Error::InvalidEdit("A profile with this identity already exists in the library".into()));
        }
        let path = directory.join(profile.filename());
        self.execute("Create generation profile".into(), Change::ProfileCreation {
            profile: Box::new(profile.clone()), directory: directory.to_owned(), file: None, removed: None,
        })?;
        Ok(path)
    }

    /// Delete a custom profile and repair active-song references as one gesture.
    pub fn delete_profile(&mut self, file: &crate::generation::profiles::ProfileFile) -> Result<(), Error> {
        if !self.allows_external_edits {
            return Err(Error::InvalidEdit("Staged chart edits cannot delete profile files".into()));
        }
        let reset = crate::generation::profile_references::Reset::capture(
            self, file.profile().config().kind(), file.profile().guid())?;
        let (path, bytes) = file.snapshot();
        let source = crate::generation::profiles::ProfileFile::capture_snapshot(
            &path, bytes, file.profile().clone())?;
        let mut temporary = Session::staged(self.document.clone());
        temporary.set_property_bytes_atomic(reset.properties(), "Repair profile references")?;
        let mut changes: Vec<_> = temporary.undo.into_iter().map(|entry| entry.change).collect();
        // Disk deletion comes last forward and first backward. A disk conflict
        // leaves/restores every metadata edit and both history stacks intact.
        changes.push(Change::ProfileDeletion { source: Box::new(source), deleted: None });
        self.execute("Delete generation profile".into(), Change::Group(changes))
    }

    pub(crate) fn apply_grid_delta(&mut self, chart: usize, staged: &Grid, label: &str) -> Result<bool, Error> {
        let original = self.grid(chart)?;
        if original.columns() != staged.columns() || original.players() != staged.players() {
            return Err(Error::InvalidEdit("Grid replacement cannot change layout or participants".into()));
        }
        let positions: std::collections::BTreeSet<_> = original.iter().map(|(pos, _)| pos)
            .chain(staged.iter().map(|(pos, _)| pos)).collect();
        let edits: Vec<_> = positions.into_iter().filter_map(|pos| {
            let after = staged.get(pos);
            (original.get(pos) != after).then(|| (pos, after.cloned()))
        }).collect();
        self.edit_notes(chart, label, edits)
    }

    pub fn undo(&mut self) -> Result<bool, Error> {
        let Some(mut entry) = self.undo.pop() else {
            return Ok(false);
        };
        if let Err(error) = self.apply(&mut entry.change, false) {
            self.undo.push(entry);
            return Err(error);
        }
        self.revision = entry.before_revision;
        self.content_revision = entry.before_content_revision;
        self.redo.push(entry);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, Error> {
        let Some(mut entry) = self.redo.pop() else {
            return Ok(false);
        };
        if let Err(error) = self.apply(&mut entry.change, true) {
            self.redo.push(entry);
            return Err(error);
        }
        self.revision = entry.after_revision;
        self.content_revision = entry.after_content_revision;
        self.undo.push(entry);
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
            if Grid::parse(&notes, columns)? != *addition.grid {
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
        let after = Document::parse(Arc::<[u8]>::from(bytes), self.document.format)?;
        if after.charts().len() != start + additions.len() {
            return Err(Error::InvalidEdit(
                "Generated chart structure is ambiguous".into(),
            ));
        }
        for (index, addition) in additions.iter().enumerate() {
            let descriptor = &after.charts()[start + index];
            if descriptor.steps_type != addition.layout
                || Grid::parse(
                    after.value(descriptor.notes.clone()),
                    addition.grid.columns(),
                )? != *addition.grid
            {
                return Err(Error::InvalidEdit(
                    "Generated notes changed inside the simfile".into(),
                ));
            }
        }
        self.execute(
            label.into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
            })),
        )?;
        Ok((start..start + additions.len()).collect())
    }

    pub(crate) fn remove_chart(&mut self, chart: usize) -> Result<(), Error> {
        let snapshot = Document::parse(self.serialize()?, self.document.format)?;
        let bytes = crate::chart_deletion::prepare(&snapshot, chart)?;
        let after = Document::parse(bytes, self.document.format)?;
        if after.charts().len() + 1 != snapshot.charts().len() {
            return Err(Error::InvalidEdit(
                "Deleted chart structure is ambiguous".into(),
            ));
        }
        self.execute(
            "Delete chart".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
            })),
        )
    }

    pub(crate) fn insert_empty_chart(
        &mut self,
        block: Vec<u8>,
        grid: &Grid,
        metadata: crate::generation::metadata::ChartData,
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
        let mut after = Document::parse(Arc::<[u8]>::from(bytes), self.document.format)?;
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
        let descriptor = &after.charts()[index];
        if layout_columns(&descriptor.steps_type) != Some(grid.columns())
            || Grid::parse(after.value(descriptor.notes.clone()), grid.columns())? != *grid
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
        self.execute(
            "Create chart".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
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
        let current = Document::parse(Arc::<[u8]>::from(bytes.as_slice()), self.document.format)?;
        let range = current.chart_block_range(chart)?;
        let index = current.charts().len();
        let block = bytes[range].to_vec();
        bytes.extend_from_slice(b"\n\n");
        bytes.extend_from_slice(&block);
        let mut after = Document::parse(Arc::<[u8]>::from(bytes), self.document.format)?;
        if after.charts().len() != index + 1 {
            return Err(Error::InvalidEdit(
                "Duplicated chart structure is ambiguous".into(),
            ));
        }
        if self.document.format == crate::document::Format::Sm {
            for prefix in ["GAChartDataVersion", "GAChartData"] {
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
                || Grid::parse(after.value(descriptor.notes.clone()), columns)? != *grid
            {
                return Err(Error::InvalidEdit(
                    "Duplicated chart notes do not match the working grid".into(),
                ));
            }
        }
        self.execute(
            "Duplicate chart".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document: after,
                grids: BTreeMap::new(),
            })),
        )?;
        Ok(index)
    }

    fn execute(&mut self, label: String, mut change: Change) -> Result<(), Error> {
        let next = self
            .next_revision
            .checked_add(1)
            .ok_or_else(|| Error::InvalidEdit("Editor revision overflow".into()))?;
        self.apply(&mut change, true)?;
        fn affects_song(change: &Change) -> bool {
            match change {
                Change::ProfileDeletion { .. } | Change::ProfileCreation { .. } => false,
                Change::Group(changes) => changes.iter().any(affects_song),
                _ => true,
            }
        }
        let content_revision = if affects_song(&change) { self.next_revision } else { self.content_revision };
        self.undo.push(Entry {
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
        Ok(())
    }

    fn apply(&mut self, change: &mut Change, forward: bool) -> Result<(), Error> {
        match change {
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
            Change::ProfileCreation { profile, directory, file, removed } => {
                if forward {
                    if let Some(removed) = removed {
                        removed.restore()?;
                    } else {
                        *file = Some(Box::new(profile.export_new(directory)?));
                    }
                } else if let Some(removed) = removed {
                    removed.delete_again()?;
                } else {
                    let source = file.as_ref().ok_or_else(|| Error::InvalidEdit("Missing created profile snapshot".into()))?;
                    *removed = Some(source.delete()?);
                }
                Ok(())
            }
            Change::ProfileDeletion { source, deleted } => {
                if forward {
                    match deleted {
                        Some(file) => { file.delete_again()?; }
                        None => { *deleted = Some(source.delete()?); }
                    }
                } else if let Some(file) = deleted {
                    file.restore()?;
                }
                Ok(())
            }
            Change::PlayerCount { chart, before, after } => {
                self.grid(*chart)?;
                self.grids.get_mut(chart).unwrap().set_players(if forward { *after } else { *before })
            }
            Change::DocumentSwap(state) => {
                std::mem::swap(&mut self.document, &mut state.document);
                std::mem::swap(&mut self.grids, &mut state.grids);
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
                Ok(())
            }
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Error> {
        let mut document = self.document.clone();
        // Merely viewing a chart must never change its source formatting.
        fn collect(change: &Change, charts: &mut std::collections::BTreeSet<usize>) {
            match change {
                Change::Notes { chart, .. } | Change::PlayerCount { chart, .. } => {
                    charts.insert(*chart);
                }
                Change::Group(changes) => {
                    for change in changes {
                        collect(change, charts);
                    }
                }
                _ => {}
            }
        }
        let mut changed = std::collections::BTreeSet::new();
        for entry in self
            .undo
            .iter()
            .rev()
            .take_while(|entry| !matches!(entry.change, Change::DocumentSwap(..)))
        {
            collect(&entry.change, &mut changed);
        }
        for chart in changed {
            let descriptor = &document.charts()[chart];
            let grid = &self.grids[&chart];
            let original = Grid::parse(document.value(descriptor.notes.clone()), grid.columns())?;
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
        let mut owner = None;
        let mut single_player = true;
        let notes = grid
            .iter()
            .filter(|(pos, _)| {
                pos.row >= start
                    && pos.row <= end
                    && player.is_none_or(|player| pos.player == player)
            })
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
    pub fn paste_force(&self, session: &mut Session, chart: usize, row: u64, active_player: u32) -> Result<bool, Error> {
        let original = session.grid(chart)?;
        if original.columns() != self.columns {
            return Err(Error::InvalidEdit("Clipboard layout has a different number of lanes".into()));
        }
        let mut staged = original.clone();
        let mut notes = self.notes.clone();
        notes.sort_by_key(|(pos, _)| (pos.row, pos.lane, pos.player));
        for (pos, mut note) in notes {
            let player = if self.single_player { active_player }
                else if pos.player < staged.players() { pos.player } else { 0 };
            let position = Position { player, row: pos.row.checked_add(row)
                .ok_or_else(|| Error::InvalidEdit("Paste exceeds the row range".into()))?, ..pos };
            note.end = note.end.map(|end| end.checked_add(row)
                .ok_or_else(|| Error::InvalidEdit("Paste exceeds the row range".into()))).transpose()?;
            staged.force_insert(position, note)?;
        }
        session.apply_grid_delta(chart, &staged, "Paste notes")
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
