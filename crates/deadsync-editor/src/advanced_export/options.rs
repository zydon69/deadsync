//! Six original top-level GA save preferences; unknown fields remain caller-owned.
use crate::Error;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub require_identical_timing_in_sm: bool,
    pub omit_chart_timing: bool,
    pub omit_custom_data: bool,
    pub anonymize: bool,
    pub stepf2_pump_multiplayer: bool,
    pub outfox_smx: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            require_identical_timing_in_sm: true,
            omit_chart_timing: false,
            omit_custom_data: false,
            anonymize: false,
            stepf2_pump_multiplayer: false,
            outfox_smx: true,
        }
    }
}
impl Options {
    pub const NAMES: [&'static str; 6] = [
        "RequireIdenticalTimingInSmFiles",
        "OmitChartTimingData",
        "OmitCustomSaveData",
        "AnonymizeSaveData",
        "UseStepF2ForPumpRoutine",
        "UseOutfoxFormatForSmx",
    ];
    pub fn read(root: &Map<String, Value>) -> Result<Self, Error> {
        let mut values = [true, false, false, false, false, true];
        for (index, name) in Self::NAMES.iter().enumerate() {
            if let Some(value) = root.get(*name) {
                values[index] = value
                    .as_bool()
                    .ok_or_else(|| Error::InvalidFormat(format!("{name} must be a boolean")))?;
            }
        }
        Ok(Self {
            require_identical_timing_in_sm: values[0],
            omit_chart_timing: values[1],
            omit_custom_data: values[2],
            anonymize: values[3],
            stepf2_pump_multiplayer: values[4],
            outfox_smx: values[5],
        })
    }
    /// Update only the six owned keys, preserving every unrelated preference.
    pub fn write(self, root: &mut Map<String, Value>) {
        for (name, value) in Self::NAMES.into_iter().zip([
            self.require_identical_timing_in_sm,
            self.omit_chart_timing,
            self.omit_custom_data,
            self.anonymize,
            self.stepf2_pump_multiplayer,
            self.outfox_smx,
        ]) {
            root.insert(name.into(), Value::Bool(value));
        }
    }
}
