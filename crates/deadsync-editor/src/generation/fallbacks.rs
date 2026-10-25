//! StepTypeFallbacks and GraphLinkInstanceCache (MIT, bdbc427f).
use super::{
    expressed::{InstanceType, LinkInstance},
    graph::{FootAction, PortionAction},
    reference_sort,
    search::BLANK_LINK,
    step_data::StepType,
};
use crate::Error;
use serde::Deserialize;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
};

const BLANK: LinkInstance = LinkInstance {
    link: BLANK_LINK,
    types: [[InstanceType::Default; 2]; 2],
};
#[derive(Deserialize)]
struct Definition {
    #[serde(rename = "StepTypeFallbacks")]
    lists: HashMap<String, Vec<String>>,
}

pub struct Fallbacks {
    lists: Vec<Vec<StepType>>,
    indexes: Vec<[Option<usize>; 58]>,
    cache: Mutex<HashMap<LinkInstance, Arc<[LinkInstance]>>>,
}
impl Fallbacks {
    pub fn parse(input: &[u8]) -> Result<Self, Error> {
        let definition: Definition = serde_json::from_slice(&super::jsonc::normalize(input)?)
            .map_err(|e| {
                Error::InvalidFormat(format!("Invalid step fallback configuration: {e}"))
            })?;
        let mut raw: Vec<Option<Vec<String>>> = vec![None; 58];
        for (name, list) in definition.lists {
            let step = StepType::ALL
                .into_iter()
                .find(|step| format!("{step:?}").eq_ignore_ascii_case(name.trim()))
                .or_else(|| name.trim().parse::<u8>().ok().and_then(StepType::from_byte))
                .ok_or_else(|| Error::InvalidFormat(format!("Unknown fallback key: {name}")))?;
            if raw[step as usize].replace(list).is_some() {
                return invalid("Duplicate step fallback key");
            }
        }
        let mut lists = Vec::new();
        let mut indexes = Vec::new();
        for step in StepType::ALL {
            let mut seen = HashSet::from([step]);
            let mut list = Vec::new();
            expand(step, &raw, &mut seen, &mut list)?;
            if list.is_empty() {
                return invalid("Every step type requires at least one fallback");
            }
            let mut index = [None; 58];
            for (i, &fallback) in list.iter().enumerate() {
                if index[fallback as usize].replace(i).is_some() {
                    return invalid("Duplicate expanded step fallback");
                }
            }
            lists.push(list);
            indexes.push(index);
        }
        Ok(Self {
            lists,
            indexes,
            cache: Mutex::new(HashMap::new()),
        })
    }
    pub fn builtin() -> &'static Self {
        static DEFAULT: OnceLock<Fallbacks> = OnceLock::new();
        DEFAULT.get_or_init(|| {
            Self::parse(include_bytes!(
                "../../reference/fallbacks/default-steptype-fallbacks.json"
            ))
            .expect("Pinned fallback data is valid")
        })
    }
    pub fn list(&self, step: StepType) -> &[StepType] {
        &self.lists[step as usize]
    }
    pub fn index(&self, step: StepType, replacement: StepType) -> Option<usize> {
        self.indexes[step as usize][replacement as usize]
    }
    pub fn replacements(
        &self,
        source: LinkInstance,
        cancelled: &AtomicBool,
    ) -> Result<Arc<[LinkInstance]>, Error> {
        check(cancelled)?;
        if let Some(links) = self
            .cache
            .lock()
            .map_err(|_| Error::InvalidEdit("Fallback cache lock failed".into()))?
            .get(&source)
            .cloned()
        {
            return Ok(links);
        }
        let links: Arc<[LinkInstance]> = self.generate(source, cancelled)?.into();
        check(cancelled)?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| Error::InvalidEdit("Fallback cache lock failed".into()))?;
        Ok(Arc::clone(cache.entry(source).or_insert(links)))
    }
    fn foot(
        &self,
        source: LinkInstance,
        foot: usize,
        cancelled: &AtomicBool,
    ) -> Result<Vec<LinkInstance>, Error> {
        let original = source.link.feet[foot];
        let mut output = Vec::new();
        if let Some(action) = original.iter().find(|a| a.valid) {
            let original_data = action.step.data();
            let original_portion = original_data.single_step_portion.unwrap_or(0);
            for &step in self.list(action.step) {
                check(cancelled)?;
                let data = step.data();
                if !data.can_release
                    && original
                        .iter()
                        .any(|a| a.valid && a.action == FootAction::Release)
                {
                    continue;
                }
                if !data.bracket {
                    let new_portion = data.single_step_portion.unwrap_or(0);
                    let old_portion = if original_data.bracket {
                        original
                            .iter()
                            .position(|a| a.action == FootAction::Hold)
                            .unwrap_or(0)
                    } else {
                        original_portion
                    };
                    let mut link = BLANK;
                    link.link.feet[foot][new_portion] = PortionAction {
                        valid: true,
                        step,
                        action: original[old_portion].action,
                    };
                    // The reference retains the original instance slot even when
                    // a one-arrow bracket changes heel/toe portion.
                    link.types[foot][old_portion] = source.types[foot][old_portion];
                    output.push(link);
                } else {
                    let (heel, toe, heel_type, toe_type) = if original_data.bracket {
                        (
                            original[0].action,
                            original[1].action,
                            source.types[foot][0],
                            source.types[foot][1],
                        )
                    } else {
                        let heel = original[original_portion].action;
                        (
                            heel,
                            if heel == FootAction::Release {
                                FootAction::Release
                            } else {
                                FootAction::Tap
                            },
                            source.types[foot][original_portion],
                            InstanceType::Default,
                        )
                    };
                    let mut link = BLANK;
                    link.link.feet[foot] = [
                        PortionAction {
                            valid: true,
                            step,
                            action: heel,
                        },
                        PortionAction {
                            valid: true,
                            step,
                            action: toe,
                        },
                    ];
                    link.types[foot] = [heel_type, toe_type];
                    output.push(link);
                    if heel != toe {
                        link.link.feet[foot].swap(0, 1);
                        link.types[foot].swap(0, 1);
                        output.push(link);
                    }
                }
            }
        }
        output.push(BLANK);
        Ok(output)
    }
    fn generate(
        &self,
        source: LinkInstance,
        cancelled: &AtomicBool,
    ) -> Result<Vec<LinkInstance>, Error> {
        if source.link.is_blank() {
            return Ok(Vec::new());
        }
        let left = self.foot(source, 0, cancelled)?;
        let right = self.foot(source, 1, cancelled)?;
        let valid: [bool; 2] = std::array::from_fn(|f| source.link.feet[f].iter().any(|a| a.valid));
        let mut output = if valid == [true, false] {
            left
        } else if valid == [false, true] {
            right
        } else {
            let mut output = Vec::new();
            let allowed = |link: &LinkInstance, foot: usize| {
                link.link.feet[foot]
                    .iter()
                    .all(|a| !a.valid || a.step.data().can_jump)
            };
            for l in left.iter().filter(|l| allowed(l, 0)) {
                for r in right.iter().filter(|r| allowed(r, 1)) {
                    check(cancelled)?;
                    let mut link = BLANK;
                    link.link.feet[0] = l.link.feet[0];
                    link.types[0] = l.types[0];
                    link.link.feet[1] = r.link.feet[1];
                    link.types[1] = r.types[1];
                    output.push(link);
                }
            }
            output
        };
        // The reference assumes compatible step types on each foot. Reject
        // malformed external links before the infallible comparer sees them.
        for replacement in &output {
            check(cancelled)?;
            for (a, b) in source
                .link
                .feet
                .iter()
                .flatten()
                .zip(replacement.link.feet.iter().flatten())
            {
                if a.valid && b.valid && self.index(a.step, b.step).is_none() {
                    return Err(Error::InvalidEdit(
                        "Incompatible step types in fallback source".into(),
                    ));
                }
            }
        }
        reference_sort::sort(&mut output, |a, b| self.compare(source, *a, *b));
        check(cancelled)?;
        Ok(output)
    }
    fn rank(&self, source: LinkInstance, replacement: LinkInstance) -> (bool, usize, [f64; 2]) {
        let mut costs = [0.0; 2];
        let mut dropped = 0;
        let mut dropped_foot = false;
        for foot in 0..2 {
            let mut source_valid = false;
            let mut replacement_valid = false;
            for portion in 0..2 {
                let a = source.link.feet[foot][portion];
                let b = replacement.link.feet[foot][portion];
                if a.valid {
                    source_valid = true;
                    if !b.valid {
                        dropped += 1;
                    } else {
                        replacement_valid = true;
                    }
                    if costs[foot] == 0.0 && b.valid {
                        costs[foot] = self
                            .index(a.step, b.step)
                            .expect("Generated step belongs to its fallback list")
                            as f64
                            / self.list(a.step).len() as f64;
                    }
                }
            }
            dropped_foot |= source_valid && !replacement_valid;
        }
        (dropped_foot, dropped, costs)
    }
    fn compare(&self, source: LinkInstance, a: LinkInstance, b: LinkInstance) -> Ordering {
        let blank = a.link.is_blank().cmp(&b.link.is_blank());
        if blank != Ordering::Equal {
            return blank;
        }
        let (a_foot, a_removed, a_cost) = self.rank(source, a);
        let (b_foot, b_removed, b_cost) = self.rank(source, b);
        a_foot
            .cmp(&b_foot)
            .then_with(|| a_removed.cmp(&b_removed))
            .then_with(|| {
                (a_cost[0] + a_cost[1])
                    .partial_cmp(&(b_cost[0] + b_cost[1]))
                    .unwrap()
            })
            .then_with(|| a_cost[0].partial_cmp(&b_cost[0]).unwrap())
    }
}
pub fn contains_blank_foot(source: LinkInstance, replacement: LinkInstance) -> bool {
    (0..2).any(|f| {
        source.link.feet[f].iter().any(|a| a.valid)
            && replacement.link.feet[f].iter().all(|a| !a.valid)
    })
}
pub fn removed_steps(source: LinkInstance, replacement: LinkInstance) -> usize {
    source
        .link
        .feet
        .iter()
        .flatten()
        .zip(replacement.link.feet.iter().flatten())
        .filter(|(a, b)| a.valid && !b.valid)
        .count()
}
pub fn replacements(
    source: LinkInstance,
    fallbacks: Option<&Fallbacks>,
    cancelled: &AtomicBool,
) -> Result<Arc<[LinkInstance]>, Error> {
    check(cancelled)?;
    match fallbacks {
        Some(fallbacks) => fallbacks.replacements(source, cancelled),
        None => Ok(Arc::from([source])),
    }
}
fn parse_step(name: &str) -> Result<StepType, Error> {
    let name = name.trim();
    StepType::ALL
        .into_iter()
        .find(|step| format!("{step:?}") == name)
        .or_else(|| name.parse::<u8>().ok().and_then(StepType::from_byte))
        .ok_or_else(|| Error::InvalidFormat(format!("Unknown fallback step type: {name}")))
}
fn expand(
    step: StepType,
    raw: &[Option<Vec<String>>],
    visited: &mut HashSet<StepType>,
    output: &mut Vec<StepType>,
) -> Result<(), Error> {
    let values = raw[step as usize]
        .as_ref()
        .ok_or_else(|| Error::InvalidFormat(format!("Missing fallback list for {step:?}")))?;
    for value in values {
        if let Some(include) = value.strip_prefix('*') {
            let base = parse_step(include)?;
            if !visited.insert(base) {
                return invalid("Cycle or repeated include in step fallbacks");
            }
            expand(base, raw, visited, output)?;
        } else {
            output.push(parse_step(value)?);
        }
    }
    Ok(())
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(AtomicOrdering::Relaxed) {
        Err(Error::InvalidEdit("Fallback generation cancelled".into()))
    } else {
        Ok(())
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::InvalidFormat(message.into()))
}
