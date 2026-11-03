//! Viewport preferences from GrooveAuthor PreferencesMiniMap (48e18f71).
use crate::{Error, minimap_viewport::Axis};
use serde_json::{Map, Value};
/// Original Editor.SpacingMode preference. Only ConstantRow selects row rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariableSpacing {
    ConstantTime,
    ConstantRow,
    Variable,
    Unknown(i32),
}
impl VariableSpacing {
    pub const fn supports_navigation(self) -> bool {
        matches!(self, Self::ConstantTime | Self::ConstantRow)
    }
    pub const fn axis(self) -> Axis {
        match self {
            Self::ConstantRow => Axis::Row,
            _ => Axis::Time,
        }
    }
    fn read(raw: Option<&Value>) -> Self {
        match raw.and_then(Value::as_str) {
            Some("ConstantRow") => Self::ConstantRow,
            Some("Variable") => Self::Variable,
            Some("ConstantTime") => Self::ConstantTime,
            _ => match raw
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok())
                .or_else(|| {
                    raw.and_then(Value::as_str)
                        .and_then(|value| value.parse::<i32>().ok())
                }) {
                Some(0) => Self::ConstantTime,
                Some(1) => Self::ConstantRow,
                Some(2) => Self::Variable,
                Some(value) => Self::Unknown(value),
                None => Self::ConstantTime,
            },
        }
    }
    fn value(self) -> Value {
        match self {
            Self::ConstantTime => "ConstantTime".into(),
            Self::ConstantRow => "ConstantRow".into(),
            Self::Variable => "Variable".into(),
            Self::Unknown(value) => value.into(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    variable_spacing: VariableSpacing,
    time_range: f64,
    row_range: f64,
    quantize: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            variable_spacing: VariableSpacing::ConstantTime,
            time_range: 240.0,
            row_range: 24576.0,
            quantize: false,
        }
    }
}
impl Preferences {
    pub fn variable_axis(self) -> Axis {
        self.variable_spacing.axis()
    }
    pub fn variable_spacing(self) -> VariableSpacing {
        self.variable_spacing
    }
    pub fn set_variable_spacing(&mut self, value: VariableSpacing) {
        self.variable_spacing = value;
    }
    pub fn set_variable_axis(&mut self, axis: Axis) {
        self.variable_spacing = match axis {
            Axis::Time => VariableSpacing::ConstantTime,
            Axis::Row => VariableSpacing::ConstantRow,
        };
    }
    pub fn visible_range(self, axis: Axis) -> f64 {
        match axis {
            Axis::Time => self.time_range,
            Axis::Row => self.row_range,
        }
    }
    pub fn set_visible_range(&mut self, axis: Axis, value: f64) -> Result<(), Error> {
        if !value.is_finite() || value < 0.0 || value > u32::MAX as f64 || value.fract() != 0.0 {
            return Err(Error::InvalidEdit("Invalid minimap visible range".into()));
        }
        match axis {
            Axis::Time => self.time_range = value,
            Axis::Row => self.row_range = value,
        }
        Ok(())
    }
    pub fn quantize(self) -> bool {
        self.quantize
    }
    pub fn set_quantize(&mut self, value: bool) {
        self.quantize = value;
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut result = Self::default();
        result.variable_spacing =
            VariableSpacing::read(fields.get("MiniMapSpacingModeForVariable"));
        for (name, axis) in [
            ("MiniMapVisibleTimeRange", Axis::Time),
            ("MiniMapVisibleRowRange", Axis::Row),
        ] {
            if let Some(value) = fields.get(name) {
                result.set_visible_range(
                    axis,
                    value
                        .as_f64()
                        .ok_or_else(|| Error::InvalidFormat(format!("Invalid {name}")))?,
                )?;
            }
        }
        if let Some(value) = fields.get("QuantizePositions") {
            result.quantize = value
                .as_bool()
                .ok_or_else(|| Error::InvalidFormat("Invalid QuantizePositions".into()))?;
        }
        Ok(result)
    }
    pub(crate) fn write(self, fields: &mut Map<String, Value>) {
        fields.insert(
            "MiniMapSpacingModeForVariable".into(),
            match self.variable_spacing {
                VariableSpacing::Unknown(n) => crate::preference_enum::unknown_ordinal(
                    n,
                    fields.get("MiniMapSpacingModeForVariable"),
                ),
                known => known.value(),
            },
        );
        fields.insert(
            "MiniMapVisibleTimeRange".into(),
            (self.time_range as u32).into(),
        );
        fields.insert(
            "MiniMapVisibleRowRange".into(),
            (self.row_range as u32).into(),
        );
        fields.insert("QuantizePositions".into(), self.quantize.into());
    }
}

#[cfg(test)]
mod raw_source_tests {
    use super::*;
    #[test]
    fn raw_enum_and_zero_are_distinct_from_effective_axis_and_ui_limits() {
        for spacing in [
            VariableSpacing::ConstantTime,
            VariableSpacing::ConstantRow,
            VariableSpacing::Variable,
            VariableSpacing::Unknown(99),
        ] {
            let mut p = Preferences::default();
            p.set_variable_spacing(spacing);
            p.set_visible_range(Axis::Time, 0.0).unwrap();
            p.set_visible_range(Axis::Row, 0.0).unwrap();
            assert_eq!(
                p.variable_axis(),
                if spacing == VariableSpacing::ConstantRow {
                    Axis::Row
                } else {
                    Axis::Time
                }
            );
            assert_eq!(
                spacing.supports_navigation(),
                matches!(
                    spacing,
                    VariableSpacing::ConstantTime | VariableSpacing::ConstantRow
                )
            );
            let mut fields = Map::new();
            p.write(&mut fields);
            assert_eq!(Preferences::read(&fields).unwrap(), p);
        }
        let mut p = Preferences::default();
        p.set_variable_spacing(VariableSpacing::Variable);
        assert_ne!(p, Preferences::default());
        assert_eq!(p.variable_axis(), Preferences::default().variable_axis());
    }
}
