//! Keyboard lane transactions derived from GrooveAuthor ActiveEditorChart's
//! Lane Input region and LaneEditState (48e18f71, MIT). Pending notes are detached
//! previews: no partial document/history edits escape before the lane release.
use crate::{
    Error,
    chart_order::Binding,
    history::Session,
    notes::{Grid, Kind, Note, Position},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draft {
    pub position: Position,
    pub kind: Kind,
    pub end: Option<u64>,
}
#[derive(Clone, Debug)]
struct Gesture {
    anchor: u64,
    initial: Option<Position>,
    draft: Option<Draft>,
}
#[derive(Clone, Copy, Debug)]
pub struct Press {
    pub lane: u16,
    pub player: u32,
    pub row: f64,
    pub snap_rows: u64,
    pub modified: bool,
    pub playing: bool,
    pub advance_by_snap: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub changed: bool,
    pub advance_to: Option<u64>,
}
pub struct Inputs {
    chart: usize,
    binding: Binding,
    lanes: BTreeMap<u16, Gesture>,
}
fn invalid(s: &str) -> Error {
    Error::InvalidEdit(s.into())
}
/// Source GetNearestRow truncates a nonnegative exact half downward. Down's snap
/// is a second operation and resolves an exact snap midpoint upward.
pub fn nearest(row: f64) -> Result<u64, Error> {
    if !row.is_finite() || row < 0.0 || row > f64::from(i32::MAX) {
        return Err(invalid("Lane input row outside editor range"));
    }
    let n = row.trunc() as u64;
    Ok(n + u64::from(row - n as f64 > 0.5))
}
pub fn snap(row: u64, step: u64) -> Result<u64, Error> {
    if step == 0 {
        return Ok(row);
    }
    if step > i32::MAX as u64 {
        return Err(invalid("Lane input snap outside editor range"));
    }
    let base = row / step * step;
    let result = if (row - base) as f64 >= step as f64 * 0.5 {
        base.checked_add(step)
            .ok_or_else(|| invalid("Lane input snap overflow"))?
    } else {
        base
    };
    if result > i32::MAX as u64 {
        return Err(invalid("Lane input snap exceeds editor rows"));
    }
    Ok(result)
}
impl Inputs {
    pub fn capture(session: &mut Session, chart: usize) -> Result<Self, Error> {
        let _ = session.grid(chart)?;
        Ok(Self {
            chart,
            binding: Binding::capture(session, chart)?,
            lanes: BTreeMap::new(),
        })
    }
    fn validate(&self, session: &Session) -> Result<(), Error> {
        if self.binding.is_current(session, self.chart) {
            Ok(())
        } else {
            Err(invalid("Pending lane input belongs to stale chart content"))
        }
    }
    pub fn chart(&self) -> usize {
        self.chart
    }
    pub fn is_current(&self, session: &Session) -> bool {
        self.binding.is_current(session, self.chart)
    }
    /// Hide these live heads while rendering the detached preview. Cancellation
    /// restores visibility without mutating the underlying document.
    pub fn deleted_heads(&self) -> impl Iterator<Item = Position> + '_ {
        self.lanes.values().filter_map(|gesture| gesture.initial)
    }
    pub fn drafts(&self) -> impl Iterator<Item = Draft> + '_ {
        self.lanes.values().filter_map(|g| g.draft)
    }
    pub fn is_held(&self, lane: u16) -> bool {
        self.lanes.contains_key(&lane)
    }
    pub fn cancel(&mut self) -> usize {
        let n = self.lanes.len();
        self.lanes.clear();
        n
    }
    pub fn press(&mut self, session: &mut Session, p: Press) -> Result<Outcome, Error> {
        self.validate(session)?;
        let grid = session.grid(self.chart)?;
        if p.lane >= grid.columns() || p.player >= grid.players() {
            return Err(invalid("Lane input participant or lane outside chart"));
        }
        if !p.row.is_finite() {
            return Err(invalid("Nonfinite lane input position"));
        }
        if p.row < 0.0 || self.lanes.contains_key(&p.lane) {
            return Ok(Outcome::default());
        }
        let row = snap(nearest(p.row)?, p.snap_rows)?;
        let initial = find_at(grid, p.lane, row)
            .filter(|(pos, _)| pos.row == row)
            .map(|(pos, _)| pos);
        let draft = initial.is_none().then_some(Draft {
            position: Position {
                player: p.player,
                lane: p.lane,
                row,
            },
            kind: if p.modified { Kind::Mine } else { Kind::Tap },
            end: None,
        });
        let advance_to = if !p.playing && p.advance_by_snap && p.snap_rows != 0 && initial.is_none()
        {
            Some(
                ((p.row as u64 / p.snap_rows) * p.snap_rows)
                    .checked_add(p.snap_rows)
                    .filter(|r| *r <= i32::MAX as u64)
                    .ok_or_else(|| invalid("Lane input advance exceeds editor rows"))?,
            )
        } else {
            None
        };
        self.lanes.insert(
            p.lane,
            Gesture {
                anchor: row,
                initial,
                draft,
            },
        );
        if p.playing || (p.advance_by_snap && p.snap_rows != 0 && initial.is_none()) {
            let changed = self.release(session, p.lane)?;
            Ok(Outcome {
                changed,
                advance_to,
            })
        } else {
            Ok(Outcome::default())
        }
    }
    pub fn move_to(
        &mut self,
        session: &Session,
        row: f64,
        current_player: u32,
        modified: bool,
    ) -> Result<(), Error> {
        self.validate(session)?;
        if !row.is_finite() {
            return Err(invalid("Nonfinite lane input position"));
        }
        let row = nearest(row.max(0.0))?;
        let players = session
            .loaded_grid(self.chart)
            .ok_or_else(|| invalid("Lane input grid not loaded"))?
            .players();
        if current_player >= players {
            return Err(invalid("Lane input participant outside chart"));
        }
        for (&lane, g) in &mut self.lanes {
            if row == g.anchor {
                if g.draft.is_some_and(|d| d.end.is_some()) {
                    let player = g.draft.unwrap().position.player;
                    g.draft = Some(Draft {
                        position: Position { player, lane, row },
                        kind: if modified { Kind::Mine } else { Kind::Tap },
                        end: None,
                    });
                }
            } else {
                let player = g.draft.map_or(current_player, |d| d.position.player);
                g.draft = Some(Draft {
                    position: Position {
                        player,
                        lane,
                        row: row.min(g.anchor),
                    },
                    kind: if modified { Kind::Roll } else { Kind::Hold },
                    end: Some(row.max(g.anchor)),
                });
            }
        }
        Ok(())
    }
    pub fn modifier(&mut self, session: &Session, modified: bool) -> Result<(), Error> {
        self.validate(session)?;
        for g in self.lanes.values_mut() {
            if let Some(d) = &mut g.draft {
                d.kind = if d.end.is_some() {
                    if modified { Kind::Roll } else { Kind::Hold }
                } else if modified {
                    Kind::Mine
                } else {
                    Kind::Tap
                };
            }
        }
        Ok(())
    }
    pub fn release(&mut self, session: &mut Session, lane: u16) -> Result<bool, Error> {
        self.validate(session)?;
        let Some(gesture) = self.lanes.get(&lane) else {
            return Ok(false);
        };
        let original = session.grid(self.chart)?.clone();
        let mut staged = original.clone();
        resolve(&mut staged, lane, gesture)?;
        let before: BTreeMap<_, _> = original.iter().map(|(p, n)| (p, n.clone())).collect();
        let after: BTreeMap<_, _> = staged.iter().map(|(p, n)| (p, n.clone())).collect();
        let keys: BTreeSet<_> = before.keys().chain(after.keys()).copied().collect();
        let edits = keys
            .into_iter()
            .filter(|p| before.get(p) != after.get(p))
            .map(|p| (p, after.get(&p).cloned()))
            .collect::<Vec<_>>();
        let changed = session.edit_notes(self.chart, "Enter lane note", edits)?;
        self.lanes.remove(&lane);
        // Other independently held physical lanes survive our own atomic history
        // operation; any unrelated mutation is still rejected by validate().
        self.binding = Binding::capture(session, self.chart)?;
        Ok(changed)
    }
}
fn find_at(grid: &Grid, lane: u16, row: u64) -> Option<(Position, Note)> {
    // EventTree.FindNoteAt scans backwards from the latest lane head. A later
    // nonoverlapping note blocks an older hold rather than searching past it.
    grid.iter()
        .filter(|(p, _)| p.lane == lane && p.row <= row)
        .max_by_key(|(p, n)| {
            (
                p.row,
                match n.kind {
                    Kind::Tap | Kind::Fake | Kind::Lift => 0,
                    Kind::Hold | Kind::Roll => 1,
                    _ => 3,
                },
                p.player,
            )
        })
        .filter(|(p, n)| n.end.unwrap_or(p.row) >= row)
        .map(|(p, n)| (p, n.clone()))
}
fn fresh(d: Draft) -> Note {
    Note {
        kind: d.kind,
        end: d.end,
        attributes: "".into(),
        tail_attributes: "".into(),
    }
}
fn resolve(grid: &mut Grid, lane: u16, g: &Gesture) -> Result<(), Error> {
    if let Some(p) = g.initial {
        grid.set(p, None)?;
    }
    let Some(mut draft) = g.draft else {
        return Ok(());
    };
    if draft.end.is_none() {
        if let Some((p, mut old)) = find_at(grid, lane, draft.position.row) {
            if old.end.is_some() {
                if p.row == draft.position.row {
                    grid.set(p, None)?;
                } else {
                    let end = draft.position.row.saturating_sub(12);
                    grid.set(p, None)?;
                    if end <= p.row {
                        old.kind = Kind::Tap;
                        old.end = None;
                        old.tail_attributes = "".into();
                    } else {
                        old.end = Some(end);
                    }
                    grid.set(p, Some(old))?;
                }
            } else if matches!(old.kind, Kind::Tap | Kind::Mine) {
                grid.set(p, None)?;
                if old.kind == Kind::Mine || draft.kind != Kind::Mine {
                    return Ok(());
                }
            }
        }
        return grid.set(draft.position, Some(fresh(draft))).map(|_| ());
    }
    let mut end = draft.end.unwrap();
    if let Some((p, mut old)) = find_at(grid, lane, draft.position.row) {
        if let Some(tail) = old.end {
            if p.row <= draft.position.row && tail >= end {
                if old.kind != draft.kind {
                    grid.set(p, None)?;
                    old.kind = draft.kind;
                    let target = Position {
                        player: draft.position.player,
                        ..p
                    };
                    grid.set(target, Some(old))?;
                }
                return Ok(());
            }
            if p.row < draft.position.row && tail >= draft.position.row && tail < end {
                draft.position.row = p.row;
            }
        }
    }
    if let Some((p, old)) = find_at(grid, lane, end) {
        if let Some(tail) = old.end {
            if p.row <= end && tail >= end && p.row > draft.position.row {
                end = tail;
            }
        }
    }
    let remove = grid
        .iter()
        .filter(|(p, n)| {
            p.lane == lane
                && p.row >= draft.position.row
                && p.row <= end
                && (matches!(n.kind, Kind::Tap | Kind::Mine | Kind::Fake | Kind::Lift)
                    || n.end.is_some_and(|tail| tail <= end))
        })
        .map(|(p, _)| p)
        .collect::<Vec<_>>();
    for p in remove {
        grid.set(p, None)?;
    }
    draft.end = Some(end);
    grid.set(draft.position, Some(fresh(draft))).map(|_| ())
}
