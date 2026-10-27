use deadsync_editor::{
    chart_creation,
    document::{Document, Format},
    generation::metadata::ChartData,
    history::Session,
    notes::{self, Grid, Note, Position},
};
use serde_json::Value;
use std::sync::Arc;
fn decode(s: &str) -> Vec<u8> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut bits, mut count, mut out) = (0u32, 0, Vec::new());
    for b in s.bytes().take_while(|v| *v != b'=') {
        bits = (bits << 6) | alphabet.iter().position(|v| *v == b).unwrap() as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}
#[test]
fn real_reader_writer_defaults_and_original_metadata_setter_match_public_paths() {
    let request: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/player-default.request.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/player-default.expected.json"
    ))
    .unwrap();
    let mut checked = 0;
    let mut rejected = 0;
    let mut empty_extension = 0;
    for (input, result) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        assert_eq!(input["Name"], result["Name"]);
        let name = input["Name"].as_str().unwrap();
        if result["Error"].is_string() {
            rejected += 1;
            continue;
        }
        let format = if input["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let bytes = decode(input["Bytes"].as_str().unwrap());
        let document = Document::parse(Arc::<[u8]>::from(bytes.clone()), format).unwrap();
        let layout = &document.charts()[0].steps_type;
        let raw =
            Grid::parse_layout(document.value(document.charts()[0].notes.clone()), layout).unwrap();
        assert_eq!(
            raw.players() as u64,
            result["FumenPlayers"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            result["FumenPlayers"], result["AfterWriterPlayers"],
            "{name}"
        );
        assert_eq!(
            notes::default_players(layout).unwrap() as u64,
            result["DefaultPlayers"].as_u64().unwrap(),
            "{name}"
        );
        let variable = notes::supports_variable_players(layout);
        let explicit = !input["MetadataIndex"].is_null();
        let expected_players = if variable && explicit {
            result["EditorPlayers"].as_u64().unwrap()
        } else {
            result["FumenPlayers"].as_u64().unwrap()
        };
        if variable && !explicit && result["EditorPlayers"] != result["FumenPlayers"] {
            assert!(name.contains("empty3"));
            empty_extension += 1;
        }
        let mut session = Session::new(document);
        assert_eq!(
            session.grid(0).unwrap().players() as u64,
            expected_players,
            "{name}"
        );
        assert_eq!(session.serialize().unwrap(), bytes, "{name}");
        assert!(!session.is_dirty());
        checked += 1;
    }
    assert_eq!(checked, 272);
    assert_eq!(rejected, 160);
    assert_eq!(empty_extension, 6);
}
fn session(format: Format, layout: &str) -> Session {
    let columns = notes::layout_columns(layout).unwrap();
    let notes = "0".repeat(columns as usize);
    let chart = match format {
        Format::Sm => format!("#NOTES:{layout}::Hard:9::{notes};"),
        Format::Ssc => format!("#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:{notes};"),
    };
    Session::new(
        Document::parse(
            format!("#VERSION:0.83;#BPMS:0=120;{chart}").into_bytes(),
            format,
        )
        .unwrap(),
    )
}
#[test]
fn initial_metadata_and_creation_share_family_defaults_including_aliases() {
    for format in [Format::Sm, Format::Ssc] {
        for layout in [
            "dance-routine",
            "pump-routine",
            "smx-team",
            "dance-couple",
            "pump-couple",
            "dance-single",
            "pump-double",
        ] {
            let mut s = session(format, layout);
            let expected = notes::default_players(layout).unwrap();
            assert_eq!(s.grid(0).unwrap().players(), expected);
            let data = ChartData::existing_or_initial(s.document(), 0).unwrap();
            assert_eq!(data.max_player_index + 1, expected);
            let draft = chart_creation::Draft::capture(&s).unwrap();
            let index = draft.apply(&mut s, layout).unwrap();
            assert_eq!(s.grid(index).unwrap().players(), expected);
        }
        for alias in ["42", "smx_couple", "dance_routine"] {
            let mut s = session(format, "dance-single");
            let draft = chart_creation::Draft::capture(&s).unwrap();
            let index = draft.apply(&mut s, alias).unwrap();
            assert_eq!(s.grid(index).unwrap().players(), 2);
            assert_eq!(
                s.document().charts()[index].steps_type,
                notes::canonical_layout(alias).unwrap()
            );
        }
    }
}
#[test]
fn explicit_one_player_is_valid_but_never_discards_existing_owner_and_history_is_exact() {
    for format in [Format::Sm, Format::Ssc] {
        for layout in ["dance-routine", "pump-routine", "smx-team"] {
            let mut s = session(format, layout);
            let before = s.serialize().unwrap();
            assert_eq!(s.grid(0).unwrap().players(), 2);
            assert!(s.set_participants(0, 1).unwrap());
            assert_eq!(s.grid(0).unwrap().players(), 1);
            let one = s.serialize().unwrap();
            let mut reopened = Session::new(Document::parse(one.clone(), format).unwrap());
            assert_eq!(reopened.grid(0).unwrap().players(), 1);
            assert!(s.undo().unwrap());
            assert_eq!(s.serialize().unwrap(), before);
            assert_eq!(s.grid(0).unwrap().players(), 2);
            assert!(s.redo().unwrap());
            assert_eq!(s.serialize().unwrap(), one);
            s.set_participants(0, 4).unwrap();
            let p = Position {
                player: 3,
                lane: 0,
                row: 48,
            };
            s.edit_notes(0, "P4", [(p, Some(Note::tap()))]).unwrap();
            let mut metadata = ChartData::existing_or_initial(s.document(), 0).unwrap();
            metadata.max_player_index = 0;
            metadata.save(&mut s, 0).unwrap();
            let mut saved = Session::new(Document::parse(s.serialize().unwrap(), format).unwrap());
            assert_eq!(saved.grid(0).unwrap().players(), 4);
            assert!(saved.grid(0).unwrap().get(p).is_some());
        }
    }
}
#[test]
fn stepf2_double_p1_default_stays_one_and_routine_starts_two() {
    for layout in ["pump-double", "pump_double", "8"] {
        assert_eq!(
            Grid::parse_layout(b"X000000000", layout).unwrap().players(),
            1
        );
    }
    for layout in ["pump-routine", "pump_routine", "10"] {
        assert_eq!(
            Grid::parse_layout(b"X000000000", layout).unwrap().players(),
            2
        );
    }
}
