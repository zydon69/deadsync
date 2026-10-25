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
}

/// A revision is only meaningful within one live session. Recovery/reload creates
/// a new identity even when bytes and revision numbers happen to be identical.
#[derive(Clone, Debug)]
pub(crate) struct RevisionToken {
    identity: Arc<()>,
    revision: u64,
}

pub struct Session {
    identity: Arc<()>,
    document: Document,
    grids: BTreeMap<usize, Grid>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    revision: u64,
    saved_revision: u64,
    next_revision: u64,
}

impl Session {
    pub fn new(document: Document) -> Self {
        Self {
            identity: Arc::new(()),
            document,
            grids: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            saved_revision: 0,
            next_revision: 1,
        }
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

    pub(crate) fn accepts_token(&self, token: &RevisionToken) -> bool {
        Arc::ptr_eq(&self.identity, &token.identity) && self.revision == token.revision
    }

    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
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
        self.saved_revision = self.revision;
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
            let grid = Grid::parse(self.document.value(descriptor.notes.clone()), columns)?;
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
        if matches!(
            name.to_ascii_uppercase().as_str(),
            "NOTES" | "NOTES2" | "NOTEDATA" | "STEPSTYPE"
        ) {
            return Err(Error::InvalidEdit(
                "Structural chart properties require a chart operation".into(),
            ));
        }
        let after: Arc<[u8]> = escape_text(text).into();
        self.set_property_bytes(chart, name, after)
    }

    pub(crate) fn set_property_bytes(
        &mut self,
        chart: Option<usize>,
        name: &str,
        after: Arc<[u8]>,
    ) -> Result<bool, Error> {
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

    pub fn undo(&mut self) -> Result<bool, Error> {
        let Some(mut entry) = self.undo.pop() else {
            return Ok(false);
        };
        if let Err(error) = self.apply(&mut entry.change, false) {
            self.undo.push(entry);
            return Err(error);
        }
        self.revision = entry.before_revision;
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
        if self.revision != expected_revision {
            return Err(Error::InvalidEdit(
                "Document changed since chart preview".into(),
            ));
        }
        let columns = layout_columns(layout)
            .ok_or_else(|| Error::Unsupported(format!("Unknown chart layout {layout}")))?;
        if columns != grid.columns() {
            return Err(Error::InvalidEdit(
                "Generated grid does not match its chart layout".into(),
            ));
        }
        let notes: Arc<[u8]> = grid.serialize()?.into();
        // Re-parse before changing history to reject unsafe attributes or a
        // malformed chart body without clearing redo or changing revisions.
        if Grid::parse(&notes, columns)? != *grid {
            return Err(Error::InvalidEdit(
                "Generated notes do not round-trip".into(),
            ));
        }
        let block = self
            .document
            .cloned_chart_bytes(source_chart, layout, notes)?;
        let mut bytes = self.serialize()?;
        bytes.extend_from_slice(b"\n\n");
        bytes.extend_from_slice(&block);
        let after = Document::parse(Arc::<[u8]>::from(bytes), self.document.format)?;
        let index = self.document.charts().len();
        if after.charts().len() != index + 1 || after.charts()[index].steps_type != layout {
            return Err(Error::InvalidEdit(
                "Generated chart structure is ambiguous".into(),
            ));
        }
        let added = &after.charts()[index];
        if Grid::parse(after.value(added.notes.clone()), columns)? != *grid {
            return Err(Error::InvalidEdit(
                "Generated notes changed inside the simfile".into(),
            ));
        }
        self.execute(
            label.into(),
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
        self.undo.push(Entry {
            label,
            change,
            before_revision: self.revision,
            after_revision: self.next_revision,
        });
        self.revision = self.next_revision;
        self.next_revision = next;
        self.redo.clear();
        Ok(())
    }

    fn apply(&mut self, change: &mut Change, forward: bool) -> Result<(), Error> {
        match change {
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
        let changed: std::collections::BTreeSet<_> = self
            .undo
            .iter()
            .rev()
            .take_while(|entry| !matches!(entry.change, Change::DocumentSwap(..)))
            .filter_map(|entry| match entry.change {
                Change::Notes { chart, .. } => Some(chart),
                _ => None,
            })
            .collect();
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
