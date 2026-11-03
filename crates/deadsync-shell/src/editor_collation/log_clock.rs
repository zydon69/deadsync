//! ICU-backed formatting of source UILog's four civil timestamp modes.
use super::{Collator, Error};
use std::{
    ffi::{CString, c_char, c_void},
    ptr::NonNull,
};
type PatternOpen = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;
type Close = unsafe extern "C" fn(*mut c_void);
type Best = unsafe extern "C" fn(*const c_void, *const u16, i32, *mut u16, i32, *mut i32) -> i32;
type DateOpen = unsafe extern "C" fn(
    i32,
    i32,
    *const c_char,
    *const u16,
    i32,
    *const u16,
    i32,
    *mut i32,
) -> *mut c_void;
type Format = unsafe extern "C" fn(*const c_void, f64, *mut u16, i32, *mut c_void, *mut i32) -> i32;
#[derive(Clone, Copy)]
struct Functions {
    pattern_open: PatternOpen,
    pattern_close: Close,
    best: Best,
    date_open: DateOpen,
    date_close: Close,
    format: Format,
}
impl Functions {
    unsafe fn load(collator: &Collator) -> Result<Self, Error> {
        unsafe fn get<T: Copy>(c: &Collator, name: &str) -> Result<T, Error> {
            let name = format!("{name}{}\0", c.symbol_suffix);
            unsafe { c._library.get::<T>(name.as_bytes()) }
                .map(|s| *s)
                .map_err(|_| Error("Native ICU log formatter is unavailable".into()))
        }
        // SAFETY: exact stable ICU C signatures, all resolved from the common
        // Collator's owned library and the very same detected major-version suffix.
        unsafe {
            Ok(Self {
                pattern_open: get(collator, "udatpg_open")?,
                pattern_close: get(collator, "udatpg_close")?,
                best: get(collator, "udatpg_getBestPattern")?,
                date_open: get(collator, "udat_open")?,
                date_close: get(collator, "udat_close")?,
                format: get(collator, "udat_format")?,
            })
        }
    }
}
pub(crate) struct LogClock {
    handle: NonNull<c_void>,
    functions: Functions,
    _collator: Collator,
}
impl Drop for LogClock {
    fn drop(&mut self) {
        unsafe {
            (self.functions.date_close)(self.handle.as_ptr());
        }
    }
}
fn separator(pattern: &str) -> Result<String, Error> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut quoted = false;
    let mut hour_end = None;
    let mut minute = None;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' {
            if i + 1 < chars.len() && chars[i + 1] == '\'' {
                i += 2;
                continue;
            }
            quoted = !quoted;
            i += 1;
            continue;
        }
        if !quoted && matches!(ch, 'H' | 'h' | 'K' | 'k') {
            while i + 1 < chars.len() && chars[i + 1] == ch {
                i += 1;
            }
            hour_end = Some(i + 1);
        }
        if !quoted && ch == 'm' && hour_end.is_some() {
            minute = Some(i);
            break;
        }
        i += 1;
    }
    let (a, b) = (
        hour_end.ok_or_else(|| Error("ICU time pattern has no hour".into()))?,
        minute.ok_or_else(|| Error("ICU time pattern has no minute".into()))?,
    );
    let raw: String = chars[a..b].iter().collect();
    let separator = raw
        .replace("''", "\u{0}")
        .replace('\'', "")
        .replace('\0', "'");
    if separator.is_empty() || separator.chars().count() > 16 {
        return Err(Error("ICU returned an invalid time separator".into()));
    }
    Ok(separator)
}
impl LogClock {
    pub(crate) fn new(language: &str, style: i32) -> Result<Self, Error> {
        if !(1..=3).contains(&style) {
            return Err(Error("Unsupported log date style".into()));
        }
        let collator = Collator::for_language(language)?;
        let functions = unsafe { Functions::load(&collator)? };
        let locale = CString::new(format!("{}@numbers=latn", collator.culture))
            .map_err(|_| Error("Invalid log culture".into()))?;
        let mut status = 0;
        let generator = unsafe { (functions.pattern_open)(locale.as_ptr(), &mut status) };
        if generator.is_null() || status > 0 {
            if !generator.is_null() {
                unsafe { (functions.pattern_close)(generator) }
            }
            return Err(Error(format!("ICU time pattern open failed: {status}")));
        }
        let skeleton: Vec<u16> = "Hms".encode_utf16().collect();
        let mut pattern = [0u16; 256];
        status = 0;
        let length = unsafe {
            (functions.best)(
                generator,
                skeleton.as_ptr(),
                skeleton.len() as i32,
                pattern.as_mut_ptr(),
                pattern.len() as i32,
                &mut status,
            )
        };
        unsafe { (functions.pattern_close)(generator) };
        if status > 0 || length <= 0 || length as usize > pattern.len() {
            return Err(Error("ICU time pattern failed".into()));
        }
        let pattern = String::from_utf16(&pattern[..length as usize])
            .map_err(|_| Error("ICU time pattern is invalid UTF16".into()))?;
        let separator = separator(&pattern)?.replace('\'', "''");
        // C# ':' expands to DateTimeFormatInfo.TimeSeparator, while the dot before
        // fff is literal. Latin digits match .NET custom DateTime formatting; the
        // locale's default calendar (e.g. ar-SA UmAlQura) remains effective.
        let time = format!(
            "HH'{separator}'mm'{separator}'ss{}",
            if style >= 2 { ".SSS" } else { "" }
        );
        let pattern = if style == 3 {
            format!("yyyy-MM-dd {time}")
        } else {
            time
        };
        let pattern: Vec<u16> = pattern.encode_utf16().collect();
        let zone: Vec<u16> = "UTC".encode_utf16().collect();
        status = 0;
        let handle = unsafe {
            (functions.date_open)(
                -2,
                -2,
                locale.as_ptr(),
                zone.as_ptr(),
                zone.len() as i32,
                pattern.as_ptr(),
                pattern.len() as i32,
                &mut status,
            )
        };
        let handle = NonNull::new(handle)
            .ok_or_else(|| Error(format!("ICU log date open failed: {status}")))?;
        if status > 0 {
            unsafe { (functions.date_close)(handle.as_ptr()) };
            return Err(Error(format!("ICU log date open failed: {status}")));
        }
        Ok(Self {
            handle,
            functions,
            _collator: collator,
        })
    }
    pub(crate) fn format_civil_millis(&mut self, millis: i64) -> Result<String, Error> {
        let mut text = [0u16; 256];
        let mut status = 0;
        let length = unsafe {
            (self.functions.format)(
                self.handle.as_ptr(),
                millis as f64,
                text.as_mut_ptr(),
                text.len() as i32,
                std::ptr::null_mut(),
                &mut status,
            )
        };
        if status > 0 || length < 0 || length as usize > text.len() {
            return Err(Error(format!("ICU log date format failed: {status}")));
        }
        String::from_utf16(&text[..length as usize])
            .map_err(|_| Error("ICU log date is invalid UTF16".into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn separators_come_from_patterns_not_language_heuristics() {
        assert_eq!(separator("HH.mm.ss").unwrap(), ".");
        assert_eq!(separator("HH':'mm':'ss").unwrap(), ":");
        assert!(separator("invalid").is_err());
    }
}
