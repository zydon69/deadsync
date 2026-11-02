use deadsync_editor::{
    Error,
    audio_preferences::{Audio, Field, FieldValue},
    audio_ticks::{self, Clock, Head, Input},
};
struct SourceClock(bool);
impl Clock for SourceClock {
    fn chart_from_song(&self, t: f64) -> Result<f64, Error> {
        Ok(t)
    }
    fn song_from_chart(&self, t: f64) -> Result<f64, Error> {
        Ok(t)
    }
    fn row_from_song(&self, t: f64) -> Result<f64, Error> {
        Ok(if self.0 && t >= 0.5 {
            t * 96. + 96.
        } else {
            t * 96.
        })
    }
    fn song_from_row(&self, row: i64) -> Result<f64, Error> {
        Ok(if self.0 && row >= 48 {
            48f64.max(row as f64 - 96.) / 96.
        } else {
            row as f64 / 96.
        })
    }
}
#[test]
fn source_music_manager_tick_windows_match_all_eleven_oracle_cases() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/audio-ticks.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let name = case["Name"].as_str().unwrap();
        let mut audio = Audio::default();
        for (field, key) in [
            (Field::UseAssistTick, "Assist"),
            (Field::UseBeatTick, "Beat"),
            (Field::SkipAssistTickOnBeatTick, "SkipAssist"),
            (Field::SkipBeatTickOnAssistTick, "SkipBeat"),
        ] {
            audio
                .set_field(field, FieldValue::Bool(case[key].as_bool().unwrap()))
                .unwrap();
        }
        for (field, key) in [
            (Field::AssistTickAttackTime, "AssistAttack"),
            (Field::BeatTickAttackTime, "BeatAttack"),
            (Field::AudioOffset, "Offset"),
        ] {
            audio
                .set_field(field, FieldValue::Float(case[key].as_f64().unwrap()))
                .unwrap();
        }
        let heads: Vec<_> = case["Events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| Head {
                row: e["Row"].as_i64().unwrap(),
                chart_time: e["Time"].as_f64().unwrap(),
                step: matches!(
                    e["Kind"].as_str().unwrap(),
                    "EditorTapNoteEvent" | "EditorHoldNoteEvent" | "EditorLiftNoteEvent"
                ),
                fake: e["Fake"].as_bool().unwrap(),
            })
            .collect();
        let sample = audio_ticks::sample(
            case["Visual"].as_f64().unwrap() + case["Offset"].as_f64().unwrap(),
            48000,
        )
        .unwrap();
        assert_eq!(sample, case["Result"]["SampleIndex"].as_i64().unwrap());
        let result = audio_ticks::window(
            &SourceClock(name == "warped-beat-duplicates"),
            &heads,
            audio.effective(),
            Input {
                current_music_samples: sample,
                sample_rate: 48000,
                assist_length: 0.2,
                beat_length: 0.2,
                budget: 4096,
            },
        )
        .unwrap();
        for (actual, key) in [(result.assist, "Assist"), (result.beat, "Beat")] {
            let expected = case["Result"][key]
                .as_array()
                .map(|v| v.iter().map(|n| n.as_i64().unwrap()).collect::<Vec<_>>())
                .unwrap_or_default();
            assert_eq!(
                actual.iter().map(|v| v.start_sample).collect::<Vec<_>>(),
                expected,
                "{name} {key}"
            );
        }
    }
}
#[test]
fn tick_window_budget_prevents_unbounded_warp_beat_loops() {
    let mut audio = Audio::default();
    audio
        .set_field(Field::UseBeatTick, FieldValue::Bool(true))
        .unwrap();
    assert!(
        audio_ticks::window(
            &SourceClock(true),
            &[],
            audio.effective(),
            Input {
                current_music_samples: 0,
                sample_rate: 48000,
                assist_length: 0.2,
                beat_length: 0.2,
                budget: 1
            }
        )
        .is_err()
    );
}
