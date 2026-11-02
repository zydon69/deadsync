use deadsync_editor::{audio_preferences::File, ftue::Preferences};
#[test]
fn progression_is_persistent_and_preserves_unknown_fields() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    std::fs::write(
        &path,
        br#"{"Version":1,"Vendor":{"future":true},"LastCompletedFtueVersion":"0.1","FtueIndex":2}"#,
    )
    .unwrap();
    let mut file = File::open(&path).unwrap();
    assert!(!file.ftue().completed());
    assert_eq!(file.ftue().page(), Some(2));
    let mut progress = file.ftue().clone();
    progress.next();
    file.set_ftue(progress);
    file.save().unwrap();
    let mut file = File::open(&path).unwrap();
    assert_eq!(file.ftue().page(), Some(3));
    let mut progress = file.ftue().clone();
    for _ in 0..3 {
        progress.next();
    }
    file.set_ftue(progress);
    file.save().unwrap();
    let file = File::open(&path).unwrap();
    assert!(file.ftue().completed());
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["LastCompletedFtueVersion"], "0.1.0");
    assert_eq!(json["Vendor"]["future"], true);
}
#[test]
fn stale_progress_save_refuses_to_overwrite_newer_completion() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let mut first = File::open(&path).unwrap();
    let mut stale = first.clone();
    let mut complete = Preferences::default();
    for _ in 0..6 {
        complete.next();
    }
    first.set_ftue(complete);
    first.save().unwrap();
    let mut progress = Preferences::default();
    progress.next();
    stale.set_ftue(progress);
    assert!(stale.save().is_err());
    assert!(File::open(&path).unwrap().ftue().completed());
}

#[test]
fn original_six_step_navigation_matches_all_first_install_cases() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ftue-source.expected.json")).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 384);
    let mut checked = 0;
    for case in cases.as_array().unwrap() {
        // Older completed versions deliberately resume in native: the source
        // Draw early return can hide an incomplete wizard indefinitely.
        if case["completed"] != "null" {
            continue;
        }
        let mut progress = Preferences {
            completed: None,
            index: case["index"].as_i64().unwrap() as i32,
        };
        progress.finish_invalid();
        if let Some(page) = progress.page() {
            match case["click"].as_str().unwrap() {
                "Back" if page > 0 => progress.back(),
                "Next" if page < 5 => progress.next(),
                "Done" if page == 5 => progress.next(),
                _ => {}
            }
        }
        assert_eq!(
            progress.index,
            case["resultIndex"].as_i64().unwrap() as i32,
            "{case}"
        );
        assert_eq!(
            progress.completed(),
            case["hasCompleted"].as_bool().unwrap(),
            "{case}"
        );
        checked += 1;
    }
    assert_eq!(checked, 96);
}
