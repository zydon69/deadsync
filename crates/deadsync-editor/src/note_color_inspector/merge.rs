//! Own changed typed leaves, not entire NoteColor/Options groups or RGB vectors.
use crate::{Error, note_color_preferences::Preferences};
use serde_json::{Map, Value};
fn fields(p: &Preferences) -> Result<Value, Error> {
    let (mut note, mut options) = (Map::new(), Map::new());
    p.write(&mut note, &mut options)?;
    Ok(serde_json::json!({"NoteColor":note,"Options":options}))
}
fn apply(current: &mut Value, before: &Value, after: &Value) -> Result<(), Error> {
    if before == after {
        return Ok(());
    }
    if let (Value::Object(current), Value::Object(before), Value::Object(after)) =
        (&mut *current, before, after)
    {
        for (key, value) in after {
            let previous = before
                .get(key)
                .ok_or_else(|| Error::InvalidEdit("Color preference schema mismatch".into()))?;
            let target = current
                .get_mut(key)
                .ok_or_else(|| Error::InvalidEdit("Color preference schema mismatch".into()))?;
            apply(target, previous, value)?;
        }
        return Ok(());
    }
    if current != before {
        return Err(Error::ExternalChange);
    }
    *current = after.clone();
    Ok(())
}
pub fn owned(
    current: &Preferences,
    before: &Preferences,
    after: &Preferences,
) -> Result<Preferences, Error> {
    let mut current = fields(current)?;
    apply(&mut current, &fields(before)?, &fields(after)?)?;
    Preferences::read(
        current["NoteColor"].as_object().unwrap(),
        current["Options"].as_object().unwrap(),
    )
}
