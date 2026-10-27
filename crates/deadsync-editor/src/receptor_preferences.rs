//! PreferencesReceptors source effects, shared by autoplay policy and renderer.
//! Position/drag/window controls are preserved as unknown fields, not claimed supported.
pub(crate) mod history;
use crate::{Error, receptors};
pub use history::Update;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub hide_arrows: bool,
    pub light_holds: bool,
    pub animation: receptors::Preferences,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            hide_arrows: true,
            light_holds: true,
            animation: Default::default(),
        }
    }
}
impl Preferences {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut p = Self::default();
        for (name, target) in [
            ("AutoPlayHideArrows", &mut p.hide_arrows),
            ("AutoPlayLightHolds", &mut p.light_holds),
            ("AutoPlayRimEffect", &mut p.animation.autoplay_rim),
            ("AutoPlayGlowEffect", &mut p.animation.autoplay_glow),
            ("AutoPlayShrinkEffect", &mut p.animation.autoplay_shrink),
            ("TapRimEffect", &mut p.animation.tap_rim),
            ("TapShrinkEffect", &mut p.animation.tap_shrink),
            ("PulseReceptorsWithTempo", &mut p.animation.pulse),
        ] {
            if let Some(value) = fields.get(name) {
                *target = value.as_bool().ok_or_else(|| {
                    Error::InvalidFormat(format!("Invalid receptor preference {name}"))
                })?;
            }
        }
        Ok(p)
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        for (name, value) in [
            ("AutoPlayHideArrows", self.hide_arrows),
            ("AutoPlayLightHolds", self.light_holds),
            ("AutoPlayRimEffect", self.animation.autoplay_rim),
            ("AutoPlayGlowEffect", self.animation.autoplay_glow),
            ("AutoPlayShrinkEffect", self.animation.autoplay_shrink),
            ("TapRimEffect", self.animation.tap_rim),
            ("TapShrinkEffect", self.animation.tap_shrink),
            ("PulseReceptorsWithTempo", self.animation.pulse),
        ] {
            fields.insert(name.into(), Value::Bool(value));
        }
    }
}
