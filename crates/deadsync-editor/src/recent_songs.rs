//! SavedSongInformation matching (GrooveAuthor 48e18f71, MIT).
//! Source indexes are ephemeral outputs; only chart descriptors are persisted.
use crate::{
    Error,
    chart_order::{Key, Snapshot},
    open_preferences::Preferences,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedChart {
    pub steps_type: String,
    pub difficulty: i32,
    pub rating: i32,
    /// Deprecated source records leave these null, distinct from empty strings.
    pub name: Option<String>,
    pub description: Option<String>,
}
impl SavedChart {
    pub fn capture(key: &Key) -> Self {
        Self {
            steps_type: key.steps_type.clone(),
            difficulty: key.difficulty,
            rating: key.rating,
            name: Some(key.name.clone()),
            description: Some(key.description.clone()),
        }
    }
    fn same_type(&self, key: &Key) -> bool {
        self.steps_type == key.steps_type && self.difficulty == key.difficulty
    }
    fn exact(&self, key: &Key) -> bool {
        self.same_type(key)
            && self.rating == key.rating
            && self.name.as_deref() == Some(key.name.as_str())
            && self.description.as_deref() == Some(key.description.as_str())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Song {
    pub file: PathBuf,
    pub title: Option<String>,
    pub pack: Option<String>,
    pub spacing_zoom: f64,
    pub chart_position: f64,
    pub active: Vec<SavedChart>,
    pub focused: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restoration {
    pub active: Vec<usize>,
    pub focused: Option<usize>,
}
impl Song {
    /// The native cultural ChartComparer provides order, never source order.
    /// Validate its exhaustive permutation through the existing preferred-open API.
    pub fn resolve(
        &self,
        snapshot: &Snapshot,
        order: &[usize],
        defaults: Preferences,
    ) -> Result<Restoration, Error> {
        if !self.spacing_zoom.is_finite()
            || self.spacing_zoom <= 0.0
            || !self.chart_position.is_finite()
        {
            return Err(Error::InvalidFormat(
                "Invalid recent song position/zoom".into(),
            ));
        }
        let fallback = defaults.select(snapshot, order)?;
        let keys: std::collections::HashMap<_, _> = snapshot
            .source_indices()
            .iter()
            .copied()
            .zip(snapshot.keys())
            .collect();
        Ok(self.resolve_ordered(&keys, order, fallback))
    }
    fn resolve_ordered(
        &self,
        keys: &std::collections::HashMap<usize, &Key>,
        order: &[usize],
        fallback: Option<usize>,
    ) -> Restoration {
        let find = |saved: &SavedChart, exact: bool| {
            order.iter().copied().find(|index| {
                if exact {
                    saved.exact(keys[index])
                } else {
                    saved.same_type(keys[index])
                }
            })
        };
        let focused_record = self.focused.and_then(|i| self.active.get(i));
        let focused = focused_record
            .and_then(|c| find(c, true).or_else(|| find(c, false)))
            .or_else(|| {
                self.active
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| Some(*i) != self.focused)
                    .find_map(|(_, c)| find(c, true))
            })
            .or_else(|| {
                self.active
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| Some(*i) != self.focused)
                    .find_map(|(_, c)| find(c, false))
            })
            .or(fallback);
        let mut active = Vec::new();
        for (index, saved) in self.active.iter().enumerate() {
            let chart = if Some(index) == self.focused {
                focused
            } else {
                find(saved, true).or_else(|| find(saved, false))
            };
            if let Some(chart) = chart {
                if !active.contains(&chart) {
                    active.push(chart);
                }
            }
        }
        // Source GetChartsToUseForActiveCharts may return empty while focus has
        // a fallback. The caller installs that focus using ActiveChartViews;
        // do not insert an invented descriptor into the persisted active list.
        Restoration { active, focused }
    }
}

/// MRU path equality intentionally follows source string equality, with no
/// case folding/canonicalization that could merge unrelated filesystem names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    songs: Vec<Song>,
}
impl History {
    pub fn entries(&self) -> &[Song] {
        &self.songs
    }
    pub fn from_entries(songs: Vec<Song>) -> Self {
        Self { songs }
    }
    pub fn record(&mut self, song: Song, maximum: usize) {
        self.songs
            .retain(|saved| saved.file.as_os_str() != song.file.as_os_str());
        self.songs.insert(0, song);
        self.songs.truncate(maximum);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn song() -> Song {
        Song {
            file: "pack/song/song.ssc".into(),
            title: None,
            pack: None,
            spacing_zoom: 1.0,
            chart_position: 0.0,
            active: vec![],
            focused: None,
        }
    }
    #[test]
    fn unchanged_saved_song_matching_preserves_original_order_and_focus() {
        let request: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/grooveauthor/recent-song.request.json"
        ))
        .unwrap();
        let expected: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/grooveauthor/recent-song.expected.json"
        ))
        .unwrap();
        let mut compared = 0;
        for culture in expected["Results"].as_array().unwrap() {
            for (input, output) in request["Cases"]
                .as_array()
                .unwrap()
                .iter()
                .zip(culture["Cases"].as_array().unwrap())
            {
                if !output["Error"].is_null() {
                    continue;
                }
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
                let order: Vec<usize> = output["Order"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|index| index.as_u64().unwrap() as usize)
                    .collect();
                let map = order.iter().map(|index| (*index, &keys[*index])).collect();
                let mut saved = song();
                saved.focused = input["Saved"]["FocusedChartIndex"]
                    .as_i64()
                    .and_then(|index| usize::try_from(index).ok());
                saved.active = input["Saved"]["ActiveCharts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| SavedChart {
                        steps_type: c["ChartType"].as_str().unwrap().replace('_', "-"),
                        difficulty: c["ChartDifficultyType"].as_str().unwrap().parse().unwrap(),
                        rating: c["Rating"].as_i64().unwrap() as i32,
                        name: c["Name"].as_str().map(str::to_owned),
                        description: c["Description"].as_str().map(str::to_owned),
                    })
                    .collect();
                let result = saved.resolve_ordered(
                    &map,
                    &order,
                    output["Fallback"].as_u64().map(|v| v as usize),
                );
                assert_eq!(
                    serde_json::json!(result.focused),
                    output["Selected"],
                    "{} {}",
                    culture["Culture"],
                    input["Name"]
                );
                assert_eq!(
                    serde_json::json!(result.active),
                    output["Active"],
                    "{} {}",
                    culture["Culture"],
                    input["Name"]
                );
                compared += 1;
            }
        }
        assert!(
            compared >= 300,
            "oracle coverage unexpectedly decreased: {compared}"
        );
    }
    #[test]
    fn mru_moves_exact_name_to_front_and_caps_without_path_normalization() {
        let mut history = History::default();
        for name in ["a/song.ssc", "a/./song.ssc", "A/song.ssc"] {
            let mut value = song();
            value.file = name.into();
            history.record(value, 20);
        }
        assert_eq!(history.entries().len(), 3);
        let mut updated = song();
        updated.file = "a/./song.ssc".into();
        updated.chart_position = 160.5;
        history.record(updated.clone(), 2);
        assert_eq!(
            history.entries(),
            &[
                updated,
                Song {
                    file: "A/song.ssc".into(),
                    ..song()
                }
            ]
        );
        history.record(song(), 0);
        assert!(history.entries().is_empty());
    }
    #[test]
    fn null_legacy_metadata_never_becomes_an_empty_exact_match() {
        let key = Key {
            steps_type: "dance-single".into(),
            difficulty: 4,
            rating: 0,
            name: String::new(),
            description: String::new(),
            step_count: 0,
            guid: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
        };
        let mut saved = SavedChart::capture(&key);
        assert!(saved.exact(&key));
        saved.name = None;
        assert!(!saved.exact(&key));
        assert!(saved.same_type(&key));
    }
}
