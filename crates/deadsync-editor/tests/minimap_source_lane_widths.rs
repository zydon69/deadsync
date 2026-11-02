
use deadsync_editor::{audio_preferences::{File,source_import::Draft},document::{Document,Format},history::Session,key_binding_preferences::source_profile::Platform,minimap_content::Raster};
#[test]
fn source_ui_widths_and_independent_note_widths_match_real_minimap_lane_method() {
    let cases:serde_json::Value=serde_json::from_str(include_str!("fixtures/minimap_source_lane_widths.expected.json")).unwrap();
    for case in cases.as_array().unwrap(){
        let width=case["width"].as_u64().unwrap()as u32;let note=case["note_width"].as_u64().unwrap()as u32;let spacing=case["note_spacing"].as_u64().unwrap()as u32;
        let expected:Vec<u32>=case["positions"].as_array().unwrap().iter().map(|p|p.as_u64().unwrap()as u32).collect();
        let mut raster=Raster::new(width,128,4,note,spacing,true,0).unwrap();assert_eq!(raster.lane_positions(),expected.as_slice(),"{case}");
        for lane in 0..4{raster.short_note(lane,10.,0xffffffff).unwrap();raster.hold(lane,20.,40.,0xffffffff,0xff888888).unwrap();}
        let current=File::memory().unwrap();let mut session=Session::new(Document::parse(b"#TITLE:MiniMap;".as_slice(),Format::Sm).unwrap());
        let raw=serde_json::json!({"PreferencesMiniMap":{"MiniMapWidth":width,"MiniMapNoteWidth":note,"MiniMapNoteSpacing":spacing}}).to_string();
        let draft=Draft::prepare(&mut session,None,&current,raw.as_bytes(),Platform::Mac).unwrap();
        let p=draft.target().minimap_content().unwrap();assert_eq!(p.width,width);assert_eq!(p.note_width,note);assert_eq!(p.note_spacing,spacing);
    }
}
