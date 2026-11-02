
use deadsync_editor::{audio_preferences::{File,source_import::{Draft,Disposition}},document::{Document,Format},history::Session,key_binding_preferences::source_profile::Platform};
#[test]
fn source_parallelism_is_consumed_postload_zero_is_reported_and_non_ui_raw_values_survive(){
    let current=File::memory().unwrap();let mut session=Session::new(Document::parse(b"#TITLE:Parallelism;".as_slice(),Format::Sm).unwrap());
    for raw in [-7,0,1,128,513,i32::MAX]{let source=format!("{{\"PreferencesWaveForm\":{{\"WaveFormLoadingMaxParallelism\":{raw}}}}}");let draft=Draft::prepare(&mut session,None,&current,source.as_bytes(),Platform::Mac).unwrap();let value=draft.target().waveform().loading_parallelism();assert_eq!(value,if raw==0{current.waveform().loading_parallelism()}else{raw});assert!(draft.report().items.iter().any(|row|row.path=="PreferencesWaveForm.WaveFormLoadingMaxParallelism"&&row.disposition==if raw==0{Disposition::Normalized}else{Disposition::Applied}&&row.effective.as_ref().and_then(serde_json::Value::as_i64)==Some(i64::from(value))));}
}
