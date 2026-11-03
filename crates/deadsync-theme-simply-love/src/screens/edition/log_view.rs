//! Keyboard adaptation of the original UILog view, using the real app logger.
use super::{Panel,State};use crate::{act,i18n};use deadlib_platform::{input::KeyCode,logging::records};use deadlib_present::{actors::Actor,space::{screen_width,screen_height}};use deadsync_editor::log_preferences::Preferences;use std::sync::Arc;
pub trait LogFormatter{fn timestamp(&mut self,record:&records::Record,style:i32)->Result<String,String>;}
#[derive(Default)]pub(super) struct View{pub preferences:Preferences,snapshot:Option<records::Snapshot>,rows:Vec<Row>,row:usize,first:usize,horizontal:usize,vertical:usize,dirty:bool,locale:String,locale_revision:u64,copy_requested:bool}
#[derive(Clone)]struct Row{level:records::Level,text:String}
impl View{
 fn refresh(&mut self){if let Some(snapshot)=records::snapshot_if_changed(self.snapshot.as_ref().map(|s|&s.revision)){self.snapshot=Some(snapshot);self.rebuild();}}
 fn rebuild(&mut self){self.rows.clear();self.dirty=true;}
 fn copy_text(&self)->String{self.rows.iter().map(|row|row.text.as_str()).collect::<Vec<_>>().join("\n")}
}
impl State{
 pub fn log_preferences(&self)->Preferences{self.log_view.preferences}
 pub fn configure_log_preferences(&mut self,value:Preferences){self.log_view.preferences=value;self.log_view.rebuild();}
 fn commit_log_preferences(&mut self,value:Preferences)->bool{
  let result=(||->Result<_,deadsync_editor::Error>{
   let current=self.capture_application_preferences_file()?;let before=self.log_preferences();let mut owner=Some(current.clone());
   // Direct Log controls adopt disjoint disk edits, matching their original
   // fresh-file contract. Memory has no separate external baseline.
   let live=current.path().as_os_str().is_empty().then_some(&current);
   let merged=deadsync_editor::audio_preferences::publication::publish(&mut owner,live,value,|current|{
    let mut file=current.clone();let merged=Preferences::merge_changed(before,value,file.log_preferences())?;
    file.set_log_preferences(merged);Ok((merged,file))
   })?;Ok((owner,merged))
  })();
  match result{Ok((owner,merged))=>{if let Some(file)=owner{self.publish_application_preferences_owner(file);}self.configure_log_preferences(merged);true},Err(error)=>{self.status=i18n::tr_fmt("Edition","LogPreferencesFailed",&[("error",&i18n::editor_error(&error))]);false}}

 }
 pub(super) fn open_log(&mut self){if !matches!(self.panel,None|Some(Panel::Menu(_)|Panel::EmptyMenu(_)|Panel::EditorHelp(_))){return;}let mut preferences=self.log_preferences();preferences.show=true;let saved=self.commit_log_preferences(preferences);self.text_edit_serial=self.text_edit_serial.wrapping_add(1);self.log_view.row=0;self.log_view.copy_requested=false;self.log_view.refresh();self.panel=Some(Panel::Log);if saved{self.status=i18n::tr("Edition","LogTitle");}}
 pub(super) fn update_log(&mut self){if matches!(self.panel,Some(Panel::Log)){self.log_view.refresh();}}
 pub(super) fn log_key(&mut self,code:KeyCode){
  if code==KeyCode::Escape{let mut preferences=self.log_preferences();preferences.show=false;self.commit_log_preferences(preferences);self.text_edit_serial=self.text_edit_serial.wrapping_add(1);self.log_view.copy_requested=false;self.panel=None;return;}
  if code==KeyCode::PageDown{if self.shift{self.log_view.vertical=self.log_view.vertical.saturating_add(8).min(records::MAX_MESSAGE_BYTES);}else{self.log_view.first=(self.log_view.first+1).min(self.log_view.rows.len().saturating_sub(1));self.log_view.vertical=0;}return;}
  if code==KeyCode::PageUp{if self.shift{self.log_view.vertical=self.log_view.vertical.saturating_sub(8);}else{self.log_view.first=self.log_view.first.saturating_sub(1);self.log_view.vertical=0;}return;}
  if matches!(code,KeyCode::ArrowUp|KeyCode::ArrowDown|KeyCode::Tab){let back=code==KeyCode::ArrowUp||(code==KeyCode::Tab&&self.shift);self.log_view.row=(self.log_view.row+if back{5}else{1})%6;return;}
  if self.shift&&matches!(code,KeyCode::ArrowLeft|KeyCode::ArrowRight){self.log_view.horizontal=if code==KeyCode::ArrowLeft{self.log_view.horizontal.saturating_sub(20)}else{self.log_view.horizontal.saturating_add(20).min(records::MAX_MESSAGE_BYTES)};return;}
  let enter=matches!(code,KeyCode::Enter|KeyCode::NumpadEnter);let adjust=matches!(code,KeyCode::ArrowLeft|KeyCode::ArrowRight)||enter;
  if self.control&&code==KeyCode::KeyC{self.copy_log();return;}
  let mut p=self.log_preferences();match self.log_view.row{
   0 if adjust=>{let current=p.level.ordinal();let next=if code==KeyCode::ArrowLeft{if (1..=2).contains(&current){current-1}else{2}}else if (0..=1).contains(&current){current+1}else{0};p.level=deadsync_editor::log_preferences::Level::known(next as u8).expect("bounded log level");self.commit_log_preferences(p);}
   1 if adjust=>{p.date=if code==KeyCode::ArrowLeft{if (1..=3).contains(&p.date){p.date-1}else{3}}else if (0..=2).contains(&p.date){p.date+1}else{0};self.commit_log_preferences(p);}
   2 if adjust=>{p.wrap=!p.wrap;self.commit_log_preferences(p);}
   3 if enter=>{records::clear();self.log_view.refresh();self.status=i18n::tr("Edition","LogCleared");}
   4 if enter=>self.copy_log(),
   5 if enter=>{if deadlib_platform::logging::current_log_path().is_some(){self.editor_external_open=Some(super::editor_help::ExternalOpen::LogFile);self.status=i18n::tr("Edition","LogOpenRequested");}else{self.status=i18n::tr("Edition","LogOpenUnavailable");}}
   _=>{}
  }
 }
 fn copy_log(&mut self){self.log_view.refresh();self.log_view.copy_requested=true;}
 pub fn log_formatting_needed(&self)->bool{matches!(self.panel,Some(Panel::Log))&&(self.log_view.dirty||self.log_view.copy_requested||self.log_view.locale!=i18n::current_locale()||self.log_view.locale_revision!=i18n::revision())}
 pub(super) fn log_copy_context_current(&self)->bool{matches!(self.panel,Some(Panel::Log))&&!self.log_view.dirty&&self.log_view.locale_revision==i18n::revision()}
 pub fn process_log_formatting(&mut self,formatter:&mut impl LogFormatter){
  if !matches!(self.panel,Some(Panel::Log)){self.log_view.copy_requested=false;return;}
  self.log_view.refresh();if !self.log_formatting_needed(){return;}
  let locale_revision=i18n::revision();let locale=i18n::current_locale();let p=self.log_preferences();let mut rows=Vec::new();let result=(||->Result<(),String>{
   if p.date_format().is_err(){return Err(i18n::tr("Edition","LogUnknownDate").to_string());}
   if let Some(snapshot)=&self.log_view.snapshot{for record in &snapshot.records{
    let level=match record.level{records::Level::Info=>0,records::Level::Warn=>1,records::Level::Error=>2};if level<p.level.ordinal(){continue;}
    let text=if p.date==0{record.message.to_string()}else{format!("{} {}",formatter.timestamp(record,p.date)?,record.message)};
    rows.push(Row{level:record.level,text});
   }}Ok(())
  })();
  if locale_revision!=i18n::revision()||locale!=i18n::current_locale(){self.log_view.rows.clear();self.log_view.dirty=true;self.log_view.copy_requested=false;self.status=i18n::tr("Edition","LogFormatChanged");return;}
  self.log_view.dirty=false;self.log_view.locale=locale;self.log_view.locale_revision=locale_revision;self.log_view.rows=rows;self.log_view.first=self.log_view.first.min(self.log_view.rows.len().saturating_sub(1));
  if let Err(error)=result{self.log_view.rows.clear();self.log_view.copy_requested=false;self.status=i18n::tr_fmt("Edition","LogFormatFailed",&[("error",&error)]);return;}
  if std::mem::take(&mut self.log_view.copy_requested){let text=self.log_view.copy_text();if text.is_empty(){self.status=i18n::tr("Edition","LogCopyEmpty");log::warn!("No log text available to copy.");}else{self.queue_log_clipboard(text);}}
 }
 pub(super) fn log_actors(&self,actors:&mut Vec<Actor>){
  let width=(screen_width()-30.).min(900.);let height=(screen_height()-30.).min(600.);let left=(screen_width()-width)*0.5;let top=(screen_height()-height)*0.5;
  actors.push(act!(quad:xy(left+width*0.5,top+height*0.5):setsize(width,height):diffuse(0.04,0.06,0.1,0.97):z(8070)));
  let label=|actors:&mut Vec<Actor>,text:Arc<str>,y:f32,color:[f32;4]|{let start=actors.len();actors.push(act!(text:font("miso"):settext(text):align(0.,0.):xy(left+12.,y):zoom(0.5):maxwidth(width-24.):diffuse(color[0],color[1],color[2],color[3]):z(8071)));start};
  label(actors,i18n::tr("Edition","LogTitle"),top+10.,[1.;4]);let p=self.log_preferences();
  let date=match p.date{0=>i18n::tr("Edition","LogTimeNone").to_string(),1=>"HH:mm:ss".into(),2=>"HH:mm:ss.fff".into(),3=>"yyyy-MM-dd HH:mm:ss.fff".into(),v=>v.to_string()};
  let values=[format!("{}: {}",i18n::tr("Edition","LogLevel"),match p.level.ordinal(){0=>i18n::tr("Edition","LogInfo").to_string(),1=>i18n::tr("Edition","LogWarn").to_string(),2=>i18n::tr("Edition","LogError").to_string(),_=>p.level.source_name()}),format!("{}: {date}",i18n::tr("Edition","LogTime")),format!("{}: {}",i18n::tr("Edition","LogWrap"),i18n::tr("Edition",if p.wrap{"LogWrapOn"}else{"LogWrapOff"})),i18n::tr("Edition","LogClear").to_string(),i18n::tr_fmt("Edition","LogCopy",&[("count",&self.log_view.rows.len().to_string())]).to_string(),i18n::tr("Edition","LogOpen").to_string()];
  for(index,text)in values.into_iter().enumerate(){let start=label(actors,format!("{} {text}",if index==self.log_view.row{">"}else{" "}).into(),top+40.+index as f32*21.,if index==self.log_view.row{[1.,0.85,0.3,1.]}else{[0.85,0.9,1.,1.]});self.ui_row(actors,start,index==self.log_view.row,21.);}
  if p.date_format().is_err(){label(actors,i18n::tr("Edition","LogUnknownDate"),top+178.,[1.,0.7,0.2,1.]);}
  let room=(height-265.).max(30.);let page=(room/18.).floor() as usize;
  if let Some(row)=self.log_view.rows.get(self.log_view.first){let color=match row.level{records::Level::Info=>[1.,1.,1.,1.],records::Level::Warn=>[1.,1.,0.,1.],records::Level::Error=>[1.,0.,0.,1.]};let columns=(width/6.).max(1.)as usize;
   // Only one bounded record is expanded at a time. Every line is reachable;
   // no unbounded formatted-log rebuild or discarded message tail in render.
   let mut lines=Vec::new();for line in row.text.split('\n'){if p.wrap{let chars:Vec<char>=line.chars().collect();if chars.is_empty(){lines.push(String::new());}for chunk in chars.chunks(columns){lines.push(chunk.iter().collect::<String>());}}else{lines.push(line.chars().skip(self.log_view.horizontal).take(columns).collect());}}
   let first=self.log_view.vertical.min(lines.len().saturating_sub(1));for(index,line)in lines.into_iter().skip(first).take(page).enumerate(){actors.push(act!(text:font("miso"):settext(line):align(0.,0.):xy(left+12.,top+180.+index as f32*18.):zoom(0.45):maxwidth(width-24.):diffuse(color[0],color[1],color[2],color[3]):z(8071)));}
   actors.push(act!(text:font("miso"):settext(format!("{}/{}",self.log_view.first+1,self.log_view.rows.len())):xy(left+width-35.,top+158.):zoom(0.4):z(8071)));
  }
  actors.push(act!(text:font("miso"):settext(self.status.clone()):xy(left+width*0.5,top+height-80.):zoom(0.45):wrapwidthpixels((width-24.)/0.45):maxwidth(width-24.):maxheight(30.):z(8071)));
  actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","LogHelp")):xy(left+width*0.5,top+height-44.):zoom(0.45):wrapwidthpixels((width-24.)/0.45):maxwidth(width-24.):z(8071)));
 }
}
#[cfg(test)]mod tests{
 use super::*;use chrono::{TimeZone,Timelike};use deadsync_editor::clipboard::{TextClipboard,Error as ClipboardError};
 struct NoDate;impl LogFormatter for NoDate{fn timestamp(&mut self,_:&records::Record,_:i32)->Result<String,String>{panic!("date-free source mode must not call date formatter")}}
 #[derive(Default)]struct Clipboard{writes:Vec<String>}impl TextClipboard for Clipboard{fn read_text(&mut self)->Result<String,ClipboardError>{panic!("Log must never read clipboard")}fn write_text(&mut self,text:&str)->Result<(),ClipboardError>{self.writes.push(text.into());Ok(())}}
 fn fixture(state:&mut State){let mut snapshot=records::snapshot_if_changed(None).unwrap();let time=chrono::Local.with_ymd_and_hms(2026,10,29,12,34,56).single().unwrap().with_nanosecond(123_000_000).unwrap();snapshot.records=vec![records::Record{time,level:records::Level::Info,message:"Info\nsecond line".into()},records::Record{time,level:records::Level::Warn,message:"Attention é".into()},records::Record{time,level:records::Level::Error,message:"Erreur 😀".into()}];state.log_view.snapshot=Some(snapshot);let mut p=state.log_preferences();p.date=0;p.level=deadsync_editor::log_preferences::Level::known(1).unwrap();state.configure_log_preferences(p);}
 #[test]fn no_song_log_copy_matches_filtered_visible_content_and_is_explicit_en_fr(){for language in ["en","fr"]{i18n::init(deadsync_assets::language::load_for_tests(language));i18n::set_locale(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();state.panel=None;let before=state.workspace.file().session.serialize().unwrap();state.open_log();fixture(&mut state);state.process_log_formatting(&mut NoDate);assert_eq!(state.log_view.copy_text(),"Attention é\nErreur 😀");let mut p=state.log_preferences();p.wrap=false;state.configure_log_preferences(p);state.process_log_formatting(&mut NoDate);assert_eq!(state.log_view.copy_text(),"Attention é\nErreur 😀");let mut clipboard=Clipboard::default();let mut actors=Vec::new();state.log_actors(&mut actors);assert!(!state.process_text_clipboard(&mut clipboard));assert!(state.take_editor_external_open().is_none());state.log_view.row=4;state.log_key(KeyCode::Enter);state.process_log_formatting(&mut NoDate);assert!(state.process_text_clipboard(&mut clipboard));assert_eq!(clipboard.writes,["Attention é\nErreur 😀"]);assert!(!state.has_song());assert_eq!(state.workspace.file().session.serialize().unwrap(),before);}}
 #[test]fn stale_copy_and_pending_document_review_are_preserved(){i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();state.panel=Some(Panel::OutputLoss(0));state.open_log();assert!(matches!(state.panel,Some(Panel::OutputLoss(0))));state.panel=None;state.open_log();fixture(&mut state);state.log_view.row=4;state.log_key(KeyCode::Enter);state.process_log_formatting(&mut NoDate);state.log_key(KeyCode::Escape);state.open_log();let mut clipboard=Clipboard::default();assert!(!state.process_text_clipboard(&mut clipboard));assert!(clipboard.writes.is_empty());}
 #[test]fn unknown_level_and_date_keep_their_real_distinct_semantics(){i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();state.panel=None;state.open_log();fixture(&mut state);let mut p=state.log_preferences();p.level=deadsync_editor::log_preferences::Level::read_source(Some(&serde_json::json!("99")));state.configure_log_preferences(p);state.process_log_formatting(&mut NoDate);assert!(state.log_view.rows.is_empty());p.date=99;state.configure_log_preferences(p);state.control=true;state.log_key(KeyCode::KeyC);state.process_log_formatting(&mut NoDate);assert!(state.log_view.rows.is_empty());assert_eq!(state.log_preferences().date,99);assert!(state.status.contains(&*i18n::tr("Edition","LogUnknownDate")));}
 #[test]fn log_preferences_survive_file_swaps_and_preserve_raw_vendor_fields(){i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("prefs.json");std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"Version":1,"LogWindowLevel":"99","LogWindowDateDisplay":99,"Vendor":{"retain":7}})).unwrap()).unwrap();let mut state=State::without_song(root.path()).unwrap();state.configure_audio_preferences(&path).unwrap();let before=state.log_preferences();let mut candidate=State::without_song(root.path()).unwrap();super::super::pack_song_list::application_preferences::Snapshot::capture(&state).install(&mut candidate).unwrap();assert_eq!(candidate.log_preferences(),before);let mut changed=before;changed.wrap=false;candidate.commit_log_preferences(changed);let value:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();assert_eq!(value["LogWindowLevel"],"99");assert_eq!(value["LogWindowDateDisplay"],99);assert_eq!(value["Vendor"]["retain"],7);assert_eq!(value["LogWindowLineWrap"],false);}
}
#[cfg(test)]mod locale_guard_tests{
 use super::*;use chrono::TimeZone;
 struct SwitchLocale;impl LogFormatter for SwitchLocale{fn timestamp(&mut self,_:&records::Record,_:i32)->Result<String,String>{i18n::set_locale(deadsync_assets::language::load_for_tests("fr"));Ok("12:34:56".into())}}
 #[test]fn culture_change_during_format_or_before_clipboard_never_writes_stale_text(){i18n::init_for_tests();i18n::set_locale(deadsync_assets::language::load_for_tests("en"));let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();state.panel=None;state.open_log();let mut snapshot=records::snapshot_if_changed(None).unwrap();snapshot.records=vec![records::Record{time:chrono::Local.with_ymd_and_hms(2026,10,29,12,34,56).single().unwrap(),level:records::Level::Info,message:"message".into()}];state.log_view.snapshot=Some(snapshot);state.log_view.rebuild();state.log_view.copy_requested=true;state.process_log_formatting(&mut SwitchLocale);assert!(state.log_view.rows.is_empty());assert!(!state.log_view.copy_requested);assert!(state.text_clipboard_request.is_none());}
}
#[cfg(test)]mod leaf_persistence_tests{use super::*;
 #[test]fn unrelated_log_fields_are_preserved_across_new_file_baselines(){i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("prefs.json");std::fs::write(&path,b"{\"Version\":1}").unwrap();let mut state=State::without_song(root.path()).unwrap();state.configure_audio_preferences(&path).unwrap();std::fs::write(&path,b"{\"Version\":1,\"LogWindowLevel\":\"99\",\"LogWindowLineWrap\":true,\"Vendor\":7}").unwrap();let mut p=state.log_preferences();p.wrap=false;assert!(state.commit_log_preferences(p));assert_eq!(state.log_preferences().level.ordinal(),99);let value:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();assert_eq!(value["LogWindowLevel"],"99");assert_eq!(value["Vendor"],7);std::fs::write(&path,b"{\"Version\":1,\"LogWindowLevel\":\"99\",\"LogWindowLineWrap\":true,\"Vendor\":7}").unwrap();let before=std::fs::read(&path).unwrap();let mut p=state.log_preferences();p.wrap=true;assert!(!state.commit_log_preferences(p));assert_eq!(std::fs::read(&path).unwrap(),before);}
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn opening_log_publishes_short_status_and_renders_help_once_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let directory = tempfile::tempdir().unwrap();
            let mut state = State::without_song(directory.path()).unwrap();
            state.panel = None;
            state.open_log();
            assert!(matches!(state.panel, Some(Panel::Log)));
            assert_eq!(state.status, i18n::tr("Edition", "LogTitle"));
            let mut actors = Vec::new();
            state.log_actors(&mut actors);
            assert_eq!(actors.iter().filter(|actor| matches!(actor,
                Actor::Text { content, .. }
                    if content.as_str() == i18n::tr("Edition", "LogHelp").as_ref()
            )).count(), 1);
        }
    }
}
