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
    for name in ["Escape", "F2"] {
        add(&[name], false, false);
    }
    for name in ["Tab", "A", "Up", "Down"] {
        add(&[name], true, false);
    }
    for name in ["Left", "Right"] {
        add(&["Ctrl", "LeftShift", name], false, false);
    }
    for name in [
        "P",
        "W",
        "T",
        "M",
        "J",
        "Up",
        "Down",
        "D0",
        "Enter",
        "NumPadEnter",
    ] {
        add(&["Ctrl", name], false, false);
    }
    for name in ["Y", "E", "C", "X", "V"] {
        add(&["Ctrl", "LeftShift", name], false, false);
    }
    for name in ["S", "Z", "V"] {
        add(&["LeftShift", name], true, false);
    }
    add(&["Ctrl", "LeftShift", "W"], false, false);
    add(&["Ctrl", "LeftShift", "B"], false, false);
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
        // Waveform settings (Shift+W) is now executable. Ctrl+W still owns
        // closing tabs; its explicit Ctrl+Shift+W blocker suppresses both
        // migrated waveform commands before that legacy callback runs.
        if final_key == "W"
            && candidate.len() == 2
            && !candidate.contains("Ctrl")
            && candidate.contains("LeftShift")
            && required.contains("Ctrl")
        {
            continue;
        }
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
