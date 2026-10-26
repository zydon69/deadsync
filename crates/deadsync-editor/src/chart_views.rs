//! Ordered open chart views sharing one external Session and musical instant.
//! Mirrors GrooveAuthor Editor.cs open/dedicated/focus/close semantics (MIT,
//! 48e18f71655d0f0d86144c63239331833501fda2). No document or history is copied.
use crate::{
    Error,
    chart_focus::Position,
    generation::metadata::Guid,
    history::{ContentToken, Session},
    playback::Clock,
};

#[derive(Clone)]
pub struct ViewState {
    identity: Guid,
    dedicated: bool,
    position: Position,
    clock: std::sync::Arc<Clock>,
    clock_token: ContentToken,
    selection_anchor: Option<u64>,
}
impl std::fmt::Debug for ViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewState")
            .field("identity", &self.identity)
            .field("dedicated", &self.dedicated)
            .field("position", &self.position)
            .field("selection_anchor", &self.selection_anchor)
            .finish_non_exhaustive()
    }
}
impl ViewState {
    pub fn identity(&self) -> &Guid {
        &self.identity
    }
    pub fn dedicated(&self) -> bool {
        self.dedicated
    }
    pub fn position(&self) -> &Position {
        &self.position
    }
    pub fn source_index(&self) -> usize {
        self.position.chart()
    }
    /// Immutable clock shared with native projection caches. The handle is
    /// reusable only for this resident chart and exact song content state.
    pub fn clock(&self, session: &Session) -> Result<std::sync::Arc<Clock>, Error> {
        if !session.accepts_content_token(&self.clock_token)
            || session.chart_ids().get(self.source_index()) != Some(&self.identity)
        {
            return Err(Error::InvalidEdit("Chart-view clock is stale".into()));
        }
        Ok(self.clock.clone())
    }
    /// Map another instant through this resident view's validated immutable clock.
    pub fn position_at_time(&self, session: &Session, music_time: f64) -> Result<Position, Error> {
        let clock = self.clock(session)?;
        Position::with_clock(session, self.source_index(), music_time, &clock)
    }
    /// Use this resident view's validated shared clock for row-preserving focus.
    pub fn position_at_row(&self, session: &Session, row: f64) -> Result<Position, Error> {
        let clock = self.clock(session)?;
        Position::with_row_clock(session, self.source_index(), row, &clock)
    }
    pub fn selection_anchor(&self) -> Option<u64> {
        self.selection_anchor
    }
    pub fn selection(&self) -> Option<(u64, u64)> {
        self.selection_anchor.map(|anchor| {
            (
                anchor.min(self.position.edit_row()),
                anchor.max(self.position.edit_row()),
            )
        })
    }
}

#[derive(Clone)]
pub struct ActiveChartViews {
    owner: ContentToken,
    entries: Vec<ViewState>,
    focused: Option<Guid>,
    music_time: f64,
}
impl ActiveChartViews {
    pub fn empty(session: &Session, music_time: f64) -> Result<Self, Error> {
        finite_time(music_time)?;
        Ok(Self {
            owner: session.content_token(),
            entries: Vec::new(),
            focused: None,
            music_time,
        })
    }
    pub fn entries(&self) -> &[ViewState] {
        &self.entries
    }
    pub fn focused(&self) -> Option<&ViewState> {
        self.focused
            .as_ref()
            .and_then(|id| self.entries.iter().find(|entry| &entry.identity == id))
    }
    pub fn music_time(&self) -> f64 {
        self.music_time
    }
    pub fn belongs_to(&self, session: &Session) -> bool {
        session.owns_content_token(&self.owner)
    }

    /// Single-select focuses an existing view, or inserts a temporary view at
    /// the front. The previous temporary view is removed; dedicated views stay.
    pub fn focus(&mut self, session: &Session, chart: usize) -> Result<(), Error> {
        self.transaction(session, |next| next.focus_inner(session, chart))
    }
    /// Show without changing focus when other views are open. Already shown
    /// charts retain their dedicated flag, matching upstream ShowChart.
    pub fn show(&mut self, session: &Session, chart: usize, dedicated: bool) -> Result<(), Error> {
        self.transaction(session, |next| {
            let id = identity(session, chart)?;
            if next.entries.iter().any(|entry| entry.identity == id) {
                return Ok(());
            }
            let view = next.capture(session, chart, dedicated)?;
            next.entries.push(view);
            if next.entries.len() == 1 {
                next.set_focus(id.clone());
            }
            next.remove_other_temporary(&id);
            Ok(())
        })
    }
    /// Dedicated-tab toggle follows SetChartHasDedicatedTab: unpinning the sole
    /// focused view retains it as temporary; otherwise unpin closes the view.
    pub fn set_dedicated(
        &mut self,
        session: &Session,
        chart: usize,
        dedicated: bool,
    ) -> Result<(), Error> {
        self.transaction(session, |next| {
            let id = identity(session, chart)?;
            if dedicated {
                if let Some(entry) = next.entries.iter_mut().find(|entry| entry.identity == id) {
                    entry.dedicated = true;
                } else {
                    let view = next.capture(session, chart, true)?;
                    next.entries.push(view);
                    if next.entries.len() == 1 {
                        next.set_focus(id);
                    }
                }
            } else if next.entries.len() == 1 && next.focused.as_ref() == Some(&id) {
                next.entries[0].dedicated = false;
            } else if let Some(index) = next.entries.iter().position(|entry| entry.identity == id) {
                next.close_index(index);
            }
            Ok(())
        })
    }
    pub fn close(&mut self, session: &Session, id: &Guid) -> Result<(), Error> {
        self.transaction(session, |next| {
            if let Some(index) = next.entries.iter().position(|entry| &entry.identity == id) {
                next.close_index(index);
            }
            Ok(())
        })
    }
    /// Independent open-view order, with no source document movement and no
    /// wrap at either endpoint, matching MoveActiveChartLeft/Right.
    pub fn move_view(&mut self, session: &Session, id: &Guid, right: bool) -> Result<(), Error> {
        self.transaction(session, |next| {
            if let Some(index) = next.entries.iter().position(|entry| &entry.identity == id) {
                let neighbor = if right {
                    index
                        .checked_add(1)
                        .filter(|index| *index < next.entries.len())
                } else {
                    index.checked_sub(1)
                };
                if let Some(neighbor) = neighbor {
                    next.entries.swap(index, neighbor);
                }
            }
            Ok(())
        })
    }
    pub fn cycle_focus(&mut self, session: &Session, backwards: bool) -> Result<(), Error> {
        self.transaction(session, |next| {
            if next.entries.is_empty() {
                return Ok(());
            }
            let index = next
                .focused
                .as_ref()
                .and_then(|id| next.entries.iter().position(|entry| &entry.identity == id))
                .unwrap_or(0);
            let delta = if backwards { next.entries.len() - 1 } else { 1 };
            next.set_focus(
                next.entries[(index + delta) % next.entries.len()]
                    .identity
                    .clone(),
            );
            Ok(())
        })
    }
    /// Every visible position is mapped independently using its chart clock.
    /// Failure in any chart leaves all positions and the shared time unchanged.
    pub fn set_music_time(&mut self, session: &Session, music_time: f64) -> Result<(), Error> {
        finite_time(music_time)?;
        self.transaction(session, |next| {
            next.music_time = music_time;
            Ok(())
        })
    }
    /// The selection belongs to the focused view only. Losing focus clears it,
    /// matching ActiveEditorChart.SetFocused(false), rather than retaining an
    /// unfocused selection which could later receive edits unexpectedly.
    pub fn set_selection_anchor(
        &mut self,
        session: &Session,
        anchor: Option<u64>,
    ) -> Result<(), Error> {
        self.transaction(session, |next| {
            let id = next
                .focused
                .as_ref()
                .ok_or_else(|| Error::InvalidEdit("No focused chart view".into()))?;
            let entry = next
                .entries
                .iter_mut()
                .find(|entry| &entry.identity == id)
                .expect("focused view exists");
            entry.selection_anchor = anchor;
            Ok(())
        })
    }
    /// Rebind surviving resident IDs after structural edits. The optional
    /// source focus comes from Workspace's existing structural undo contract;
    /// restoring it can reopen one temporary view, without resurrecting tabs
    /// that the reference does not promise to restore.
    pub fn reconcile(
        &mut self,
        session: &Session,
        preferred_focus: Option<usize>,
    ) -> Result<(), Error> {
        self.synchronize(session, preferred_focus, self.music_time)
    }
    /// Reconcile identities and change the shared instant in one preparation.
    /// Clocks are rebuilt only when their content token/index changes.
    pub fn synchronize(
        &mut self,
        session: &Session,
        preferred_focus: Option<usize>,
        music_time: f64,
    ) -> Result<(), Error> {
        finite_time(music_time)?;
        self.ensure_owner(session)?;
        let mut next = self.clone();
        next.music_time = music_time;
        let eligible = eligible_indices(session);
        let previous_focus = next.focused.clone();
        let previous_index = previous_focus
            .as_ref()
            .and_then(|id| next.entries.iter().position(|entry| &entry.identity == id));
        let right_neighbor = previous_index.and_then(|index| {
            next.entries[index + 1..]
                .iter()
                .find(|entry| eligible.contains_key(&entry.identity))
                .map(|entry| entry.identity.clone())
        });
        let left_neighbor = previous_index.and_then(|index| {
            next.entries[..index]
                .iter()
                .rev()
                .find(|entry| eligible.contains_key(&entry.identity))
                .map(|entry| entry.identity.clone())
        });
        next.entries
            .retain(|entry| eligible.contains_key(&entry.identity));
        if !next
            .focused
            .as_ref()
            .is_some_and(|id| next.entries.iter().any(|entry| &entry.identity == id))
        {
            next.focused = None;
            if let Some(id) = right_neighbor.or(left_neighbor) {
                next.set_focus(id);
            }
        }
        if let Some(chart) = preferred_focus {
            next.focus_inner(session, chart)?;
        }
        next.sync_positions(session)?;
        *self = next;
        Ok(())
    }
    fn transaction(
        &mut self,
        session: &Session,
        operation: impl FnOnce(&mut Self) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.ensure_owner(session)?;
        let mut next = self.clone();
        operation(&mut next)?;
        next.sync_positions(session)?;
        *self = next;
        Ok(())
    }
    fn ensure_owner(&self, session: &Session) -> Result<(), Error> {
        if self.belongs_to(session) {
            Ok(())
        } else {
            Err(Error::InvalidEdit(
                "Chart views belong to another song session".into(),
            ))
        }
    }
    fn capture(
        &self,
        session: &Session,
        chart: usize,
        dedicated: bool,
    ) -> Result<ViewState, Error> {
        let clock = std::sync::Arc::new(Clock::capture(session, chart)?);
        Ok(ViewState {
            identity: identity(session, chart)?,
            dedicated,
            position: Position::with_clock(session, chart, self.music_time, &clock)?,
            clock,
            clock_token: session.content_token(),
            selection_anchor: None,
        })
    }
    fn focus_inner(&mut self, session: &Session, chart: usize) -> Result<(), Error> {
        let id = identity(session, chart)?;
        if self.focused.as_ref() == Some(&id) {
            return Ok(());
        }
        if !self.entries.iter().any(|entry| entry.identity == id) {
            let view = self.capture(session, chart, false)?;
            self.entries.insert(0, view);
        }
        self.set_focus(id.clone());
        self.remove_other_temporary(&id);
        Ok(())
    }
    fn set_focus(&mut self, id: Guid) {
        if self.focused.as_ref() == Some(&id) {
            return;
        }
        if let Some(old) = self
            .focused
            .as_ref()
            .and_then(|old| self.entries.iter_mut().find(|entry| &entry.identity == old))
        {
            old.selection_anchor = None;
        }
        self.focused = Some(id);
    }
    fn close_index(&mut self, index: usize) {
        if self.focused.as_ref() == Some(&self.entries[index].identity) {
            let neighbor = self
                .entries
                .get(index + 1)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|index| self.entries.get(index))
                })
                .map(|entry| entry.identity.clone());
            if let Some(id) = neighbor {
                self.set_focus(id);
            } else {
                self.focused = None;
            }
        }
        self.entries.remove(index);
    }
    fn remove_other_temporary(&mut self, keep: &Guid) {
        // Only a newly shown temporary view can make the count exceed one.
        if self.entries.iter().filter(|entry| !entry.dedicated).count() <= 1 {
            return;
        }
        for index in (0..self.entries.len()).rev() {
            if !self.entries[index].dedicated && &self.entries[index].identity != keep {
                self.close_index(index);
            }
        }
    }
    fn sync_positions(&mut self, session: &Session) -> Result<(), Error> {
        let eligible = eligible_indices(session);
        for entry in &mut self.entries {
            let chart = *eligible.get(&entry.identity).ok_or_else(|| {
                Error::InvalidEdit(
                    "Open chart view is stale; reconcile structural changes first".into(),
                )
            })?;
            let current =
                session.accepts_content_token(&entry.clock_token) && entry.source_index() == chart;
            if current && entry.position.music_time().to_bits() == self.music_time.to_bits() {
                continue;
            }
            if !current {
                entry.clock = std::sync::Arc::new(Clock::capture(session, chart)?);
                entry.clock_token = session.content_token();
            }
            entry.position = Position::with_clock(session, chart, self.music_time, &entry.clock)?;
        }
        Ok(())
    }
}
fn identity(session: &Session, chart: usize) -> Result<Guid, Error> {
    let descriptor = session
        .document()
        .charts()
        .get(chart)
        .ok_or_else(|| Error::InvalidEdit("Invalid chart view index".into()))?;
    if crate::notes::layout_columns(&descriptor.steps_type).is_none() {
        return Err(Error::Unsupported("Unsupported chart view layout".into()));
    }
    Ok(session.chart_ids()[chart].clone())
}
fn eligible_indices(session: &Session) -> std::collections::BTreeMap<&Guid, usize> {
    session
        .chart_ids()
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            crate::notes::layout_columns(&session.document().charts()[*index].steps_type).is_some()
        })
        .map(|(index, id)| (id, index))
        .collect()
}
fn finite_time(time: f64) -> Result<(), Error> {
    if time.is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidEdit(
            "Nonfinite shared chart-view time".into(),
        ))
    }
}
