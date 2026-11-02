//! Explicit musical focus transitions for native chart selection. The legacy
//! SelectChart command retains its row-based behavior for existing callers.
mod rebind;
use crate::{
    Error,
    chart_order::{Binding, SessionBinding},
    generation::metadata::Guid,
    history::{ContentToken, Session},
    playback::Clock,
    workspace::Workspace,
};

/// Exact presentation position, including fractional/negative rows and time
/// inside stops. The edit cursor remains nonnegative and integral.
#[derive(Clone, Debug)]
pub struct Position {
    token: ContentToken,
    identity: Guid,
    chart: usize,
    music_time: f64,
    chart_time: f64,
    row: f64,
    edit_row: u64,
}
impl Position {
    pub fn at_time(session: &Session, chart: usize, music_time: f64) -> Result<Self, Error> {
        let descriptor = session
            .document()
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Invalid focus chart".into()))?;
        if crate::notes::layout_columns(&descriptor.steps_type).is_none() {
            return Err(Error::Unsupported(
                "Focus chart has an unsupported layout".into(),
            ));
        }
        let clock = Clock::capture(session, chart)?;
        Self::with_clock(session, chart, music_time, &clock)
    }
    pub(crate) fn with_clock(
        session: &Session,
        chart: usize,
        music_time: f64,
        clock: &Clock,
    ) -> Result<Self, Error> {
        let chart_time = clock.music_to_chart_time(music_time)?;
        let row = clock.position(music_time)?;
        if !row.is_finite() {
            return Err(Error::InvalidEdit("Nonfinite focus position".into()));
        }
        Ok(Self {
            token: session.content_token(),
            identity: session.chart_ids()[chart].clone(),
            chart,
            music_time,
            chart_time,
            row,
            // Match the existing audio transport's integral cursor policy.
            edit_row: row.max(0.0).min(i32::MAX as f64).floor() as u64,
        })
    }
    pub fn at_row(session: &Session, chart: usize, row: u64) -> Result<Self, Error> {
        Self::at_fractional_row(session, chart, row as f64)
    }
    /// Preserve a chosen row through equal-time warps; never round-trip via time.
    pub fn at_fractional_row(session: &Session, chart: usize, row: f64) -> Result<Self, Error> {
        let clock = Clock::capture(session, chart)?;
        Self::with_row_clock(session, chart, row, &clock)
    }
    pub(crate) fn with_row_clock(
        session: &Session,
        chart: usize,
        row: f64,
        clock: &Clock,
    ) -> Result<Self, Error> {
        let descriptor = session
            .document()
            .charts()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Invalid focus chart".into()))?;
        if crate::notes::layout_columns(&descriptor.steps_type).is_none() {
            return Err(Error::Unsupported(
                "Focus chart has an unsupported layout".into(),
            ));
        }
        if !row.is_finite() {
            return Err(Error::InvalidEdit("Nonfinite focus position".into()));
        }
        let identity = session
            .chart_ids()
            .get(chart)
            .cloned()
            .ok_or_else(|| Error::InvalidEdit("Missing focus chart identity".into()))?;
        Ok(Self {
            token: session.content_token(),
            identity,
            chart,
            row,
            chart_time: clock.fractional_chart_time(row)?,
            music_time: clock.fractional_music_time(row)?,
            edit_row: row.max(0.0).min(i32::MAX as f64).floor() as u64,
        })
    }
    pub fn music_time(&self) -> f64 {
        self.music_time
    }
    pub fn chart_time(&self) -> f64 {
        self.chart_time
    }
    pub fn row(&self) -> f64 {
        self.row
    }
    pub fn edit_row(&self) -> u64 {
        self.edit_row
    }
    pub fn chart(&self) -> usize {
        self.chart
    }
    /// Validate a captured endpoint without requiring it to be the active edit
    /// cursor. Region boundaries may refer to different rows in the same clock.
    pub fn is_bound_to(&self, session: &Session, chart: usize) -> bool {
        self.chart == chart
            && session.accepts_content_token(&self.token)
            && session.chart_ids().get(chart) == Some(&self.identity)
    }
    pub fn is_current(&self, workspace: &Workspace) -> bool {
        self.chart == workspace.chart()
            && self.edit_row == workspace.row()
            && workspace.file().session.accepts_content_token(&self.token)
            && workspace.file().session.chart_ids().get(self.chart) == Some(&self.identity)
    }
}

/// Prepare all clock conversions before changing focus. Binding also rejects
/// deferred results after source focus, cursor, content, or session changes.
pub struct Transition {
    source: Option<Binding>,
    session: SessionBinding,
    source_row: u64,
    target: Position,
}
impl Transition {
    pub fn at_time(workspace: &Workspace, target: usize, music_time: f64) -> Result<Self, Error> {
        Ok(Self {
            source: Some(Binding::capture(
                &workspace.file().session,
                workspace.chart(),
            )?),
            session: SessionBinding::capture(&workspace.file().session),
            source_row: workspace.row(),
            target: Position::at_time(&workspace.file().session, target, music_time)?,
        })
    }
    /// Reopen a view when the song has no currently focused chart.
    pub fn open_at_time(
        workspace: &Workspace,
        target: usize,
        music_time: f64,
    ) -> Result<Self, Error> {
        if workspace.has_chart() {
            return Self::at_time(workspace, target, music_time);
        }
        Ok(Self {
            source: None,
            session: SessionBinding::capture(&workspace.file().session),
            source_row: workspace.row(),
            target: Position::at_time(&workspace.file().session, target, music_time)?,
        })
    }
    pub fn at_cursor(workspace: &Workspace, target: usize) -> Result<Self, Error> {
        let clock = Clock::capture(&workspace.file().session, workspace.chart())?;
        Self::at_time(workspace, target, clock.music_time(workspace.row())?)
    }
    pub fn position(&self) -> &Position {
        &self.target
    }
    pub fn apply(self, workspace: &mut Workspace) -> Result<Position, Error> {
        if self.source_row != workspace.row()
            || !self.session.is_current(&workspace.file().session)
            || !self
                .source
                .as_ref()
                .map_or(!workspace.has_chart(), |source| {
                    source.is_current(&workspace.file().session, workspace.chart())
                })
        {
            return Err(Error::InvalidEdit("Chart focus transition is stale".into()));
        }
        workspace.commit_focus_position(self.target.chart, self.target.edit_row)?;
        Ok(self.target)
    }
}
