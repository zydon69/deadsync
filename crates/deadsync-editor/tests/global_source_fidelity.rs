use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
use serde_json::Value;
fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            (a - b).abs() <= 1e-6_f64.max(b.abs() * 1e-6)
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|b| close(v, b)))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b))
        }
        _ => a == b,
    }
}
fn group(source: &str, field: &str) -> Option<&'static str> {
    Some(match source {
        "PreferencesWaveForm" => "Waveform",
        "PreferencesDark" => "Dark",
        "PreferencesScroll" => {
            if field == "RowBasedWaveFormScrollMode" {
                "Waveform"
            } else if matches!(
                field,
                "SizeCap" | "LimitZoomToSize" | "ScrollInterpolationDuration" | "ZoomMultiplier"
            ) {
                "Zoom"
            } else {
                "Scroll"
            }
        }
        "PreferencesSelection" => "Selection",
        "PreferencesMiniMap" => "MiniMap",
        "PreferencesOptions" => "Options",
        "PreferencesAudio" => "Audio",
        "PreferencesReceptors" => "Receptors",
        "PreferencesPerformance" => "Performance",
        "PreferencesStream" => "Stream",
        "PreferencesDensityGraph" => "DensityGraph",
        "PreferencesMultiplayer" => "NoteColor",
        _ => return None,
    })
}
#[test]
fn all_mapped_source_constructor_and_postload_fields_match_actual_pinned_models() {
    let data: Value = serde_json::from_str(include_str!(
        "fixtures/global_source_fidelity.expected.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for case in data["cases"].as_array().unwrap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        std::fs::write(&path, b"{\"Version\":1}").unwrap();
        let file = File::open(&path).unwrap();
        let mut session =
            Session::new(Document::parse(b"#TITLE:Oracle;".as_slice(), Format::Sm).unwrap());
        let name = case["name"].as_str().unwrap();
        if !case["accepted"].as_bool().unwrap() {
            assert!(
                Draft::prepare(
                    &mut session,
                    None,
                    &file,
                    case["raw"].as_str().unwrap().as_bytes(),
                    Platform::Mac
                )
                .is_err(),
                "{name}: original loader rejected, native accepted"
            );
            continue;
        }
        let raw: Value = serde_json::from_str(case["raw"].as_str().unwrap()).unwrap();
        if raw["PreferencesStream"]["NoteType"]
            .as_str()
            .and_then(|v| v.trim().parse::<i32>().ok())
            .is_some_and(|v| !(0..9).contains(&v))
        {
            let error = Draft::prepare(
                &mut session,
                None,
                &file,
                case["raw"].as_str().unwrap().as_bytes(),
                Platform::Mac,
            )
            .err()
            .expect("unsafe consumer must reject");
            assert!(
                matches!(error, deadsync_editor::Error::Unsupported(ref message) if message.starts_with("Unsafe source stream NoteType: "))
            );
            continue;
        }
        let draft = match Draft::prepare(
            &mut session,
            None,
            &file,
            case["raw"].as_str().unwrap().as_bytes(),
            Platform::Mac,
        ) {
            Ok(d) => d,
            Err(error) => {
                failures.push(format!(
                    "{name}: import rejected accepted source input: {error}"
                ));
                continue;
            }
        };
        let mut target = draft.target().clone();
        target.save().unwrap();
        let actual: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for item in &draft.report().items {
            if !matches!(
                item.disposition,
                Disposition::Applied | Disposition::Defaulted
            ) {
                continue;
            }
            let Some((source, field)) = item.path.split_once('.') else {
                continue;
            };
            if field.contains('.') {
                continue;
            }
            let Some(native) = group(source, field) else {
                continue;
            };
            let Some(expected) = case["value"].get(source).and_then(|v| v.get(field)) else {
                continue;
            };
            let native_field =
                if source == "PreferencesMultiplayer" && field == "ColorHoldsAndRolls" {
                    "ColorMultiplayerHoldsAndRolls"
                } else {
                    field
                };
            let actual = actual.get(native).and_then(|v| v.get(native_field));
            checked += 1;
            if !actual.is_some_and(|v| close(v, expected)) {
                failures.push(format!(
                    "{name} {}: expected {expected}, persisted {:?}, report {:?}",
                    item.path, actual, item.effective
                ));
            }
        }
    }
    assert!(checked > 10000, "only {checked} fields checked");
    assert!(
        failures.is_empty(),
        "{} discrepancies across {checked} consumed fields:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
