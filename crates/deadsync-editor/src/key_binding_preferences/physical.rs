//! Native accelerator lowering. The generic source registry stays unchanged.
use super::Preferences;
use crate::key_commands::{Effect, Key, Registry};
use std::collections::BTreeSet;
/// Persisted native Ctrl means either Control family or Super/Command family.
/// Source LeftControl and LeftWindows keep their physical distinction.
pub fn expand(keys: &[String]) -> Vec<Vec<Key>> {
    if keys.iter().any(|key| key == "Ctrl") {
        ["LeftControl", "LeftWindows"]
            .into_iter()
            .map(|physical| {
                keys.iter()
                    .map(|key| Key::named(if key == "Ctrl" { physical } else { key }))
                    .collect()
            })
            .collect()
    } else {
        vec![keys.iter().map(|key| Key::named(key)).collect()]
    }
}
pub fn conflicts(preferences: &Preferences, id: &str, keys: &[String]) -> Vec<String> {
    let registry = Registry::new(preferences.bindings());
    let mut result = BTreeSet::new();
    for expanded in expand(keys) {
        result.extend(registry.conflicts(id, &expanded));
    }
    result.into_iter().collect()
}
/// Coalesce ONLY at the native adaptation boundary. Source Registry still
/// faithfully represents every alternative, including duplicate IDs.
#[derive(Default)]
pub struct Effects {
    active: BTreeSet<String>,
}
impl Effects {
    pub fn is_active(&self, id: &str) -> bool {
        self.active.contains(id)
    }
    pub fn coalesce(&mut self, registry: &Registry, raw: Vec<Effect>) -> Vec<Effect> {
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        let mut repeats = BTreeSet::new();
        for effect in &raw {
            let id = match effect {
                Effect::Press(id) | Effect::Repeat(id) | Effect::Release(id) => id,
            };
            if matches!(effect, Effect::Repeat(_)) {
                repeats.insert(id.clone());
            }
            if seen.insert(id.clone()) {
                order.push(id.clone());
            }
        }
        // Defensive cleanup if a registry implementation changes without
        // emitting a Release; a logical held command may never stick.
        for id in &self.active {
            if !registry.is_active(id) && seen.insert(id.clone()) {
                order.push(id.clone());
            }
        }
        let mut result = Vec::new();
        for id in order {
            let before = self.active.contains(&id);
            let after = registry.is_active(&id);
            match (before, after) {
                (false, true) => {
                    self.active.insert(id.clone());
                    result.push(Effect::Press(id));
                }
                (true, false) => {
                    self.active.remove(&id);
                    result.push(Effect::Release(id));
                }
                (true, true) if repeats.contains(&id) => result.push(Effect::Repeat(id)),
                _ => {}
            }
        }
        result
    }
}
