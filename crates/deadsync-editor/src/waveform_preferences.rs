//! Validated native waveform controls from GrooveAuthor PreferencesWaveForm.
//! Defaults/ranges follow UIWaveFormPreferences at pinned commit 48e18f71 (MIT).
use crate::{Error, waveform_palette::Palette};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    show: bool,
    enabled: bool,
    channel_width: f64,
    dense_scale: f64,
    palette: Palette,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            show: true,
            enabled: true,
            channel_width: 0.9,
            dense_scale: 6.0,
            palette: Palette::default(),
        }
    }
}
impl Preferences {
    pub fn palette(&self) -> Palette {
        self.palette
    }
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }
    pub fn show(&self) -> bool {
        self.show
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn channel_width(&self) -> f64 {
        self.channel_width
    }
    pub fn dense_scale(&self) -> f64 {
        self.dense_scale
    }
    pub fn set_show(&mut self, value: bool) {
        self.show = value;
    }
    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }
    pub fn set_channel_width(&mut self, value: f64) -> Result<(), Error> {
        self.channel_width = valid(value, 1.0)?;
        Ok(())
    }
    pub fn set_dense_scale(&mut self, value: f64) -> Result<(), Error> {
        self.dense_scale = valid(value, 10.0)?;
        Ok(())
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut value = Self::default();
        for (key, target) in [
            ("ShowWaveForm", &mut value.show),
            ("EnableWaveForm", &mut value.enabled),
        ] {
            if let Some(field) = fields.get(key) {
                *target = field
                    .as_bool()
                    .ok_or_else(|| Error::InvalidFormat(format!("Invalid {key}")))?;
            }
        }
        if let Some(field) = fields.get("WaveFormMaxXPercentagePerChannel") {
            value.set_channel_width(
                field
                    .as_f64()
                    .ok_or_else(|| Error::InvalidFormat("Invalid waveform channel width".into()))?,
            )?;
        }
        if let Some(field) = fields.get("DenseScale") {
            value.set_dense_scale(
                field
                    .as_f64()
                    .ok_or_else(|| Error::InvalidFormat("Invalid waveform dense scale".into()))?,
            )?;
        }
        value.palette = Palette::read(fields)?;
        Ok(value)
    }
    pub(crate) fn write(&self, fields: &mut Map<String, Value>) {
        fields.insert("ShowWaveForm".into(), self.show.into());
        fields.insert("EnableWaveForm".into(), self.enabled.into());
        fields.insert(
            "WaveFormMaxXPercentagePerChannel".into(),
            self.channel_width.into(),
        );
        fields.insert("DenseScale".into(), self.dense_scale.into());
        self.palette.write(fields);
    }
}
fn valid(value: f64, max: f64) -> Result<f64, Error> {
    if value.is_finite() && (0.0..=max).contains(&value) {
        Ok(value)
    } else {
        Err(Error::InvalidEdit(format!(
            "Waveform value must be finite and between 0 and {max}"
        )))
    }
}
