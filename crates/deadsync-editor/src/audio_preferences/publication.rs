//! One publication boundary for typed global preference owners.
//! Both baselines must pass before any disk or memory publication occurs.
use super::File;
use crate::Error;

/// Publish a typed changed-leaf transaction against independent runtime and
/// persisted baselines. The merger must be pure and return a private candidate.
pub fn publish<T>(
    owner: &mut Option<File>,
    live: Option<&File>,
    requested: T,
    merge: impl Fn(&File) -> Result<(T, File), Error>,
) -> Result<T, Error> {
    let runtime = live.map(&merge).transpose()?.map(|(value, _)| value);
    let Some(previous) = owner.as_ref() else {
        return Ok(runtime.unwrap_or(requested));
    };
    let baseline = if previous.path().as_os_str().is_empty() {
        live.unwrap_or(previous).clone()
    } else {
        File::open(previous.path())?
    };
    let (value, mut next) = merge(&baseline)?;
    if next.path().as_os_str().is_empty() {
        next.commit_memory_preferences()?;
    } else {
        // save returns success after rename even if directory sync is pending;
        // its canonical receipt survives owner/history replacement.
        next.save()?;
    }
    *owner = Some(next);
    Ok(runtime.unwrap_or(value))
}

/// Merge into a private candidate. A conflict never publishes a partial tree.
pub(crate) fn merge_fields(
    current: &mut serde_json::Map<String, serde_json::Value>,
    before: &serde_json::Map<String, serde_json::Value>,
    after: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), Error> {
    use serde_json::Value;
    for (key, value) in after {
        let expected = before.get(key);
        if expected == Some(value) {
            continue;
        }
        if let (Some(Value::Object(old)), Value::Object(new), Some(Value::Object(now))) =
            (expected, value, current.get_mut(key))
        {
            merge_fields(now, old, new)?;
        } else {
            if current.get(key) != expected {
                return Err(Error::ExternalChange);
            }
            current.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}

/// Opaque identity for a deferred global-preference edit. Every executed action,
/// Undo and Redo renews this identity; restoring a numeric revision cannot revive it.
pub struct EditBinding(crate::history::ApplicationReviewToken);
impl EditBinding {
    pub fn capture(session:&crate::history::Session)->Self{Self(session.application_review_token())}
    pub fn is_current(&self,session:&crate::history::Session)->bool{session.accepts_application_review(&self.0)}
}
