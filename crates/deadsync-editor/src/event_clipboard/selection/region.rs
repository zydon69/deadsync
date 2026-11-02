use super::{Event, Selection};
use crate::{Error, history::Session, selection_preferences::RegionMode};
#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
}
impl Selection {
    /// Apply collected spatial candidates in their original traversal order.
    /// Every identity is validated before membership or the anchor changes.
    pub fn finish_region(
        &mut self,
        session: &Session,
        candidates: &[Event],
        modifiers: Modifiers,
        region_mode: RegionMode,
    ) -> Result<(), Error> {
        self.validate(session)?;
        // Large region confirmations must not scan the complete inventory once
        // per candidate. Keep traversal order in `events`, and index membership
        // only for this confirmation.
        let inventory: std::collections::BTreeSet<_> = self.events.iter().copied().collect();
        if candidates.iter().any(|e| !inventory.contains(e)) {
            return Err(Error::InvalidEdit(
                "Region candidate is not part of the current chart inventory".into(),
            ));
        }
        let selectable = |e: Event| match e {
            Event::SongWidget(_) => false,
            Event::Note(_) => !modifiers.alt,
            _ => modifiers.alt,
        };
        let lane = |e: Event| match e {
            Event::Note(p) => i32::from(p.lane),
            _ => -1,
        };
        if modifiers.shift {
            if let (Some(anchor), Some(&first), Some(&last)) =
                (self.last_selected, candidates.first(), candidates.last())
            {
                let a = self
                    .events
                    .iter()
                    .position(|e| *e == anchor)
                    .ok_or_else(|| {
                        Error::InvalidEdit("Region anchor is no longer in this chart".into())
                    })?;
                let b = self
                    .events
                    .iter()
                    .position(|e| *e == first)
                    .expect("candidate validated");
                let min_lane = lane(first).min(lane(anchor)).min(lane(last));
                let max_lane = lane(first).max(lane(anchor)).max(lane(last));
                let interval = self.events[a.min(b)..=a.max(b)].to_vec();
                let end = if a < b { first } else { anchor };
                for event in interval {
                    if selectable(event)
                        && (region_mode != RegionMode::TimeOrPositionAndLane
                            || (lane(event) >= min_lane && lane(event) <= max_lane))
                    {
                        self.choose_region(event, true, event == end);
                    }
                }
            }
            for (i, &event) in candidates.iter().enumerate() {
                self.choose_region(event, true, i + 1 == candidates.len());
            }
        } else if modifiers.control {
            for (i, &event) in candidates.iter().enumerate() {
                self.choose_region(
                    event,
                    !self.selected.contains(&event),
                    i + 1 == candidates.len(),
                );
            }
        } else {
            self.select_all(false);
            for (i, &event) in candidates.iter().enumerate() {
                self.choose_region(event, true, i + 1 == candidates.len());
            }
        }
        Ok(())
    }
    fn choose_region(&mut self, event: Event, selected: bool, set_anchor: bool) {
        if selected {
            if set_anchor {
                self.last_selected = Some(event);
            }
            if self.selected.insert(event) {
                self.order.add(event);
            }
        } else {
            self.choose(event, false);
        }
    }
}
