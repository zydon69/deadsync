//! Source RecentFiles JSON boundary, preserving unknown fields for native edits.
use crate::{
    Error,
    document::chart_type,
    open_preferences,
    recent_songs::{SavedChart, Song},
};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    fields: Map<String, Value>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub entries: Vec<Record>,
    pub maximum: usize,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            maximum: 20,
        }
    }
}
fn enum_fields(
    kind: Option<&Value>,
    difficulty: Option<&Value>,
) -> Result<open_preferences::Preferences, Error> {
    let mut fields = Map::new();
    if let Some(kind) = kind {
        fields.insert("DefaultStepsType".into(), kind.clone());
    }
    // SavedChartInformation fields are enum zero by default, unlike Options'
    // initialized Challenge difficulty. Supply that source default explicitly.
    fields.insert(
        "DefaultDifficultyType".into(),
        difficulty.cloned().unwrap_or(Value::String("0".into())),
    );
    open_preferences::Preferences::read(&fields)
}
fn optional_text(fields: &Map<String, Value>, name: &str) -> Result<Option<String>, Error> {
    match fields.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        _ => Err(Error::InvalidFormat(format!("Invalid recent song {name}"))),
    }
}
fn integer(fields: &Map<String, Value>, name: &str, default: i32) -> Result<i32, Error> {
    fields
        .get(name)
        .map(|v| {
            v.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| Error::InvalidFormat(format!("Invalid recent song {name}")))
        })
        .unwrap_or(Ok(default))
}
fn chart(fields: &Map<String, Value>) -> Result<SavedChart, Error> {
    let preferences = enum_fields(fields.get("ChartType"), fields.get("ChartDifficultyType"))?;
    Ok(SavedChart {
        steps_type: preferences
            .steps_type()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("enum:{}", preferences.steps_type_value())),
        difficulty: preferences.difficulty_value(),
        rating: integer(fields, "Rating", 0)?,
        name: optional_text(fields, "Name")?,
        description: optional_text(fields, "Description")?,
    })
}
impl Record {
    pub fn read(fields:Map<String,Value>)->Result<Self,Error>{
        // A native chartless descriptor is intentional. Only obsolete fields or
        // a missing/null modern list identify a legacy record at this boundary.
        let migrate=fields.contains_key("LastChartType")||fields.contains_key("LastChartDifficultyType")||!matches!(fields.get("ActiveCharts"),Some(Value::Array(_)));
        Self::read_with_migration(fields,migrate)
    }
    pub fn read_source(fields:Map<String,Value>)->Result<Self,Error>{Self::read_with_migration(fields,true)}
    fn read_with_migration(mut fields: Map<String, Value>,migrate:bool) -> Result<Self, Error> {
        let empty = match fields.get("ActiveCharts") {
            None | Some(Value::Null) => true,
            Some(Value::Array(values)) => values.is_empty(),
            _ => {
                return Err(Error::InvalidFormat(
                    "Recent ActiveCharts must be an array".into(),
                ));
            }
        };
        if empty && migrate {
            // SavedSongInformation.PostLoad migration, not a modern empty list.
            let old = enum_fields(
                fields.get("LastChartType"),
                fields.get("LastChartDifficultyType"),
            )?;
            fields.insert("ActiveCharts".into(),serde_json::json!([{"ChartType":old.steps_type_value().to_string(),"ChartDifficultyType":old.difficulty_value().to_string(),"Rating":0,"Name":null,"Description":null}]));
            fields.insert("FocusedChartIndex".into(), 0.into());
        }
        fields.remove("LastChartType");
        fields.remove("LastChartDifficultyType");
        let record = Self { fields };
        record.charts()?;
        Ok(record)
    }
    fn charts(&self) -> Result<Vec<SavedChart>, Error> {
        self.fields["ActiveCharts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| {
                chart(
                    value.as_object().ok_or_else(|| {
                        Error::InvalidFormat("Recent chart must be an object".into())
                    })?,
                )
            })
            .collect()
    }
    pub fn song(&self) -> Result<Song, Error> {
        let file = optional_text(&self.fields, "FileName")?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::InvalidFormat("Recent song has no file name".into()))?;
        let number = |name: &str, default: f64| {
            self.fields
                .get(name)
                .map(|v| {
                    v.as_f64()
                        .filter(|n| n.is_finite())
                        .ok_or_else(|| Error::InvalidFormat(format!("Invalid recent song {name}")))
                })
                .unwrap_or(Ok(default))
        };
        let spacing_zoom = number("SpacingZoom", 1.0)?;
        if spacing_zoom <= 0.0 {
            return Err(Error::InvalidFormat(
                "Invalid recent song SpacingZoom".into(),
            ));
        }
        Ok(Song {
            file: file.into(),
            title: optional_text(&self.fields, "SongName")?,
            pack: optional_text(&self.fields, "PackName")?,
            spacing_zoom,
            chart_position: number("ChartPosition", 0.0)?,
            active: self.charts()?,
            focused: usize::try_from(integer(&self.fields, "FocusedChartIndex", -1)?).ok(),
        })
    }
    pub fn update(&mut self, song: &Song) -> Result<(), Error> {
        let file = song
            .file
            .to_str()
            .ok_or_else(|| Error::Unsupported("Recent file name is not Unicode".into()))?;
        if !song.spacing_zoom.is_finite()
            || song.spacing_zoom <= 0.0
            || !song.chart_position.is_finite()
        {
            return Err(Error::InvalidEdit(
                "Invalid recent song presentation".into(),
            ));
        }
        let mut charts = Vec::with_capacity(song.active.len());
        for saved in &song.active {
            let kind = chart_type::ALL
                .iter()
                .position(|kind| *kind == saved.steps_type)
                .map(|i| i as i32)
                .or_else(|| {
                    saved
                        .steps_type
                        .strip_prefix("enum:")
                        .and_then(|s| s.parse().ok())
                })
                .ok_or_else(|| Error::InvalidEdit("Unknown recent chart type".into()))?;
            charts.push(serde_json::json!({"ChartType":kind.to_string(),"ChartDifficultyType":saved.difficulty.to_string(),"Rating":saved.rating,"Name":saved.name,"Description":saved.description}));
        }
        let focused = song
            .focused
            .map(i32::try_from)
            .transpose()
            .map_err(|_| Error::InvalidEdit("Recent focused chart index overflows".into()))?
            .unwrap_or(-1);
        let updates = serde_json::json!({"FileName":file,"SongName":song.title,"PackName":song.pack,"SpacingZoom":song.spacing_zoom,"ChartPosition":song.chart_position,"ActiveCharts":charts,"FocusedChartIndex":focused});
        self.fields.extend(updates.as_object().unwrap().clone());
        Ok(())
    }
}
impl Preferences {
    pub fn read(root:&Map<String,Value>)->Result<Self,Error>{Self::read_with_source(root,false)}
    pub fn read_source(root:&Map<String,Value>)->Result<Self,Error>{Self::read_with_source(root,true)}
    fn read_with_source(root: &Map<String, Value>,source:bool) -> Result<Self, Error> {
        let options = match root.get("Options") {
            None => None,
            Some(Value::Object(options)) => Some(options),
            Some(_) => {
                return Err(Error::InvalidFormat(
                    "Recent Options must be an object".into(),
                ));
            }
        };
        let maximum = match options.and_then(|o| o.get("RecentFilesHistorySize")) {
            None => 20,
            Some(value) => value
                .as_u64()
                .and_then(|v| i32::try_from(v).ok())
                .map(|v| v as usize)
                .ok_or_else(|| Error::InvalidFormat("Invalid recent history size".into()))?,
        };
        let entries = match root.get("RecentFiles") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(values)) => values
                .iter()
                .map(|v| {
                    let fields=v.as_object()
                            .ok_or_else(|| {
                                Error::InvalidFormat("Recent song must be an object".into())
                            })?
                            .clone();
                    if source{Record::read_source(fields)}else{Record::read(fields)}
                })
                .collect::<Result<_, _>>()?,
            _ => return Err(Error::InvalidFormat("RecentFiles must be an array".into())),
        };
        Ok(Self { entries, maximum })
    }
    pub fn record(&mut self, song: &Song) -> Result<(), Error> {
        // Construct every fallible field before touching the MRU.
        let file = song
            .file
            .to_str()
            .ok_or_else(|| Error::Unsupported("Recent file name is not Unicode".into()))?;
        let mut record = self
            .entries
            .iter()
            .find(|r| r.fields.get("FileName").and_then(Value::as_str) == Some(file))
            .cloned()
            .unwrap_or(Record { fields: Map::new() });
        record.update(song)?;
        self.entries
            .retain(|r| r.fields.get("FileName").and_then(Value::as_str) != Some(file));
        self.entries.insert(0, record);
        self.entries.truncate(self.maximum);
        Ok(())
    }
    pub fn validate(&self) -> Result<(), Error> {
        i32::try_from(self.maximum)
            .map_err(|_| Error::InvalidEdit("Recent history size overflows source int".into()))?;
        Ok(())
    }
    pub fn write_options(&self, options: &mut Map<String, Value>) -> Result<(), Error> {
        self.validate()?;
        options.insert(
            "RecentFilesHistorySize".into(),
            Value::from(self.maximum as u64),
        );
        Ok(())
    }
    pub fn write(&self, root: &mut Map<String, Value>) {
        root.insert(
            "RecentFiles".into(),
            Value::Array(
                self.entries
                    .iter()
                    .map(|r| Value::Object(r.fields.clone()))
                    .collect(),
            ),
        );
        // Options size is only read here; a list update does not overwrite
        // other staged typed Options owners or alter the user's maximum.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn postload_matches_original_saved_song_migration_in_both_cultures() {
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
                let mut fields = input["Saved"].as_object().unwrap().clone();
                fields.insert("FileName".into(), "pack/song/song.ssc".into());
                let record = Record::read_source(fields).unwrap();
                let song = record.song().unwrap();
                assert_eq!(
                    serde_json::json!(song.focused.map(|v| v as i64).unwrap_or(-1)),
                    output["Migrated"]["Focused"],
                    "{}",
                    input["Name"]
                );
                let active:Vec<_>=song.active.iter().map(|chart|{
                    let kind=chart_type::ALL.iter().position(|k|*k==chart.steps_type).map(|v|v as i32).or_else(||chart.steps_type.strip_prefix("enum:").and_then(|s|s.parse().ok())).unwrap();
                    serde_json::json!({"Type":kind,"Difficulty":chart.difficulty,"Rating":chart.rating,"Name":chart.name,"Description":chart.description})
                }).collect();
                assert_eq!(
                    serde_json::json!(active),
                    output["Migrated"]["Active"],
                    "{} {}",
                    culture["Culture"],
                    input["Name"]
                );
                assert!(!record.fields.contains_key("LastChartType"));
                assert!(!record.fields.contains_key("LastChartDifficultyType"));
                compared += 1;
            }
        }
        assert_eq!(compared, 348);
    }
    #[test]
    fn mru_updates_only_owned_fields_and_preserves_shared_options() {
        let mut root=serde_json::json!({"Version":1,"Options":{"RecentFilesHistorySize":2,"StepColorMethodValue":"Beat","MiniMapMaxNotesToDraw":731},"RecentFiles":[{"FileName":"a/./song.ssc","Future":17,"ActiveCharts":[],"LastChartType":"dance_single","LastChartDifficultyType":"Hard"}],"Other":{"Keep":true}}).as_object().unwrap().clone();
        let mut preferences = Preferences::read(&root).unwrap();
        let mut song = preferences.entries[0].song().unwrap();
        song.chart_position = 160.5;
        song.spacing_zoom = 2.0;
        preferences.record(&song).unwrap();
        preferences.write(&mut root);
        assert_eq!(root["RecentFiles"][0]["Future"], 17);
        assert_eq!(root["RecentFiles"][0]["ChartPosition"], 160.5);
        assert_eq!(root["Options"]["StepColorMethodValue"], "Beat");
        assert_eq!(root["Options"]["MiniMapMaxNotesToDraw"], 731);
        assert_eq!(root["Other"]["Keep"], true);
        song.file = "a/song.ssc".into();
        preferences.record(&song).unwrap();
        assert_eq!(preferences.entries.len(), 2);
        let before = preferences.clone();
        song.chart_position = f64::NAN;
        assert!(preferences.record(&song).is_err());
        assert_eq!(preferences, before);
    }
    #[test]
    fn invalid_recent_shape_is_a_reviewable_error_not_a_panic() {
        for value in [
            serde_json::json!({"Options":true}),
            serde_json::json!({"Options":{"RecentFilesHistorySize":2147483648u64}}),
            serde_json::json!({"RecentFiles":true}),
            serde_json::json!({"RecentFiles":[null]}),
            serde_json::json!({"RecentFiles":[{"ActiveCharts":{}}]}),
            serde_json::json!({"Options":{"RecentFilesHistorySize":-1}}),
        ] {
            assert!(Preferences::read(value.as_object().unwrap()).is_err());
        }
        let record = Record::read(Map::new()).unwrap();
        assert!(record.song().is_err(), "a missing path cannot be opened");
    }
}

#[cfg(test)]mod native_chartless_boundary_tests{
 use super::*;
 #[test]fn native_empty_chart_descriptor_survives_file_read_cas_save_and_reopen(){
  let root=tempfile::tempdir().unwrap();let path=root.path().join("prefs.json");std::fs::write(&path,br#"{"Version":1,"RecentFiles":[{"FileName":"song.ssc","ActiveCharts":[],"FocusedChartIndex":-1,"Future":17}]}"#).unwrap();
  let mut file=crate::audio_preferences::File::open(&path).unwrap();let mut song=file.recent_songs().entries[0].song().unwrap();assert!(song.active.is_empty());assert_eq!(song.focused,None);assert!(!file.has_changes());
  song.chart_position=12.5;file.record_recent_song(&song).unwrap();file.save().unwrap();let reopened=crate::audio_preferences::File::open(&path).unwrap();assert_eq!(reopened.recent_songs(),file.recent_songs());assert_eq!(reopened.recent_songs().entries[0].fields["Future"],17);assert!(reopened.recent_songs().entries[0].song().unwrap().active.is_empty());
 }
 #[test]fn explicit_source_empty_still_receives_original_postload_defaults(){let fields=serde_json::json!({"FileName":"song.ssc","ActiveCharts":[],"FocusedChartIndex":-1}).as_object().unwrap().clone();assert!(Record::read(fields.clone()).unwrap().song().unwrap().active.is_empty());let source=Record::read_source(fields).unwrap().song().unwrap();assert_eq!(source.active.len(),1);assert_eq!(source.active[0].steps_type,"dance-single");assert_eq!(source.focused,Some(0));}
}
