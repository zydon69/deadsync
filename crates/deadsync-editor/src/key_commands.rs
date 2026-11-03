//! Ordered keyboard commands, adapted from pinned GrooveAuthor KeyCommandManager.
//! Semantic action dispatch belongs to the frontend; no synthetic key events.
use std::collections::BTreeSet;

#[path = "key_commands/source.rs"]
pub mod source;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key(String);
impl Key {
    pub fn named(name: &str) -> Self {
        Self(
            match name {
                "RightShift" => "LeftShift",
                "RightControl" => "LeftControl",
                "RightAlt" => "LeftAlt",
                "RightWindows" => "LeftWindows",
                other => other,
            }
            .to_owned(),
        )
    }
    pub fn name(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug)]
pub struct Binding {
    pub id: String,
    pub alternatives: Vec<Vec<Key>>,
    pub repeat: bool,
    pub blocks: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Press(String),
    Release(String),
    Repeat(String),
}
#[derive(Clone, Default)]
struct State {
    id: String,
    keys: Vec<Key>,
    repeat: bool,
    blocks: bool,
    active: bool,
    blocked: bool,
    any: Option<f64>,
    last: Option<f64>,
    next: f64,
}
#[derive(Default)]
pub struct Registry {
    states: Vec<State>,
    previous: BTreeSet<Key>,
}
fn subset(a: &[Key], b: &[Key]) -> bool {
    a.iter().all(|key| b.contains(key))
}
fn strict_subset(a: &[Key], b: &[Key]) -> bool {
    !a.is_empty() && a.len() < b.len() && subset(a, b)
}
fn conflicts(a: &[Key], b: &[Key]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a.len() == b.len() {
        return subset(a, b);
    }
    a.len() < b.len() && subset(a, &b[..b.len() - 1])
}
impl Registry {
    pub fn new(bindings: impl IntoIterator<Item = Binding>) -> Self {
        let mut states = Vec::new();
        for binding in bindings {
            for keys in binding.alternatives {
                states.push(State {
                    id: binding.id.clone(),
                    keys,
                    repeat: binding.repeat,
                    blocks: binding.blocks,
                    ..Default::default()
                });
            }
        }
        states.sort_by_key(|s| std::cmp::Reverse(s.keys.len()));
        Self {
            states,
            previous: BTreeSet::new(),
        }
    }
    pub fn is_active_prefix(&self, prefix: &str) -> bool {
        self.states
            .iter()
            .any(|state| state.active && state.id.starts_with(prefix))
    }
    pub fn is_active(&self, id: &str) -> bool {
        self.states
            .iter()
            .any(|state| state.id == id && state.active)
    }
    /// Source warnings are advisory; this does not reject a binding.
    pub fn conflicts(&self, id: &str, keys: &[Key]) -> Vec<String> {
        let Some(candidate) = self.states.iter().find(|state| state.id == id) else {
            return Vec::new();
        };
        self.conflicts_with_policy(id, keys, candidate.blocks)
    }
    /// Original GetConflictingCommands obtains the candidate blocking policy
    /// from PreferencesKeyBinds, independently of its registered command state.
    /// Native callers normally use conflicts(), whose policy is its binding.
    pub fn conflicts_with_policy(&self, id: &str, keys: &[Key], blocks_input: bool) -> Vec<String> {
        if !self.states.iter().any(|state|state.id==id) {return Vec::new();}
        let mut result = BTreeSet::new();
        for state in &self.states {
            if state.id != id
                && ((blocks_input && conflicts(keys, &state.keys))
                    || (state.blocks && conflicts(&state.keys, keys)))
            {
                result.insert(state.id.clone());
            }
        }
        result.into_iter().collect()
    }
    /// Explicit frontend cleanup on modal/focus transitions. Unlike source
    /// CancelAllCommands, emits releases so native detached gestures cannot stick.
    pub fn cancel(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        for state in &mut self.states {
            if state.active {
                effects.push(Effect::Release(state.id.clone()));
            }
            state.active = false;
            state.blocked = false;
            state.any = None;
            state.last = None;
        }
        self.previous.clear();
        effects
    }
    /// `down` contains the complete physical snapshot; names normalize modifier
    /// sides before evaluating. Time is monotonic UI time, never audio position.
    pub fn update(&mut self, time: f64, down: impl IntoIterator<Item = Key>) -> Vec<Effect> {
        if !time.is_finite() {
            return Vec::new();
        }
        let down: BTreeSet<_> = down.into_iter().collect();
        // Source seeds only the first active command before walking sorted states.
        let mut active: Vec<usize> = self
            .states
            .iter()
            .position(|s| s.active)
            .into_iter()
            .collect();
        let mut effects = Vec::new();
        for index in 0..self.states.len() {
            if self.states[index].keys.is_empty() {
                continue;
            }
            let allowed = active.iter().all(|&other| {
                other == index
                    || !self.states[other].blocks
                    || !(strict_subset(&self.states[index].keys, &self.states[other].keys)
                        || strict_subset(&self.states[other].keys, &self.states[index].keys))
            });
            let s = &mut self.states[index];
            let mut all_down = true;
            let mut all_up = true;
            for (i, key) in s.keys.iter().enumerate() {
                let last = i + 1 == s.keys.len();
                if down.contains(key) {
                    all_up = false;
                    s.any.get_or_insert(time);
                    if last {
                        if !self.previous.contains(key) && !allowed {
                            s.blocked = true;
                        }
                        s.last.get_or_insert(time);
                    }
                } else {
                    all_down = false;
                    if last {
                        s.last = None;
                        s.blocked = false;
                    }
                }
            }
            let inputs = all_down && s.last.zip(s.any).is_some_and(|(last, any)| last >= any);
            if !all_down && s.active {
                s.active = false;
                effects.push(Effect::Release(s.id.clone()));
            }
            if all_up {
                s.any = None;
                s.blocked = false;
            }
            if allowed && !s.blocked && inputs && !s.active {
                s.active = true;
                s.next = time + 0.5;
                effects.push(Effect::Press(s.id.clone()));
            }
            if s.repeat && inputs && s.active && s.next < time {
                s.next += 0.02;
                effects.push(Effect::Repeat(s.id.clone()));
            }
            if s.active {
                active.push(index);
            }
        }
        self.previous = down;
        effects
    }
}
