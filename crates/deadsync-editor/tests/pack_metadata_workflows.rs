use deadsync_editor::pack::{Data, Document, Draft, MAX_BYTES};
use serde_json::{Value, json};
fn decode(s: &str) -> Vec<u8> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::new();
    for b in s.bytes().take_while(|b| *b != b'=') {
        let v = alphabet.iter().position(|v| *v == b).unwrap() as u32;
        bits = (bits << 6) | v;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}
fn projection(d: &Data) -> Value {
    json!({"Title":d.title,"TitleTransliteration":d.title_transliteration,"TitleSort":d.title_sort,"Series":d.series,"Year":d.year,"Banner":d.banner,"SyncOffset":d.sync_offset,"SyncName":match d.sync_offset {0=>"NULL".into(),1=>"ITG".into(),v=>v.to_string()}})
}
fn fixture() -> (Value, Value) {
    (
        serde_json::from_str(include_str!("fixtures/grooveauthor/pack.request.json")).unwrap(),
        serde_json::from_str(include_str!("fixtures/grooveauthor/pack.expected.json")).unwrap(),
    )
}
#[test]
fn original_reader_and_canonical_writer_match_37_real_cases() {
    let (request, expected) = fixture();
    let cases = request["Loads"].as_array().unwrap();
    assert_eq!(cases.len(), 37);
    for (input, output) in cases.iter().zip(expected["Loads"].as_array().unwrap()) {
        let bytes = decode(input["Bytes"].as_str().unwrap());
        let parsed = Document::parse(bytes.clone());
        let name = input["Name"].as_str().unwrap();
        assert_eq!(
            parsed.is_ok(),
            output["Loaded"].as_bool().unwrap(),
            "{name}"
        );
        if let Ok(doc) = parsed {
            assert_eq!(projection(doc.data()), output["Data"], "{name}");
            assert_eq!(doc.render(doc.data()).unwrap(), bytes, "{name}");
            assert_eq!(
                doc.data().canonical().unwrap(),
                decode(output["Written"].as_str().unwrap()),
                "{name}"
            );
            assert_eq!(
                projection(
                    Document::parse(decode(output["Written"].as_str().unwrap()))
                        .unwrap()
                        .data()
                ),
                output["Reloaded"],
                "{name}"
            );
        }
    }
}
#[test]
fn creation_defaults_match_six_original_clock_injected_cases() {
    let (request, expected) = fixture();
    assert_eq!(request["Creations"].as_array().unwrap().len(), 6);
    for (input, output) in request["Creations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Creations"].as_array().unwrap())
    {
        let data = Data::new(
            input["Title"].as_str().unwrap(),
            input["Banner"].as_str().unwrap(),
            input["Year"].as_i64().unwrap() as i32,
            input["Active"].as_f64(),
            input["Preferred"].as_f64().unwrap(),
        );
        assert_eq!(projection(&data), output["Data"]);
        assert_eq!(
            data.canonical().unwrap(),
            decode(output["Written"].as_str().unwrap())
        );
    }
}
#[test]
fn preservation_patches_first_effective_value_and_inserts_inside_first_group() {
    let source=b";keep\r\n[Group]\r\nVersion=7\r\n DisplayTitle =old\r\nDisplayTitle=shadow\r\nUnknown=\xff\r\n[Other]\r\nx=y\r\n[Group]\r\nSeries=shadow\r\n";
    let doc = Document::parse(source.to_vec()).unwrap();
    let mut data = doc.data().clone();
    data.title = Some("new = title".into());
    data.series = Some("added".into());
    let result = doc.render(&data).unwrap();
    assert_eq!(result,b";keep\r\n[Group]\r\nVersion=7\r\n DisplayTitle =new = title\r\nDisplayTitle=shadow\r\nUnknown=\xff\r\nSeries=added\r\n[Other]\r\nx=y\r\n[Group]\r\nSeries=shadow\r\n");
    assert_eq!(Document::parse(result).unwrap().version(), 7);
}
#[test]
fn edits_preserve_boms_encoding_and_invalid_bytes_outside_the_changed_value() {
    let (request, _) = fixture();
    for input in request["Loads"].as_array().unwrap() {
        let name = input["Name"].as_str().unwrap();
        if !name.starts_with("utf") && name != "invalid-utf8" {
            continue;
        }
        let bytes = decode(input["Bytes"].as_str().unwrap());
        let doc = Document::parse(bytes.clone()).unwrap();
        let mut data = doc.data().clone();
        data.series = Some("Été 日本".into());
        let result = doc.render(&data).unwrap();
        assert_eq!(Document::parse(result.clone()).unwrap().data(), &data);
        if name != "invalid-utf8" {
            let bom = if name.starts_with("utf32") {
                4
            } else if name == "utf8-bom" {
                3
            } else {
                2
            };
            assert_eq!(&result[..bom], &bytes[..bom]);
        } else {
            assert!(result.windows(4).any(|v| v == b"A\xc3(B"));
        }
    }
}
#[test]
fn dirty_nullable_values_and_save_snapshot_baselines_are_independent_of_live_edits() {
    let doc = Document::parse(b"[Group]\nVersion=1\n".to_vec()).unwrap();
    let mut draft = Draft::existing(doc);
    assert!(!draft.has_unsaved_changes());
    let original = draft.data().clone();
    let mut first = original.clone();
    first.title = Some(String::new());
    draft.replace(first.clone()).unwrap();
    assert!(draft.has_unsaved_changes());
    draft.replace(original).unwrap();
    assert!(!draft.has_unsaved_changes());
    draft.replace(first).unwrap();
    let save = draft.prepare().unwrap();
    let mut later = draft.data().clone();
    later.title = Some("later".into());
    draft.replace(later).unwrap();
    draft.acknowledge_saved(&save).unwrap();
    assert!(draft.has_unsaved_changes());
    assert_eq!(draft.data().title.as_deref(), Some("later"));
    assert!(draft.acknowledge_saved(&save).is_err());
    let mut other = Draft::create(Data::new("Other", "", 2026, None, 0.009)).unwrap();
    assert!(other.acknowledge_saved(&save).is_err());
    let saved = other.prepare().unwrap();
    assert!(saved.baseline().is_none());
    other.acknowledge_saved(&saved).unwrap();
    assert!(!other.has_unsaved_changes());
    assert!(other.prepare().unwrap().baseline().is_some());
}
#[test]
fn invalid_inputs_and_unrepresentable_null_edits_fail_without_mutation() {
    assert!(Document::parse(vec![b'x'; MAX_BYTES + 1]).is_err());
    let doc = Document::parse(
        b"[Group]\nVersion=1\nDisplayTitle=present\nYear=-17\nSyncOffset=7\n".to_vec(),
    )
    .unwrap();
    assert_eq!(doc.data().year, -17);
    assert_eq!(doc.data().sync_offset, 7);
    let mut draft = Draft::existing(doc.clone());
    assert!(draft.set_year(-1).is_err());
    assert!(draft.set_sync_offset(7).is_err());
    assert!(!draft.has_unsaved_changes());
    let mut data = doc.data().clone();
    data.title = None;
    assert!(doc.render(&data).is_err());
    data.title = Some("bad\nnewkey=x".into());
    assert!(doc.render(&data).is_err());
    assert_eq!(doc.data().title.as_deref(), Some("present"));
}
