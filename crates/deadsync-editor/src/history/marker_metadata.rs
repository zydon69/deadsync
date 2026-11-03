//! Provenance minted only by the validated marker metadata producer.
use super::{Change,Session};
impl Session {
    pub(crate) fn wrap_marker_metadata(&mut self,changed:bool){
        if changed{let entry=self.undo.last_mut().expect("committed marker metadata");let change=std::mem::replace(&mut entry.change,Change::Group(Vec::new()));entry.change=Change::MarkerMetadata(Box::new(change));}
    }
    pub fn history_preserves_marker_metadata(&self,redo:bool)->bool{
        (if redo{self.redo.last()}else{self.undo.last()}).is_some_and(|entry|matches!(entry.change,Change::MarkerMetadata(_)))
    }
}
#[cfg(test)]
mod tests {
 use super::*;
 use crate::{document::{Document,Format},generation::marker_edit::Draft,chart_focus::Position,history::song_presentation,quick_add::{Plan,Kind}};
 fn session(format:Format)->Session{
  let bytes=if format==Format::Sm{b"#BPMS:0=120;#VENDOR:keep;#NOTES:dance-single::Hard:9::1000;#NOTES:dance-single::Hard:9::0100;".as_slice()}else{b"#VERSION:0.83;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;".as_slice()};
  let mut s=Session::new(Document::parse(bytes,format).unwrap());Plan::prepare(&s,0,Kind::Pattern,96.).unwrap().apply(&mut s,0).unwrap();s
 }
 #[test]fn marker_metadata_receipts_keep_exact_signed_fractional_positions_and_typed_history_sm_ssc(){
  for format in[Format::Sm,Format::Ssc]{for delete in[false,true]{
   let mut s=session(format);let before=s.serialize().unwrap();let mut d=Draft::at_cursor(&s,0,96).unwrap();d.definition.ignore_preceding_distribution=!d.definition.ignore_preceding_distribution;
   let focus=Position::at_fractional_row(&s,0,-0.25).unwrap();let position=(focus.row(),focus.music_time(),focus.chart_time());
   let applied=song_presentation::apply_marker_metadata(&d,&mut s,0,delete).unwrap();assert!(applied.changed());let focus=focus.rebind_song_properties(&s,applied.presentation_receipt().unwrap()).unwrap();assert_eq!((focus.row(),focus.music_time(),focus.chart_time()),position);assert!(s.history_preserves_marker_metadata(false));let after=s.serialize().unwrap();assert_ne!(after,before);
   let mut focus=focus;
   for redo in[false,true]{let result=song_presentation::apply_marker_metadata_history(&mut s,redo).unwrap();focus=focus.rebind_song_properties(&s,result.presentation_receipt().unwrap()).unwrap();assert!(focus.is_bound_to(&s,0));assert_eq!((focus.row(),focus.music_time(),focus.chart_time()),position);assert_eq!(s.serialize().unwrap(),if redo{after.clone()}else{before.clone()});}
   assert_eq!(s.document().text(None,"VENDOR").as_deref(),Some("keep"));
  }}
 }
 #[test]fn unchanged_and_stale_marker_edits_do_not_publish_receipts_or_touch_existing_redo(){
  let mut s=session(Format::Ssc);s.set_text(None,"TITLE","Redo").unwrap();s.undo().unwrap();let d=Draft::at_cursor(&s,0,96).unwrap();let before=s.serialize().unwrap();let revision=s.revision();let redo=s.redo_label().unwrap().to_owned();let result=song_presentation::apply_marker_metadata(&d,&mut s,0,false).unwrap();assert!(!result.changed());assert!(result.presentation_receipt().is_none());assert_eq!(s.revision(),revision);assert_eq!(s.redo_label(),Some(redo.as_str()));assert_eq!(s.serialize().unwrap(),before);
  s.redo().unwrap();let changed=s.serialize().unwrap();assert!(song_presentation::apply_marker_metadata(&d,&mut s,0,false).is_err());assert_eq!(s.serialize().unwrap(),changed);
 }
}
