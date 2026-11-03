//! Source general Options policies shared by rendering and region selection.
use crate::Error;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderPolicy {
    pub notes: bool,
    pub markers: bool,
    pub regions: bool,
    pub misc_events: bool,
}
impl Default for RenderPolicy {
    fn default() -> Self {
        Self {
            notes: true,
            markers: true,
            regions: true,
            misc_events: true,
        }
    }
}
impl RenderPolicy {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut result = Self::default();
        for (key, target) in [
            ("RenderNotes", &mut result.notes),
            ("RenderMarkers", &mut result.markers),
            ("RenderRegions", &mut result.regions),
            ("RenderMiscEvents", &mut result.misc_events),
        ] {
            if let Some(value) = fields.get(key) {
                *target = value
                    .as_bool()
                    .ok_or_else(|| Error::InvalidFormat(format!("Invalid {key}")))?;
            }
        }
        Ok(result)
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        for (key, value) in [
            ("RenderNotes", self.notes),
            ("RenderMarkers", self.markers),
            ("RenderRegions", self.regions),
            ("RenderMiscEvents", self.misc_events),
        ] {
            fields.insert(key.into(), value.into());
        }
    }
}
/// Original hidden Options budgets. Raw signed values remain representable;
/// draw loops interpret nonpositive budgets as no emitted items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderLimits {
    pub max_markers: i32,
    pub max_events: i32,
    pub max_rate_events: i32,
}
impl Default for RenderLimits {
    fn default() -> Self {
        Self {
            max_markers: 256,
            max_events: 2048,
            max_rate_events: 256,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawBudget {
    pub markers: i32,
    pub events: i32,
    pub rate_events: i32,
}
impl RenderLimits {
    /// ActiveEditorChart.GetMaxEventsToDrawPerFrame: half scaled by authored
    /// count, half divided evenly among visible charts; original truncation.
    pub fn per_chart(
        self,
        visible: i32,
        events: i32,
        total_events: i32,
        rates: i32,
        total_rates: i32,
    ) -> DrawBudget {
        let visible = visible.max(1);
        let total_events = total_events.max(1);
        let total_rates = total_rates.max(1);
        DrawBudget {
            markers: self.max_markers / visible,
            events: (f64::from(events) / f64::from(total_events) * 0.5 * f64::from(self.max_events)
                + 1.0 / f64::from(visible) * 0.5 * f64::from(self.max_events))
                as i32,
            rate_events: (f64::from(rates) / f64::from(total_rates)
                * 0.5
                * f64::from(self.max_rate_events)
                + 1.0 / f64::from(visible) * 0.5 * f64::from(self.max_rate_events))
                as i32,
        }
    }
}
impl RenderLimits {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut limits = Self::default();
        for (key, target) in [
            ("MaxMarkersToDraw", &mut limits.max_markers),
            ("MaxEventsToDraw", &mut limits.max_events),
            (
                "MaxRateAlteringEventsToProcessPerFrame",
                &mut limits.max_rate_events,
            ),
        ] {
            if let Some(value) = fields.get(key) {
                *target = value
                    .as_i64()
                    .and_then(|v| v.try_into().ok())
                    .ok_or_else(|| Error::InvalidFormat(format!("Invalid {key}")))?;
            }
        }
        Ok(limits)
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        for (key, value) in [
            ("MaxMarkersToDraw", self.max_markers),
            ("MaxEventsToDraw", self.max_events),
            (
                "MaxRateAlteringEventsToProcessPerFrame",
                self.max_rate_events,
            ),
        ] {
            fields.insert(key.into(), value.into());
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackgroundSize {
    #[default]
    ChartArea,
    Window,
    Unknown(i32),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackgroundPolicy {
    pub hide: bool,
    pub size: BackgroundSize,
}
impl BackgroundPolicy {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let hide = fields
            .get("HideSongBackground")
            .map(|v| {
                v.as_bool()
                    .ok_or_else(|| Error::InvalidFormat("Invalid HideSongBackground".into()))
            })
            .transpose()?
            .unwrap_or(false);
        let size = match fields.get("BackgroundImageSize") {
            None => BackgroundSize::ChartArea,
            Some(Value::String(value)) => {
                let value = value.trim();
                let parsed = if let Ok(value) = value.parse::<i32>() {
                    Some(value)
                } else {
                    let mut result = 0i32;
                    let mut valid = true;
                    for member in value.split(',') {
                        if member.trim().eq_ignore_ascii_case("Window") {
                            result |= 1;
                        } else if !member.trim().eq_ignore_ascii_case("ChartArea") {
                            valid = false;
                            break;
                        }
                    }
                    valid.then_some(result)
                };
                match parsed {
                    Some(1) => BackgroundSize::Window,
                    Some(0) | None => BackgroundSize::ChartArea,
                    Some(other) => BackgroundSize::Unknown(other),
                }
            }
            Some(Value::Array(_) | Value::Object(_)) => {
                return Err(Error::InvalidFormat("Invalid BackgroundImageSize".into()));
            }
            // Actual permissive converter returns enum zero for scalar nonstrings.
            Some(_) => BackgroundSize::ChartArea,
        };
        Ok(Self { hide, size })
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        fields.insert("HideSongBackground".into(), self.hide.into());
        let value = match self.size {
            BackgroundSize::ChartArea => "ChartArea".to_owned(),
            BackgroundSize::Window => "Window".to_owned(),
            BackgroundSize::Unknown(value) => value.to_string(),
        };
        fields.insert("BackgroundImageSize".into(), value.into());
    }
}
/// One owner for the ten source display Options, shared by all native consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Display {
    pub render: RenderPolicy,
    pub limits: RenderLimits,
    pub background: BackgroundPolicy,
    pub misc_width: i32,
}
impl Default for Display {
    fn default() -> Self {
        Self {
            render: Default::default(),
            limits: Default::default(),
            background: Default::default(),
            misc_width: 100,
        }
    }
}
impl Display {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        Ok(Self {
            render: RenderPolicy::read(fields)?,
            limits: RenderLimits::read(fields)?,
            background: BackgroundPolicy::read(fields)?,
            misc_width: fields
                .get("MiscEventAreaWidth")
                .map(|v| {
                    v.as_i64()
                        .and_then(|v| v.try_into().ok())
                        .ok_or_else(|| Error::InvalidFormat("Invalid MiscEventAreaWidth".into()))
                })
                .transpose()?
                .unwrap_or(100),
        })
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        self.render.write(fields);
        self.limits.write(fields);
        self.background.write(fields);
        fields.insert("MiscEventAreaWidth".into(), self.misc_width.into());
    }
}
impl Display {
    /// Source general Restore does not touch the hidden rendering budgets.
    pub fn restore_defaults(self) -> Self {
        Self {
            limits: self.limits,
            ..Self::default()
        }
    }
}

pub(crate) mod history;
pub use history::Update;

pub(crate) mod general;
pub use general::{General, Update as GeneralUpdate};

pub mod startup;

pub mod restore;

pub mod dpi;
