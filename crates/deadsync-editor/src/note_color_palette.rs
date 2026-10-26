//! Cached large-chart XNA variants from the shared raw NoteColor/Options DTO.
//! UI/minimap packing is intentionally separate. No preference storage lives here.
use crate::{Error, note_color_preferences::Preferences, timing_edit::signature::Signatures};
use std::sync::Arc;
type Rgb = [f32; 3];
#[derive(Clone, Copy, Debug)]
struct Variants {
    normal: [u8; 4],
    held: [u8; 4],
    selected: [u8; 4],
    both: [u8; 4],
}
impl Variants {
    fn build(rgb: Rgb, p: &Preferences) -> Self {
        let pack = |held, selected| {
            let mut out = [0, 0, 0, 255];
            for (i, mut value) in rgb.into_iter().enumerate() {
                if held {
                    value *= p.held_multiplier;
                }
                if selected {
                    value *= p.selection_multiplier;
                }
                out[i] = (value.clamp(0.0, 1.0) * 255.0) as u8;
            }
            out
        };
        Self {
            normal: pack(false, false),
            held: pack(true, false),
            selected: pack(false, true),
            both: pack(true, true),
        }
    }
    fn get(self, held: bool, selected: bool) -> [u8; 4] {
        match (held, selected) {
            (false, false) => self.normal,
            (true, false) => self.held,
            (false, true) => self.selected,
            (true, true) => self.both,
        }
    }
}
struct Player {
    rows: [Variants; 48],
    hold: Variants,
    roll: Variants,
    mine: Variants,
    piu_notes: [Variants; 5],
    piu_holds: [Variants; 5],
}
impl Player {
    fn build(p: &Preferences, owner: Option<Rgb>) -> Self {
        let rgb = &p.colors;
        let blend = |c: Rgb| {
            owner.map_or(c, |o| {
                std::array::from_fn(|i| {
                    (c[i] * (1.0 - rgb.routine_alpha) + o[i] * rgb.routine_alpha).clamp(0.0, 1.0)
                })
            })
        };
        let variant = |c| Variants::build(blend(c), p);
        let piu = |i| match i {
            1 | 3 => rgb.piu_top,
            2 => rgb.piu_middle,
            _ => rgb.piu_bottom,
        };
        Self {
            rows: std::array::from_fn(|row| variant(rgb.subdivisions[subdivision(row as u8)])),
            hold: variant(rgb.hold),
            roll: variant(rgb.roll),
            mine: variant(rgb.mine),
            piu_notes: std::array::from_fn(|i| variant(piu(i))),
            piu_holds: std::array::from_fn(|i| {
                Variants::build(
                    crate::minimap_palette::desaturate(blend(piu(i)), rgb.piu_hold_saturation),
                    p,
                )
            }),
        }
    }
}
fn subdivision(row: u8) -> usize {
    fn gcd(mut a: u8, mut b: u8) -> u8 {
        while b != 0 {
            (a, b) = (b, a % b);
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fills {
    pub head: [u8; 4],
    pub start: [u8; 4],
    pub body: [u8; 4],
    pub cap: [u8; 4],
}
pub struct Palette {
    source: Arc<Preferences>,
    ordinary: Player,
    players: Vec<Player>,
}
impl Palette {
    pub fn capture(source: Arc<Preferences>) -> Result<Self, Error> {
        source.validate()?;
        let ordinary = Player::build(&source, None);
        let players = source
            .colors
            .players
            .iter()
            .copied()
            .map(|owner| Player::build(&source, Some(owner)))
            .collect();
        Ok(Self {
            source,
            ordinary,
            players,
        })
    }
    pub fn is_current(&self, source: &Arc<Preferences>) -> bool {
        Arc::ptr_eq(&self.source, source)
    }
    pub fn color_row(&self, row: u64, signatures: Option<&Signatures>) -> Result<u8, Error> {
        use crate::minimap_palette::ColorMethod;
        let method = self.source.step_color_method;
        if method == ColorMethod::Stepmania {
            return Ok((row % 48) as u8);
        }
        let signatures = signatures.ok_or_else(|| {
            Error::InvalidEdit("Missing chart signature phase for note colors".into())
        })?;
        let phase = signatures.measure_position(row)?;
        method.color_row(row, phase.row_in_measure, signatures.at(row).denominator)
    }
    /// Tap/head color is never desaturated, including lane-based PIU.
    pub fn tap(
        &self,
        layout: &str,
        color_row: u8,
        lane: u16,
        player: u32,
        selected: bool,
    ) -> Result<[u8; 4], Error> {
        Ok(self
            .fills(
                layout, color_row, lane, player, selected, false, false, false,
            )?
            .head)
    }
    pub fn mine(&self, layout: &str, player: u32, selected: bool) -> Result<[u8; 4], Error> {
        if crate::notes::layout_columns(layout).is_none() {
            return Err(Error::Unsupported(
                "Unknown large-chart color layout".into(),
            ));
        }
        let owner = if matches!(layout, "dance-routine" | "pump-routine" | "smx-team") {
            &self.players[player as usize % self.players.len()]
        } else {
            &self.ordinary
        };
        Ok(owner.mine.get(false, selected))
    }
    pub fn fills(
        &self,
        layout: &str,
        color_row: u8,
        lane: u16,
        player: u32,
        selected: bool,
        roll: bool,
        active: bool,
        light_holds: bool,
    ) -> Result<Fills, Error> {
        let columns = crate::notes::layout_columns(layout)
            .ok_or_else(|| Error::Unsupported("Unknown large-chart color layout".into()))?;
        if lane >= columns {
            return Err(Error::InvalidEdit(
                "Large-chart color lane outside layout".into(),
            ));
        }
        let multiplayer = matches!(layout, "dance-routine" | "pump-routine" | "smx-team");
        let owner = if multiplayer {
            &self.players[player as usize % self.players.len()]
        } else {
            &self.ordinary
        };
        let pump = layout.starts_with("pump-");
        let lane_based = pump && !self.source.piu_row_based;
        let index = (usize::from(lane) + if layout == "pump-halfdouble" { 2 } else { 0 }) % 5;
        let head = if lane_based {
            owner.piu_notes[index]
        } else {
            owner.rows[usize::from(color_row % 48)]
        };
        let bodies = if multiplayer && (lane_based || self.source.colors.color_multiplayer_holds) {
            owner
        } else {
            &self.ordinary
        };
        let body = if lane_based {
            bodies.piu_holds[index]
        } else if roll {
            bodies.roll
        } else {
            bodies.hold
        };
        let head = head.get(false, selected);
        let start = body.get(active, selected);
        let body = body.get(active && light_holds, selected);
        Ok(Fills {
            head,
            start,
            body,
            cap: if pump { head } else { body },
        })
    }
}
