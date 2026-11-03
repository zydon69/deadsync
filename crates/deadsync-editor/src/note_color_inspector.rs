//! Keyboard color inspector over the single shared immutable NoteColor DTO.
pub mod merge;
mod presets;
use crate::{Error, minimap_palette::ColorMethod, note_color_preferences::Preferences};
use std::sync::Arc;
pub const FIELD_COUNT: usize = 35;
pub const LABELS: [&str; FIELD_COUNT] = [
    "ColorQuarter",
    "ColorEighth",
    "ColorTwelfth",
    "ColorSixteenth",
    "ColorTwentyFourth",
    "ColorThirtySecond",
    "ColorFortyEighth",
    "ColorSixtyFourth",
    "Color192",
    "ColorHold",
    "ColorRoll",
    "ColorMine",
    "ColorPiuTop",
    "ColorPiuMiddle",
    "ColorPiuBottom",
    "ColorPlayer1",
    "ColorPlayer2",
    "ColorPlayer3",
    "ColorPlayer4",
    "ColorPlayer5",
    "ColorPlayer6",
    "ColorPlayer7",
    "ColorPlayer8",
    "ColorPlayer9",
    "ColorPlayer10",
    "ColorSelectionMultiplier",
    "ColorHeldMultiplier",
    "ColorUiMultiplier",
    "ColorUiSelectedMultiplier",
    "ColorRoutineAlpha",
    "ColorPiuSaturation",
    "ColorPiuRowBased",
    "ColorMultiplayerHolds",
    "ColorTimeSignatureMethod",
    "ColorPreset",
];
fn rgb(p: &mut Preferences, index: usize) -> Option<&mut [f32; 3]> {
    match index {
        0..=8 => Some(&mut p.colors.subdivisions[index]),
        9 => Some(&mut p.colors.hold),
        10 => Some(&mut p.colors.roll),
        11 => Some(&mut p.colors.mine),
        12 => Some(&mut p.colors.piu_top),
        13 => Some(&mut p.colors.piu_middle),
        14 => Some(&mut p.colors.piu_bottom),
        15..=24 => p.colors.players.get_mut(index - 15),
        _ => None,
    }
}
fn scalar(p: &mut Preferences, index: usize) -> Option<&mut f32> {
    match index {
        25 => Some(&mut p.selection_multiplier),
        26 => Some(&mut p.held_multiplier),
        27 => Some(&mut p.colors.normal_multiplier),
        28 => Some(&mut p.colors.selected_multiplier),
        29 => Some(&mut p.colors.routine_alpha),
        30 => Some(&mut p.colors.piu_hold_saturation),
        _ => None,
    }
}
fn boolean(p: &mut Preferences, index: usize) -> Option<&mut bool> {
    match index {
        31 => Some(&mut p.piu_row_based),
        32 => Some(&mut p.colors.color_multiplayer_holds),
        _ => None,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Grooveauthor,
    Itg,
    Stepmania,
}
impl Preset {
    fn index(self) -> usize {
        match self {
            Self::Grooveauthor => 0,
            Self::Itg => 1,
            Self::Stepmania => 2,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Grooveauthor => "GrooveAuthor",
            Self::Itg => "ITG",
            Self::Stepmania => "StepMania",
        }
    }
}
pub struct Draft {
    baseline: Arc<Preferences>,
    staged: Arc<Preferences>,
    field: usize,
    preset: Preset,
}
impl Draft {
    pub fn capture(baseline: Arc<Preferences>) -> Result<Self, Error> {
        baseline.validate()?;
        Ok(Self {
            staged: Arc::clone(&baseline),
            baseline,
            field: 0,
            preset: Preset::Grooveauthor,
        })
    }
    pub fn baseline(&self) -> Arc<Preferences> {
        Arc::clone(&self.baseline)
    }
    pub fn staged(&self) -> Arc<Preferences> {
        Arc::clone(&self.staged)
    }
    pub fn changed(&self) -> bool {
        self.baseline != self.staged
    }
    pub fn field(&self) -> usize {
        self.field
    }
    pub fn select(&mut self, index: usize) -> Result<(), Error> {
        if index >= FIELD_COUNT {
            return Err(Error::InvalidEdit("Invalid note color field".into()));
        }
        self.field = index;
        Ok(())
    }
    pub fn cycle_field(&mut self, later: bool) {
        self.field = (self.field + if later { 1 } else { FIELD_COUNT - 1 }) % FIELD_COUNT;
    }
    pub fn components(&self) -> usize {
        if self.field < 25 { 3 } else { 1 }
    }
    pub fn value(&self, component: usize) -> Result<String, Error> {
        if component >= self.components() {
            return Err(Error::InvalidEdit("Invalid note color component".into()));
        }
        let mut p = (*self.staged).clone();
        if let Some(rgb) = rgb(&mut p, self.field) {
            return Ok(format!("{:.6}", rgb[component]));
        }
        if let Some(value) = scalar(&mut p, self.field) {
            return Ok(format!("{value:.6}"));
        }
        if let Some(value) = boolean(&mut p, self.field) {
            return Ok(value.to_string());
        }
        Ok(match self.field {
            33 => p.step_color_method.source_name(),
            34 => self.preset.name().to_owned(),
            _ => unreachable!(),
        })
    }
    /// Exact initial value for keyboard editing; display rounding must not alter a no-op.
    pub fn component_value(&self, component:usize)->Option<f32>{
        if component>=self.components(){return None;}
        let mut p=(*self.staged).clone();
        if let Some(rgb)=rgb(&mut p,self.field){return Some(rgb[component]);}
        scalar(&mut p,self.field).copied()
    }
    /// Exact component entry uses the same bounded, non-HDR controls as the source UI.
    pub fn set_component(&mut self, component: usize, value: f32) -> Result<bool, Error> {
        if component>=self.components() || !value.is_finite(){return Err(Error::InvalidEdit("Invalid note color component".into()));}
        let mut next=(*self.staged).clone();
        if let Some(rgb)=rgb(&mut next,self.field){rgb[component]=value.clamp(0.0,1.0);}
        else if let Some(target)=scalar(&mut next,self.field){*target=value;}
        else{return Err(Error::InvalidEdit("Invalid note color component".into()));}
        next.validate()?;let changed=next!=*self.staged;
        if changed{self.staged=Arc::new(next);}
        Ok(changed)
    }
    pub fn adjust(&mut self, component: usize, later: bool, fine: bool) -> Result<bool, Error> {
        if component >= self.components() {
            return Err(Error::InvalidEdit("Invalid note color component".into()));
        }
        let mut p = (*self.staged).clone();
        let sign = if later { 1.0 } else { -1.0 };
        if let Some(rgb) = rgb(&mut p, self.field) {
            rgb[component] =
                (rgb[component] + sign * if fine { 0.001 } else { 1.0 / 255.0 }).clamp(0.0, 1.0);
        } else if let Some(value) = scalar(&mut p, self.field) {
            let max = match self.field {
                29 => 1.0,
                30 => 8.0,
                _ => 10.0,
            };
            *value = (*value + sign * if fine { 0.0001 } else { 0.001 }).clamp(0.0, max);
        } else if let Some(value) = boolean(&mut p, self.field) {
            *value = !*value;
        } else if self.field == 33 {
            p.step_color_method = match (p.step_color_method, later) {
                (ColorMethod::Stepmania, true) | (ColorMethod::Beat, false) => ColorMethod::Note,
                (ColorMethod::Note, true) | (ColorMethod::Stepmania, false) => ColorMethod::Beat,
                _ => ColorMethod::Stepmania,
            };
        } else if self.field == 34 {
            let index = (self.preset.index() + if later { 1 } else { 2 }) % 3;
            self.preset = [Preset::Grooveauthor, Preset::Itg, Preset::Stepmania][index];
            apply_preset(&mut p, self.preset);
        }
        p.validate()?;
        if *self.staged == p {
            return Ok(false);
        }
        self.staged = Arc::new(p);
        Ok(true)
    }
    pub fn defaults(&mut self) -> Result<bool, Error> {
        // The source NoteColor defaults action does not own Options.StepColorMethodValue.
        let p = Preferences {
            step_color_method: self.staged.step_color_method,
            ..Default::default()
        };
        p.validate()?;
        if *self.staged == p {
            return Ok(false);
        }
        self.staged = Arc::new(p);
        Ok(true)
    }
    pub fn reset_component(&mut self, component: usize) -> Result<bool, Error> {
        if component >= self.components() {
            return Err(Error::InvalidEdit("Invalid note color component".into()));
        }
        let mut p = (*self.staged).clone();
        let mut defaults = Preferences::default();
        if let Some(target) = rgb(&mut p, self.field) {
            target[component] = rgb(&mut defaults, self.field).unwrap()[component];
        } else if let Some(target) = scalar(&mut p, self.field) {
            *target = *scalar(&mut defaults, self.field).unwrap();
        } else if let Some(target) = boolean(&mut p, self.field) {
            *target = *boolean(&mut defaults, self.field).unwrap();
        } else if self.field == 33 {
            p.step_color_method = defaults.step_color_method;
        } else {
            self.preset = Preset::Grooveauthor;
            apply_preset(&mut p, self.preset);
        }
        p.validate()?;
        if *self.staged == p {
            return Ok(false);
        }
        self.staged = Arc::new(p);
        Ok(true)
    }
}
fn apply_preset(p: &mut Preferences, preset: Preset) {
    let c = presets::COLORS[preset.index()];
    p.colors.subdivisions.copy_from_slice(&c[..9]);
    p.colors.hold = c[9];
    p.colors.roll = c[10];
    p.colors.mine = c[11];
}
