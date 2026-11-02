//! Preserve accepted native ordinal tokens while using the source converter's
//! string representation when publishing a new unknown enum value.
use serde_json::Value;

pub(crate) fn unknown_ordinal(ordinal: i32, previous: Option<&Value>) -> Value {
    if let Some(previous) = previous {
        let parsed = previous
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .or_else(|| previous.as_str().and_then(|s| s.parse::<i32>().ok()));
        if parsed == Some(ordinal) {
            return previous.clone();
        }
    }
    Value::String(ordinal.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_unknown_values_are_strings_and_existing_native_tokens_stay_exact() {
        for ordinal in [i32::MIN, -3, 99, i32::MAX] {
            assert_eq!(
                unknown_ordinal(ordinal, None),
                Value::String(ordinal.to_string())
            );
            for token in [Value::from(ordinal), Value::String(format!("{ordinal:+}"))] {
                assert_eq!(unknown_ordinal(ordinal, Some(&token)), token);
            }
            assert_eq!(
                unknown_ordinal(ordinal, Some(&Value::Null)),
                Value::String(ordinal.to_string())
            );
        }
    }
}
