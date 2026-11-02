use deadsync_editor::{
    audio_preferences::{Audio, Field, FieldValue},
    audio_ticks::{self, Head, Input},
    document::{Document, Format},
    history::Session,
    playback::Clock,
};
use std::collections::BTreeSet;
#[test]
fn dense_original_music_manager_lists_match_compressed_real_clock_windows() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/audio-dense-ticks.expected.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let name = case["Name"].as_str().unwrap();
        let warp = if name == "fast-tempo" {
            String::new()
        } else {
            "#WARPS:1=3000000;".into()
        };
        let text = format!(
            "#VERSION:0.83;#OFFSET:0;#BPMS:0={};{}#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
            case["Bpm"], warp
        );
        let s = Session::new(Document::parse(text.into_bytes(), Format::Ssc).unwrap());
        let clock = Clock::capture(&s, 0).unwrap();
        let mut audio = Audio::default();
        audio
            .set_field(Field::UseBeatTick, FieldValue::Bool(true))
            .unwrap();
        let mut heads = Vec::new();
        if name == "huge-warp-skip-assist" {
            audio
                .set_field(Field::UseAssistTick, FieldValue::Bool(true))
                .unwrap();
            audio
                .set_field(Field::SkipAssistTickOnBeatTick, FieldValue::Bool(true))
                .unwrap();
            heads.push(Head {
                row: 144000000,
                chart_time: 0.5,
                step: true,
                fake: false,
            });
        }
        let result = audio_ticks::window(
            &clock,
            &heads,
            audio.effective(),
            Input {
                current_music_samples: 0,
                sample_rate: 48000,
                assist_length: 0.2,
                beat_length: 0.2,
                budget: 2_000_000,
            },
        )
        .unwrap();
        let summary = &case["Summary"];
        let mut count = 0u64;
        let mut sum = 0i64;
        let mut digest = 14695981039346656037u64;
        let mut distinct = BTreeSet::new();
        for run in &result.beat {
            let length =
                u64::try_from((i128::from(run.end_row) - i128::from(run.row)) / 48 + 1).unwrap();
            count += length;
            sum += run.start_sample * i64::try_from(length).unwrap();
            distinct.insert(run.start_sample);
            for _ in 0..length {
                digest = (digest ^ (run.start_sample as u64)).wrapping_mul(1099511628211);
            }
        }
        assert_eq!(
            count,
            summary["BeatCount"].as_u64().unwrap(),
            "{name} count"
        );
        assert_eq!(sum, summary["BeatSum"].as_i64().unwrap(), "{name} sum");
        assert_eq!(
            digest,
            summary["BeatDigest"].as_u64().unwrap(),
            "{name} complete order-sensitive source list checksum"
        );
        assert_eq!(
            distinct.len() as u64,
            summary["BeatDistinct"].as_u64().unwrap(),
            "{name} distinct"
        );
        assert_eq!(
            result.beat.first().unwrap().start_sample,
            summary["BeatFirst"].as_i64().unwrap()
        );
        assert_eq!(
            result.beat.last().unwrap().start_sample,
            summary["BeatLast"].as_i64().unwrap()
        );
        assert!(
            result.assist.is_empty(),
            "{name}: compressed beat rows still suppress an assist in the middle of a huge warp"
        );
        if name.starts_with("huge-warp") {
            assert!(
                result.beat.len() < 10,
                "three million source rows must not require three million stored triggers"
            );
        } else {
            assert!(
                distinct.len() > 4096,
                "distinct valid onsets must not be truncated by the former arbitrary limit"
            );
        }
    }
}
