//! Common Options preferences and source-fidelity default chart selection.
//! The caller supplies the existing cultural ChartComparer order; source block
//! order is not the editor's sorted per-type list.
use crate::{
    Error,
    chart_identity::Difficulty,
    chart_order::{Key, Snapshot, TYPE_ORDER},
    document::chart_type,
};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    steps: i32,
    difficulty: i32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            steps: 0,
            difficulty: 4,
        }
    }
}
impl Preferences {
    pub fn steps_type(&self) -> Option<&'static str> {
        usize::try_from(self.steps)
            .ok()
            .and_then(|index| chart_type::ALL.get(index).copied())
    }
    pub fn steps_type_value(&self) -> i32 {
        self.steps
    }
    pub fn difficulty_value(&self) -> i32 {
        self.difficulty
    }
    pub fn set_steps_type(&mut self, value: &str) -> Result<(), Error> {
        let canonical = chart_type::canonical(value)
            .ok_or_else(|| Error::InvalidEdit("Unknown default chart type".into()))?;
        self.steps = chart_type::ALL
            .iter()
            .position(|kind| *kind == canonical)
            .unwrap() as i32;
        Ok(())
    }
    pub fn set_difficulty(&mut self, value: Difficulty) {
        self.difficulty = Difficulty::ALL
            .iter()
            .position(|candidate| *candidate == value)
            .unwrap() as i32;
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        // Fumen's real permissive converter handles strings as enum names or
        // signed numeric strings. A present invalid scalar returns enum zero,
        // rather than the field initializer (Challenge). Missing keeps defaults.
        let names: Vec<_> = chart_type::ALL
            .iter()
            .map(|name| name.replace('-', "_"))
            .collect();
        let steps = enum_value(fields.get("DefaultStepsType"), &names, 0)?;
        let difficulties: Vec<_> = Difficulty::ALL
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect();
        let difficulty = enum_value(fields.get("DefaultDifficultyType"), &difficulties, 4)?;
        Ok(Self { steps, difficulty })
    }
    /// Select using an exhaustive existing ChartComparer permutation. Neither
    /// chart order, current selection nor source bytes are changed.
    pub fn select(&self, snapshot: &Snapshot, order: &[usize]) -> Result<Option<usize>, Error> {
        let mut expected = snapshot.source_indices().to_vec();
        expected.sort_unstable();
        let mut actual = order.to_vec();
        actual.sort_unstable();
        if expected != actual {
            return Err(Error::InvalidEdit(
                "Open chart order is not an exhaustive source permutation".into(),
            ));
        }
        let keys: HashMap<_, _> = snapshot
            .source_indices()
            .iter()
            .copied()
            .zip(snapshot.keys())
            .collect();
        let ordered: Vec<_> = order.iter().map(|index| (*index, keys[index])).collect();
        Ok(self.select_ordered(&ordered))
    }
    fn select_ordered(&self, ordered: &[(usize, &Key)]) -> Option<usize> {
        let kind = self.steps_type();
        let exact = |kind: &str, difficulty: i32| {
            ordered
                .iter()
                .find(|(_, key)| key.steps_type == kind && key.difficulty == difficulty)
                .map(|(index, _)| *index)
        };
        if let Some(kind) = kind {
            if let Some(index) = exact(kind, self.difficulty) {
                return Some(index);
            }
            for difficulty in [4, 3, 2, 1, 0, 5] {
                if let Some(index) = exact(kind, difficulty) {
                    return Some(index);
                }
            }
        }
        let pair = match kind {
            Some("dance-single") => Some("dance-double"),
            Some("dance-double") => Some("dance-single"),
            Some("pump-single") => Some("pump-double"),
            Some("pump-double") => Some("pump-single"),
            _ => None,
        };
        if let Some(pair) = pair {
            for difficulty in [4, 3, 2, 1, 0, 5] {
                if let Some(index) = exact(pair, difficulty) {
                    return Some(index);
                }
            }
        }
        // The two Couple entries deliberately retain the user-approved native
        // extension. The 14 original supported types keep their exact order.
        for kind in TYPE_ORDER {
            if let Some((index, _)) = ordered.iter().find(|(_, key)| key.steps_type == kind) {
                return Some(*index);
            }
        }
        None
    }
}
fn enum_value(value: Option<&Value>, names: &[String], missing: i32) -> Result<i32, Error> {
    let Some(value) = value else {
        return Ok(missing);
    };
    let Some(text) = value.as_str() else {
        if value.is_array() || value.is_object() {
            return Err(Error::InvalidFormat(
                "Default chart enum preference must be a scalar".into(),
            ));
        }
        return Ok(0);
    };
    let text = text.trim();
    if let Ok(number) = text.parse::<i32>() {
        return Ok(number);
    }
    // Enum.TryParse accepts comma-separated named members even for non-Flags
    // enums, combining their values by bitwise OR. Numeric/name mixtures fail.
    let mut result = 0;
    for member in text.split(',') {
        let Some(index) = names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(member.trim()))
        else {
            return Ok(0);
        };
        result |= index as i32;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_original_preferences_and_select_best_chart_match_both_cultures() {
        let request: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/grooveauthor/open-chart.request.json"
        ))
        .unwrap();
        let expected: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/grooveauthor/open-chart.expected.json"
        ))
        .unwrap();
        for culture in expected["Results"].as_array().unwrap() {
            for (input, output) in request["Cases"]
                .as_array()
                .unwrap()
                .iter()
                .zip(culture["Cases"].as_array().unwrap())
            {
                let preferences = Preferences::read(input["Options"].as_object().unwrap());
                if !output["Error"].is_null() {
                    assert!(preferences.is_err());
                    continue;
                }
                let preferences = preferences.unwrap();
                assert_eq!(
                    preferences.steps_type_value(),
                    output["Steps"].as_i64().unwrap() as i32,
                    "{}",
                    input["Name"]
                );
                assert_eq!(
                    preferences.difficulty_value(),
                    output["Difficulty"].as_i64().unwrap() as i32,
                    "{}",
                    input["Name"]
                );
                let keys: Vec<Key> = input["Charts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|key| Key {
                        steps_type: key["steps_type"].as_str().unwrap().into(),
                        difficulty: key["difficulty"].as_i64().unwrap() as i32,
                        rating: key["rating"].as_i64().unwrap() as i32,
                        name: key["name"].as_str().unwrap().into(),
                        description: key["description"].as_str().unwrap().into(),
                        step_count: key["step_count"].as_u64().unwrap() as usize,
                        guid: key["guid"].as_str().unwrap().parse().unwrap(),
                    })
                    .collect();
                let ordered: Vec<_> = output["Order"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|index| {
                        let index = index.as_u64().unwrap() as usize;
                        (index, &keys[index])
                    })
                    .collect();
                let selected = preferences.select_ordered(&ordered);
                assert_eq!(
                    serde_json::json!(selected),
                    output["Selected"],
                    "{} {}",
                    culture["Culture"],
                    input["Name"]
                );
            }
        }
    }
}
