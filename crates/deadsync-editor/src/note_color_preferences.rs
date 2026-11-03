//! Shared raw note colors and UI/XNA policy, kept in original distinct groups.
//! Application group names NoteColor/Options map GA's PreferencesNoteColor/Options.
pub(crate) mod history;
use crate::{
    Error,
    minimap_palette::{self, ColorMethod, Palette},
};
pub use history::Update;
use serde_json::{Map, Value};
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub colors: minimap_palette::Preferences,
    pub selection_multiplier: f32,
    pub held_multiplier: f32,
    pub step_color_method: ColorMethod,
    pub piu_row_based: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            colors: minimap_palette::Preferences::default(),
            selection_multiplier: 1.7,
            held_multiplier: 1.5,
            step_color_method: ColorMethod::Stepmania,
            piu_row_based: false,
        }
    }
}
fn invalid(name: &str) -> Error {
    Error::InvalidFormat(format!("Invalid note color preference {name}"))
}
fn color(fields: &Map<String, Value>, name: &str, target: &mut [f32; 3]) -> Result<(), Error> {
    if let Some(value) = fields.get(name) {
        let rgb = value.as_object().ok_or_else(|| invalid(name))?;
        for (index, key) in ["X", "Y", "Z"].into_iter().enumerate() {
            target[index] = rgb
                .get(key)
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid(name))? as f32;
        }
    }
    Ok(())
}
fn scalar(fields: &Map<String, Value>, name: &str, target: &mut f32) -> Result<(), Error> {
    if let Some(value) = fields.get(name) {
        *target = value.as_f64().ok_or_else(|| invalid(name))? as f32;
    }
    Ok(())
}
fn boolean(fields: &Map<String, Value>, name: &str, target: &mut bool) -> Result<(), Error> {
    if let Some(value) = fields.get(name) {
        *target = value.as_bool().ok_or_else(|| invalid(name))?;
    }
    Ok(())
}
impl Preferences {
    pub fn read(note: &Map<String, Value>, options: &Map<String, Value>) -> Result<Self, Error> {
        let mut result = Self::default();
        for (index, name) in [
            "QuarterColor",
            "EighthColor",
            "TwelfthColor",
            "SixteenthColor",
            "TwentyForthColor",
            "ThirtySecondColor",
            "FortyEighthColor",
            "SixtyForthColor",
            "OneHundredNinetySecondColor",
        ]
        .into_iter()
        .enumerate()
        {
            color(note, name, &mut result.colors.subdivisions[index])?;
        }
        for (name, target) in [
            ("HoldColor", &mut result.colors.hold),
            ("RollColor", &mut result.colors.roll),
            ("MineColor", &mut result.colors.mine),
            ("PiuTopColor", &mut result.colors.piu_top),
            ("PiuMiddleColor", &mut result.colors.piu_middle),
            ("PiuBottomColor", &mut result.colors.piu_bottom),
        ] {
            color(note, name, target)?;
        }
        for (index, target) in result.colors.players.iter_mut().enumerate() {
            color(note, &format!("Player{index}Color"), target)?;
        }
        for (name, target) in [
            (
                "ArrowUIColorMultiplier",
                &mut result.colors.normal_multiplier,
            ),
            (
                "ArrowUISelectedColorMultiplier",
                &mut result.colors.selected_multiplier,
            ),
            ("RoutineNoteColorAlpha", &mut result.colors.routine_alpha),
            (
                "PiuHoldSaturationMultiplier",
                &mut result.colors.piu_hold_saturation,
            ),
            ("SelectionColorMultiplier", &mut result.selection_multiplier),
            ("HeldColorMultiplier", &mut result.held_multiplier),
        ] {
            scalar(note, name, target)?;
        }
        boolean(
            note,
            "ColorMultiplayerHoldsAndRolls",
            &mut result.colors.color_multiplayer_holds,
        )?;
        boolean(note, "UseRowBasedColoringForPiu", &mut result.piu_row_based)?;
        // Real Fumen converter semantics; unknown numeric strings stay raw.
        result.step_color_method = ColorMethod::read_source(options.get("StepColorMethodValue"))
            .map_err(|_| invalid("StepColorMethodValue"))?;
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), Error> {
        Palette::new(&self.colors)?;
        if self.colors.players.len() != 10
            || [self.selection_multiplier, self.held_multiplier]
                .iter()
                .any(|v| !v.is_finite())
        {
            return Err(invalid("player count or XNA multiplier"));
        }
        Ok(())
    }
    pub fn write(
        &self,
        note: &mut Map<String, Value>,
        options: &mut Map<String, Value>,
    ) -> Result<(), Error> {
        self.validate()?;
        let mut put = |name: &str, rgb: [f32; 3]| {
            let mut value = note
                .get(name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (key, channel) in ["X", "Y", "Z"].into_iter().zip(rgb) {
                value.insert(key.into(), channel.into());
            }
            note.insert(name.into(), Value::Object(value));
        };
        for (name, rgb) in [
            "QuarterColor",
            "EighthColor",
            "TwelfthColor",
            "SixteenthColor",
            "TwentyForthColor",
            "ThirtySecondColor",
            "FortyEighthColor",
            "SixtyForthColor",
            "OneHundredNinetySecondColor",
        ]
        .into_iter()
        .zip(self.colors.subdivisions)
        {
            put(name, rgb);
        }
        for (name, rgb) in [
            ("HoldColor", self.colors.hold),
            ("RollColor", self.colors.roll),
            ("MineColor", self.colors.mine),
            ("PiuTopColor", self.colors.piu_top),
            ("PiuMiddleColor", self.colors.piu_middle),
            ("PiuBottomColor", self.colors.piu_bottom),
        ] {
            put(name, rgb);
        }
        for (index, rgb) in self.colors.players.iter().copied().enumerate() {
            put(&format!("Player{index}Color"), rgb);
        }
        for (name, value) in [
            ("ArrowUIColorMultiplier", self.colors.normal_multiplier),
            (
                "ArrowUISelectedColorMultiplier",
                self.colors.selected_multiplier,
            ),
            ("RoutineNoteColorAlpha", self.colors.routine_alpha),
            (
                "PiuHoldSaturationMultiplier",
                self.colors.piu_hold_saturation,
            ),
            ("SelectionColorMultiplier", self.selection_multiplier),
            ("HeldColorMultiplier", self.held_multiplier),
        ] {
            note.insert(name.into(), value.into());
        }
        note.insert(
            "ColorMultiplayerHoldsAndRolls".into(),
            self.colors.color_multiplayer_holds.into(),
        );
        note.insert(
            "UseRowBasedColoringForPiu".into(),
            self.piu_row_based.into(),
        );
        options.insert(
            "StepColorMethodValue".into(),
            self.step_color_method.source_name().into(),
        );
        Ok(())
    }
}
