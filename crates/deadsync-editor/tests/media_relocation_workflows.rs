use deadsync_editor::media_relocation;
use std::path::Path;

#[test]
#[cfg(unix)]
fn original_unix_media_relocation_cases_match_without_requiring_assets_to_exist() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/media-relocation.reference.json"
    ))
    .unwrap();
    assert_eq!(
        oracle["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let cases = oracle["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 21);
    for case in cases {
        let input = &case["input"];
        let old = input["Old"].as_str().unwrap();
        let result = media_relocation::rebase(
            (!old.is_empty()).then(|| Path::new(old)),
            Path::new(input["New"].as_str().unwrap()),
            input["Media"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(result, case["expected"].as_str().unwrap(), "{input}");
    }
}

#[test]
fn invalid_destination_and_null_media_fail_before_any_filesystem_operation() {
    for destination in ["relative/chart.ssc", "chart.ssc", ""] {
        assert!(media_relocation::rebase(None, Path::new(destination), "music.ogg").is_err());
    }
    assert!(media_relocation::rebase(None, Path::new("/exports/chart.ssc"), "a\0b").is_err());
}

#[test]
#[cfg(unix)]
fn lexical_parent_components_are_bounded_by_root_and_dotted_assets_remain_distinct() {
    let old = Path::new("/songs/chart.ssc");
    let new = Path::new("/export/chart.ssc");
    assert_eq!(
        media_relocation::rebase(Some(old), new, "../../../asset.png").unwrap(),
        "../asset.png"
    );
    assert_eq!(
        media_relocation::rebase(Some(old), new, ".hidden/a..b.png").unwrap(),
        "../songs/.hidden/a..b.png"
    );
    assert_eq!(
        media_relocation::rebase(None, new, "./unresolved/../asset.png").unwrap(),
        "./unresolved/../asset.png"
    );
}
