//! Source application choices for chart autogen, independent of song history.
//! Unknown enum strings/numeric values are retained by the preference envelope.
use crate::{Error,generation::{job::TARGET_LAYOUTS,metadata::Guid}};
use serde_json::{Map,Value};
pub const BALANCED:&str="6276c906-ea8f-43b3-9500-0ddeac7bdc22";
pub const STAMINA_SINGLE:&str="aa41f2c5-4da1-41ad-b645-b18ed7c11d47";
pub const STAMINA_DOUBLE:&str="c0334922-6105-4703-add2-3de261b2ff19";
use crate::document::chart_type::ALL as CHART_TYPES;
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Preferences {pub target:Target,pub performed:Guid}
impl Default for Preferences {fn default()->Self{Self{target:Target(0),performed:BALANCED.parse().expect("constant GUID")}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Target(i32);
impl Target {
    fn read(value:Option<&Value>)->Self {
        // Original permissive converter: JSON number/non-string => default;
        // enum numeric *string* and comma-combined named values are accepted.
        let Some(text)=value.and_then(Value::as_str) else{return Self(0);};
        let text=text.trim();
        if let Ok(value)=text.parse::<i32>() {return Self(value);}
        let value=text.split(',').try_fold(0_i32,|value,part|CHART_TYPES.iter().position(|name|name.replace('-',"_").eq_ignore_ascii_case(part.trim())).map(|index|value|index as i32));
        Self(value.unwrap_or(0))
    }
    pub fn from_layout(layout:&str)->Result<Self,Error>{
        if !TARGET_LAYOUTS.contains(&layout){return Err(Error::Unsupported(format!("Unsupported autogen target {layout}")));}
        CHART_TYPES.iter().position(|name|*name==layout).map(|i|Self(i as i32)).ok_or_else(||Error::Unsupported("Autogen chart type is missing from source enum".into()))
    }
    pub fn source_name(self)->String{usize::try_from(self.0).ok().and_then(|i|CHART_TYPES.get(i)).map_or_else(||self.0.to_string(),|s|s.replace('-',"_"))}
    pub fn layout(self)->Result<&'static str,Error>{
        let name=self.source_name().replace('_',"-");
        TARGET_LAYOUTS.iter().copied().find(|layout|*layout==name).ok_or_else(||Error::Unsupported(format!("Autogen target {} is not supported; choose a supported target",self.source_name())))
    }
}
impl Preferences {
    pub fn read(root:&Map<String,Value>)->Result<Self,Error>{
        let performed=match root.get("LastSelectedAutogenPerformedChartConfig") {None=>BALANCED.parse()?,Some(value)=>serde_json::from_value(value.clone()).map_err(|e|Error::InvalidFormat(e.to_string()))?};
        Ok(Self{target:Target::read(root.get("LastSelectedAutogenChartType")),performed})
    }
    pub fn write_changed(&self,previous:&Self,root:&mut Map<String,Value>){
        if self.target!=previous.target {root.insert("LastSelectedAutogenChartType".into(),self.target.source_name().into());}
        if self.performed!=previous.performed {root.insert("LastSelectedAutogenPerformedChartConfig".into(),self.performed.to_string().into());}
    }
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]fn all_supported_layouts_map_to_source_enum_and_back(){for layout in TARGET_LAYOUTS {assert_eq!(Target::from_layout(layout).unwrap().layout().unwrap(),layout);}}
 #[test]fn source_string_enum_and_default_rules_are_preserved(){
  for (raw,expected) in [(serde_json::json!("pump_double"),"pump-double"),(serde_json::json!("8"),"pump-double"),(serde_json::json!(8),"dance-single"),(serde_json::json!("future"),"dance-single"),(serde_json::json!("dance_double, pump_single"),"pump-halfdouble")] {
   assert_eq!(Target::read(Some(&raw)).layout().unwrap(),expected);
  }
  assert!(Target::read(Some(&serde_json::json!("dance_routine"))).layout().is_err());
  assert!(Target::read(Some(&serde_json::json!("999"))).layout().is_err());
 }
 #[test]fn unrelated_changes_preserve_original_unknown_enum_and_vendor_values(){
  let mut root=serde_json::json!({"LastSelectedAutogenChartType":"future","Vendor":{"x":99}}).as_object().unwrap().clone();
  let before=Preferences::read(&root).unwrap();let mut after=before.clone();after.performed=STAMINA_DOUBLE.parse().unwrap();after.write_changed(&before,&mut root);
  assert_eq!(root["LastSelectedAutogenChartType"],"future");assert_eq!(root["Vendor"],serde_json::json!({"x":99}));
  after.target=Target::from_layout("smx-full").unwrap();after.write_changed(&before,&mut root);assert_eq!(root["LastSelectedAutogenChartType"],"smx_full");
 }
}
