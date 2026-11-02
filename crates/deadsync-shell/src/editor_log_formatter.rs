//! Host formatter uses the existing ICU owner and captured civil wall-clock.
use deadsync_theme_simply_love::screens::edition::log_view::LogFormatter;
struct CachedClock{language:String,style:i32,clock:Result<crate::editor_collation::LogClock,String>}
thread_local!{static CLOCK:std::cell::RefCell<Option<CachedClock>>=const{std::cell::RefCell::new(None)};}
pub(crate) struct NativeLogFormatter{style:i32,language:String}
impl NativeLogFormatter{pub(crate) fn new(language:&str,style:i32)->Self{Self{style,language:language.into()}}}
impl LogFormatter for NativeLogFormatter{
 fn timestamp(&mut self,record:&deadlib_platform::logging::records::Record,style:i32)->Result<String,String>{
  if style!=self.style{return Err("Log date style changed during formatting".into());}
  let millis=record.time.timestamp_millis().checked_add(i64::from(record.time.offset().local_minus_utc())*1000).ok_or("Log timestamp overflow")?;
  CLOCK.with_borrow_mut(|cache|{
   if cache.as_ref().is_none_or(|c|c.language!=self.language||c.style!=style){*cache=Some(CachedClock{language:self.language.clone(),style,clock:crate::editor_collation::LogClock::new(&self.language,style).map_err(|e|e.to_string())});}
   cache.as_mut().expect("installed clock").clock.as_mut().map_err(|e|e.clone())?.format_civil_millis(millis).map_err(|e|e.to_string())
  })
 }
}
#[cfg(test)]mod tests{
 #[test]fn captured_local_civil_time_is_not_rendered_as_utc(){use super::*;use deadlib_platform::logging::records::{Record,Time,Level};for(utc,offset)in[("2026-10-29T09:34:56.123","+03:00"),("2026-10-29T17:04:56.123","-04:30")]{let time=Time::from_naive_utc_and_offset(utc.parse().unwrap(),offset.parse().unwrap());let record=Record{time,level:Level::Info,message:"local".into()};let mut formatter=NativeLogFormatter::new("en-US",3);assert_eq!(formatter.timestamp(&record,3).unwrap(),"2026-10-29 12:34:56.123");}}
 #[test]fn actual_icu_log_dates_filter_and_copy_match_original_six_cultures(){
  let cases:serde_json::Value=serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../deadsync-editor/tests/fixtures/editor-log/expected.json"))).unwrap();
  for case in cases.as_array().unwrap(){let style=case["date"].as_i64().unwrap()as i32;let threshold=case["level"].as_i64().unwrap();let culture=case["culture"].as_str().unwrap();let mut clock=if style==0{None}else{Some(crate::editor_collation::LogClock::new(culture,style).unwrap())};let mut rows=Vec::new();
   for(level,delta,text)in[(0,0,"Info français\nsecond line"),(1,1,"Attention é"),(2,2,"Erreur 😀")]{if level<threshold{continue;}rows.push(match &mut clock{None=>text.to_string(),Some(clock)=>format!("{} {text}",clock.format_civil_millis(1793277296123+delta).unwrap())});}
   assert_eq!(rows.join("\n"),case["text"].as_str().unwrap(),"{culture},style{style},threshold{threshold}");
  }
 }
}
