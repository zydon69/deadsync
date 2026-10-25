//! JSON comments and trailing commas accepted by the reference configuration
//! loader. Preserve quoted strings and line positions for serde diagnostics.
use crate::Error;

pub fn strip_comments(input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut bytes = input.to_vec();
    let (mut i, mut quoted, mut escaped) = (0, false, false);
    while i < bytes.len() {
        let byte = bytes[i];
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            i += 1;
            continue;
        }
        if byte == b'"' {
            quoted = true;
            i += 1;
            continue;
        }
        if bytes.get(i..i + 2) == Some(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                bytes[i] = b' ';
                i += 1;
            }
        } else if bytes.get(i..i + 2) == Some(b"/*") {
            bytes[i] = b' ';
            bytes[i + 1] = b' ';
            i += 2;
            while i < bytes.len() && bytes.get(i..i + 2) != Some(b"*/") {
                if !matches!(bytes[i], b'\n' | b'\r') {
                    bytes[i] = b' ';
                }
                i += 1;
            }
            if i == bytes.len() {
                return Err(Error::InvalidFormat(
                    "Unterminated configuration comment".into(),
                ));
            }
            bytes[i] = b' ';
            bytes[i + 1] = b' ';
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(bytes)
}

pub fn normalize(input: &[u8]) -> Result<Vec<u8>, Error> {
    let mut bytes = strip_comments(input)?;
    let (mut quoted, mut escaped) = (false, false);
    for i in 0..bytes.len() {
        let byte = bytes[i];
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
        } else if byte == b','
            && bytes[..i]
                .iter()
                .rev()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| !matches!(b, b'[' | b'{' | b',' | b':'))
            && bytes[i + 1..]
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| matches!(b, b']' | b'}'))
        {
            bytes[i] = b' ';
        }
    }
    Ok(bytes)
}
