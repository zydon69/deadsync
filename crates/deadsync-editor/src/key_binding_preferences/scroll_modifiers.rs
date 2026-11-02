//! Configurable held queries (wheel, selection and receptor movement). These are not executable key commands and do
//! not inherit callback chord reservations or the callback alternative budget.
use crate::Error;
use serde_json::Value;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Zoom,
    Spacing,
    SelectionMisc,
    SelectionToggle,
    SelectionExtend,
    ReceptorAxis,
}
impl Kind {
    pub const ALL: [Self; 6] = [
        Self::Zoom,
        Self::Spacing,
        Self::SelectionMisc,
        Self::SelectionToggle,
        Self::SelectionExtend,
        Self::ReceptorAxis,
    ];
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Zoom => "ScrollZoom",
            Self::Spacing => "ScrollSpacing",
            Self::SelectionMisc => "MouseSelectionAltBehavior",
            Self::SelectionToggle => "MouseSelectionControlBehavior",
            Self::SelectionExtend => "MouseSelectionShiftBehavior",
            Self::ReceptorAxis => "LockReceptorMoveAxis",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    alternatives: Vec<Vec<String>>,
}
impl Query {
    /// Input must already have passed original PreferencesKeyBinds.PostLoad.
    /// Keeping an empty chord is essential: original All(empty) is true.
    pub fn from_post_load(value: &Value) -> Result<Self, Error> {
        let alternatives = value
            .as_array()
            .ok_or_else(|| Error::InvalidFormat("Invalid scroll modifier alternatives".into()))?;
        let mut result = Vec::with_capacity(alternatives.len());
        for chord in alternatives {
            let keys = chord
                .as_array()
                .ok_or_else(|| Error::InvalidFormat("Invalid scroll modifier chord".into()))?;
            let mut compiled = Vec::with_capacity(keys.len());
            for key in keys {
                let key = key
                    .as_str()
                    .ok_or_else(|| Error::InvalidFormat("Invalid scroll modifier key".into()))?;
                compiled.push(family(key).to_owned());
            }
            result.push(compiled);
        }
        Ok(Self {
            alternatives: result,
        })
    }
    /// Build a held query from typed host capture. Preserve empty chords and
    /// unsupported names, just as the source PostLoad projection does.
    pub fn from_alternatives(alternatives: Vec<Vec<String>>) -> Self {
        Self {
            alternatives: alternatives
                .into_iter()
                .map(|keys| {
                    keys.into_iter()
                        .map(|key| family(&key).to_owned())
                        .collect()
                })
                .collect(),
        }
    }
    pub fn defaults(kind: Kind, mac: bool) -> Self {
        let key = match kind {
            Kind::Zoom => {
                if mac {
                    "LeftWindows"
                } else {
                    "LeftControl"
                }
            }
            Kind::Spacing | Kind::SelectionExtend | Kind::ReceptorAxis => "LeftShift",
            Kind::SelectionMisc => "LeftAlt",
            Kind::SelectionToggle => {
                if mac {
                    "LeftWindows"
                } else {
                    "LeftControl"
                }
            }
        };
        Self {
            alternatives: vec![vec![key.into()]],
        }
    }
    pub fn alternatives(&self) -> &[Vec<String>] {
        &self.alternatives
    }
    pub fn write(&self) -> Value {
        serde_json::json!(self.alternatives)
    }
    /// The host supplies physical-family state with quarantined inputs excluded.
    /// Unknown host keys remain false; they must never be removed into an empty
    /// chord, which would turn an unsupported import into an always-active query.
    pub fn is_down(&self, mut physical_down: impl FnMut(&str) -> bool) -> bool {
        self.alternatives
            .iter()
            .any(|keys| keys.iter().all(|key| physical_down(key)))
    }
    pub fn always_active(&self) -> bool {
        self.alternatives.iter().any(Vec::is_empty)
    }
}
fn family(key: &str) -> &str {
    match key {
        "RightControl" => "LeftControl",
        "RightShift" => "LeftShift",
        "RightAlt" => "LeftAlt",
        "RightWindows" => "LeftWindows",
        other => other,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    zoom: Query,
    spacing: Query,
    selection_misc: Query,
    selection_toggle: Query,
    selection_extend: Query,
    receptor_axis: Query,
}
impl Default for Preferences {
    fn default() -> Self {
        Self::defaults(cfg!(target_os = "macos"))
    }
}
impl Preferences {
    pub fn defaults(mac: bool) -> Self {
        Self {
            zoom: Query::defaults(Kind::Zoom, mac),
            spacing: Query::defaults(Kind::Spacing, mac),
            selection_misc: Query::defaults(Kind::SelectionMisc, mac),
            // Keep the native pre-existing Ctrl/Super family aliases. A source
            // preset/import uses its exact platform defaults instead.
            selection_toggle: Query::from_alternatives(vec![
                vec!["LeftControl".into()],
                vec!["LeftWindows".into()],
            ]),
            selection_extend: Query::defaults(Kind::SelectionExtend, mac),
            receptor_axis: Query::defaults(Kind::ReceptorAxis, mac),
        }
    }
    pub fn get(&self, kind: Kind) -> &Query {
        match kind {
            Kind::Zoom => &self.zoom,
            Kind::Spacing => &self.spacing,
            Kind::SelectionMisc => &self.selection_misc,
            Kind::SelectionToggle => &self.selection_toggle,
            Kind::SelectionExtend => &self.selection_extend,
            Kind::ReceptorAxis => &self.receptor_axis,
        }
    }
    pub fn set(&mut self, kind: Kind, query: Query) {
        match kind {
            Kind::Zoom => self.zoom = query,
            Kind::Spacing => self.spacing = query,
            Kind::SelectionMisc => self.selection_misc = query,
            Kind::SelectionToggle => self.selection_toggle = query,
            Kind::SelectionExtend => self.selection_extend = query,
            Kind::ReceptorAxis => self.receptor_axis = query,
        }
    }
    pub fn read_native(fields: &serde_json::Map<String, Value>) -> Result<Self, Error> {
        let mut result = Self::default();
        for kind in Kind::ALL {
            if let Some(value) = fields.get(kind.source_name()) {
                result.set(kind, Query::from_post_load(value)?);
            }
        }
        Ok(result)
    }
    pub fn write(&self, fields: &mut serde_json::Map<String, Value>) {
        for kind in Kind::ALL {
            fields.insert(kind.source_name().into(), self.get(kind).write());
        }
    }
    pub fn merge(current: &Self, before: &Self, after: &Self) -> Result<Self, Error> {
        let mut result = current.clone();
        for kind in Kind::ALL {
            if before.get(kind) == after.get(kind) {
                continue;
            }
            if current.get(kind) != before.get(kind) {
                return Err(Error::ExternalChange);
            }
            result.set(kind, after.get(kind).clone());
        }
        Ok(result)
    }
}
