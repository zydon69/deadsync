//! Explicit musical focus transitions for native chart selection. The legacy
//! SelectChart command retains its row-based behavior for existing callers.
use crate::{
    Error,
    chart_order::Binding,
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
        let music_time = Clock::capture(session, chart)?.music_time(row)?;
        Self::at_time(session, chart, music_time)
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
    source: Binding,
    source_row: u64,
    target: Position,
}
impl Transition {
    pub fn at_time(workspace: &Workspace, target: usize, music_time: f64) -> Result<Self, Error> {
        Ok(Self {
            source: Binding::capture(&workspace.file().session, workspace.chart())?,
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
            || !self
                .source
                .is_current(&workspace.file().session, workspace.chart())
        {
            return Err(Error::InvalidEdit("Chart focus transition is stale".into()));
        }
        workspace.commit_focus_position(self.target.chart, self.target.edit_row)?;
        Ok(self.target)
    }
}
