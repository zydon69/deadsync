use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    playback::Preview,
};
use std::{fs, path::Path};
fn session(local: &str) -> Session {
    Session::new(Document::parse(format!("#VERSION:0.83;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#MUSIC:song.ogg;#OFFSET:0.25;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;{local}#NOTES:1000\n0000\n0000\n0000; ").into_bytes(), Format::Ssc).unwrap())
}
fn audio(dir: &Path) -> std::path::PathBuf {
    fs::write(
        dir.join("song.ogg"),
        b"test asset; decoding belongs to audio engine",
    )
    .unwrap();
    dir.join("song.ssc")
}
#[test]
fn playback_snapshot_uses_unsaved_tempo_and_effective_offset_without_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let path = audio(dir.path());
    let mut session = session("");
    let before = Preview::capture(&session, 0, &path).unwrap();
    assert_eq!(before.music_time(48).unwrap(), 0.25);
    session.set_text(None, "BPMS", "0=60").unwrap();
    session.set_text(None, "OFFSET", "0.5").unwrap();
    let bytes = session.serialize().unwrap();
    let revision = session.revision();
    let after = Preview::capture(&session, 0, &path).unwrap();
    assert_eq!(after.music_time(48).unwrap(), 0.5);
    assert_eq!(after.position(0.5).unwrap(), 48.0);
    assert_eq!(before.music_time(48).unwrap(), 0.25);
    assert_eq!(session.serialize().unwrap(), bytes);
    assert_eq!(session.revision(), revision);
    assert!(session.is_dirty());
}
#[test]
fn chart_audio_and_offset_override_song_and_empty_chart_music_inherits() {
    let dir = tempfile::tempdir().unwrap();
    let path = audio(dir.path());
    fs::write(dir.path().join("chart.wav"), b"asset").unwrap();
    let local = session("#MUSIC:chart.wav;#OFFSET:-0.5;#BPMS:0=120;");
    let preview = Preview::capture(&local, 0, &path).unwrap();
    assert_eq!(preview.music(), dir.path().join("chart.wav"));
    assert_eq!(preview.music_time(0).unwrap(), 0.5);
    assert_eq!(preview.position(0.5).unwrap(), 0.0);
    let inherited = Preview::capture(&session("#MUSIC:;"), 0, &path).unwrap();
    assert_eq!(inherited.music(), dir.path().join("song.ogg"));
    assert_eq!(inherited.music_time(0).unwrap(), -0.25);
}
#[test]
fn playback_handles_pause_warp_and_negative_lead_in_with_current_timing() {
    let dir = tempfile::tempdir().unwrap();
    let path = audio(dir.path());
    let local = session("#BPMS:0=120;#OFFSET:0;#STOPS:1=1;#WARPS:3=2;");
    let preview = Preview::capture(&local, 0, &path).unwrap();
    assert_eq!(preview.position(-0.5).unwrap(), -48.0);
    assert_eq!(preview.position(0.75).unwrap(), 48.0);
    assert_eq!(preview.position(1.25).unwrap(), 48.0);
    assert_eq!(preview.position(2.75).unwrap(), 264.0);
    assert!(preview.position(f64::INFINITY).is_err());
    assert!(preview.position(f64::NAN).is_err());
}
#[test]
fn missing_music_invalid_chart_and_invalid_offset_fail_explicitly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    assert!(Preview::capture(&session(""), 0, &path).is_err());
    audio(dir.path());
    assert!(Preview::capture(&session(""), 1, &path).is_err());
    let mut invalid = session("");
    invalid.set_text(None, "OFFSET", "NaN").unwrap();
    assert!(Preview::capture(&invalid, 0, &path).is_err());
}

#[test]
fn sm_and_multiplayer_charts_can_preview_without_generation_layout_restrictions() {
    let dir = tempfile::tempdir().unwrap();
    let path = audio(dir.path());
    for (format, source) in [
        (
            Format::Sm,
            "#MUSIC:song.ogg;#OFFSET:0;#BPMS:0=120;#NOTES:dance-couple::Hard:9::10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000;",
        ),
        (
            Format::Ssc,
            "#VERSION:0.83;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n00000000\n00000000\n00000000\n&\n00001000\n00000000\n00000000\n00000000;",
        ),
    ] {
        let session = Session::new(
            Document::parse(
                format!("#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{source}")
                    .into_bytes(),
                format,
            )
            .unwrap(),
        );
        let preview = Preview::capture(&session, 0, &path).unwrap();
        assert_eq!(preview.music_time(96).unwrap(), 1.0);
        assert_eq!(preview.position(1.0).unwrap(), 96.0);
    }
}

#[test]
fn captured_preview_exposes_chart_time_without_offset_round_trip_or_file_reopen() {
    let dir = tempfile::tempdir().unwrap(); let path = audio(dir.path());
    let local = session("#OFFSET:10000000000000000;");
    let preview = Preview::capture(&local,0,&path).unwrap();
    fs::remove_file(dir.path().join("song.ogg")).unwrap();
    assert_eq!(preview.chart_time(48).unwrap(),0.5);
    assert_ne!(preview.music_to_chart_time(preview.music_time(48).unwrap()).unwrap(),0.5);
    assert!(!local.is_dirty());
}
