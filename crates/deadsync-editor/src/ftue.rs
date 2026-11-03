//! GrooveAuthor UIFTUE v0.1.0 progression. Native always resumes older versions:
//! the original Draw hides them despite HasCompletedFTUE=false, blocking updates.
use crate::Error;
use serde_json::{Map, Value};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub completed: Option<[i32; 4]>,
    pub index: i32,
}
impl Preferences {
    pub fn merge_changes(&self, expected: &Self, target: &Self) -> Result<Self, Error> {
        fn fields(value: &Preferences) -> Map<String, Value> {
            let mut fields = Map::from_iter([
                ("LastCompletedFtueVersion".into(), Value::Null),
                ("FtueIndex".into(), Value::from(0)),
            ]);
            value.write_changed(&Preferences::default(), &mut fields);
            fields
        }
        let mut current = fields(self);
        crate::audio_preferences::publication::merge_fields(
            &mut current,
            &fields(expected),
            &fields(target),
        )?;
        Self::read(&current)
    }

    pub fn read(root: &Map<String, Value>) -> Result<Self, Error> {
        let completed = match root.get("LastCompletedFtueVersion") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) => Some(parse_version(text)?),
            _ => {
                return Err(Error::InvalidFormat(
                    "Invalid LastCompletedFtueVersion".into(),
                ));
            }
        };
        let index = match root.get("FtueIndex") {
            None => 0,
            Some(value) => value
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| Error::InvalidFormat("Invalid FtueIndex".into()))?,
        };
        Ok(Self { completed, index })
    }
    pub fn write_changed(&self, old: &Self, root: &mut Map<String, Value>) {
        if self.completed != old.completed {
            root.insert(
                "LastCompletedFtueVersion".into(),
                self.completed
                    .map(|v| Value::String(format_version(v)))
                    .unwrap_or(Value::Null),
            );
        }
        if self.index != old.index {
            root.insert("FtueIndex".into(), self.index.into());
        }
    }
    pub fn completed(&self) -> bool {
        self.completed.is_some_and(|v| v >= [0, 1, 0, -1])
    }
    pub fn page(&self) -> Option<usize> {
        if self.completed() {
            None
        } else {
            usize::try_from(self.index).ok().filter(|i| *i < 6)
        }
    }
    pub fn next(&mut self) {
        self.index = self.index.saturating_add(1);
        self.finish_invalid();
    }
    pub fn back(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }
    pub fn finish_invalid(&mut self) {
        if self.page().is_none() && !self.completed() {
            self.completed = Some([0, 1, 0, -1]);
        }
    }
    pub fn restart(&mut self) {
        self.index = 0;
        self.completed = None;
    }
}
// System.Version leaves omitted Build/Revision at -1, which affects ordering:
// 0.1 < 0.1.0 < 0.1.0.0. Components are nonnegative signed Int32.
fn parse_version(text: &str) -> Result<[i32; 4], Error> {
    let parts = text.split('.').collect::<Vec<_>>();
    let invalid = || Error::InvalidFormat("Invalid LastCompletedFtueVersion".into());
    if !(2..=4).contains(&parts.len()) {
        return Err(invalid());
    }
    let mut result = [-1; 4];
    for (index, part) in parts.into_iter().enumerate() {
        let component = part.trim().parse::<i32>().map_err(|_| invalid())?;
        if component < 0 {
            return Err(invalid());
        }
        result[index] = component;
    }
    Ok(result)
}
fn format_version(version: [i32; 4]) -> String {
    let count = if version[2] < 0 {
        2
    } else if version[3] < 0 {
        3
    } else {
        4
    };
    version[..count]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(".")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_pages_back_done_and_old_version_resume() {
        let mut p = Preferences {
            completed: Some([0, 0, 9, 0]),
            index: 0,
        };
        assert!(!p.completed());
        for index in 0..6 {
            assert_eq!(p.page(), Some(index));
            if index > 0 {
                p.back();
                assert_eq!(p.page(), Some(index - 1));
                p.next();
            }
            p.next();
        }
        assert!(p.completed());
        assert_eq!(p.page(), None);
        p.restart();
        assert_eq!(p.page(), Some(0));
    }
    #[test]
    fn invalid_index_matches_original_completion() {
        for index in [-1, 6, i32::MAX] {
            let mut p = Preferences {
                completed: None,
                index,
            };
            p.finish_invalid();
            assert!(p.completed());
        }
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;
    #[test]
    fn system_version_omitted_components_and_int32_limits() {
        assert!(parse_version("0.1").unwrap() < parse_version("0.1.0").unwrap());
        assert!(parse_version("0.1.0").unwrap() < parse_version("0.1.0.0").unwrap());
        for text in ["0.1", "0.1.0", "0.1.0.0", "2147483647.1"] {
            assert_eq!(format_version(parse_version(text).unwrap()), text);
        }
        for text in ["0", "0.1.0.0.0", "-1.0", "2147483648.0", "0..1"] {
            assert!(parse_version(text).is_err(), "{text}");
        }
    }
}

#[cfg(test)]
mod changed_leaf_publication_tests {
    use super::*;
    #[test]
    fn progression_owns_index_and_completion_independently() {
        let before = Preferences {
            completed: None,
            index: 2,
        };
        let mut target = before.clone();
        target.index = 3;
        let live = Preferences {
            completed: Some([0, 1, 0, 0]),
            index: 2,
        };
        let merged = live.merge_changes(&before, &target).unwrap();
        assert_eq!(merged.completed, live.completed);
        assert_eq!(merged.index, 3);
        assert!(
            Preferences { index: 4, ..live }
                .merge_changes(&before, &target)
                .is_err()
        );
    }
}
