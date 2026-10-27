//! Cached UI palette policy from GrooveAuthor PreferencesNoteColor/ArrowGraphicManager.
//! Does not read application preferences or select the editor's color-row method.
use crate::Error;
type Rgb = [f32; 3];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Row,
    RoutineRow,
    PumpLane { start_index: u8, routine: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMethod {
    Stepmania,
    Note,
    Beat,
}
impl ColorMethod {
    pub fn color_row(
        self,
        row: u64,
        relative_to_measure: u64,
        signature_denominator: u32,
    ) -> Result<u8, Error> {
        let value = match self {
            Self::Stepmania => row,
            Self::Note => relative_to_measure,
            Self::Beat => {
                relative_to_measure
                    .checked_mul(signature_denominator as u64)
                    .ok_or_else(|| Error::InvalidEdit("Color row overflow".into()))?
                    / 4
            }
        };
        Ok((value % 48) as u8)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    /// Quarter, eighth, twelfth, sixteenth,24th,32nd,48th,64th,192nd.
    pub subdivisions: [Rgb; 9],
    pub hold: Rgb,
    pub roll: Rgb,
    pub mine: Rgb,
    pub piu_top: Rgb,
    pub piu_middle: Rgb,
    pub piu_bottom: Rgb,
    pub players: Vec<Rgb>,
    pub normal_multiplier: f32,
    pub selected_multiplier: f32,
    pub routine_alpha: f32,
    pub piu_hold_saturation: f32,
    pub color_multiplayer_holds: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            subdivisions: [
                [0.7137255, 0.0941176, 0.0941176],
                [0.0941176, 0.2078431, 0.7137255],
                [0.2117647, 0.6784314, 0.2156863],
                [0.8745098, 0.8470588, 0.0588235],
                [0.5176471, 0.0941176, 0.7137255],
                [0.0941176, 0.7137255, 0.5960784],
                [0.7137255, 0.0941176, 0.5019607],
                [0.3098039, 0.4352941, 0.3450980],
                [0.3098039, 0.4352941, 0.3450980],
            ],
            hold: [0.2666667; 3],
            roll: [0.7803921, 0.4666667, 0.1607843],
            mine: [0.8196078, 0.0078431, 0.0078431],
            piu_top: [0.7019608, 0.1058824, 0.2156863],
            piu_middle: [1.0, 0.9176471, 0.0],
            piu_bottom: [0.1058824, 0.2509804, 0.7019608],
            players: vec![
                [0.7109375, 0.09375, 0.7109375],
                [0.09375, 0.7109375, 0.7109375],
                [0.7109375, 0.7109375, 0.09375],
                [0.09375, 0.7109375, 0.09375],
                [0.7109375, 0.390625, 0.09375],
                [0.7109375; 3],
                [0.09375; 3],
                [0.7109375, 0.390625, 0.390625],
                [0.390625, 0.390625, 0.7109375],
                [0.390625, 0.7109375, 0.390625],
            ],
            normal_multiplier: 1.5,
            selected_multiplier: 8.0,
            routine_alpha: 0.9,
            piu_hold_saturation: 0.8,
            color_multiplayer_holds: true,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    pub head: u32,
    pub hold: u32,
    pub roll: u32,
    pub mine: u32,
}
#[derive(Clone)]
struct Player {
    rows: [[Colors; 2]; 48],
    piu: [[Colors; 2]; 5],
}
#[derive(Clone)]
pub struct Palette {
    ordinary: Player,
    players: Vec<Player>,
    color_multiplayer_holds: bool,
}
impl Palette {
    pub fn new(preferences: &Preferences) -> Result<Self, Error> {
        let colors = preferences
            .subdivisions
            .iter()
            .chain([
                &preferences.hold,
                &preferences.roll,
                &preferences.mine,
                &preferences.piu_top,
                &preferences.piu_middle,
                &preferences.piu_bottom,
            ])
            .chain(preferences.players.iter());
        if preferences.players.is_empty()
            || preferences.players.len() > 64
            || colors
                .flatten()
                .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(channel))
            || [
                preferences.normal_multiplier,
                preferences.selected_multiplier,
                preferences.routine_alpha,
                preferences.piu_hold_saturation,
            ]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
            || preferences.routine_alpha > 1.0
        {
            return Err(Error::InvalidEdit(
                "Invalid minimap palette preferences".into(),
            ));
        }
        Ok(Self {
            ordinary: Player::build(preferences, None),
            players: preferences
                .players
                .iter()
                .copied()
                .map(|player| Player::build(preferences, Some(player)))
                .collect(),
            color_multiplayer_holds: preferences.color_multiplayer_holds,
        })
    }
    pub fn colors(
        &self,
        family: Family,
        color_row: u8,
        lane: u16,
        player: u32,
        selected: bool,
    ) -> Result<Colors, Error> {
        let selected = usize::from(selected);
        let row = color_row as usize % 48;
        let routine = &self.players[player as usize % self.players.len()];
        Ok(match family {
            Family::Row => self.ordinary.rows[row][selected],
            Family::RoutineRow => {
                let mut colors = routine.rows[row][selected];
                if !self.color_multiplayer_holds {
                    let ordinary = self.ordinary.rows[row][selected];
                    colors.hold = ordinary.hold;
                    colors.roll = ordinary.roll;
                }
                colors
            }
            Family::PumpLane {
                start_index,
                routine: multiplayer,
            } => {
                if start_index > 4 {
                    return Err(Error::InvalidEdit("Invalid PIU start lane".into()));
                }
                let index = (lane as usize + start_index as usize) % 5;
                (if multiplayer { routine } else { &self.ordinary }).piu[index][selected]
            }
        })
    }
}
impl Player {
    fn build(p: &Preferences, player: Option<Rgb>) -> Self {
        let mix = |base: Rgb| -> Rgb {
            match player {
                None => base,
                Some(player) => std::array::from_fn(|i| {
                    (base[i] * (1.0 - p.routine_alpha) + player[i] * p.routine_alpha)
                        .clamp(0.0, 1.0)
                }),
            }
        };
        let variant = |base: Rgb, selected: usize| {
            ui(
                base,
                if selected == 1 {
                    p.selected_multiplier
                } else {
                    p.normal_multiplier
                },
            )
        };
        let rows = std::array::from_fn(|row| {
            std::array::from_fn(|selected| Colors {
                head: variant(mix(p.subdivisions[subdivision_index(row as u8)]), selected),
                hold: variant(mix(p.hold), selected),
                roll: variant(mix(p.roll), selected),
                mine: variant(mix(p.mine), selected),
            })
        });
        let piu = std::array::from_fn(|lane| {
            std::array::from_fn(|selected| {
                let base = mix(match lane {
                    0 | 4 => p.piu_bottom,
                    1 | 3 => p.piu_top,
                    _ => p.piu_middle,
                });
                let body = variant(desaturate(base, p.piu_hold_saturation), selected);
                Colors {
                    head: variant(base, selected),
                    hold: body,
                    roll: body,
                    mine: variant(mix(p.mine), selected),
                }
            })
        });
        Self { rows, piu }
    }
}
fn subdivision_index(row: u8) -> usize {
    fn gcd(mut a: u8, mut b: u8) -> u8 {
        while b != 0 {
            let next = a % b;
            a = b;
            b = next;
        }
        a
    }
    match 48 / gcd(row % 48, 48) {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        6 => 4,
        8 => 5,
        12 => 6,
        16 => 7,
        _ => 8,
    }
}
fn ui(rgb: Rgb, multiplier: f32) -> u32 {
    let mut packed = 0xff000000;
    for (i, channel) in rgb.into_iter().enumerate() {
        let byte = (channel * 255.0) as u8;
        packed |= ((byte as f32 * multiplier).min(255.0) as u32) << (8 * i);
    }
    packed
}
fn equal(a: f32, b: f32) -> bool {
    a - 0.0000001 <= b && a + 0.0000001 >= b
}
/// Fumen ColorUtils at0fe292f9: preserve float ordering, radians and epsilon.
pub(crate) fn desaturate([r, g, b]: Rgb, multiplier: f32) -> Rgb {
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let mut hue = 0.0f32;
    let saturation = if equal(max, 0.0) {
        0.0
    } else {
        (max - min) / max
    };
    if !equal(saturation, 0.0) {
        let d = max - min;
        hue = if equal(r, max) {
            (g - b) / d
        } else if equal(g, max) {
            2.0 + (b - r) / d
        } else {
            4.0 + (r - g) / d
        };
        hue *= (std::f64::consts::PI / 3.0) as f32;
        if hue < 0.0 {
            hue += (2.0 * std::f64::consts::PI) as f32;
        }
    }
    let saturation = (saturation * multiplier).clamp(0.0, 1.0);
    if equal(saturation, 0.0) {
        return [max; 3];
    }
    hue = if equal(hue, (std::f64::consts::PI * 2.0) as f32) {
        0.0
    } else {
        ((hue * 3.0) as f64 / std::f64::consts::PI) as f32
    };
    let sextant = hue.floor();
    let f = hue - sextant;
    let p = max * (1.0 - saturation);
    let q = max * (1.0 - saturation * f);
    let t = max * (1.0 - saturation * (1.0 - f));
    match sextant as i32 {
        1 => [q, max, p],
        2 => [p, max, t],
        3 => [p, q, max],
        4 => [t, p, max],
        5 => [max, p, q],
        _ => [max, t, p],
    }
}
