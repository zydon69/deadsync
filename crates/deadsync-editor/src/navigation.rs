//! GrooveAuthor Editor navigation callbacks, pinned 48e18f71655d0f0d86144c63239331833501fda2.
//! Strict fractional positions, no wrapping, and IsStep rather than autoplay
//! eligibility. Stream boundaries preserve chord multiplicity and infer spacing
//! from the opposite neighbor before the search-direction neighbor.
use crate::{Error, chart_order::Binding, history::Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    PreviousStep,
    NextStep,
    PreviousEvent,
    NextEvent,
    StreamStart,
    StreamEnd,
}
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub position: f64,
    pub step: bool,
}

/// Compact immutable row index. Equal-row steps must not be deduplicated:
/// the original event tree includes each lane of a chord independently.
#[derive(Debug)]
pub struct Lookup {
    events: Vec<f64>,
    steps: Vec<f64>,
}
impl Lookup {
    pub fn new(events: impl IntoIterator<Item = Event>) -> Result<Self, Error> {
        let mut rows = Vec::new();
        let mut steps = Vec::new();
        for event in events {
            if !event.position.is_finite() || (event.step && event.position.fract() != 0.0) {
                return Err(Error::InvalidEdit(
                    "Invalid navigation event position".into(),
                ));
            }
            rows.push(event.position);
            if event.step {
                steps.push(event.position);
            }
        }
        rows.sort_by(f64::total_cmp);
        rows.dedup();
        steps.sort_by(f64::total_cmp);
        Ok(Self {
            events: rows,
            steps,
        })
    }
    /// SelectedRows is a transient editor widget at the range start, not a
    /// persisted event and not part of the content-bound cache.
    pub fn target(
        &self,
        command: Command,
        cursor: f64,
        selected_rows: Option<f64>,
    ) -> Result<Option<f64>, Error> {
        if !cursor.is_finite() || selected_rows.is_some_and(|row| !row.is_finite()) {
            return Err(Error::InvalidEdit(
                "Invalid navigation cursor or selection".into(),
            ));
        }
        let preceding = matches!(
            command,
            Command::PreviousStep | Command::PreviousEvent | Command::StreamStart
        );
        let events = matches!(command, Command::PreviousEvent | Command::NextEvent);
        let rows = if events { &self.events } else { &self.steps };
        let index = if preceding {
            rows.partition_point(|row| *row < cursor).checked_sub(1)
        } else {
            let index = rows.partition_point(|row| *row <= cursor);
            (index < rows.len()).then_some(index)
        };
        let mut target = index.map(|index| rows[index]);
        if events {
            if let Some(row) = selected_rows.filter(|row| {
                if preceding {
                    *row < cursor
                } else {
                    *row > cursor
                }
            }) {
                target = Some(target.map_or(row, |target| {
                    if preceding {
                        target.max(row)
                    } else {
                        target.min(row)
                    }
                }));
            }
            return Ok(target);
        }
        if !matches!(command, Command::StreamStart | Command::StreamEnd) {
            return Ok(target);
        }
        let Some(mut index) = index else {
            return Ok(None);
        };
        let advance = |index: usize, backwards: bool| {
            if backwards {
                index.checked_sub(1)
            } else {
                index.checked_add(1).filter(|next| *next < rows.len())
            }
        };
        let spacing = advance(index, !preceding)
            .or_else(|| advance(index, preceding))
            .map(|other| (rows[other] - rows[index]).abs());
        if let Some(spacing) = spacing {
            while let Some(next) = advance(index, preceding) {
                if (rows[next] - rows[index]).abs() != spacing {
                    break;
                }
                index = next;
            }
        }
        Ok(Some(rows[index]))
    }
}
/// Content/chart-bound cache. Capturing shares the initialized editor event
/// inventory with autoplay; the temporary timeline is released after indexing.
pub struct Index {
    binding: Binding,
    lookup: Lookup,
}
impl Index {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let binding = Binding::capture(session, chart)?;
        let timeline = crate::autoplay::Timeline::capture(session, chart)?;
        let lookup = Lookup::new(timeline.navigation_events())?;
        Ok(Self { binding, lookup })
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        self.binding.is_current(session, chart)
    }
    pub fn target(
        &self,
        session: &Session,
        chart: usize,
        command: Command,
        cursor: f64,
        selected_rows: Option<f64>,
    ) -> Result<Option<f64>, Error> {
        if !self.is_current(session, chart) {
            return Err(Error::InvalidEdit("Stale navigation index".into()));
        }
        self.lookup.target(command, cursor, selected_rows)
    }
}

/// Source EditorChart.GetEndPosition uses the final event in row/type order,
/// excluding the preview widget, and returns that event's GetEndRow. It does
/// not take the maximum of all region ends or only the last note.
pub fn chart_end_position(session: &mut Session, chart: usize) -> Result<f64, Error> {
    chart_end_position_with_selection(session, chart, None)
}
/// SelectedRows sorts before every other event at the same chart position.
/// Its END is used only if its START sorts after the last non-preview event.
pub fn chart_end_position_with_selection(
    session: &mut Session,
    chart: usize,
    selected_rows: Option<(i32, i32)>,
) -> Result<f64, Error> {
    use crate::event_clipboard::{Event, TimingKind, WidgetKind};
    let inventory = crate::region_selection::inventory::Inventory::capture(session, chart)?;
    let last = inventory.entries().iter().filter(|entry| !matches!(entry.event, Event::SongWidget(widget) if widget.kind()==WidgetKind::Preview))
        .max_by(|a,b| a.event.chart_position().total_cmp(&b.event.chart_position()).then(a.phase.cmp(&b.phase)));
    let Some(last) = last else {
        return end_with_selected_rows(None, selected_rows);
    };
    let row = last.event.source_row() as f64;
    let length = match last.event {
        Event::Note(position) => session
            .grid(chart)?
            .get(position)
            .and_then(|note| note.end)
            .map(|end| end as f64 - row)
            .unwrap_or(0.0),
        Event::Pattern { row } => {
            crate::generation::metadata::ChartData::read(session.document(), chart)?
                .and_then(|data| data.patterns)
                .and_then(|patterns| patterns.get(&row).map(|pattern| pattern.length as f64))
                .unwrap_or(0.0)
        }
        Event::Timing {
            kind: TimingKind::Warp | TimingKind::Fake,
            ..
        } => {
            inventory
                .display_value(last.event)
                .ok_or_else(|| Error::InvalidEdit("Region end is missing".into()))?
                .parse::<f64>()
                .map_err(|_| Error::InvalidEdit("Region end is invalid".into()))?
                * 48.0
        }
        _ => 0.0,
    };
    end_with_selected_rows(
        Some((last.event.chart_position(), row + length)),
        selected_rows,
    )
}
fn end_with_selected_rows(
    last: Option<(f64, f64)>,
    selected: Option<(i32, i32)>,
) -> Result<f64, Error> {
    if let Some((start, end)) = selected {
        if start > end {
            return Err(Error::InvalidEdit(
                "Invalid selected row widget bounds".into(),
            ));
        }
        if last.is_none_or(|(position, _)| f64::from(start) > position) {
            return Ok(f64::from(end));
        }
    }
    Ok(last.map_or(0., |(_, end)| end))
}

#[cfg(test)]
mod selected_rows_source_tests {
    use super::*;
    #[test]
    fn actual_source_last_event_custom_order_and_preview_exclusion_180_cases() {
        let rows: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/editor-end-selected-rows/source.expected.json"
        ))
        .unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 180);
        for row in rows.as_array().unwrap() {
            let expected = row["result"].as_f64().unwrap();
            let actual = end_with_selected_rows(
                Some((
                    row["ordinaryStart"].as_f64().unwrap(),
                    row["ordinaryEnd"].as_f64().unwrap(),
                )),
                Some((
                    row["start"].as_i64().unwrap() as i32,
                    row["end"].as_i64().unwrap() as i32,
                )),
            )
            .unwrap();
            assert_eq!(actual.to_bits(), expected.to_bits(), "{row}");
        }
    }
}
