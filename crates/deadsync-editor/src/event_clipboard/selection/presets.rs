//! UIEditEvents Select All predicates. Source operations add to the selection;
//! Replace and Remove are explicit native conveniences, never default behavior.
use super::{Event, Kind, Selection};
use crate::{Error, history::Session};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Notes,
    Taps,
    Mines,
    Fakes,
    Lifts,
    Holds,
    Rolls,
    HoldsAndRolls,
    Miscellaneous,
    NotesAndMiscellaneous,
    Patterns,
}
impl Preset {
    pub const ALL: [Self; 11] = [
        Self::Notes,
        Self::Taps,
        Self::Mines,
        Self::Fakes,
        Self::Lifts,
        Self::Holds,
        Self::Rolls,
        Self::HoldsAndRolls,
        Self::Miscellaneous,
        Self::NotesAndMiscellaneous,
        Self::Patterns,
    ];
    pub fn supports_player(self) -> bool {
        matches!(
            self,
            Self::Notes
                | Self::Taps
                | Self::Mines
                | Self::Fakes
                | Self::Lifts
                | Self::Holds
                | Self::Rolls
                | Self::HoldsAndRolls
        )
    }
    pub fn matches(self, event: Event, note_kind: Option<Kind>, player: Option<u32>) -> bool {
        let owner_matches = match event {
            Event::Note(position) => player.is_none_or(|p| p == position.player),
            _ => player.is_none(),
        };
        if !owner_matches {
            return false;
        }
        match self {
            Self::Notes => matches!(event, Event::Note(_)),
            Self::Taps => note_kind == Some(Kind::Tap),
            Self::Mines => note_kind == Some(Kind::Mine),
            Self::Fakes => note_kind == Some(Kind::Fake),
            Self::Lifts => note_kind == Some(Kind::Lift),
            Self::Holds => note_kind == Some(Kind::Hold),
            Self::Rolls => note_kind == Some(Kind::Roll),
            Self::HoldsAndRolls => matches!(note_kind, Some(Kind::Hold | Kind::Roll)),
            Self::Miscellaneous => !matches!(event, Event::Note(_)),
            Self::NotesAndMiscellaneous => true,
            Self::Patterns => matches!(event, Event::Pattern { .. }),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Add,
    Replace,
    Remove,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub matched: usize,
    pub selected: usize,
    pub last_match: Option<Event>,
}
impl Selection {
    pub fn apply_preset(
        &mut self,
        preset: Preset,
        player: Option<u32>,
        mode: Mode,
    ) -> Result<Report, Error> {
        if player.is_some() && !preset.supports_player() {
            return Err(Error::InvalidEdit(
                "Player filters apply only to note selection presets".into(),
            ));
        }
        let matches: Vec<_> = self
            .events
            .iter()
            .copied()
            .filter(|event| {
                preset.matches(
                    *event,
                    match event {
                        Event::Note(p) => self.note_kinds.get(p).copied(),
                        _ => None,
                    },
                    player,
                )
            })
            .collect();
        let mut selected = if mode == Mode::Replace {
            BTreeSet::new()
        } else {
            self.selected.clone()
        };
        for event in &matches {
            if mode == Mode::Remove {
                selected.remove(event);
            } else {
                selected.insert(*event);
            }
        }
        self.selected = selected;
        Ok(Report {
            matched: matches.len(),
            selected: self.selected.len(),
            last_match: matches.last().copied(),
        })
    }
    /// Expand the available choices without selecting additional events. Exact
    /// prior identities survive; the source revision is validated before capture.
    pub fn expand_to_chart(&self, session: &mut Session) -> Result<Self, Error> {
        self.validate(session)?;
        let mut expanded = Self::capture(session, self.chart, 0, i32::MAX as u64)?;
        expanded
            .selected
            .retain(|event| self.selected.contains(event));
        Ok(expanded)
    }
}
