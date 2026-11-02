//! Source Audio storage and explicit effective playback values.
use super::{Audio, Error};
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    MusicRate,
    AudioOffset,
    MainVolume,
    MusicVolume,
    AssistTickVolume,
    AssistTickAttackTime,
    UseAssistTick,
    SkipAssistTickOnBeatTick,
    BeatTickVolume,
    BeatTickAttackTime,
    UseBeatTick,
    SkipBeatTickOnAssistTick,
    DspBufferSize,
    DspNumBuffers,
    PreviewFadeInTime,
    PreviewFadeOutTime,
}
impl Field {
    pub const ALL: [Self; 16] = [
        Self::MusicRate,
        Self::MainVolume,
        Self::MusicVolume,
        Self::PreviewFadeInTime,
        Self::PreviewFadeOutTime,
        Self::UseAssistTick,
        Self::SkipAssistTickOnBeatTick,
        Self::AssistTickVolume,
        Self::AssistTickAttackTime,
        Self::UseBeatTick,
        Self::SkipBeatTickOnAssistTick,
        Self::BeatTickVolume,
        Self::BeatTickAttackTime,
        Self::AudioOffset,
        Self::DspNumBuffers,
        Self::DspBufferSize,
    ];
    pub const fn key(self) -> &'static str {
        match self {
            Self::MusicRate => "MusicRate",
            Self::AudioOffset => "AudioOffset",
            Self::MainVolume => "MainVolume",
            Self::MusicVolume => "MusicVolume",
            Self::AssistTickVolume => "AssistTickVolume",
            Self::AssistTickAttackTime => "AssistTickAttackTime",
            Self::UseAssistTick => "UseAssistTick",
            Self::SkipAssistTickOnBeatTick => "SkipAssistTickOnBeatTick",
            Self::BeatTickVolume => "BeatTickVolume",
            Self::BeatTickAttackTime => "BeatTickAttackTime",
            Self::UseBeatTick => "UseBeatTick",
            Self::SkipBeatTickOnAssistTick => "SkipBeatTickOnAssistTick",
            Self::DspBufferSize => "DspBufferSize",
            Self::DspNumBuffers => "DspNumBuffers",
            Self::PreviewFadeInTime => "PreviewFadeInTime",
            Self::PreviewFadeOutTime => "PreviewFadeOutTime",
        }
    }
    pub fn is_dsp(self) -> bool {
        matches!(self, Self::DspBufferSize | Self::DspNumBuffers)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldValue {
    Float(f64),
    Bool(bool),
    Int(i32),
}
/// Independent source mixer controls. Main is applied after summing Main voices;
/// Preview intentionally uses the combined effective gain instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixGains {
    pub main: f32,
    pub music: f32,
    pub assist: f32,
    pub beat: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectiveAudio {
    pub rate: f64,
    pub offset: f64,
    pub music_gain: f32,
    pub assist_gain: f32,
    pub beat_gain: f32,
    pub assist_attack: f32,
    pub beat_attack: f32,
    pub assist_enabled: bool,
    pub beat_enabled: bool,
    pub assist_skip_beat: bool,
    pub beat_skip_assist: bool,
    pub fade_in: f64,
    pub fade_out: f64,
}
impl Audio {
    /// PreferencesAudio.IsUsingDefaults uses source equality tolerances.
    /// Preserve a no-op Restore rather than creating a spurious history entry.
    pub fn restore_source_defaults(&mut self) -> bool {
        let defaults = Self::default();
        let same =
            Field::ALL
                .into_iter()
                .all(|field| match (self.value(field), defaults.value(field)) {
                    (FieldValue::Float(a), FieldValue::Float(b)) => {
                        if matches!(
                            field,
                            Field::MainVolume
                                | Field::MusicVolume
                                | Field::AssistTickVolume
                                | Field::AssistTickAttackTime
                                | Field::BeatTickVolume
                                | Field::BeatTickAttackTime
                        ) {
                            float_equal(a as f32, b as f32)
                        } else {
                            crate::number::reference_equal(a, b)
                        }
                    }
                    (a, b) => a == b,
                });
        if same {
            return false;
        }
        *self = defaults;
        true
    }

    pub fn value(self, f: Field) -> FieldValue {
        use Field::*;
        match f {
            MusicRate => FieldValue::Float(self.music_rate),
            AudioOffset => FieldValue::Float(self.offset),
            MainVolume => FieldValue::Float(self.main_volume.into()),
            MusicVolume => FieldValue::Float(self.music_volume.into()),
            AssistTickVolume => FieldValue::Float(self.assist_volume.into()),
            AssistTickAttackTime => FieldValue::Float(self.assist_attack.into()),
            UseAssistTick => FieldValue::Bool(self.assist_enabled),
            SkipAssistTickOnBeatTick => FieldValue::Bool(self.assist_skip_beat),
            BeatTickVolume => FieldValue::Float(self.beat_volume.into()),
            BeatTickAttackTime => FieldValue::Float(self.beat_attack.into()),
            UseBeatTick => FieldValue::Bool(self.beat_enabled),
            SkipBeatTickOnAssistTick => FieldValue::Bool(self.beat_skip_assist),
            DspBufferSize => FieldValue::Int(self.dsp_size),
            DspNumBuffers => FieldValue::Int(self.dsp_buffers),
            PreviewFadeInTime => FieldValue::Float(self.fade_in),
            PreviewFadeOutTime => FieldValue::Float(self.fade_out),
        }
    }
    pub fn mix_gains(self) -> MixGains {
        MixGains {
            main: self.main_volume.clamp(0.0, 1.0),
            music: self.music_volume.clamp(0.0, 1.0),
            assist: self.assist_volume.clamp(0.0, 1.0),
            beat: self.beat_volume.clamp(0.0, 1.0),
        }
    }
    pub fn effective(self) -> EffectiveAudio {
        let gains = self.mix_gains();
        EffectiveAudio {
            rate: self.music_rate.clamp(0.25, 4.0),
            offset: self.offset,
            music_gain: gains.main * gains.music,
            assist_gain: gains.main * gains.assist,
            beat_gain: gains.main * gains.beat,
            assist_attack: self.assist_attack.max(0.0),
            beat_attack: self.beat_attack.max(0.0),
            assist_enabled: self.assist_enabled,
            beat_enabled: self.beat_enabled,
            assist_skip_beat: self.assist_skip_beat,
            beat_skip_assist: self.beat_skip_assist,
            fade_in: self.fade_in,
            fade_out: self.fade_out,
        }
    }
    pub fn set_field(&mut self, field: Field, value: FieldValue) -> Result<(), Error> {
        let mut object = Map::new();
        self.write(&mut object);
        let v = match value {
            FieldValue::Float(v) if v.is_finite() => {
                let v = match field {
                    Field::MusicRate => v.clamp(0.25, 4.0),
                    Field::MainVolume
                    | Field::MusicVolume
                    | Field::AssistTickVolume
                    | Field::BeatTickVolume => v.clamp(0.0, 1.0),
                    Field::AudioOffset => v,
                    _ => v.max(0.0),
                };
                Value::from(v)
            }
            FieldValue::Bool(v) => Value::from(v),
            FieldValue::Int(v) => Value::from(match field {
                Field::DspBufferSize => v.clamp(32, 8192),
                Field::DspNumBuffers => v.clamp(2, 32),
                _ => v,
            }),
            _ => return Err(invalid()),
        };
        object.insert(field.key().into(), v);
        let candidate = Self::read(&object)?;
        if let (FieldValue::Float(before), FieldValue::Float(after)) =
            (self.value(field), candidate.value(field))
        {
            let equal = if matches!(
                field,
                Field::MainVolume
                    | Field::MusicVolume
                    | Field::AssistTickVolume
                    | Field::AssistTickAttackTime
                    | Field::BeatTickVolume
                    | Field::BeatTickAttackTime
            ) {
                float_equal(before as f32, after as f32)
            } else {
                crate::number::reference_equal(before, after)
            };
            if equal {
                return Ok(());
            }
        }
        *self = candidate;
        Ok(())
    }
    pub(crate) fn read(f: &Map<String, Value>) -> Result<Self, Error> {
        let mut p = Self::default();
        for field in Field::ALL {
            let Some(v) = f.get(field.key()) else {
                continue;
            };
            use Field::*;
            match field {
                MusicRate | AudioOffset | PreviewFadeInTime | PreviewFadeOutTime => {
                    let n = v.as_f64().filter(|v| v.is_finite()).ok_or_else(invalid)?;
                    if matches!(field, MusicRate | AudioOffset) {
                        if let FieldValue::Float(current) = p.value(field) {
                            if crate::number::reference_equal(current, n) {
                                continue;
                            }
                        }
                    }
                    match field {
                        MusicRate => p.music_rate = n,
                        AudioOffset => p.offset = n,
                        PreviewFadeInTime => p.fade_in = n,
                        _ => p.fade_out = n,
                    }
                }
                MainVolume | MusicVolume | AssistTickVolume | AssistTickAttackTime
                | BeatTickVolume | BeatTickAttackTime => {
                    let n = v.as_f64().ok_or_else(invalid)? as f32;
                    if !n.is_finite() {
                        return Err(invalid());
                    }
                    if let FieldValue::Float(current) = p.value(field) {
                        if float_equal(current as f32, n) {
                            continue;
                        }
                    }
                    match field {
                        MainVolume => p.main_volume = n,
                        MusicVolume => p.music_volume = n,
                        AssistTickVolume => p.assist_volume = n,
                        AssistTickAttackTime => p.assist_attack = n,
                        BeatTickVolume => p.beat_volume = n,
                        _ => p.beat_attack = n,
                    }
                }
                UseAssistTick
                | SkipAssistTickOnBeatTick
                | UseBeatTick
                | SkipBeatTickOnAssistTick => {
                    let n = v.as_bool().ok_or_else(invalid)?;
                    match field {
                        UseAssistTick => p.assist_enabled = n,
                        SkipAssistTickOnBeatTick => p.assist_skip_beat = n,
                        UseBeatTick => p.beat_enabled = n,
                        _ => p.beat_skip_assist = n,
                    }
                }
                DspBufferSize | DspNumBuffers => {
                    let n = v
                        .as_i64()
                        .and_then(|n| n.try_into().ok())
                        .ok_or_else(invalid)?;
                    if field == DspBufferSize {
                        p.dsp_size = n
                    } else {
                        p.dsp_buffers = n
                    }
                }
            }
        }
        Ok(p)
    }
    pub(crate) fn write(self, f: &mut Map<String, Value>) {
        for field in Field::ALL {
            let value = match self.value(field) {
                FieldValue::Float(v) => Value::from(v),
                FieldValue::Bool(v) => Value::from(v),
                FieldValue::Int(v) => Value::from(v),
            };
            f.insert(field.key().into(), value);
        }
    }
}
fn invalid() -> Error {
    Error::InvalidFormat("Invalid Audio preference field".into())
}

fn float_equal(a: f32, b: f32) -> bool {
    a - 0.0000001_f32 <= b && a + 0.0000001_f32 >= b
}

#[cfg(test)]
mod source_defaults_tests {
    use super::*;
    #[test]
    fn restore_is_a_no_op_with_source_double_tolerance() {
        let mut values = Map::new();
        values.insert(
            "PreviewFadeOutTime".into(),
            Value::from(f64::from_bits(1.5_f64.to_bits() + 1)),
        );
        let mut audio = Audio::read(&values).unwrap();
        let before = audio;
        assert!(!audio.restore_source_defaults());
        assert_eq!(audio, before);
        audio
            .set_field(Field::UseBeatTick, FieldValue::Bool(true))
            .unwrap();
        assert!(audio.restore_source_defaults());
        assert_eq!(audio, Audio::default());
        assert!(!audio.restore_source_defaults());
    }
    #[test]
    fn restore_respects_exact_boolean_and_integer_defaults() {
        for field in Field::ALL {
            let mut audio = Audio::default();
            let value = match audio.value(field) {
                FieldValue::Bool(v) => FieldValue::Bool(!v),
                FieldValue::Int(v) => FieldValue::Int(v + 1),
                FieldValue::Float(v) => FieldValue::Float(if v >= 1.0 { v - 0.1 } else { v + 0.1 }),
            };
            audio.set_field(field, value).unwrap();
            assert!(audio.restore_source_defaults(), "{field:?}");
            assert_eq!(audio, Audio::default());
        }
    }
}

#[cfg(test)]
mod signed_source_fades_tests {
    use super::*;
    #[test]
    fn imported_signed_fades_reach_consumer_but_ui_edits_keep_nonnegative_bounds() {
        let mut fields = Map::new();
        fields.insert("PreviewFadeInTime".into(), Value::from(-0.01));
        fields.insert("PreviewFadeOutTime".into(), Value::from(-0.02));
        let mut audio = Audio::read(&fields).unwrap();
        assert_eq!(audio.effective().fade_in, -0.01);
        assert_eq!(audio.effective().fade_out, -0.02);
        let mut encoded = Map::new();
        audio.write(&mut encoded);
        assert_eq!(Audio::read(&encoded).unwrap(), audio);
        audio
            .set_field(Field::PreviewFadeInTime, FieldValue::Float(-0.5))
            .unwrap();
        assert_eq!(audio.effective().fade_in, 0.);
        assert!(audio.set_fade_out(-0.5).is_err());
        assert!(audio.restore_source_defaults());
        assert_eq!(audio, Audio::default());
    }
}

#[cfg(test)]
mod mix_gain_tests {
    use super::*;
    #[test]
    fn source_bus_controls_stay_independent_while_preview_gain_keeps_its_order() {
        let mut audio = Audio::default();
        for (field, value) in [
            (Field::MainVolume, 0.625),
            (Field::MusicVolume, 0.375),
            (Field::AssistTickVolume, 0.5),
            (Field::BeatTickVolume, 0.75),
        ] {
            audio.set_field(field, FieldValue::Float(value)).unwrap();
        }
        assert_eq!(
            audio.mix_gains(),
            MixGains {
                main: 0.625,
                music: 0.375,
                assist: 0.5,
                beat: 0.75
            }
        );
        let preview = audio.effective();
        assert_eq!(preview.music_gain.to_bits(), (0.625f32 * 0.375).to_bits());
        assert_eq!(preview.assist_gain.to_bits(), (0.625f32 * 0.5).to_bits());
        assert_eq!(preview.beat_gain.to_bits(), (0.625f32 * 0.75).to_bits());
    }
}
