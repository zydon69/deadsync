//! Unmigrated native shortcuts. Conditions mirror State's chart keyboard routes,
//! chart_views_key, zoom_key and minimap_key, independent of translated labels.
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
    // Marker navigation remains modal as well as global; minimap commands
    // share their focus context. Their migration needs a contextual registry.
    for name in ["P", "M"] {
        add(&["Ctrl", name], false, false);
    }
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
