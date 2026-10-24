//! Rust reader for the original StepManiaLibrary FSG v1 graphs (MIT).
//! Source: StepGraph.cs serialization, GraphNode.cs, and GraphLink.cs at bdbc427f.
use super::step_data::StepType;
use crate::{Error, pad::Pad};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
    ops::Range,
    sync::{Arc, OnceLock},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArrowState {
    Resting,
    Held,
    Lifted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Normal,
    InvertedRightOverLeft,
    InvertedLeftOverRight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FootAction {
    Tap,
    Hold,
    Release,
}

impl FootAction {
    pub const fn resulting_state(self) -> ArrowState {
        match self {
            Self::Hold => ArrowState::Held,
            Self::Tap | Self::Release => ArrowState::Resting,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FootState {
    pub lane: Option<u16>,
    pub state: ArrowState,
}

impl FootState {
    pub const INVALID: Self = Self {
        lane: None,
        state: ArrowState::Resting,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeState {
    pub feet: [[FootState; 2]; 2],
    pub orientation: Orientation,
}

impl NodeState {
    pub fn same_footing(self, other: Self) -> bool {
        self.orientation == other.orientation
            && self
                .feet
                .iter()
                .flatten()
                .zip(other.feet.iter().flatten())
                .all(|(a, b)| a.lane == b.lane)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortionAction {
    pub valid: bool,
    pub step: StepType,
    pub action: FootAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GraphLink {
    pub feet: [[PortionAction; 2]; 2],
}

impl GraphLink {
    pub fn is_blank(&self) -> bool {
        self.feet.iter().flatten().all(|portion| !portion.valid)
    }
    pub fn is_jump(&self) -> bool {
        self.involves_step(0) && self.involves_step(1)
    }
    pub fn is_release(&self) -> bool {
        self.feet
            .iter()
            .flatten()
            .any(|portion| portion.valid && portion.action == FootAction::Release)
    }
    pub fn involves_step(&self, foot: usize) -> bool {
        self.feet.get(foot).is_some_and(|foot| {
            foot.iter()
                .any(|portion| portion.valid && portion.action != FootAction::Release)
        })
    }
    pub fn is_step_with_foot(&self, foot: usize) -> bool {
        foot < 2
            && self.involves_step(foot)
            && self.feet[1 - foot].iter().all(|portion| !portion.valid)
    }
    pub fn single_step(&self) -> Option<(StepType, usize)> {
        let mut steps = self.feet.iter().enumerate().flat_map(|(foot, portions)| {
            portions
                .iter()
                .filter(|portion| portion.valid && portion.action != FootAction::Release)
                .map(move |portion| (portion.step, foot))
        });
        let step = steps.next()?;
        steps.next().is_none().then_some(step)
    }
    pub fn is_bracket_step(&self) -> bool {
        let mut feet_with_actions = 0;
        let mut bracket = false;
        for foot in &self.feet {
            feet_with_actions += usize::from(foot.iter().any(|portion| portion.valid));
            bracket |= foot
                .iter()
                .all(|portion| portion.valid && portion.action != FootAction::Release);
        }
        feet_with_actions == 1 && bracket
    }
    pub fn foot_swap(&self) -> Option<(usize, usize)> {
        for (foot, portions) in self.feet.iter().enumerate() {
            for (portion, action) in portions.iter().enumerate() {
                if action.valid && action.step.data().foot_swap[portion] {
                    return Some((foot, portion));
                }
            }
        }
        None
    }
    pub fn involves_bracket(&self, include_single_arrow: bool) -> bool {
        self.feet.iter().flatten().any(|portion| {
            portion.valid
                && portion.action != FootAction::Release
                && (portion.step.data().bracket
                    || (include_single_arrow && portion.step.data().one_arrow_bracket))
        })
    }
}

#[derive(Debug)]
pub struct Node {
    pub state: NodeState,
    pub edges: Range<usize>,
}
#[derive(Debug)]
pub struct Edge {
    pub link: GraphLink,
    pub targets: Range<usize>,
}

/// Flat adjacency and target arrays avoid a heap allocation for each edge.
/// Graphs are immutable and shared; only requested layouts are decompressed.
#[derive(Debug)]
pub struct StepGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub targets: Vec<usize>,
}

impl StepGraph {
    pub fn decode_fsg(compressed: &[u8], pad: &Pad) -> Result<Self, Error> {
        let mut output = LimitedOutput(Vec::new());
        lzma_rs::lzma_decompress_with_options(
            &mut Cursor::new(compressed),
            &mut output,
            &lzma_rs::decompress::Options {
                memlimit: Some(128 * 1024 * 1024),
                ..Default::default()
            },
        )
        .map_err(|error| Error::InvalidFormat(format!("Invalid compressed step graph: {error}")))?;
        Self::decode_raw(&output.0, pad)
    }

    pub fn decode_raw(raw: &[u8], pad: &Pad) -> Result<Self, Error> {
        let mut reader = Reader { raw, pos: 0 };
        if reader.u32()? != 1 {
            return Err(Error::Unsupported("Unsupported FSG version".into()));
        }
        let count = reader.count(13)?;
        if count == 0 {
            return Err(Error::InvalidFormat("Step graph has no root".into()));
        }
        let mut nodes = Vec::with_capacity(count);
        let mut ids = BTreeMap::new();
        let mut states = BTreeSet::new();
        for index in 0..count {
            let id = reader.u32()?;
            if ids.insert(id, index).is_some() {
                return Err(Error::InvalidFormat("Duplicate graph node ID".into()));
            }
            let orientation = match reader.byte()? {
                0 => Orientation::Normal,
                1 => Orientation::InvertedRightOverLeft,
                2 => Orientation::InvertedLeftOverRight,
                _ => {
                    return Err(Error::InvalidFormat(
                        "Invalid graph body orientation".into(),
                    ));
                }
            };
            let mut feet = [[FootState::INVALID; 2]; 2];
            let mut state_key = vec![orientation as u8];
            for portion in feet.iter_mut().flatten() {
                let lane = reader.byte()?;
                let state = reader.byte()?;
                state_key.extend_from_slice(&[lane, state]);
                portion.lane = if lane == 255 {
                    None
                } else if usize::from(lane) < pad.arrows.len() {
                    Some(u16::from(lane))
                } else {
                    return Err(Error::InvalidFormat(
                        "Graph lane lies outside its pad".into(),
                    ));
                };
                portion.state = match state {
                    0 => ArrowState::Resting,
                    1 => ArrowState::Held,
                    2 => ArrowState::Lifted,
                    _ => return Err(Error::InvalidFormat("Invalid graph arrow state".into())),
                };
            }
            if !states.insert(state_key) {
                return Err(Error::InvalidFormat("Duplicate graph node state".into()));
            }
            nodes.push(Node {
                state: NodeState { feet, orientation },
                edges: 0..0,
            });
        }
        let mut edges = Vec::new();
        let mut targets = Vec::new();
        for node in &mut nodes {
            let edge_count = reader.count(20)?;
            let start = edges.len();
            for _ in 0..edge_count {
                let default = PortionAction {
                    valid: false,
                    step: StepType::SameArrow,
                    action: FootAction::Tap,
                };
                let mut feet = [[default; 2]; 2];
                for portion in feet.iter_mut().flatten() {
                    portion.valid = match reader.byte()? {
                        0 => false,
                        1 => true,
                        _ => {
                            return Err(Error::InvalidFormat("Invalid graph validity flag".into()));
                        }
                    };
                    portion.step = StepType::from_byte(reader.byte()?)
                        .ok_or_else(|| Error::InvalidFormat("Invalid graph step type".into()))?;
                    portion.action = match reader.byte()? {
                        0 => FootAction::Tap,
                        1 => FootAction::Hold,
                        2 => FootAction::Release,
                        _ => return Err(Error::InvalidFormat("Invalid graph foot action".into())),
                    };
                }
                let target_count = reader.count(4)?;
                if target_count == 0 {
                    return Err(Error::InvalidFormat("Graph edge has no destination".into()));
                }
                let start_targets = targets.len();
                for _ in 0..target_count {
                    let id = reader.u32()?;
                    targets.push(*ids.get(&id).ok_or_else(|| {
                        Error::InvalidFormat("Graph edge refers to a missing node".into())
                    })?);
                }
                edges.push(Edge {
                    link: GraphLink { feet },
                    targets: start_targets..targets.len(),
                });
            }
            node.edges = start..edges.len();
        }
        if reader.pos != raw.len() {
            return Err(Error::InvalidFormat(
                "Unexpected data after step graph".into(),
            ));
        }
        Ok(Self {
            nodes,
            edges,
            targets,
        })
    }

    pub fn find_normal_stance(&self, left: FootState, right: FootState) -> Option<usize> {
        let state = NodeState {
            feet: [[left, FootState::INVALID], [right, FootState::INVALID]],
            orientation: Orientation::Normal,
        };
        self.nodes.iter().position(|node| node.state == state)
    }

    pub fn builtin(layout: &str) -> Result<Arc<Self>, Error> {
        let layout = match layout {
            "dance-couple" | "dance-routine" => "dance-double",
            "pump-couple" | "pump-routine" => "pump-double",
            "smx-team" => "smx-full",
            other => other,
        };
        macro_rules! load {
            ($name:literal) => {{
                static GRAPH: OnceLock<Result<Arc<StepGraph>, String>> = OnceLock::new();
                GRAPH
                    .get_or_init(|| {
                        Pad::builtin($name)
                            .and_then(|pad| {
                                Self::decode_fsg(
                                    include_bytes!(concat!(
                                        "../../reference/graphs/",
                                        $name,
                                        ".fsg"
                                    )),
                                    pad,
                                )
                            })
                            .map(Arc::new)
                            .map_err(|error| error.to_string())
                    })
                    .as_ref()
                    .map(Arc::clone)
                    .map_err(|error| Error::InvalidFormat(error.clone()))
            }};
        }
        match layout {
            "dance-single" => load!("dance-single"),
            "dance-double" => load!("dance-double"),
            "dance-solo" => load!("dance-solo"),
            "dance-threepanel" => load!("dance-threepanel"),
            "pump-single" => load!("pump-single"),
            "pump-halfdouble" => load!("pump-halfdouble"),
            "pump-double" => load!("pump-double"),
            "smx-beginner" => load!("smx-beginner"),
            "smx-single" => load!("smx-single"),
            "smx-dual" => load!("smx-dual"),
            "smx-full" => load!("smx-full"),
            _ => Err(Error::Unsupported(format!(
                "No reference step graph for {layout}"
            ))),
        }
    }
}

struct Reader<'a> {
    raw: &'a [u8],
    pos: usize,
}
impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, Error> {
        let value = *self
            .raw
            .get(self.pos)
            .ok_or_else(|| Error::InvalidFormat("Truncated step graph".into()))?;
        self.pos += 1;
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        let bytes = self
            .raw
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| Error::InvalidFormat("Truncated step graph".into()))?;
        self.pos += 4;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }
    fn count(&mut self, minimum_bytes: usize) -> Result<usize, Error> {
        let count = self.u32()? as usize;
        if count > (self.raw.len() - self.pos) / minimum_bytes {
            return Err(Error::InvalidFormat(
                "Invalid step graph collection size".into(),
            ));
        }
        Ok(count)
    }
}

struct LimitedOutput(Vec<u8>);
impl Write for LimitedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > 128 * 1024 * 1024 {
            return Err(std::io::Error::other(
                "Step graph exceeds decompression limit",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
