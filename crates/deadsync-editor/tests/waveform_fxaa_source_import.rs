use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
};
#[test]
fn waveform_fxaa_source_fields_have_real_typed_consumers_and_report_presence() {
    let current = File::memory().unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:Waveform;".as_slice(), Format::Sm).unwrap());
    for (raw,present) in [(br#"{"PreferencesWaveForm":{"AntiAlias":false,"AntiAliasSubpix":0.75,"AntiAliasEdgeThreshold":0.125,"AntiAliasEdgeThresholdMin":0.0312}}"#.as_slice(),true),(b"{}".as_slice(),false)] {
        let draft=Draft::prepare(&mut session,None,&current,raw,Platform::Mac).unwrap();
        let p=draft.target().waveform();
        assert_eq!(p.antialias(),!present);assert_eq!(p.subpix(),if present{0.75}else{0.2});assert_eq!(p.edge_threshold(),if present{0.125}else{0.166});assert_eq!(p.edge_threshold_min(),if present{0.0312}else{0.0833});
        for field in ["AntiAlias","AntiAliasSubpix","AntiAliasEdgeThreshold","AntiAliasEdgeThresholdMin"] {
            let path=format!("PreferencesWaveForm.{field}");
            let rows:Vec<_>=draft.report().items.iter().filter(|row|row.path==path).collect();
            assert_eq!(rows.len(),1,"{path}");
            assert_eq!(rows[0].disposition,if present{Disposition::Applied}else{Disposition::Defaulted},"{path}");
        }
        let mut published=File::memory().unwrap();published.set_waveform(p);
        assert_eq!(published.waveform(),p);
    }
}
