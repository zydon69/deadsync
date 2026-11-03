//! Typed controls for every serialized generation configuration leaf. Drafts may
//! temporarily violate cross-field rules; finish validates the complete config.
mod gates;
pub use gates::Availability;
use super::profiles::{Config, Kind};
use crate::Error;
use serde_json::Value;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Integer,
    Number,
    Boolean,
    OptionalBoolean,
    Choice(&'static [&'static str]),
}
#[derive(Clone, Debug)]
pub struct Field {
    path: Vec<String>,
    pointer: String,
    pub control: Control,
}
impl Field {
    pub fn path(&self) -> &[String] {
        &self.path
    }
    pub fn name(&self) -> &str {
        self.path.last().unwrap()
    }
}
pub struct Editor {
    kind: Kind,
    value: Value,
    fields: Vec<Field>,
}
fn choices(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "BeatSubDivision" => &["1", "2", "3", "4", "6", "8", "12", "16", "48"],
        "StartingFootSpecified" => &["0", "1"],
        "StartingFootChoice" => &["random", "automatic", "specified"],
        "LeftFootStartChoice" | "RightFootStartChoice" => &[
            "automaticSameLane",
            "automaticNewLane",
            "automaticSameOrNewLane",
            "specifiedLane",
        ],
        "LeftFootEndChoice" | "RightFootEndChoice" => &[
            "automaticIgnoreFollowingSteps",
            "automaticSameLaneToFollowing",
            "automaticNewLaneToFollowing",
            "automaticSameOrNewLaneAsFollowing",
            "specifiedLane",
        ],
        "DefaultBracketParsingMethod" => &["aggressive", "balanced", "noBrackets"],
        "BracketParsingDetermination" => &["chooseMethodDynamically", "useDefaultMethod"],
        _ => return None,
    })
}
impl Editor {
    pub fn new(config: &Config) -> Result<Self, Error> {
        Self::from_value(config.kind(), config.json()?)
    }
    fn from_value(kind: Kind, value: Value) -> Result<Self, Error> {
        let mut fields = Vec::new();
        collect(&value, &mut Vec::new(), &mut fields)?;
        // Preserve the established compact interpretation panel's field order.
        if kind == Kind::Expressed {
            let order = [
                "DefaultBracketParsingMethod",
                "BracketParsingDetermination",
                "MinLevelForBrackets",
                "UseAggressiveBracketsWhenMoreSimultaneousNotesThanCanBeCoveredWithoutBrackets",
                "BalancedBracketsPerMinuteForAggressiveBrackets",
                "BalancedBracketsPerMinuteForNoBrackets",
            ];
            fields.sort_by_key(|field| {
                order
                    .iter()
                    .position(|name| *name == field.name())
                    .unwrap_or(usize::MAX)
            });
        }
        if kind == Kind::Pattern {
            let order = [
                "BeatSubDivision",
                "StartingFootChoice",
                "StartingFootSpecified",
                "LeftFootStartChoice",
                "LeftFootStartLaneSpecified",
                "LeftFootEndChoice",
                "LeftFootEndLaneSpecified",
                "RightFootStartChoice",
                "RightFootStartLaneSpecified",
                "RightFootEndChoice",
                "RightFootEndLaneSpecified",
                "SameArrowStepWeight",
                "NewArrowStepWeight",
                "StepTypeCheckPeriod",
                "LimitSameArrowsInARowPerFoot",
                "MaxSameArrowsInARowPerFoot",
            ];
            fields.sort_by_key(|field| {
                order
                    .iter()
                    .position(|name| *name == field.name())
                    .unwrap_or(usize::MAX)
            });
        }
        if kind == Kind::Performed {
            let groups = [
                "Facing",
                "LateralTightening",
                "StepTightening",
                "Transitions",
                "ArrowWeights",
            ];
            fields.sort_by_key(|field| {
                groups
                    .iter()
                    .position(|name| *name == field.path[0])
                    .unwrap_or(usize::MAX)
            });
        }
        Ok(Self {
            kind,
            value,
            fields,
        })
    }
    pub fn has_arrow_weights(&self, layout: &str) -> Result<bool, Error> {
        if self.kind != Kind::Performed {
            return Err(invalid("Only movement profiles have arrow weights"));
        }
        if !super::job::TARGET_LAYOUTS.contains(&layout) {
            return Err(invalid("Unsupported arrow-weight layout"));
        }
        Ok(self
            .value
            .get("ArrowWeights")
            .and_then(Value::as_object)
            .is_some_and(|weights| weights.contains_key(layout)))
    }
    /// Add missing default weights or remove an explicit override. Rebuild only
    /// field descriptors; other staged values (even invalid ranges) stay intact.
    pub fn set_arrow_weights_presence(
        &mut self,
        layout: &str,
        present: bool,
    ) -> Result<bool, Error> {
        if self.has_arrow_weights(layout)? == present {
            return Ok(false);
        }
        let mut value = self.value.clone();
        let weights = value
            .get_mut("ArrowWeights")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid("Missing arrow-weight configuration"))?;
        if present {
            let defaults = super::config::PerformedConfig::balanced();
            let defaults = defaults
                .arrow_weights
                .get(layout)
                .ok_or_else(|| invalid("Missing default arrow weights"))?;
            weights.insert(
                layout.into(),
                serde_json::to_value(defaults).map_err(|e| invalid(&e.to_string()))?,
            );
        } else {
            weights.remove(layout);
        }
        let candidate = Self::from_value(self.kind, value)?;
        *self = candidate;
        Ok(true)
    }
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
    pub fn value(&self, index: usize) -> Option<&Value> {
        self.fields
            .get(index)
            .and_then(|field| self.value.pointer(&field.pointer))
    }
    pub fn set_text(&mut self, index: usize, text: &str) -> Result<(), Error> {
        let field = self
            .fields
            .get(index)
            .ok_or_else(|| invalid("Unknown profile field"))?;
        let value = match field.control {
            Control::Integer => integer_value(
                field,
                text.trim()
                    .parse::<i32>()
                    .map_err(|_| invalid("Enter a 32-bit integer"))?,
            ),
            Control::Number => Value::from(
                text.trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid("Enter a finite number"))?,
            ),
            _ => return Err(invalid("This profile field is not numeric")),
        };
        *self
            .value
            .pointer_mut(&field.pointer)
            .ok_or_else(|| invalid("Missing profile field"))? = value;
        Ok(())
    }
    /// Source ImGui numeric inputs clamp on edit. Keep set_text raw for
    /// advanced drafts/import APIs; the native inspector uses this boundary.
    pub fn set_source_text(&mut self,index:usize,text:&str)->Result<(),Error>{
        let field=self.fields.get(index).cloned().ok_or_else(||invalid("Unknown profile field"))?;
        self.set_text(index,text)?;
        let value=self.value.pointer(&field.pointer).expect("set_text verified numeric field");
        let bounded=match field.control{
            Control::Integer=>integer_value(&field,integer_nudge(&field,value.as_i64().expect("typed integer")as i32)),
            Control::Number=>{let(_,min,max)=number_nudge(&field,&self.value);Value::from(value.as_f64().expect("typed number").max(min).min(max))},
            _=>unreachable!("set_text rejects nonnumeric controls"),
        };
        *self.value.pointer_mut(&field.pointer).expect("known field")=bounded;Ok(())
    }
    pub fn step(&mut self, index: usize, forward: bool, large: bool) -> Result<(), Error> {
        let field = self
            .fields
            .get(index)
            .ok_or_else(|| invalid("Unknown profile field"))?;
        let value = self
            .value
            .pointer(&field.pointer)
            .ok_or_else(|| invalid("Missing profile field"))?;
        let next = match field.control {
            Control::Integer => integer_value(
                field,
                integer_nudge(field,(value
                    .as_i64()
                    .ok_or_else(|| invalid("Invalid integer profile field"))?
                    as i32)
                    .saturating_add(if forward {
                        if large { 10 } else { 1 }
                    } else if large {
                        -10
                    } else {
                        -1
                    })),
            ),
            Control::Number => {
                let current=value.as_f64().ok_or_else(||invalid("Invalid numeric profile field"))?;
                let (step,min,max)=number_nudge(field,&self.value);
                let delta=step*if large{10.0}else{1.0};
                let next=current+if forward{delta}else{-delta};
                if !next.is_finite(){return Err(invalid("Profile value overflow"));}
                Value::from(next.max(min).min(max))
            }
            Control::Boolean => Value::Bool(
                !value
                    .as_bool()
                    .ok_or_else(|| invalid("Invalid boolean profile field"))?,
            ),
            Control::OptionalBoolean => match (value, forward) {
                (Value::Null, true) | (Value::Bool(true), false) => Value::Bool(false),
                (Value::Bool(false), true) | (Value::Null, false) => Value::Bool(true),
                _ => Value::Null,
            },
            Control::Choice(values) => {
                let text = value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string());
                let current = values
                    .iter()
                    .position(|choice| *choice == text)
                    .ok_or_else(|| invalid("Unknown profile choice"))?;
                let next =
                    values[(current + if forward { 1 } else { values.len() - 1 }) % values.len()];
                if value.is_string() {
                    Value::String(next.into())
                } else {
                    Value::from(
                        next.parse::<u32>()
                            .map_err(|_| invalid("Invalid numeric profile choice"))?,
                    )
                }
            }
        };
        *self
            .value
            .pointer_mut(&field.pointer)
            .ok_or_else(|| invalid("Missing profile field"))? = next;
        Ok(())
    }
    pub fn apply_travel_speed(&mut self, speed: super::profiles::TravelSpeed) -> Result<(), Error> {
        if self.kind != Kind::Performed {
            return Err(invalid("Only movement profiles have travel-speed settings"));
        }
        let (min, max) = speed.intervals()?;
        let step = self
            .value
            .get_mut("StepTightening")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid("Missing travel-speed configuration"))?;
        if !step.contains_key("SpeedMinTimeSeconds") || !step.contains_key("SpeedMaxTimeSeconds") {
            return Err(invalid("Missing travel-speed intervals"));
        }
        step.insert("SpeedMinTimeSeconds".into(), Value::from(min));
        step.insert("SpeedMaxTimeSeconds".into(), Value::from(max));
        Ok(())
    }
    pub fn finish(&self) -> Result<Config, Error> {
        let mut config = match self.kind {
            Kind::Pattern => Config::Pattern(
                serde_json::from_value(self.value.clone()).map_err(|e| invalid(&e.to_string()))?,
            ),
            Kind::Performed => Config::Performed(
                serde_json::from_value(self.value.clone()).map_err(|e| invalid(&e.to_string()))?,
            ),
            Kind::Expressed => Config::Expressed(
                serde_json::from_value(self.value.clone()).map_err(|e| invalid(&e.to_string()))?,
            ),
        };
        config.validate()?;
        Ok(config)
    }
}
fn inherited_boolean(path: &[String]) -> bool {
    match path {
        [group, field] if group == "StepTightening" => matches!(
            field.as_str(),
            "SpeedTighteningEnabled" | "DistanceTighteningEnabled" | "StretchTighteningEnabled"
        ),
        [group, field] => {
            matches!(group.as_str(), "LateralTightening" | "Transitions") && field == "Enabled"
        }
        _ => false,
    }
}
fn collect(value: &Value, path: &mut Vec<String>, fields: &mut Vec<Field>) -> Result<(), Error> {
    match value {
        Value::Object(values) => {
            for (key, value) in values {
                path.push(key.clone());
                collect(value, path, fields)?;
                path.pop();
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                path.push(index.to_string());
                collect(value, path, fields)?;
                path.pop();
            }
        }
        _ => {
            let name = path
                .last()
                .ok_or_else(|| invalid("Invalid profile configuration root"))?;
            let control = if let Some(values) = choices(name) {
                Control::Choice(values)
            } else {
                match value {
                    Value::Bool(_) | Value::Null if inherited_boolean(path) => {
                        Control::OptionalBoolean
                    }
                    Value::Bool(_) => Control::Boolean,
                    Value::Number(number) if number.is_i64() || number.is_u64() => Control::Integer,
                    Value::Number(_) => Control::Number,
                    _ => return Err(invalid(&format!("Unsupported profile control: {name}"))),
                }
            };
            let pointer = format!(
                "/{}",
                path.iter()
                    .map(|p| p.replace('~', "~0").replace('/', "~1"))
                    .collect::<Vec<_>>()
                    .join("/")
            );
            fields.push(Field {
                path: path.clone(),
                pointer,
                control,
            });
        }
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

fn integer_value(field: &Field, value: i32) -> Value {
    Value::from(
        if field
            .path
            .first()
            .is_some_and(|name| name == "ArrowWeights")
        {
            value.clamp(0, 100)
        } else {
            value
        },
    )
}

/// Source drag units/bounds become one keyboard nudge. Typed input and loaded
/// override sentinels remain raw; only an explicit nudge applies these bounds.
fn number_nudge(field:&Field,value:&Value)->(f64,f64,f64){
    let at=|pointer:&str|value.pointer(pointer).and_then(Value::as_f64);
    let ordered=|other:&str,is_min:bool|{
        let partner=at(&format!("/StepTightening/{other}")).filter(|v|*v>=0.0);
        if is_min{(0.01,0.0,partner.unwrap_or(10.0).min(10.0))}else{(0.01,partner.unwrap_or(0.0).max(0.0),10.0)}
    };
    match field.path.as_slice(){
        [name] if name=="BalancedBracketsPerMinuteForAggressiveBrackets"=>(0.01,at("/BalancedBracketsPerMinuteForNoBrackets").unwrap_or(0.0).max(0.0),f64::MAX),
        [name] if name=="BalancedBracketsPerMinuteForNoBrackets"=>(0.01,0.0,at("/BalancedBracketsPerMinuteForAggressiveBrackets").unwrap_or(f64::MAX).max(0.0)),
        [group,_] if group=="Facing"=>(0.0001,0.0,1.0),
        [group,_] if group=="LateralTightening"=>(0.0001,0.0,100.0),
        [group,name] if group=="Transitions"&&name=="TransitionCutoffPercentage"=>(0.0001,0.0,1.0),
        [group,name] if group=="StepTightening"=>match name.as_str(){
            "LateralMinPanelDistance"|"LongitudinalMinPanelDistance"=>(0.001,0.0,2.0),
            "SpeedTighteningMinDistance"=>(0.01,0.0,10.0),
            "DistanceMin"=>ordered("DistanceMax",true),
            "DistanceMax"=>ordered("DistanceMin",false),
            "StretchDistanceMin"=>ordered("StretchDistanceMax",true),
            "StretchDistanceMax"=>ordered("StretchDistanceMin",false),
            _=>(1.0,f64::MIN,f64::MAX),
        },
        _=>(1.0,f64::MIN,f64::MAX),
    }
}
#[cfg(test)]
mod source_numeric_tests {
    use super::*;
    use crate::generation::{config::PerformedConfig,profiles::Profile};
    fn index(editor:&Editor,group:&str,name:&str)->usize{editor.fields().iter().position(|field|field.path()==[group.to_owned(),name.to_owned()]).unwrap()}
    #[test]
    fn source_facing_lateral_distance_and_compensation_units_are_usable_and_bounded(){
        let config=Config::Performed(PerformedConfig::balanced());
        for (group,name,step)in[("Facing","InwardPercentageCutoff",0.0001),("LateralTightening","Speed",0.0001),("StepTightening","LateralMinPanelDistance",0.001),("StepTightening","DistanceMin",0.01),("StepTightening","SpeedTighteningMinDistance",0.01)]{
            let mut editor=Editor::new(&config).unwrap();let i=index(&editor,group,name);let old=editor.value(i).unwrap().as_f64().unwrap();editor.step(i,true,false).unwrap();let next=editor.value(i).unwrap().as_f64().unwrap();assert!((next-old-step).abs()<1e-12,"{group}.{name}");editor.finish().unwrap();
        }
        let mut editor=Editor::new(&config).unwrap();let max=index(&editor,"Facing","MaxInwardPercentage");editor.step(max,true,false).unwrap();assert_eq!(editor.value(max).unwrap().as_f64(),Some(1.0));editor.finish().unwrap();
        let cut=index(&editor,"Facing","InwardPercentageCutoff");editor.step(cut,true,true).unwrap();assert!((editor.value(cut).unwrap().as_f64().unwrap()-0.341).abs()<1e-12);editor.finish().unwrap();
        let min=index(&editor,"StepTightening","DistanceMin");let max=index(&editor,"StepTightening","DistanceMax");editor.set_text(min,"2.33").unwrap();editor.step(min,true,false).unwrap();assert_eq!(editor.value(min),editor.value(max));editor.finish().unwrap();
    }
    #[test]
    fn nudge_is_explicit_and_serialized_config_consumes_the_exact_source_unit(){
        let source=br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Config":{"Facing":{"InwardPercentageCutoff":0.34,"MaxInwardPercentage":1,"OutwardPercentageCutoff":0.34,"MaxOutwardPercentage":1}}}"#;
        let mut profile=Profile::parse(Kind::Performed,source).unwrap();let mut editor=Editor::new(profile.config()).unwrap();let i=index(&editor,"Facing","InwardPercentageCutoff");editor.step(i,true,false).unwrap();profile.set_config(editor.finish().unwrap()).unwrap();let reopened=Profile::parse(Kind::Performed,&profile.serialize().unwrap()).unwrap();let Config::Performed(config)=reopened.config()else{panic!()};assert!((config.facing.inward_percentage_cutoff-0.3401).abs()<1e-12);
        let mut raw=Editor::new(&Config::Performed(PerformedConfig::default())).unwrap();let field=index(&raw,"Facing","InwardPercentageCutoff");assert_eq!(raw.value(field).unwrap().as_f64(),Some(-1.0));raw.step(field,true,false).unwrap();assert_eq!(raw.value(field).unwrap().as_f64(),Some(0.0));
    }
}

fn integer_nudge(field:&Field,value:i32)->i32{
    let bounds=match field.path.as_slice(){
        [name]=>match name.as_str(){
            "SameArrowStepWeight"|"NewArrowStepWeight"|"MaxSameArrowsInARowPerFoot"=>(0,100),
            "StepTypeCheckPeriod"=>(0,512),
            "MinLevelForBrackets"=>(0,i32::MAX),
            name if name.ends_with("LaneSpecified")=>(0,super::job::TARGET_LAYOUTS.iter().filter_map(|layout|crate::notes::layout_columns(layout)).map(i32::from).max().unwrap_or(1)-1),
            _=>(i32::MIN,i32::MAX),
        },
        [group,name] if group=="Transitions"=>match name.as_str(){
            "StepsPerTransitionMin"|"StepsPerTransitionMax"=>(0,2048),
            "MinimumPadWidth"=>(0,32),
            _=>(i32::MIN,i32::MAX),
        },
        _=>(i32::MIN,i32::MAX),
    };
    value.clamp(bounds.0,bounds.1)
}
#[cfg(test)]
mod source_integer_nudge_tests{
    use super::*;
    use crate::generation::config::{PatternConfig,PerformedConfig};
    #[test]
    fn source_pattern_and_transition_nudges_apply_bounds_only_on_explicit_edit(){
        let mut pattern=Editor::new(&Config::Pattern(PatternConfig::sixteenths())).unwrap();
        for(name,max)in[("SameArrowStepWeight",100),("NewArrowStepWeight",100),("MaxSameArrowsInARowPerFoot",100),("StepTypeCheckPeriod",512)]{
            let i=pattern.fields().iter().position(|f|f.name()==name).unwrap();pattern.set_text(i,&(max+1).to_string()).unwrap();assert_eq!(pattern.value(i).unwrap().as_i64(),Some(i64::from(max+1)));pattern.step(i,true,false).unwrap();assert_eq!(pattern.value(i).unwrap().as_i64(),Some(i64::from(max)));pattern.set_text(i,"0").unwrap();pattern.step(i,false,false).unwrap();assert_eq!(pattern.value(i).unwrap().as_i64(),Some(0));
        }
        let lane=pattern.fields().iter().position(|f|f.name()=="LeftFootStartLaneSpecified").unwrap();pattern.set_text(lane,"9").unwrap();pattern.step(lane,true,false).unwrap();assert_eq!(pattern.value(lane).unwrap().as_i64(),Some(9));
        let mut movement=Editor::new(&Config::Performed(PerformedConfig::balanced())).unwrap();for(name,max)in[("StepsPerTransitionMin",2048),("StepsPerTransitionMax",2048),("MinimumPadWidth",32)]{let i=movement.fields().iter().position(|f|f.path()[0]=="Transitions"&&f.name()==name).unwrap();movement.set_text(i,&max.to_string()).unwrap();movement.step(i,true,true).unwrap();assert_eq!(movement.value(i).unwrap().as_i64(),Some(i64::from(max)));}
    }
}

#[cfg(test)]
mod source_text_bounds_tests{
    use super::*;
    use crate::generation::config::{PatternConfig,PerformedConfig};
    #[test]
    fn native_source_input_clamps_but_raw_api_keeps_staged_values(){
        let mut editor=Editor::new(&Config::Pattern(PatternConfig::sixteenths())).unwrap();
        for(name,input,expected)in[("SameArrowStepWeight","101",100),("NewArrowStepWeight","-1",0),("MaxSameArrowsInARowPerFoot","999",100),("StepTypeCheckPeriod","999",512),("LeftFootStartLaneSpecified","999",9)]{let i=editor.fields().iter().position(|f|f.name()==name).unwrap();editor.set_source_text(i,input).unwrap();assert_eq!(editor.value(i).unwrap().as_i64(),Some(expected));}
        let i=editor.fields().iter().position(|f|f.name()=="SameArrowStepWeight").unwrap();editor.set_text(i,"101").unwrap();assert_eq!(editor.value(i).unwrap().as_i64(),Some(101));
        let mut movement=Editor::new(&Config::Performed(PerformedConfig::balanced())).unwrap();let i=movement.fields().iter().position(|f|f.path()==["Facing".to_owned(),"InwardPercentageCutoff".to_owned()]).unwrap();movement.set_source_text(i,"1.34").unwrap();assert_eq!(movement.value(i).unwrap().as_f64(),Some(1.0));movement.finish().unwrap();
        let i=movement.fields().iter().position(|f|f.name()=="DistanceMin").unwrap();movement.set_source_text(i,"999").unwrap();assert_eq!(movement.value(i).unwrap().as_f64(),Some(2.333333));movement.finish().unwrap();let before=movement.value(i).cloned();assert!(movement.set_source_text(i,"NaN").is_err());assert_eq!(movement.value(i),before.as_ref());
    }
}
