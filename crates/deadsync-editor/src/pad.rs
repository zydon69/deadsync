//! GrooveAuthor/StepManiaLibrary pad relationships, preserved from the pinned
//! upstream data rather than approximated with lane ordering or distances.
//! Source: StepManiaLibrary bdbc427f, PadData.cs and ArrowData.cs (MIT).
//! See docs/editor/licenses/StepManiaLibrary-MIT.txt.
use crate::Error;
use serde::Deserialize;
use std::{collections::BTreeSet, sync::OnceLock};

pub const LAYOUTS: [&str; 16] = [
    "dance-single",
    "dance-double",
    "dance-routine",
    "dance-solo",
    "dance-threepanel",
    "pump-single",
    "pump-halfdouble",
    "pump-double",
    "pump-routine",
    "smx-beginner",
    "smx-single",
    "smx-dual",
    "smx-full",
    "smx-team",
    "dance-couple",
    "pump-couple",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Arrow {
    pub x: i32,
    pub y: i32,
    pub bracketable_pairings_heel: [Vec<bool>; 2],
    pub bracketable_pairings_toe: [Vec<bool>; 2],
    pub other_foot_pairings: [Vec<bool>; 2],
    pub other_foot_pairings_stretch: [Vec<bool>; 2],
    pub other_foot_pairings_crossover_front: [Vec<bool>; 2],
    pub other_foot_pairings_crossover_front_stretch: [Vec<bool>; 2],
    pub other_foot_pairings_crossover_behind: [Vec<bool>; 2],
    pub other_foot_pairings_crossover_behind_stretch: [Vec<bool>; 2],
    pub other_foot_pairings_inverted: [Vec<bool>; 2],
    pub other_foot_pairings_inverted_stretch: [Vec<bool>; 2],
}

impl Arrow {
    fn relationships(&self) -> [&[Vec<bool>; 2]; 10] {
        [
            &self.bracketable_pairings_heel,
            &self.bracketable_pairings_toe,
            &self.other_foot_pairings,
            &self.other_foot_pairings_stretch,
            &self.other_foot_pairings_crossover_front,
            &self.other_foot_pairings_crossover_front_stretch,
            &self.other_foot_pairings_crossover_behind,
            &self.other_foot_pairings_crossover_behind_stretch,
            &self.other_foot_pairings_inverted,
            &self.other_foot_pairings_inverted_stretch,
        ]
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Definition {
    starting_positions: Vec<Vec<[usize; 2]>>,
    arrow_data: Vec<Arrow>,
}

#[derive(Clone, Debug)]
pub struct Pad {
    pub starting_positions: Vec<Vec<[usize; 2]>>,
    pub arrows: Vec<Arrow>,
    pub mirror: Vec<Option<u16>>,
    pub flip: Vec<Option<u16>>,
}

impl Pad {
    /// PadData.CanFitWithin: a translated coordinate embedding must preserve
    /// every relationship supported by the source, not just its lane count.
    pub fn can_fit_within(&self, target: &Self) -> bool {
        if std::ptr::eq(self, target) {
            return true;
        }
        if self.arrows.len() > target.arrows.len() {
            return false;
        }
        let bounds = |pad: &Self| {
            let xs = pad.arrows.iter().map(|a| i64::from(a.x));
            let ys = pad.arrows.iter().map(|a| i64::from(a.y));
            (
                xs.clone().min().unwrap(),
                xs.max().unwrap(),
                ys.clone().min().unwrap(),
                ys.max().unwrap(),
            )
        };
        let (sx, ex, sy, ey) = bounds(self);
        let (tx, ux, ty, uy) = bounds(target);
        let extra_x = (ux - tx) - (ex - sx);
        let extra_y = (uy - ty) - (ey - sy);
        if extra_x < 0 || extra_y < 0 {
            return false;
        }
        // Any embedding must align the first source arrow with a target arrow.
        // Enumerating those translations avoids scanning huge empty coordinate
        // bounds in custom pad definitions while retaining the same fit test.
        let anchor = &self.arrows[0];
        let candidates: BTreeSet<_> = target
            .arrows
            .iter()
            .map(|arrow| {
                (
                    i64::from(arrow.x) - (i64::from(anchor.x) - sx),
                    i64::from(arrow.y) - (i64::from(anchor.y) - sy),
                )
            })
            .filter(|&(x, y)| x >= tx && x <= tx + extra_x && y >= ty && y <= ty + extra_y)
            .collect();
        for (x, y) in candidates {
            let mapping: Option<Vec<_>> = self
                .arrows
                .iter()
                .map(|arrow| {
                    target.arrows.iter().position(|other| {
                        i64::from(other.x) == i64::from(arrow.x) - sx + x
                            && i64::from(other.y) == i64::from(arrow.y) - sy + y
                    })
                })
                .collect();
            let Some(mapping) = mapping else {
                continue;
            };
            if self.arrows.iter().enumerate().all(|(a, source)| {
                let source = source.relationships();
                let other = target.arrows[mapping[a]].relationships();
                (0..source.len()).all(|kind| {
                    (0..2).all(|foot| {
                        (0..self.arrows.len())
                            .all(|b| !source[kind][foot][b] || other[kind][foot][mapping[b]])
                    })
                })
            }) {
                return true;
            }
        }
        false
    }

    pub fn parse(json: &str) -> Result<Self, Error> {
        let definition: Definition = serde_json::from_str(json)
            .map_err(|error| Error::InvalidFormat(format!("Invalid pad definition: {error}")))?;
        let count = definition.arrow_data.len();
        if count == 0 || count > u16::MAX as usize {
            return Err(Error::InvalidFormat("Invalid pad lane count".into()));
        }
        if definition.starting_positions.is_empty()
            || definition.starting_positions[0].len() != 1
            || definition
                .starting_positions
                .iter()
                .any(|tier| tier.is_empty() || tier.iter().flatten().any(|&lane| lane >= count))
        {
            return Err(Error::InvalidFormat(
                "Invalid pad starting positions".into(),
            ));
        }
        for arrow in &definition.arrow_data {
            let matrices = [
                &arrow.bracketable_pairings_heel,
                &arrow.bracketable_pairings_toe,
                &arrow.other_foot_pairings,
                &arrow.other_foot_pairings_stretch,
                &arrow.other_foot_pairings_crossover_front,
                &arrow.other_foot_pairings_crossover_front_stretch,
                &arrow.other_foot_pairings_crossover_behind,
                &arrow.other_foot_pairings_crossover_behind_stretch,
                &arrow.other_foot_pairings_inverted,
                &arrow.other_foot_pairings_inverted_stretch,
            ];
            if matrices
                .iter()
                .any(|matrix| matrix.iter().any(|foot| foot.len() != count))
            {
                return Err(Error::InvalidFormat(
                    "Pad relationship matrix has the wrong lane count".into(),
                ));
            }
        }
        let min_x = definition
            .arrow_data
            .iter()
            .map(|arrow| arrow.x)
            .min()
            .unwrap();
        let max_x = definition
            .arrow_data
            .iter()
            .map(|arrow| arrow.x)
            .max()
            .unwrap();
        let min_y = definition
            .arrow_data
            .iter()
            .map(|arrow| arrow.y)
            .min()
            .unwrap();
        let max_y = definition
            .arrow_data
            .iter()
            .map(|arrow| arrow.y)
            .max()
            .unwrap();
        let find = |x: i64, y: i64| {
            definition
                .arrow_data
                .iter()
                .rposition(|arrow| i64::from(arrow.x) == x && i64::from(arrow.y) == y)
                .map(|lane| lane as u16)
        };
        let mirror = definition
            .arrow_data
            .iter()
            .map(|arrow| {
                find(
                    i64::from(min_x) + i64::from(max_x) - i64::from(arrow.x),
                    i64::from(arrow.y),
                )
            })
            .collect();
        let flip = definition
            .arrow_data
            .iter()
            .map(|arrow| {
                find(
                    i64::from(arrow.x),
                    i64::from(min_y) + i64::from(max_y) - i64::from(arrow.y),
                )
            })
            .collect();
        Ok(Self {
            starting_positions: definition.starting_positions,
            arrows: definition.arrow_data,
            mirror,
            flip,
        })
    }

    pub fn builtin(layout: &str) -> Result<&'static Self, Error> {
        let layout = match layout {
            "dance-couple" => "dance-double",
            "pump-couple" => "pump-double",
            other => other,
        };
        macro_rules! definitions {
            ($($name:literal),* $(,)?) => {
                &[$(($name, include_str!(concat!("../reference/pads/", $name, ".json")))),*]
            };
        }
        static PADS: OnceLock<Result<Vec<(&'static str, Pad)>, String>> = OnceLock::new();
        let pads = PADS
            .get_or_init(|| {
                let definitions: &[(&str, &str)] = definitions![
                    "dance-single",
                    "dance-double",
                    "dance-routine",
                    "dance-solo",
                    "dance-threepanel",
                    "pump-single",
                    "pump-halfdouble",
                    "pump-double",
                    "pump-routine",
                    "smx-beginner",
                    "smx-single",
                    "smx-dual",
                    "smx-full",
                    "smx-team",
                ];
                definitions
                    .iter()
                    .map(|&(name, json)| {
                        Self::parse(json)
                            .map(|pad| (name, pad))
                            .map_err(|error| format!("{name}: {error}"))
                    })
                    .collect()
            })
            .as_ref()
            .map_err(|error| Error::InvalidFormat(error.clone()))?;
        pads.iter()
            .find(|(name, _)| *name == layout)
            .map(|(_, pad)| pad)
            .ok_or_else(|| {
                Error::Unsupported(format!("No GrooveAuthor pad definition for {layout}"))
            })
    }
}
