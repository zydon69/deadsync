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
                self.file.session.undo()?;
                self.sync_active_chart()?;
            }
            Command::Redo => {
                self.file.session.redo()?;
                self.sync_active_chart()?;
            }
            Command::Save => self.file.save()?,
            Command::SaveRecovery => self.file.save_recovery()?,
            Command::Recover => {
                self.file.recover()?;
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

    pub fn apply_generated(
        &mut self,
        preview: &crate::generation::job::ChartPreview,
    ) -> Result<usize, Error> {
        if self.chart != preview.source_chart() {
            return Err(Error::InvalidEdit(
                "Select the source chart before applying its generation preview".into(),
            ));
        }
        let chart = preview.apply(&mut self.file.session)?;
        self.file.session.grid(chart)?;
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
