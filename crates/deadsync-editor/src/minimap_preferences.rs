//! Viewport preferences from GrooveAuthor PreferencesMiniMap (48e18f71).
use crate::{Error, minimap_viewport::Axis};
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    variable_axis: Axis,
    time_range: f64,
    row_range: f64,
    quantize: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            variable_axis: Axis::Time,
            time_range: 240.0,
            row_range: 24576.0,
            quantize: false,
        }
    }
}
impl Preferences {
    pub fn variable_axis(self) -> Axis {
        self.variable_axis
    }
    pub fn set_variable_axis(&mut self, axis: Axis) {
        self.variable_axis = axis;
    }
    pub fn visible_range(self, axis: Axis) -> f64 {
        match axis {
            Axis::Time => self.time_range,
            Axis::Row => self.row_range,
        }
    }
    pub fn set_visible_range(&mut self, axis: Axis, value: f64) -> Result<(), Error> {
        if !value.is_finite() || value <= 0.0 || value > u32::MAX as f64 || value.fract() != 0.0 {
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
        // Original enum handling keeps its default for unknown variants.
        result.variable_axis = match fields
            .get("MiniMapSpacingModeForVariable")
            .and_then(Value::as_str)
        {
            Some("ConstantRow") => Axis::Row,
            _ => Axis::Time,
        };
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
            match self.variable_axis {
                Axis::Time => "ConstantTime",
                Axis::Row => "ConstantRow",
            }
            .into(),
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
