//! Typed controls for every serialized generation configuration leaf. Drafts may
//! temporarily violate cross-field rules; finish validates the complete config.
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
        let value = config.json()?;
        let mut fields = Vec::new();
        collect(&value, &mut Vec::new(), &mut fields)?;
        // Preserve the established compact interpretation panel's field order.
        if config.kind() == Kind::Expressed {
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
        if config.kind() == Kind::Pattern {
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
        if config.kind() == Kind::Performed {
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
            kind: config.kind(),
            value,
            fields,
        })
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
            Control::Integer => Value::from(
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
            Control::Integer => Value::from(
                (value
                    .as_i64()
                    .ok_or_else(|| invalid("Invalid integer profile field"))?
                    as i32)
                    .saturating_add(if forward {
                        if large { 10 } else { 1 }
                    } else if large {
                        -10
                    } else {
                        -1
                    }),
            ),
            Control::Number => {
                let value = value
                    .as_f64()
                    .ok_or_else(|| invalid("Invalid numeric profile field"))?
                    + if forward {
                        if large { 10.0 } else { 1.0 }
                    } else if large {
                        -10.0
                    } else {
                        -1.0
                    };
                if !value.is_finite() {
                    return Err(invalid("Profile value overflow"));
                }
                Value::from(value)
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
                    Value::Bool(_) => Control::Boolean,
                    Value::Null if name.ends_with("Enabled") => Control::OptionalBoolean,
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
