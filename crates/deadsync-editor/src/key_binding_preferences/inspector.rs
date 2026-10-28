//! Keyboard-only binding drafts. Capture and confirmation are separate so Enter
//! and Escape can themselves be bound without being swallowed as dialog actions.
use super::{Action, Preferences};
use crate::{Error, key_commands::Key};
use std::{collections::BTreeSet, sync::Arc};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapturePhase {
    WaitingOpeningRelease,
    Recording,
    Review,
}
#[derive(Clone, Debug)]
pub struct Capture {
    phase: CapturePhase,
    keys: Vec<String>,
}
impl Capture {
    pub fn new() -> Self {
        Self {
            phase: CapturePhase::WaitingOpeningRelease,
            keys: Vec::new(),
        }
    }
    pub fn phase(&self) -> CapturePhase {
        self.phase.clone()
    }
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
    pub fn observe(&mut self, physical_down: &[String]) {
        if self.phase == CapturePhase::Review {
            return;
        }
        if self.phase == CapturePhase::WaitingOpeningRelease {
            if physical_down.is_empty() {
                self.phase = CapturePhase::Recording;
            }
            return;
        }
        for name in physical_down {
            let name = Key::named(name).name().to_owned();
            if !self.keys.contains(&name) {
                self.keys.push(name);
            }
        }
        if physical_down.is_empty() && !self.keys.is_empty() {
            self.phase = CapturePhase::Review;
        }
    }
}
#[derive(Clone)]
pub struct Draft {
    before: Arc<Preferences>,
    staged: Arc<Preferences>,
    pub action: usize,
    pub alternative: usize,
}
impl Draft {
    pub fn capture(before: Arc<Preferences>) -> Result<Self, Error> {
        before.validate()?;
        Ok(Self {
            staged: Arc::clone(&before),
            before,
            action: 0,
            alternative: 0,
        })
    }
    pub fn baseline(&self) -> Arc<Preferences> {
        Arc::clone(&self.before)
    }
    pub fn staged(&self) -> Arc<Preferences> {
        Arc::clone(&self.staged)
    }
    pub fn changed(&self) -> bool {
        self.before != self.staged
    }
    pub fn action(&self) -> Action {
        Action::ALL[self.action % Action::ALL.len()]
    }
    pub fn alternatives(&self) -> &[Vec<String>] {
        &self.staged.commands[self.action().id()]
    }
    pub fn select_action(&mut self, forward: bool) {
        self.action =
            (self.action + if forward { 1 } else { Action::ALL.len() - 1 }) % Action::ALL.len();
        self.alternative = 0;
    }
    fn update(&mut self, change: impl FnOnce(&mut Vec<Vec<String>>)) -> Result<(), Error> {
        let mut next = (*self.staged).clone();
        change(
            next.commands
                .get_mut(self.action().id())
                .expect("validated action"),
        );
        next.validate()?;
        self.staged = Arc::new(next);
        self.alternative = self.alternative.min(self.alternatives().len() - 1);
        Ok(())
    }
    pub fn replace(&mut self, keys: Vec<String>) -> Result<(), Error> {
        let index = self.alternative;
        self.update(|alternatives| alternatives[index] = keys)
    }
    pub fn add(&mut self) -> Result<(), Error> {
        self.update(|alternatives| alternatives.push(Vec::new()))?;
        self.alternative = self.alternatives().len() - 1;
        Ok(())
    }
    pub fn delete(&mut self) -> Result<(), Error> {
        let index = self.alternative;
        self.update(|alternatives| {
            alternatives.remove(index);
            if alternatives.is_empty() {
                alternatives.push(Vec::new());
            }
        })
    }
    pub fn reset_action(&mut self) -> Result<(), Error> {
        let defaults = Preferences::default()
            .commands
            .remove(self.action().id())
            .expect("default action");
        self.update(|alternatives| *alternatives = defaults)
    }
}
/// Merge only command IDs changed by the draft, retaining unrelated external
/// edits. Both baseline comparison and validation happen before returning data.
pub fn merge(
    current: &Preferences,
    before: &Preferences,
    after: &Preferences,
) -> Result<Preferences, Error> {
    let mut result = current.clone();
    for action in Action::ALL {
        let id = action.id();
        if before.commands[id] == after.commands[id] {
            continue;
        }
        if current.commands[id] != before.commands[id] {
            return Err(Error::ExternalChange);
        }
        result
            .commands
            .insert(id.into(), after.commands[id].clone());
    }
    result.validate()?;
    Ok(result)
}
/// Source conflict equality ignores modifier spelling, preserving key order only
/// for activation. Useful for warning summaries and duplicate alternatives.
pub fn key_set(keys: &[String]) -> BTreeSet<Key> {
    keys.iter().map(|key| Key::named(key)).collect()
}
