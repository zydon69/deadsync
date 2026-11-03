//! Source DensityGraph preferences, separate from RSSP analysis options.
pub(crate) mod history;
use crate::Error;
pub use history::Update;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accumulation {
    Step,
    Row,
    /// Preserved source ordinal; original rebuild treats every non-Step value as Row.
    Unknown(i32),
}
impl Accumulation {
    pub fn name(self) -> String {
        match self {
            Self::Step => "Step".into(),
            Self::Row => "Row".into(),
            Self::Unknown(value) => value.to_string(),
        }
    }
    pub(crate) fn read(value: Option<&Value>) -> Self {
        let raw = value
            .and_then(Value::as_str)
            .and_then(|text| {
                let text = text.trim();
                text.parse::<i32>().ok().or_else(|| {
                    text.split(',').try_fold(0, |raw, name| {
                        ["Step", "Row"]
                            .iter()
                            .position(|n| n.eq_ignore_ascii_case(name.trim()))
                            .map(|index| raw | index as i32)
                    })
                })
            })
            .unwrap_or(0);
        match raw {
            0 => Self::Step,
            1 => Self::Row,
            value => Self::Unknown(value),
        }
    }
    pub(crate) fn validate(self) -> Result<(), Error> {
        if matches!(self, Self::Unknown(0 | 1)) {
            return Err(Error::InvalidFormat(
                "Known accumulation ordinal must use its named variant".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    ColorByDensity,
    ColorByHeight,
}
impl ColorMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::ColorByDensity => "ColorByDensity",
            Self::ColorByHeight => "ColorByHeight",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    RightSideOfWindow,
    LeftSideOfWindow,
    FocusedChartWithoutScaling,
    FocusedChartWithScaling,
    TopOfFocusedChart,
    BottomOfFocusedChart,
}
impl Position {
    pub const ALL: [Self; 6] = [
        Self::RightSideOfWindow,
        Self::LeftSideOfWindow,
        Self::FocusedChartWithoutScaling,
        Self::FocusedChartWithScaling,
        Self::TopOfFocusedChart,
        Self::BottomOfFocusedChart,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::RightSideOfWindow => "RightSideOfWindow",
            Self::LeftSideOfWindow => "LeftSideOfWindow",
            Self::FocusedChartWithoutScaling => "FocusedChartWithoutScaling",
            Self::FocusedChartWithScaling => "FocusedChartWithScaling",
            Self::TopOfFocusedChart => "TopOfFocusedChart",
            Self::BottomOfFocusedChart => "BottomOfFocusedChart",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub visible: bool,
    pub stream: bool,
    pub accumulation: Accumulation,
    pub position: Position,
    pub height: i32,
    pub color_mode: ColorMode,
    pub low: [f32; 4],
    pub high: [f32; 4],
    pub background: [f32; 4],
    pub position_offsets: [i32; 6],
    pub width_offsets: [i32; 6],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            visible: true,
            stream: true,
            accumulation: Accumulation::Step,
            position: Position::LeftSideOfWindow,
            height: 90,
            color_mode: ColorMode::ColorByHeight,
            low: [0.251, 0.647, 0.419, 1.0],
            high: [0.705, 0.282, 0.282, 1.0],
            background: [0.118, 0.118, 0.118, 1.0],
            position_offsets: [10, 10, 110, 110, 10, 10],
            width_offsets: [-10, -10, -10, -10, 0, 0],
        }
    }
}
impl Preferences {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut p = Self::default();
        for (name, v) in [
            ("ShowDensityGraph", &mut p.visible),
            ("ShowStream", &mut p.stream),
        ] {
            if let Some(value) = fields.get(name) {
                *v = value.as_bool().ok_or_else(|| invalid(name))?;
            }
        }
        p.accumulation = Accumulation::read(fields.get("AccumulationType"));
        p.color_mode = if fields
            .get("DensityGraphColorModeValue")
            .and_then(Value::as_str)
            == Some("ColorByDensity")
        {
            ColorMode::ColorByDensity
        } else {
            ColorMode::ColorByHeight
        };
        p.position = fields
            .get("DensityGraphPositionValue")
            .and_then(Value::as_str)
            .and_then(|n| Position::ALL.into_iter().find(|p| p.name() == n))
            .unwrap_or(Position::LeftSideOfWindow);
        if let Some(v) = fields.get("DensityGraphHeight") {
            p.height = v
                .as_i64()
                .and_then(|n| n.try_into().ok())
                .ok_or_else(|| invalid("DensityGraphHeight"))?;
        }
        for (name, target) in [
            ("DensityGraphPositionOffsets", &mut p.position_offsets),
            ("DensityGraphWidthOffsets", &mut p.width_offsets),
        ] {
            if let Some(v) = fields.get(name) {
                let map = v.as_object().ok_or_else(|| invalid(name))?;
                for position in Position::ALL {
                    if let Some(v) = map.get(position.name()) {
                        target[position.index()] = v
                            .as_i64()
                            .and_then(|v| v.try_into().ok())
                            .ok_or_else(|| invalid(name))?;
                    }
                }
            }
        }
        for (name, target) in [
            ("DensityGraphLowColor", &mut p.low),
            ("DensityGraphHighColor", &mut p.high),
            ("DensityGraphBackgroundColor", &mut p.background),
        ] {
            if let Some(v) = fields.get(name) {
                let map = v.as_object().ok_or_else(|| invalid(name))?;
                *target = [0.0; 4];
                for (i, key) in ["X", "Y", "Z", "W"].iter().enumerate() {
                    if let Some(v) = map.get(*key) {
                        target[i] =
                            v.as_f64()
                                .filter(|v| v.is_finite())
                                .ok_or_else(|| invalid(name))? as f32;
                    }
                }
            }
        }
        p.validate()?;
        Ok(p)
    }
    pub fn validate(self) -> Result<(), Error> {
        self.accumulation.validate()?;
        // Source public int32 fields have no PostLoad range clamp. UI drag
        // bounds must not reject imported or previously persisted values.
        for (name, color) in [
            ("DensityGraphLowColor", self.low),
            ("DensityGraphHighColor", self.high),
            ("DensityGraphBackgroundColor", self.background),
        ] {
            if color.into_iter().any(|v| !v.is_finite()) {
                return Err(invalid(name));
            }
        }
        Ok(())
    }
    pub fn write(self, fields: &mut Map<String, Value>) -> Result<(), Error> {
        self.validate()?;
        for (name, v) in [
            ("ShowDensityGraph", self.visible),
            ("ShowStream", self.stream),
        ] {
            fields.insert(name.into(), v.into());
        }
        for (name, v) in [
            ("AccumulationType", self.accumulation.name()),
            (
                "DensityGraphColorModeValue",
                self.color_mode.name().to_owned(),
            ),
            ("DensityGraphPositionValue", self.position.name().to_owned()),
        ] {
            fields.insert(name.into(), v.into());
        }
        fields.insert("DensityGraphHeight".into(), self.height.into());
        for (name, values) in [
            ("DensityGraphPositionOffsets", self.position_offsets),
            ("DensityGraphWidthOffsets", self.width_offsets),
        ] {
            let mut map = fields
                .get(name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for position in Position::ALL {
                map.insert(position.name().into(), values[position.index()].into());
            }
            fields.insert(name.into(), Value::Object(map));
        }
        for (name, values) in [
            ("DensityGraphLowColor", self.low),
            ("DensityGraphHighColor", self.high),
            ("DensityGraphBackgroundColor", self.background),
        ] {
            let mut map = fields
                .get(name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (key, value) in ["X", "Y", "Z", "W"].into_iter().zip(values) {
                map.insert(key.into(), (value as f64).into());
            }
            fields.insert(name.into(), Value::Object(map));
        }
        Ok(())
    }
}
fn invalid(name: &str) -> Error {
    Error::InvalidFormat(format!("Invalid density graph preference {name}"))
}
