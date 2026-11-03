//! Source application log-view preferences. No song-content owner or mutation.
use crate::Error;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level(i32);
impl Level {
    pub fn ordinal(self) -> i32 {
        self.0
    }
    pub fn known(value: u8) -> Option<Self> {
        (value <= 2).then_some(Self(i32::from(value)))
    }
    pub fn read_source(value: Option<&Value>) -> Self {
        let Some(text) = value.and_then(Value::as_str) else {
            return Self(0);
        };
        let text = text.trim();
        if let Ok(value) = text.parse::<i32>() {
            return Self(value);
        }
        Self(
            text.split(',')
                .try_fold(0, |n, part| {
                    let value = if part.trim().eq_ignore_ascii_case("Info") {
                        0
                    } else if part.trim().eq_ignore_ascii_case("Warn") {
                        1
                    } else if part.trim().eq_ignore_ascii_case("Error") {
                        2
                    } else {
                        return None;
                    };
                    Some(n | value)
                })
                .unwrap_or(0),
        )
    }
    pub fn source_name(self) -> String {
        match self.0 {
            0 => "Info".into(),
            1 => "Warn".into(),
            2 => "Error".into(),
            v => v.to_string(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub show: bool,
    pub level: Level,
    pub date: i32,
    pub wrap: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            show: true,
            level: Level(0),
            date: 1,
            wrap: true,
        }
    }
}
impl Preferences {
    pub fn read_source(root: &Map<String, Value>) -> Result<Self, Error> {
        let d = Self::default();
        let boolean = |key: &str, default| match root.get(key) {
            None => Ok(default),
            Some(Value::Bool(value)) => Ok(*value),
            _ => Err(Error::InvalidFormat(format!("Invalid {key}"))),
        };
        let date = match root.get("LogWindowDateDisplay") {
            None => d.date,
            Some(value) => value
                .as_i64()
                .and_then(|v| i32::try_from(v).ok())
                .ok_or_else(|| Error::InvalidFormat("Invalid LogWindowDateDisplay".into()))?,
        };
        Ok(Self {
            show: boolean("ShowLogWindow", d.show)?,
            level: Level::read_source(root.get("LogWindowLevel")),
            date,
            wrap: boolean("LogWindowLineWrap", d.wrap)?,
        })
    }
    pub fn write_changed(self, before: Self, root: &mut Map<String, Value>) {
        if self.show != before.show {
            root.insert("ShowLogWindow".into(), self.show.into());
        }
        if self.level != before.level {
            root.insert("LogWindowLevel".into(), self.level.source_name().into());
        }
        if self.date != before.date {
            root.insert("LogWindowDateDisplay".into(), self.date.into());
        }
        if self.wrap != before.wrap {
            root.insert("LogWindowLineWrap".into(), self.wrap.into());
        }
    }
    /// Changed-leaf ownership matches the common application-preference policy.
    pub fn merge_changed(before: Self, after: Self, mut current: Self) -> Result<Self, Error> {
        macro_rules! merge {
            ($field:ident) => {
                if before.$field != after.$field {
                    if current.$field != before.$field {
                        return Err(Error::ExternalChange);
                    }
                    current.$field = after.$field;
                }
            };
        }
        merge!(show);
        merge!(level);
        merge!(date);
        merge!(wrap);
        Ok(current)
    }
    pub fn date_format(self) -> Result<Option<&'static str>, Error> {
        match self.date {
            0 => Ok(None),
            1 => Ok(Some("HH:mm:ss")),
            2 => Ok(Some("HH:mm:ss.fff")),
            3 => Ok(Some("yyyy-MM-dd HH:mm:ss.fff")),
            _ => Err(Error::Unsupported(format!(
                "Unsupported log date display {}",
                self.date
            ))),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_level_tokens_have_real_source_threshold_and_unchanged_raw_survives() {
        for (raw, expected) in [
            (serde_json::json!("99"), 99),
            (serde_json::json!(99), 0),
            (serde_json::json!("Info, Warn"), 1),
            (serde_json::json!("future"), 0),
            (serde_json::json!("-1"), -1),
        ] {
            let mut root = Map::new();
            root.insert("LogWindowLevel".into(), raw.clone());
            let p = Preferences::read_source(&root).unwrap();
            assert_eq!(p.level.ordinal(), expected);
            p.write_changed(p, &mut root);
            assert_eq!(root["LogWindowLevel"], raw);
        }
    }
    #[test]
    fn invalid_date_stays_explicit_not_panicked_or_normalized() {
        let mut root = Map::new();
        root.insert("LogWindowDateDisplay".into(), 99.into());
        let p = Preferences::read_source(&root).unwrap();
        assert_eq!(p.date, 99);
        assert!(p.date_format().is_err());
        p.write_changed(p, &mut root);
        assert_eq!(root["LogWindowDateDisplay"], 99);
    }
}
#[cfg(test)]
mod changed_leaf_tests {
    use super::*;
    #[test]
    fn log_changes_preserve_unowned_current_fields_and_refuse_owned_conflicts() {
        let before = Preferences::default();
        let mut after = before;
        after.wrap = false;
        let mut current = before;
        current.level = Level::read_source(Some(&serde_json::json!("99")));
        let merged = Preferences::merge_changed(before, after, current).unwrap();
        assert!(!merged.wrap);
        assert_eq!(merged.level.ordinal(), 99);
        current.wrap = false;
        assert!(matches!(
            Preferences::merge_changed(before, after, current),
            Err(Error::ExternalChange)
        ));
    }
}
