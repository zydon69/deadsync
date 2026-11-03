//! Read-only next-entry effect used to preserve chart-focus transport on history.
use super::{Change,Session};
use crate::generation::metadata::Guid;
fn target<'a>(change:&'a Change,forward:bool,current:&mut &'a [Guid]) {
 match change {
  Change::DocumentSwap(state)=>*current=&state.chart_ids,
  Change::Group(changes)=>{if forward{for change in changes{target(change,forward,current);}}else{for change in changes.iter().rev(){target(change,forward,current);}}},
  Change::ChartProperties {change,..} | Change::SongTimingProperties(change) | Change::TimingMigration(change) | Change::TimingEdit(change) | Change::MarkerMetadata(change) | Change::SongPropertiesLive(change) | Change::SongPropertiesMetadata(change) | Change::ChartEventCopy(change)=>target(change,forward,current),
  _=>{},
 }
}
impl Session {
 /// True only if applying the next real history entry changes chart identity
 /// membership. Titles, notes, normalization and presentation order do not.
 pub fn history_changes_chart_set(&self,redo:bool)->bool {
  let Some(entry)=(if redo{self.redo.last()}else{self.undo.last()}) else{return false;};
  let current=self.chart_ids.as_slice();let mut after=current;target(&entry.change,redo,&mut after);
  current.len()!=after.len() || current.iter().any(|id|!after.contains(id))
 }
}
#[cfg(test)]
mod tests {
 use super::*;
 use crate::{document::{Document,Format},notes::{Note,Position}};
 fn session()->Session{Session::new(Document::parse(b"#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;".as_slice(),Format::Sm).unwrap())}
 fn unchanged(s:&Session,redo:bool,expected:bool){let bytes=s.serialize().unwrap();let revision=s.revision();let ids=s.chart_ids().to_vec();let undo=s.undo_label().map(str::to_owned);let redolabel=s.redo_label().map(str::to_owned);assert_eq!(s.history_changes_chart_set(redo),expected);assert_eq!(s.serialize().unwrap(),bytes);assert_eq!(s.revision(),revision);assert_eq!(s.chart_ids(),ids);assert_eq!(s.undo_label(),undo.as_deref());assert_eq!(s.redo_label(),redolabel.as_deref());}
 #[test]fn real_add_delete_history_effects_are_directional_read_only_and_identity_based(){
  let mut s=session();unchanged(&s,false,false);unchanged(&s,true,false);
  let draft=crate::chart_creation::Draft::capture(&s).unwrap();draft.apply(&mut s,"dance-single").unwrap();unchanged(&s,false,true);unchanged(&s,true,false);
  s.undo().unwrap();unchanged(&s,true,true);s.redo().unwrap();unchanged(&s,false,true);
  crate::chart_deletion::Draft::capture(&s,1).unwrap().apply(&mut s).unwrap();unchanged(&s,false,true);
  s.undo().unwrap();unchanged(&s,true,true);s.redo().unwrap();unchanged(&s,false,true);
 }
 #[test]fn ordinary_edits_and_stepf2_normalization_keep_identity_membership(){
  let mut s=session();s.set_text(None,"TITLE","Edit").unwrap();unchanged(&s,false,false);s.undo().unwrap();unchanged(&s,true,false);
  s.edit_notes(0,"Note",[(Position{row:48,lane:1,player:0},Some(Note::tap()))]).unwrap();unchanged(&s,false,false);s.undo().unwrap();unchanged(&s,true,false);
  let mut coop=Session::new(Document::parse(b"#BPMS:0=120;#NOTES:pump-double::Hard:9::XY00000000;".as_slice(),Format::Sm).unwrap());coop.normalize_stepf2_chart(0).unwrap();unchanged(&coop,false,false);coop.undo().unwrap();unchanged(&coop,true,false);
 }
}
