//! GrooveAuthor waveform recolouring rules (Editor.PreDrawWaveFormToRenderTarget).
use crate::Error;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color([f32; 4]);
impl Color {
    pub fn new(rgba: [f32; 4]) -> Result<Self, Error> {
        if rgba.iter().all(|v| v.is_finite()) {
            Ok(Self(rgba))
        } else {
            Err(Error::InvalidEdit(
                "Waveform RGBA components must be finite".into(),
            ))
        }
    }
    pub fn rgba(self) -> [f32; 4] {
        self.0
    }
    /// Clamping belongs to the source normalized render attachment, not persisted values.
    pub fn unorm_rgba(self) -> [f32; 4] {
        self.0.map(|v| v.clamp(0.0, 1.0))
    }
    pub fn with_component(self, index: usize, value: f32) -> Result<Self, Error> {
        let mut rgba = self.0;
        let target = rgba
            .get_mut(index)
            .ok_or_else(|| Error::InvalidEdit("Invalid RGBA component".into()))?;
        *target = value;
        Self::new(rgba)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SparseMode {
    DarkerDenseColor,
    SameAsDenseColor,
    UniqueColor,
    Unknown(i32),
}
impl SparseMode {
    pub const ALL: [Self; 3] = [
        Self::DarkerDenseColor,
        Self::SameAsDenseColor,
        Self::UniqueColor,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::DarkerDenseColor => "DarkerDenseColor",
            Self::SameAsDenseColor => "SameAsDenseColor",
            Self::UniqueColor => "UniqueColor",
            Self::Unknown(_) => "Unknown",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    background: Color,
    dense: Color,
    sparse: Color,
    mode: SparseMode,
    darken: f32,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            background: Color([0.0; 4]),
            dense: Color([0.0, 0.389, 0.183, 0.8]),
            sparse: Color([0.0, 0.350, 0.164, 0.8]),
            mode: SparseMode::DarkerDenseColor,
            darken: 0.8,
        }
    }
}
impl Palette {
    pub fn background(self) -> Color {
        self.background
    }
    pub fn dense(self) -> Color {
        self.dense
    }
    /// Retained custom colour even while another sparse mode is selected.
    pub fn sparse(self) -> Color {
        self.sparse
    }
    pub fn mode(self) -> SparseMode {
        self.mode
    }
    pub fn darken(self) -> f32 {
        self.darken
    }
    pub fn set_background(&mut self, value: Color) {
        self.background = value;
    }
    pub fn set_dense(&mut self, value: Color) {
        self.dense = value;
    }
    pub fn set_sparse(&mut self, value: Color) {
        self.sparse = value;
    }
    pub fn set_mode(&mut self, value: SparseMode) {
        self.mode = value;
    }
    pub fn set_darken(&mut self, value: f32) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(Error::InvalidEdit(
                "Waveform sparse colour scale must be finite".into(),
            ));
        }
        self.darken = value;
        Ok(())
    }
    pub fn resolved_sparse(self) -> Color {
        match self.mode {
            SparseMode::DarkerDenseColor => {
                let [r, g, b, a] = self.dense.0;
                Color([r * self.darken, g * self.darken, b * self.darken, a])
            }
            SparseMode::SameAsDenseColor => self.dense,
            SparseMode::UniqueColor | SparseMode::Unknown(_) => self.sparse,
        }
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut value = Self::default();
        for (name, target) in [
            ("WaveFormBackgroundColor", &mut value.background),
            ("WaveFormDenseColor", &mut value.dense),
            ("WaveFormSparseColor", &mut value.sparse),
        ] {
            if let Some(field) = fields.get(name) {
                *target = read_color(field)?;
            }
        }
        if let Some(field) = fields.get("WaveFormSparseColorOption") {
            let raw = field
                .as_str()
                .and_then(|value| value.parse::<i32>().ok())
                .or_else(|| field.as_i64().and_then(|value| i32::try_from(value).ok()));
            value.mode = match raw {
                Some(0) => SparseMode::DarkerDenseColor,
                Some(1) => SparseMode::SameAsDenseColor,
                Some(2) => SparseMode::UniqueColor,
                Some(value) => SparseMode::Unknown(value),
                None => SparseMode::ALL
                    .into_iter()
                    .find(|mode| Some(mode.name()) == field.as_str())
                    .ok_or_else(|| {
                        Error::InvalidFormat("Unknown waveform sparse colour mode".into())
                    })?,
            };
        }
        if let Some(field) = fields.get("WaveFormSparseColorScale") {
            value.set_darken(component(field)?)?;
        }
        Ok(value)
    }
    pub(crate) fn write(self, fields: &mut Map<String, Value>) {
        for (name, color) in [
            ("WaveFormBackgroundColor", self.background),
            ("WaveFormDenseColor", self.dense),
            ("WaveFormSparseColor", self.sparse),
        ] {
            let mut rgba = fields
                .get(name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (key, value) in ["X", "Y", "Z", "W"].into_iter().zip(color.0) {
                rgba.insert(key.into(), value.into());
            }
            fields.insert(name.into(), Value::Object(rgba));
        }
        fields.insert(
            "WaveFormSparseColorOption".into(),
            match self.mode {
                SparseMode::Unknown(value) => crate::preference_enum::unknown_ordinal(
                    value,
                    fields.get("WaveFormSparseColorOption"),
                ),
                mode => mode.name().into(),
            },
        );
        fields.insert("WaveFormSparseColorScale".into(), self.darken.into());
    }
}
fn read_color(value: &Value) -> Result<Color, Error> {
    let invalid =
        || Error::InvalidFormat("Waveform colour must contain four numeric RGBA components".into());
    let mut rgba = [0.0; 4];
    if let Some(array) = value.as_array() {
        if array.len() != 4 {
            return Err(invalid());
        }
        for (target, field) in rgba.iter_mut().zip(array) {
            *target = component(field)?;
        }
    } else if let Some(object) = value.as_object() {
        for (target, key) in rgba.iter_mut().zip(["X", "Y", "Z", "W"]) {
            *target = component(object.get(key).ok_or_else(invalid)?)?;
        }
    } else {
        return Err(invalid());
    }
    Color::new(rgba)
}

fn component(value: &Value) -> Result<f32, Error> {
    let n = value
        .as_f64()
        .ok_or_else(|| Error::InvalidFormat("Invalid waveform colour component".into()))?;
    if !n.is_finite() || !(n as f32).is_finite() {
        return Err(Error::InvalidFormat(
            "Waveform colour components must be finite".into(),
        ));
    }
    Ok(n as f32)
}

#[cfg(test)]
mod source_public_color_tests {
    use super::*;
    #[test]
    fn public_raw_colors_and_scale_survive_and_unknown_sparse_uses_custom() {
        for value in [-4.0, 0.0, 2.0, 512.0] {
            let mut p = Palette::default();
            let dense = Color::new([value, 0.25, 0.5, value]).unwrap();
            let custom = Color::new([0.25, 2.0, -1.0, 0.5]).unwrap();
            p.set_dense(dense);
            p.set_sparse(custom);
            p.set_darken(value).unwrap();
            p.set_mode(SparseMode::Unknown(99));
            assert_eq!(p.resolved_sparse(), custom);
            assert_eq!(custom.unorm_rgba(), [0.25, 1.0, 0.0, 0.5]);
            let mut fields = Map::new();
            p.write(&mut fields);
            assert_eq!(fields["WaveFormSparseColorOption"], "99");
            assert_eq!(Palette::read(&fields).unwrap(), p);
            fields.insert("WaveFormSparseColorOption".into(), "99".into());
            assert_eq!(Palette::read(&fields).unwrap(), p);
        }
        let mut p = Palette::default();
        p.set_dense(Color::new([f32::MAX, -f32::MAX, 0.0, 2.0]).unwrap());
        p.set_darken(f32::MAX).unwrap();
        assert_eq!(p.resolved_sparse().unorm_rgba(), [1.0, 0.0, 0.0, 1.0]);
    }
}
