//! Pattern-region selection and hold trimming from GrooveAuthor
//! ActionDeletePatternNotes / ActionAutoGeneratePatterns, 48e18f71 (MIT).
use super::context::ContextEvent;
use crate::{
    Error,
    notes::{Grid, Kind, Note, Position},
};
use std::sync::atomic::{AtomicBool, Ordering};

pub type NoteEdit = (Position, Option<Note>);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Deletions {
    pub removed: usize,
    pub shortened_holds: usize,
}

pub fn deletion_edits(
    grid: &Grid,
    player: u32,
    start: u64,
    end: u64,
    spacing: u64,
    cancelled: &AtomicBool,
) -> Result<(Vec<NoteEdit>, Deletions), Error> {
    check(cancelled)?;
    if start > end || spacing == 0 || player >= grid.players() {
        return invalid("Invalid pattern deletion range");
    }
    let mut edits = Vec::new();
    let mut report = Deletions::default();
    for (position, note) in grid
        .visible(start, end)
        .filter(|(position, _)| position.player == player)
    {
        check(cancelled)?;
        if position.row >= start {
            edits.push((position, None));
            report.removed += 1;
        } else if position
            .row
            .checked_add(1)
            .is_some_and(|head_plus_one| head_plus_one < start)
        {
            let desired_end = start
                .checked_sub(spacing)
                .filter(|&end| end > position.row)
                .unwrap_or(start - 1);
            let mut shortened = note.clone();
            shortened.end = Some(desired_end);
            edits.push((position, Some(shortened)));
            report.shortened_holds += 1;
        } else {
            // A one-row gap cannot contain a shortened sustain. Like the
            // original editor, delete the entire overlapping hold instead.
            edits.push((position, None));
            report.removed += 1;
        }
    }
    Ok((edits, report))
}

/// Interpret holds as complete head/tail pairs even when their tails extend
/// beyond the queried range. Overlapping heads precede other editor events in
/// lane order, exactly as EditorChart.GetEventsInRangeForPattern returns them.
pub fn extract(
    grid: &Grid,
    player: u32,
    start: u64,
    end: u64,
    cancelled: &AtomicBool,
) -> Result<(Grid, Vec<ContextEvent>), Error> {
    check(cancelled)?;
    if player >= grid.players() {
        return invalid("Pattern participant does not exist");
    }
    let mut selected = Grid::new(grid.columns(), 1)?;
    let mut overlapping = Vec::new();
    let mut normal = Vec::new();
    if start > end {
        return Ok((selected, Vec::new()));
    }
    for (position, note) in grid
        .visible(start, end)
        .filter(|(position, _)| position.player == player)
    {
        check(cancelled)?;
        selected.set(
            Position {
                player: 0,
                ..position
            },
            Some(note.clone()),
        )?;
        let event = ContextEvent {
            row: position.row,
            lane: Some(position.lane),
            is_step: is_step(note.kind),
        };
        if note.end.is_some() && position.row <= start {
            overlapping.push(event);
        } else {
            normal.push(event);
        }
    }
    overlapping.sort_by_key(|event| event.lane);
    normal.sort_by_key(|event| (event.row, event.lane, note_order(grid, player, *event)));
    overlapping.extend(normal);
    Ok((selected, overlapping))
}
fn note_order(grid: &Grid, player: u32, event: ContextEvent) -> u8 {
    match grid
        .get(Position {
            player,
            row: event.row,
            lane: event.lane.unwrap(),
        })
        .unwrap()
        .kind
    {
        Kind::Tap | Kind::Fake | Kind::Lift => 0,
        Kind::Hold | Kind::Roll => 1,
        _ => 2,
    }
}
pub fn is_step(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Tap | Kind::Hold | Kind::Roll | Kind::Fake | Kind::Lift
    )
}

/// Expand to at least 32 surrounding step heads, or the configured minimum
/// transition distance on the preceding side. Include the complete boundary row.
/// Input is editor-event order, not raw Fumen order (hold tails are not events).
pub fn bounds(
    events: &[ContextEvent],
    start: u64,
    end: u64,
    preceding_steps: usize,
    cancelled: &AtomicBool,
) -> Result<(u64, u64), Error> {
    check(cancelled)?;
    if start > end || events.windows(2).any(|pair| pair[0].row > pair[1].row) {
        return invalid("Pattern context events must be ordered");
    }
    let mut range_start = start;
    let mut range_end = end;
    let mut count = 0;
    let mut final_row = None;
    for event in events.iter().rev().filter(|event| event.row < start) {
        check(cancelled)?;
        if final_row.is_some_and(|row| event.row < row) {
            break;
        }
        range_start = event.row;
        if event.is_step {
            count += 1;
            if count == preceding_steps.max(32) {
                final_row = Some(event.row);
            }
        }
    }
    count = 0;
    final_row = None;
    for event in events.iter().filter(|event| event.row >= end) {
        check(cancelled)?;
        if final_row.is_some_and(|row| event.row > row) {
            break;
        }
        range_end = event.row;
        if event.row > end && event.is_step {
            count += 1;
            if count == 32 {
                final_row = Some(event.row);
            }
        }
    }
    Ok((range_start, range_end))
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        return invalid("Pattern region preparation cancelled");
    }
    Ok(())
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidEdit(message.into()))
}
