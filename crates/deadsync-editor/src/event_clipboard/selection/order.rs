//! Selected-event enumeration matches the pinned source HashSet slot order.
//! Membership is separate: insertion order survives selection expansion and
//! released slots are reused in LIFO order, as witnessed by the source oracle.
use super::Event;
use std::collections::BTreeMap;
#[derive(Clone, Default)]
pub(super) struct Order {
    slots: Vec<Option<Event>>,
    free: Vec<usize>,
    index: BTreeMap<Event, usize>,
}
impl Order {
    pub fn add(&mut self, event: Event) {
        if self.index.contains_key(&event) {
            return;
        }
        let slot = if let Some(slot) = self.free.pop() {
            self.slots[slot] = Some(event);
            slot
        } else {
            let slot = self.slots.len();
            self.slots.push(Some(event));
            slot
        };
        self.index.insert(event, slot);
    }
    pub fn remove(&mut self, event: Event) {
        if let Some(slot) = self.index.remove(&event) {
            self.slots[slot] = None;
            self.free.push(slot);
        }
    }
    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.index.clear();
    }
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.slots.iter().flatten().copied()
    }
}
