use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
    waveform_preferences::DrawLocation,
};
#[test]
fn waveform_source_layout_fields_have_real_typed_consumers_and_report_presence() {
    let current = File::memory().unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:Waveform;".as_slice(), Format::Sm).unwrap());
    for (raw,present) in [(br#"{"PreferencesWaveForm":{"WaveFormScaleXWhenZooming":true,"WaveFormScaleWidthToChart":false,"WaveFormDrawLocation":"AllCharts"}}"#.as_slice(),true),(b"{}".as_slice(),false)] {
        let draft=Draft::prepare(&mut session,None,&current,raw,Platform::Mac).unwrap();
        let p=draft.target().waveform();
        assert_eq!(p.scale_with_zoom(),present);assert_eq!(p.size_to_chart(),!present);
        assert_eq!(p.location(),if present{DrawLocation::AllCharts}else{DrawLocation::AllChartsWithSameMusic});
        assert_eq!(p.render_width(192.,1.5),if present{1536.}else{192.});
        for field in ["WaveFormScaleXWhenZooming","WaveFormScaleWidthToChart","WaveFormDrawLocation"] {
            let path=format!("PreferencesWaveForm.{field}");
            let rows:Vec<_>=draft.report().items.iter().filter(|row|row.path==path).collect();
            assert_eq!(rows.len(),1,"{path}");
            assert_eq!(rows[0].disposition,if present{Disposition::Applied}else{Disposition::Defaulted},"{path}");
        }
        let mut published=File::memory().unwrap();published.set_waveform(p);
        assert_eq!(published.waveform(),p);
    }
}
