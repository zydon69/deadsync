//! Executable native subset, separate from the source callback inventory.
#[path = "key_binding_preferences/history.rs"]
pub(crate) mod history;
pub use history::Update;
#[path = "key_binding_preferences/inspector.rs"]
pub mod inspector;
#[path = "key_binding_preferences/reservations.rs"]
pub mod reservations;
use crate::key_commands::{Binding, Key};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Save,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Delete,
    PreviousMeasure,
    NextMeasure,
    SelectRowRange,
    TogglePlayer,
    PlayPause,
    ChartStart,
}
impl Action {
    pub const ALL: [Self; 13] = [
        Self::Save,
        Self::Undo,
        Self::Redo,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Delete,
        Self::PreviousMeasure,
        Self::NextMeasure,
        Self::SelectRowRange,
        Self::TogglePlayer,
        Self::PlayPause,
        Self::ChartStart,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Save => "Save",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::Delete => "Delete",
            Self::PreviousMeasure => "MoveToPreviousMeasure",
            Self::NextMeasure => "MoveToNextMeasure",
            Self::SelectRowRange => "SelectRowRange",
            Self::TogglePlayer => "TogglePlayer",
            Self::PlayPause => "PlayPause",
            Self::ChartStart => "MoveToChartStart",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
    pub fn repeats(self) -> bool {
        matches!(self, Self::PreviousMeasure | Self::NextMeasure)
    }
    fn native(self) -> Vec<Vec<&'static str>> {
        match self {
            Self::Save => vec![vec!["Ctrl", "S"]],
            Self::Undo => vec![vec!["Ctrl", "Z"]],
            Self::Redo => vec![vec!["Ctrl", "Y"], vec!["Ctrl", "LeftShift", "Z"]],
            Self::Copy => vec![vec!["Ctrl", "C"]],
            Self::Cut => vec![vec!["Ctrl", "X"]],
            Self::Paste => vec![vec!["Ctrl", "V"]],
            Self::Delete => vec![vec!["Delete"], vec!["Back"]],
            Self::PreviousMeasure => vec![vec!["PageUp"]],
            Self::NextMeasure => vec![vec!["PageDown"]],
            Self::SelectRowRange => vec![vec!["Space"]],
            Self::TogglePlayer => vec![vec!["P"]],
            Self::PlayPause => vec![vec!["Enter"], vec!["NumPadEnter"]],
            Self::ChartStart => vec![vec!["Home"]],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub commands: BTreeMap<String, Vec<Vec<String>>>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            commands: Action::ALL
                .into_iter()
                .map(|action| {
                    (
                        action.id().into(),
                        action
                            .native()
                            .into_iter()
                            .map(|chord| chord.into_iter().map(str::to_owned).collect())
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}
impl Preferences {
    /// Ctrl represents the native existing accelerator (Control or Super),
    /// deliberately broader than GrooveAuthor's platform-specific default.
    pub fn bindings(&self) -> Vec<Binding> {
        Action::ALL
            .into_iter()
            .map(|action| Binding {
                id: action.id().into(),
                alternatives: self
                    .commands
                    .get(action.id())
                    .into_iter()
                    .flatten()
                    .map(|keys| keys.iter().map(|key| Key::named(key)).collect())
                    .collect(),
                repeat: action.repeats(),
                blocks: true,
            })
            .collect()
    }
}

impl Preferences {
    pub fn read(fields: &serde_json::Map<String, serde_json::Value>) -> Result<Self, crate::Error> {
        let mut prefs = Self::default();
        for action in Action::ALL {
            let Some(value) = fields.get(action.id()) else {
                continue;
            };
            let alternatives: Vec<Vec<String>> =
                serde_json::from_value(value.clone()).map_err(|_| {
                    crate::Error::InvalidFormat(format!("Invalid key binding {}", action.id()))
                })?;
            prefs.commands.insert(action.id().into(), alternatives);
        }
        prefs.validate()?;
        Ok(prefs)
    }
    pub fn write(
        &self,
        fields: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), crate::Error> {
        self.validate()?;
        for action in Action::ALL {
            fields.insert(
                action.id().into(),
                serde_json::to_value(&self.commands[action.id()]).expect("String arrays serialize"),
            );
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), crate::Error> {
        if self.commands.keys().any(|id| Action::from_id(id).is_none()) {
            return Err(crate::Error::InvalidEdit(
                "Unknown executable key binding action".into(),
            ));
        }
        for action in Action::ALL {
            let alternatives = self.commands.get(action.id()).ok_or_else(|| {
                crate::Error::InvalidEdit(format!("Missing key binding {}", action.id()))
            })?;
            if alternatives.is_empty() || alternatives.len() > 16 {
                return Err(crate::Error::InvalidEdit(format!(
                    "Invalid alternatives for {}",
                    action.id()
                )));
            }
            for chord in alternatives {
                if chord.len() > 8 {
                    return Err(crate::Error::InvalidEdit(format!(
                        "Key chord too long for {}",
                        action.id()
                    )));
                }
                let mut seen = std::collections::BTreeSet::new();
                for key in chord {
                    if !supported_key(key) || !seen.insert(Key::named(key)) {
                        return Err(crate::Error::InvalidEdit(format!(
                            "Unsupported or duplicate key {key} for {}",
                            action.id()
                        )));
                    }
                }
                if !reservations::simple_chord(chord) || reservations::conflicts(chord) {
                    return Err(crate::Error::InvalidEdit(format!(
                        "Key chord is reserved by an unmigrated native action or needs unsupported multi-key dispatch for {}",
                        action.id()
                    )));
                }
            }
        }
        Ok(())
    }
}
/// Explicit portable key names accepted by the first executable subset.
/// Unsupported source names remain untouched in unknown command fields.
pub fn supported_key(key: &str) -> bool {
    matches!(
        key,
        "Ctrl"
            | "LeftShift"
            | "RightShift"
            | "LeftAlt"
            | "RightAlt"
            | "Space"
            | "Enter"
            | "NumPadEnter"
            | "Escape"
            | "Tab"
            | "Back"
            | "Delete"
            | "Insert"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "Up"
            | "Down"
            | "Left"
            | "Right"
    ) || (key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase())
        || key
            .strip_prefix('D')
            .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
        || key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n))
}
