//! GrooveAuthor AutoPlayer.Update state machine (MIT, 48e18f71655d0f0d86144c63239331833501fda2).
//! Candidate timing/fake flags reuse the editor dependency pass. No document,
//! audio transport or lane gesture is changed by autoplay.
use crate::{
    Error,
    chart_order::Binding,
    generation::{editor_timing::EditorTimeline, timing::GenerationTiming},
    history::Session,
    notes::{Grid, Kind, Position},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InputKind {
    Tap,
    HoldStart,
    HoldEnd,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Input {
    pub kind: InputKind,
    pub time: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActionKind {
    Down,
    Up,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Action {
    pub lane: u16,
    pub kind: ActionKind,
    pub time_delta: f64,
}
#[derive(Clone, Copy, Debug)]
struct Lane {
    unset: bool,
    next: Option<Input>,
    held: bool,
}
impl Default for Lane {
    fn default() -> Self {
        Self {
            unset: true,
            next: None,
            held: false,
        }
    }
}
#[derive(Clone, Debug)]
pub struct State {
    lanes: Vec<Lane>,
}
impl State {
    pub fn new(columns: u16) -> Result<Self, Error> {
        if columns == 0 {
            return Err(Error::InvalidEdit("Autoplay requires lanes".into()));
        }
        Ok(Self {
            lanes: vec![Lane::default(); usize::from(columns)],
        })
    }
    pub fn held(&self, lane: u16) -> Option<bool> {
        self.lanes.get(usize::from(lane)).map(|lane| lane.held)
    }
    /// Exact source transition boundary. Seek, stop and session/content changes
    /// must call reset before supplying a new sequence of next-input snapshots.
    pub fn update(
        &mut self,
        chart_time: f64,
        next: &[Option<Input>],
    ) -> Result<Vec<Action>, Error> {
        if !chart_time.is_finite()
            || next.len() != self.lanes.len()
            || next.iter().flatten().any(|input| !input.time.is_finite())
        {
            return Err(Error::InvalidEdit("Invalid autoplay snapshot".into()));
        }
        let mut actions = Vec::new();
        for (index, (state, &input)) in self.lanes.iter_mut().zip(next).enumerate() {
            let lane = index as u16;
            let old_time = state.next.map_or(0.0, |input| input.time);
            let new_time = input.map_or(0.0, |input| input.time);
            if !crate::number::reference_equal(old_time, new_time) {
                let delta = if state.unset {
                    0.0
                } else {
                    chart_time - old_time
                };
                if input.is_none() && state.held {
                    emit(state, &mut actions, lane, ActionKind::Up, delta);
                }
                if let Some(old) = state.next {
                    match old.kind {
                        InputKind::HoldEnd => {
                            if !state.held {
                                emit(state, &mut actions, lane, ActionKind::Down, delta);
                            }
                            emit(state, &mut actions, lane, ActionKind::Up, delta);
                            if input.is_some_and(|input| input.kind == InputKind::HoldEnd) {
                                emit(state, &mut actions, lane, ActionKind::Down, delta);
                            }
                        }
                        InputKind::HoldStart | InputKind::Tap => {
                            emit(state, &mut actions, lane, ActionKind::Down, delta);
                            if !input.is_some_and(|input| input.kind == InputKind::HoldEnd) {
                                emit(state, &mut actions, lane, ActionKind::Up, delta);
                            }
                        }
                    }
                } else if state.unset && input.is_some_and(|input| input.kind == InputKind::HoldEnd)
                {
                    emit(state, &mut actions, lane, ActionKind::Down, delta);
                }
            }
            state.unset = false;
            state.next = input;
        }
        Ok(actions)
    }
    /// Source Stop cancels every receptor, even a lane which was not held.
    pub fn reset(&mut self) -> Vec<Action> {
        self.lanes
            .iter_mut()
            .enumerate()
            .map(|(lane, state)| {
                *state = Lane::default();
                Action {
                    lane: lane as u16,
                    kind: ActionKind::Cancel,
                    time_delta: 0.0,
                }
            })
            .collect()
    }
}
fn emit(state: &mut Lane, out: &mut Vec<Action>, lane: u16, kind: ActionKind, delta: f64) {
    state.held = kind == ActionKind::Down;
    out.push(Action {
        lane,
        kind,
        time_delta: delta,
    });
}
#[derive(Clone, Copy, Debug)]
struct Head {
    position: Position,
    kind: Kind,
    time: f64,
    end_time: Option<f64>,
    fake: bool,
}
/// Immutable content-bound candidate index. Physical lanes are shared by all
/// participants; head Position retains the owner for presentation policies.
struct GlobalEvent {
    time: f64,
    row: f64,
    custom_order: u8,
    head: Option<Head>,
}
pub struct Timeline {
    binding: Binding,
    lanes: Vec<Vec<Head>>,
    actionable: Vec<Vec<Head>>,
    heads: std::collections::BTreeMap<Position, Head>,
    global: Vec<GlobalEvent>,
}
impl Timeline {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        Binding::capture(session, chart)?;
        let parsed;
        let grid = if let Some(grid) = session.loaded_grid(chart) {
            grid
        } else {
            let descriptor = &session.document().charts()[chart];
            parsed = Grid::parse_layout(
                session.document().value(descriptor.notes.clone()),
                &descriptor.steps_type,
            )?;
            &parsed
        };
        Self::capture_preview_grid(session, chart, grid)
    }
    /// Classify an immutable editor preview with the source chart's real timing.
    /// This does not install notes or grant a generated chart transport authority.
    pub fn capture_preview_grid(
        session: &Session,
        chart: usize,
        grid: &Grid,
    ) -> Result<Self, Error> {
        let binding = Binding::capture(session, chart)?;
        let timing = GenerationTiming::from_document(session.document(), chart)?;
        let attacks = crate::attacks::placement::Timeline::capture(session, chart)?;
        let attack_rows: Vec<_> = attacks.markers().iter().map(|marker| marker.row).collect();
        let patterns = crate::generation::metadata::ChartData::read(session.document(), chart)?
            .and_then(|data| data.patterns)
            .unwrap_or_default();
        let regions: Vec<_> = patterns
            .into_iter()
            .map(|(row, pattern)| {
                row.checked_add(pattern.length)
                    .map(|end| (row, end))
                    .ok_or_else(|| {
                        Error::InvalidEdit("Pattern end overflows autoplay timeline".into())
                    })
            })
            .collect::<Result<_, _>>()?;
        let editor = EditorTimeline::build_clock(&timing, grid, &attack_rows)?
            .with_markers(&regions, &std::sync::atomic::AtomicBool::new(false))?;
        let mut lanes = vec![Vec::new(); usize::from(grid.columns())];
        for head in editor.heads() {
            let note = grid
                .get(head.position)
                .ok_or_else(|| Error::InvalidEdit("Timed autoplay head is absent".into()))?;
            lanes[usize::from(head.position.lane)].push(Head {
                position: head.position,
                kind: head.kind,
                time: head.time,
                end_time: note.end.map(|row| editor.note_time(row)).transpose()?,
                fake: head.fake,
            });
        }
        // The editor head stream retains original row/lane/type ordering for
        // equal times (including warp edges). Stable time sorting preserves it.
        for lane in &mut lanes {
            lane.sort_by(|a, b| a.time.total_cmp(&b.time));
        }
        let actionable = lanes
            .iter()
            .map(|lane| {
                lane.iter()
                    .copied()
                    .filter(|head| {
                        !head.fake
                            && matches!(head.kind, Kind::Tap | Kind::Lift | Kind::Hold | Kind::Roll)
                    })
                    .collect()
            })
            .collect();
        let heads: std::collections::BTreeMap<Position, Head> = lanes
            .iter()
            .flatten()
            .map(|head| (head.position, *head))
            .collect();
        let mut global: Vec<_> = editor
            .timed_events()
            .map(|event| GlobalEvent {
                time: event.time,
                row: event.row as f64,
                // Preview custom-order 1, then null-backed End Hint at 2,
                // then real Events (same source order 2, null sorts first).
                custom_order: 3,
                head: event
                    .head
                    .and_then(|position| heads.get(&position).copied()),
            })
            .collect();
        let song = crate::song_properties::Draft::capture(session)?;
        let clock = crate::playback::Clock::capture(session, chart)?;
        // These editor-only widgets live in the original global tree too.
        // They can displace a strict predecessor even though autoplay ignores them.
        for (seconds, order) in [
            (
                song.sample_start(),
                if song.text(crate::song_text::Field::PreviewMusic).is_empty() {
                    Some(1)
                } else {
                    None
                },
            ),
            (
                song.last_second_hint(),
                if song.supports_last_second_hint() && song.last_second_hint() > 0.0 {
                    Some(2)
                } else {
                    None
                },
            ),
        ] {
            if let Some(order) = order {
                global.push(GlobalEvent {
                    time: clock.music_to_chart_time(seconds)?,
                    row: clock.position(seconds)?,
                    custom_order: order,
                    head: None,
                });
            }
        }
        global.sort_by(|a, b| {
            a.time
                .total_cmp(&b.time)
                .then_with(|| a.row.total_cmp(&b.row))
                .then(a.custom_order.cmp(&b.custom_order))
        });
        Ok(Self {
            binding,
            lanes,
            actionable,
            heads,
            global,
        })
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        self.binding.is_current(session, chart)
    }
    /// Complete initialized editor-event rows, including timing, attacks,
    /// patterns and fractional preview/end-hint widgets. Hold tails belong to
    /// their head event and are deliberately not separate entries. Fake steps
    /// remain steps here: autoplay eligibility is a different policy.
    pub(crate) fn navigation_events(&self) -> impl Iterator<Item = crate::navigation::Event> + '_ {
        self.global.iter().map(|event| crate::navigation::Event {
            position: event.row,
            step: event.head.is_some_and(|head| {
                matches!(
                    head.kind,
                    Kind::Tap | Kind::Hold | Kind::Roll | Kind::Lift | Kind::Fake
                )
            }),
        })
    }
    pub fn columns(&self) -> u16 {
        self.lanes.len() as u16
    }
    pub fn next_inputs(
        &self,
        session: &Session,
        chart: usize,
        chart_time: f64,
    ) -> Result<Vec<Option<Input>>, Error> {
        if !self.is_current(session, chart) || !chart_time.is_finite() {
            return Err(Error::InvalidEdit("Stale or invalid autoplay time".into()));
        }
        let previous = self
            .global
            .partition_point(|event| event.time < chart_time)
            .checked_sub(1)
            .and_then(|index| self.global[index].head)
            .filter(|head| {
                !head.fake
                    && matches!(head.kind, Kind::Hold | Kind::Roll)
                    && head.end_time.is_some_and(|end| end >= chart_time)
            });
        Ok(self
            .lanes
            .iter()
            .zip(&self.actionable)
            .map(|(lane, actionable)| {
                // GetHoldsOverlappingTime examines the latest lane event first;
                // later taps/mines/fake heads prevent resurrecting an older hold.
                let before = lane.partition_point(|head| head.time <= chart_time);
                if let Some(head) = before
                    .checked_sub(1)
                    .and_then(|index| lane.get(index))
                    .filter(|head| !head.fake && head.end_time.is_some_and(|end| end >= chart_time))
                {
                    return Some(Input {
                        kind: InputKind::HoldEnd,
                        time: head.end_time.unwrap(),
                    });
                }
                if let Some(head) = previous.filter(|head| {
                    lane.first()
                        .is_some_and(|first| first.position.lane == head.position.lane)
                }) {
                    return Some(Input {
                        kind: InputKind::HoldEnd,
                        time: head.end_time.unwrap(),
                    });
                }
                let next = actionable.partition_point(|head| head.time < chart_time);
                actionable.get(next).map(|head| Input {
                    kind: if head.end_time.is_some() {
                        InputKind::HoldStart
                    } else {
                        InputKind::Tap
                    },
                    time: head.time,
                })
            })
            .collect())
    }
    /// The consumption policy concerns whole-note fake state, derived from
    /// FAKE regions/warps/negative stops as well as explicit fake notes.
    pub fn presentation(
        &self,
        session: &Session,
        chart: usize,
        position: Position,
        chart_time: f64,
        playing: bool,
        hide_arrows: bool,
        light_holds: bool,
    ) -> Result<Option<Presentation>, Error> {
        if !self.is_current(session, chart) || !chart_time.is_finite() {
            return Err(Error::InvalidEdit("Stale autoplay presentation".into()));
        }
        Ok(self.heads.get(&position).map(|head| {
            let consumed =
                !head.fake && matches!(head.kind, Kind::Tap | Kind::Lift | Kind::Hold | Kind::Roll);
            let raw_active_hold = playing
                && consumed
                && head.time < chart_time
                && head.end_time.is_some_and(|end| end > chart_time);
            Presentation {
                fake: head.fake,
                raw_active_hold,
                hidden: playing
                    && hide_arrows
                    && consumed
                    && head.end_time.unwrap_or(head.time) < chart_time,
                active_hold: playing
                    && light_holds
                    && consumed
                    && head.time < chart_time
                    && head.end_time.is_some_and(|end| end > chart_time),
                clip_hold: playing
                    && hide_arrows
                    && consumed
                    && head.time < chart_time
                    && head.end_time.is_some_and(|end| end > chart_time),
            }
        }))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presentation {
    /// Source dependency classification, also available while playback is stopped.
    pub fake: bool,
    pub hidden: bool,
    pub active_hold: bool,
    /// Raw active sustain state for source start fill; independent of LightHolds.
    pub raw_active_hold: bool,
    pub clip_hold: bool,
}

#[cfg(test)]
mod immutable_preview_classification_tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        history::Session,
        notes::{Grid, Kind, Note, Position},
    };
    #[test]
    fn preview_only_heads_lift_fake_and_hold_use_real_source_clock_without_consumption() {
        let source=b"#VERSION:0.83;#BPMS:0=120;#FAKES:2=1;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:9;#NOTES:1000;";
        let session = Session::new(Document::parse(source.as_slice(), Format::Ssc).unwrap());
        let before = session.serialize().unwrap();
        let mut grid = Grid::new(8, 1).unwrap();
        let p = |row, lane| Position {
            row,
            lane,
            player: 0,
        };
        grid.set(p(0, 6), Some(Note::tap())).unwrap();
        grid.set(
            p(48, 5),
            Some(Note {
                kind: Kind::Lift,
                ..Note::tap()
            }),
        )
        .unwrap();
        grid.set(
            p(72, 7),
            Some(Note {
                kind: Kind::Fake,
                ..Note::tap()
            }),
        )
        .unwrap();
        grid.set(p(96, 6), Some(Note::tap())).unwrap();
        grid.set(
            p(192, 4),
            Some(Note {
                kind: Kind::Hold,
                end: Some(288),
                ..Note::tap()
            }),
        )
        .unwrap();
        assert_eq!(grid.head_count(), 5);
        let timeline = Timeline::capture_preview_grid(&session, 0, &grid).unwrap();
        for position in [p(0, 6), p(48, 5), p(72, 7), p(96, 6), p(192, 4)] {
            let state = timeline
                .presentation(&session, 0, position, 2.25, false, true, true)
                .unwrap()
                .unwrap();
            assert_eq!(state.fake, position.row == 72 || position.row == 96);
            assert!(
                !state.hidden && !state.active_hold && !state.raw_active_hold && !state.clip_hold
            );
        }
        let playing = timeline
            .presentation(&session, 0, p(192, 4), 2.25, true, true, true)
            .unwrap()
            .unwrap();
        assert!(playing.raw_active_hold && playing.active_hold && playing.clip_hold);
        assert_eq!(session.serialize().unwrap(), before);
        assert!(session.undo_label().is_none());
    }
}
