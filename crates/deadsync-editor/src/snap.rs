//! Source snap semantics, isolated until Workspace/native callers migrate together.
use crate::{Error, timing_edit::signature::Signatures};
use serde_json::{Map, Value};
/// Original SnapManager ordering. Native legacy indices 0..8 remain unchanged.
pub const SOURCE_ROWS: [u64; 10] = [0, 48, 24, 16, 12, 8, 6, 4, 3, 1];
pub const LEGACY_OFF_INDEX: usize = 9;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level(usize);
impl Default for Level {
    fn default() -> Self {
        Self(0)
    }
}
impl Level {
    pub fn source(index: usize) -> Result<Self, Error> {
        if index >= SOURCE_ROWS.len() {
            return Err(Error::InvalidEdit("Invalid snap level".into()));
        }
        Ok(Self(index))
    }
    pub fn native(index: usize) -> Result<Self, Error> {
        Self::source(if index == LEGACY_OFF_INDEX {
            0
        } else {
            index
                .checked_add(1)
                .ok_or_else(|| Error::InvalidEdit("Invalid snap level".into()))?
        })
    }
    pub fn source_index(self) -> usize {
        self.0
    }
    pub fn native_index(self) -> usize {
        if self.0 == 0 {
            LEGACY_OFF_INDEX
        } else {
            self.0 - 1
        }
    }
    pub fn rows(self) -> u64 {
        SOURCE_ROWS[self.0]
    }
    /// None means the caller must use the translated Off label.
    pub fn denominator(self) -> Option<u64> {
        let rows = self.rows();
        (rows != 0).then(|| 192 / rows)
    }
    /// Snap changes are preferences, not cursor movement (SnapManager.SetSnapToLevel).
    pub fn navigation(
        self,
        position: f64,
        later: bool,
        signatures: &Signatures,
    ) -> Result<f64, Error> {
        if !position.is_finite() || position.abs() > i32::MAX as f64 {
            return Err(Error::InvalidEdit(
                "Snap position outside chart range".into(),
            ));
        }
        let rows = self.rows();
        if rows != 0 {
            return Ok(crate::scroll_preferences::snap_destination(position, rows, later)? as f64);
        }
        // The signature at departure determines the whole movement; do not snap
        // to a bar boundary or discard the fractional cursor phase.
        let sig = signatures.at(position.max(0.0).floor() as u64);
        let measure = 192.0 * f64::from(sig.numerator) / f64::from(sig.denominator);
        Ok(position + if later { measure } else { -measure })
    }
    /// Off remains selectable when a snap lock is active.
    pub fn allowed(self, lock: Self) -> bool {
        if self.rows() == 0 || lock.rows() == 0 {
            return true;
        }
        let lock_division = 48 / lock.rows();
        let division = 48 / self.rows();
        lock_division % division == 0
    }
    pub fn cycle(self, later: bool, lock: Self) -> Self {
        let mut index = self.0;
        loop {
            index = if later {
                (index + 1) % SOURCE_ROWS.len()
            } else {
                (index + SOURCE_ROWS.len() - 1) % SOURCE_ROWS.len()
            };
            let candidate = Self(index);
            if candidate.allowed(lock) {
                return candidate;
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub level: Level,
    pub lock: Level,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            level: Level(0),
            lock: Level(0),
        }
    }
}
impl Preferences {
    /// Matches original invalid-index normalization; preserves unrelated fields.
    pub fn read(map: &Map<String, Value>) -> Self {
        let read = |key| {
            map.get(key)
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .and_then(|n| Level::source(n).ok())
                .unwrap_or(Level(0))
        };
        Self {
            level: read("SnapIndex"),
            lock: read("SnapLockIndex"),
        }
    }
    pub fn write(self, map: &mut Map<String, Value>) {
        map.insert("SnapIndex".into(), Value::from(self.level.source_index()));
        map.insert(
            "SnapLockIndex".into(),
            Value::from(self.lock.source_index()),
        );
    }
}
