use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
    waveform_spacing::ScrollMode,
};
#[test]
fn waveform_scroll_mode_is_reported_under_its_actual_source_scroll_owner_and_waveform_alias_stays_unknown()
 {
    let mut session =
        Session::new(Document::parse(b"#TITLE:Source;".as_slice(), Format::Sm).unwrap());
    let file = File::memory().unwrap();
    let draft = Draft::prepare(
        &mut session,
        None,
        &file,
        br#"{"PreferencesScroll":{"RowBasedWaveFormScrollMode":"CurrentTempo"}}"#,
        Platform::Mac,
    )
    .unwrap();
    assert_eq!(
        draft.target().waveform().scroll_mode(),
        ScrollMode::CurrentTempo
    );
    assert!(
        draft
            .report()
            .items
            .iter()
            .any(|i| i.path == "PreferencesScroll.RowBasedWaveFormScrollMode"
                && i.disposition == Disposition::Applied)
    );
    assert!(
        !draft
            .report()
            .items
            .iter()
            .any(|i| i.path == "PreferencesWaveForm.RowBasedWaveFormScrollMode")
    );
    let draft = Draft::prepare(
        &mut session,
        None,
        &file,
        br#"{"PreferencesWaveForm":{"RowBasedWaveFormScrollMode":1e999}}"#,
        Platform::Mac,
    )
    .unwrap();
    assert_eq!(
        draft.target().waveform().scroll_mode(),
        ScrollMode::MostCommonTempo
    );
    assert!(draft.report().items.iter().any(|i| i.path
        == "PreferencesWaveForm.RowBasedWaveFormScrollMode"
        && i.disposition == Disposition::Retained));
    assert!(
        draft
            .report()
            .items
            .iter()
            .any(|i| i.path == "PreferencesScroll.RowBasedWaveFormScrollMode"
                && i.disposition == Disposition::Defaulted)
    );
}
