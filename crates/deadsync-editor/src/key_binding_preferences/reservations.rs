//! Physical lane entry is now semantic; no legacy digit reservations remain.
use std::collections::BTreeSet;
#[derive(Clone, Debug)]
pub struct Reservation {
    pub keys: Vec<String>,
    forbid_control: bool,
    forbid_shift: bool,
}
pub fn legacy() -> Vec<Reservation> {
    Vec::new()
}
/// Source accepts arbitrary ordered key sets, including ordinary prefixes
/// and modifier finals. Preserve every key; normalize only physical aliases.
pub const MAX_CHORD_KEYS: usize = 128;
pub fn supported_chord(keys: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    keys.len() <= MAX_CHORD_KEYS
        && keys.iter().all(|key| {
            super::supported_key(key) && seen.insert(crate::key_commands::Key::named(key))
        })
}
/// A shorter subset can coexist: a longer reserved chord wins in the evaluator.
/// Equal, longer or non-subset chords with the same terminal key are ambiguous.
pub fn conflicts(keys: &[String]) -> bool {
    let Some(final_key) = keys.last() else {
        return false;
    };
    for candidate in super::physical::expand(keys) {
        let candidate: BTreeSet<String> =
            candidate.iter().map(|key| key.name().to_owned()).collect();
        let control = candidate.contains("LeftControl") || candidate.contains("LeftWindows");
        for reserved in legacy() {
            if reserved.keys.last() != Some(final_key)
                || reserved.forbid_control && control
                || reserved.forbid_shift && candidate.contains("LeftShift")
            {
                continue;
            }
            for required in super::physical::expand(&reserved.keys) {
                let required: BTreeSet<String> =
                    required.iter().map(|key| key.name().to_owned()).collect();
                // A reservation requiring physical Control cannot collide with
                // a Super-only chord, or conversely, unless both are required.
                let required_control = required.contains("LeftControl");
                let required_super = required.contains("LeftWindows");
                if required_control
                    && !candidate.contains("LeftControl")
                    && candidate.contains("LeftWindows")
                    || required_super
                        && !candidate.contains("LeftWindows")
                        && candidate.contains("LeftControl")
                {
                    continue;
                }
                if candidate.len() < required.len() && candidate.is_subset(&required) {
                    continue;
                }
                return true;
            }
        }
    }
    false
}
pub fn blocking_chords() -> Vec<Vec<String>> {
    legacy()
        .into_iter()
        .map(|reservation| reservation.keys)
        .collect()
}
