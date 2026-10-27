//! Lexical range of an existing chart type; decoding stays in Document's codec.
use super::{decode, skip_comment};
use crate::Error;
use std::ops::Range;

pub(super) fn span(field: &[u8]) -> Result<Range<usize>, Error> {
    let mut pos = 0;
    let mut start = None;
    let mut end = 0;
    while pos < field.len() {
        if field[pos..].starts_with(b"//") {
            pos = skip_comment(field, pos);
            continue;
        }
        let begin = pos;
        if field[pos] == b'\\' && pos + 1 < field.len() {
            pos += 1;
        }
        // Keep a complete UTF-8 scalar with an escape, if present. Invalid
        // UTF-8 stays a byte unit and is checked by the existing decoder.
        let width = match field[pos] {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 1,
        };
        pos = (pos + width).min(field.len());
        if !decode(&field[begin..pos]).is_empty() {
            start.get_or_insert(begin);
            end = pos;
        }
    }
    let start = start.ok_or_else(|| Error::InvalidFormat("Chart layout token is empty".into()))?;
    let range = start..end;
    if decode(&field[range.clone()]) != decode(field)
        || !decode(&field[..start]).is_empty()
        || !decode(&field[end..]).is_empty()
    {
        return Err(Error::InvalidFormat(
            "Chart layout token is ambiguous".into(),
        ));
    }
    Ok(range)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_uses_raw_spelling_and_preserves_surrounding_comments() {
        for value in ["pump_double", "pump\\_double", "pump\\-double", "8"] {
            let field = format!(" \n// pump-double before\n {value} // after\n ");
            let range = span(field.as_bytes()).unwrap();
            assert_eq!(&field[range], value);
        }
        for field in ["", " \t", "// no token", " // no token\n "] {
            assert!(span(field.as_bytes()).is_err());
        }
        let field = "\u{2003}pump_double\u{2003}";
        assert_eq!(&field[span(field.as_bytes()).unwrap()], "pump_double");
    }
}
