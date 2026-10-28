//! Unmigrated native shortcuts. Conditions mirror State's chart keyboard routes,
//! chart_views_key, zoom_key and minimap_key, independent of translated labels.
use crate::key_commands::Key;
use std::collections::BTreeSet;
#[derive(Clone, Debug)]
pub struct Reservation {
    pub keys: Vec<String>,
    forbid_control: bool,
    forbid_shift: bool,
}
fn modifier(key: &str) -> bool {
    matches!(
        key,
        "Ctrl"
            | "LeftControl"
            | "RightControl"
            | "LeftWindows"
            | "RightWindows"
            | "LeftShift"
            | "RightShift"
            | "LeftAlt"
            | "RightAlt"
    )
}
fn canonical(key: &str) -> String {
    match Key::named(key).name() {
        "LeftControl" | "LeftWindows" => "Ctrl".into(),
        other => other.into(),
    }
}
fn set(keys: &[String]) -> BTreeSet<String> {
    keys.iter().map(|key| canonical(key)).collect()
}
pub fn legacy() -> Vec<Reservation> {
    let mut result = Vec::new();
    let mut add = |keys: &[&str], forbid_control, forbid_shift| {
        result.push(Reservation {
            keys: keys.iter().map(|s| (*s).into()).collect(),
            forbid_control,
            forbid_shift,
        })
    };
    for name in [
        "Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    ] {
        add(&[name], false, false);
    }
    for name in ["Tab", "End", "A", "Up", "Down"] {
        add(&[name], true, false);
    }
    for name in ["Left", "Right"] {
        add(&[name], true, true);
        add(&["LeftShift", name], true, false);
        add(&["Ctrl", "LeftShift", name], false, false);
    }
    for name in [
        "Tab",
        "P",
        "W",
        "B",
        "T",
        "M",
        "J",
        "N",
        "R",
        "D",
        "L",
        "Up",
        "Down",
        "D0",
        "PageUp",
        "PageDown",
        "Enter",
        "NumPadEnter",
    ] {
        add(&["Ctrl", name], false, false);
    }
    for name in ["Y", "E", "U", "H", "K", "S", "C", "X", "V", "Back"] {
        add(&["Ctrl", "LeftShift", name], false, false);
    }
    for name in ["S", "Z", "V"] {
        add(&["LeftShift", name], true, false);
    }
    add(&["W"], true, false);
    drop(add);
    for digit in 0..=9 {
        result.push(Reservation {
            keys: vec![format!("D{digit}")],
            forbid_control: true,
            forbid_shift: false,
        });
    }
    result
}
/// A shorter subset can coexist: a longer reserved chord wins in the evaluator.
/// Equal, longer or non-subset chords with the same terminal key are ambiguous.
pub fn conflicts(keys: &[String]) -> bool {
    let Some(final_key) = keys.last() else {
        return false;
    };
    let candidate = set(keys);
    for reserved in legacy() {
        if reserved.keys.last() != Some(final_key) {
            continue;
        }
        if (reserved.forbid_control && candidate.contains("Ctrl"))
            || (reserved.forbid_shift && candidate.contains("LeftShift"))
        {
            continue;
        }
        let required = set(&reserved.keys);
        if candidate.len() < required.len() && candidate.is_subset(&required) {
            continue;
        }
        return true;
    }
    false
}
pub fn simple_chord(keys: &[String]) -> bool {
    keys.is_empty()
        || (!modifier(keys.last().unwrap())
            && keys[..keys.len() - 1].iter().all(|key| modifier(key)))
}
pub fn blocking_chords() -> Vec<Vec<String>> {
    legacy()
        .into_iter()
        .map(|reservation| reservation.keys)
        .collect()
}
