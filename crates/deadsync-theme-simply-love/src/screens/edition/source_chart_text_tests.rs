use super::*;
#[test]
fn gameplay_identity_normalizes_only_at_boundary_and_refuses_collisions_en_fr() {
 for fr in [false,true] {
  if fr {i18n::init(deadsync_assets::language::load_for_tests("fr"));} else {i18n::init_for_tests();}
  let directory=tempfile::tempdir().unwrap();let path=directory.path().join("song.ssc");
  let chart="#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:  authored  ;#DIFFICULTY:Easy;#METER:1;#NOTES:0000;";
  std::fs::write(&path,format!("#TITLE:source;#BPMS:0=120;{chart}")).unwrap();
  let state=State::open_selected(&path,"dance-single","Easy","authored",1).unwrap();
  assert_eq!(state.workspace.file().session.document().charts()[0].description,"  authored  ");
  assert_eq!(deadsync_editor::chart_text::Text::read(state.workspace.file().session.document(),0).unwrap().get(deadsync_editor::chart_text::Field::Description),"  authored  ");
  std::fs::write(&path,format!("#TITLE:source;#BPMS:0=120;{chart}{}",chart.replace("  authored  ","authored"))).unwrap();
  assert!(State::open_selected(&path,"dance-single","Easy","authored",1).is_err());
 }
}
