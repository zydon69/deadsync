//! Typed real ChartProperties provenance; clock policy is exercised by native tests.
use deadsync_editor::{chart_properties::Draft,chart_focus::Position,document::{Document,Format},history::Session};
fn session()->Session {Session::new(Document::parse(b"#VERSION:0.83;#OFFSET:0;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#BPMS:0=60;#NOTES:0100;".as_slice(),Format::Ssc).unwrap())}
#[test]
fn real_offset_history_recomputes_song_time_at_exact_signed_rows_and_keeps_other_chart(){
 for row in [-12.75,60.5] {
  let mut s=session();let source=s.serialize().unwrap();let ids=s.chart_ids().to_vec();
  let a=Position::at_fractional_row(&s,0,row).unwrap();let b=Position::at_fractional_row(&s,1,row).unwrap();
  let mut draft=Draft::capture(&s,0).unwrap();draft.set_use_chart_offset(true);draft.set_music_offset(0.375).unwrap();
  let prospective=s.chart_properties_edit_position(&draft,0,&a).unwrap();
  let applied=draft.apply_with_presentation_receipt(&mut s,0).unwrap();assert!(applied.changed());assert!(applied.presentation_receipt().is_none());
  assert_eq!(s.chart_properties_history_chart(false),Some(0));assert_eq!(s.chart_properties_history_preserves_audio(false),Some(true));
  assert!(prospective.is_bound_to(&s,0));
  let changed=Position::at_fractional_row(&s,0,row).unwrap();assert_eq!(changed.row().to_bits(),row.to_bits());assert_eq!(changed.music_time(),a.music_time()-0.375);
  assert_eq!(Position::at_fractional_row(&s,1,row).unwrap().music_time().to_bits(),b.music_time().to_bits());
  let predicted=s.chart_properties_history_position(false,&changed).unwrap();
  assert!(s.apply_chart_properties_history(false).unwrap().presentation_receipt().is_none());assert!(predicted.is_bound_to(&s,0));assert_eq!(s.serialize().unwrap(),source);assert_eq!(s.chart_properties_history_chart(true),Some(0));
  assert!(s.apply_chart_properties_history(true).unwrap().changed());assert_eq!(s.chart_ids(),ids);assert_eq!(Position::at_fractional_row(&s,0,row).unwrap().music_time(),changed.music_time());
 }
}
#[test]
fn real_metadata_history_receipts_preserve_clock_and_mixed_music_history_remembers_audio(){
 let mut s=session();let mut draft=Draft::capture(&s,0).unwrap();draft.set_text(deadsync_editor::chart_text::Field::Description,"new title").unwrap();
 assert!(draft.apply_with_presentation_receipt(&mut s,0).unwrap().presentation_receipt().unwrap().preserves_audio());
 assert!(s.apply_chart_properties_history(false).unwrap().presentation_receipt().unwrap().preserves_audio());
 assert!(s.apply_chart_properties_history(true).unwrap().presentation_receipt().unwrap().preserves_audio());
 let mut draft=Draft::capture(&s,1).unwrap();draft.set_use_chart_offset(true);draft.set_music_offset(0.25).unwrap();draft.set_text(deadsync_editor::chart_text::Field::Music,"other.wav").unwrap();
 draft.apply(&mut s,1).unwrap();assert_eq!(s.chart_properties_history_chart(false),Some(1));assert_eq!(s.chart_properties_history_preserves_audio(false),Some(false));
 s.apply_chart_properties_history(false).unwrap();assert_eq!(s.chart_properties_history_preserves_audio(true),Some(false));s.apply_chart_properties_history(true).unwrap();
}
#[test]
fn failed_stale_and_noop_drafts_cannot_mint_history_provenance(){
 let mut s=session();let draft=Draft::capture(&s,0).unwrap();assert!(!draft.apply(&mut s,0).unwrap());assert_eq!(s.chart_properties_history_chart(false),None);
 let mut stale=Draft::capture(&s,0).unwrap();stale.set_use_chart_offset(true);stale.set_music_offset(0.25).unwrap();s.set_text(None,"TITLE","external").unwrap();let bytes=s.serialize().unwrap();let undo=s.undo_label().map(str::to_owned);
 assert!(stale.apply(&mut s,0).is_err());assert_eq!(s.serialize().unwrap(),bytes);assert_eq!(s.undo_label(),undo.as_deref());assert_eq!(s.chart_properties_history_chart(false),None);
 assert!(s.apply_chart_properties_history(false).is_err());assert_eq!(s.serialize().unwrap(),bytes);
}
