//! Source PackLoadTask banner inference. Directory scanning belongs on a worker.
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Cancelled,
    Io(String),
    Budget,
    NonUnicode,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Pack banner scan cancelled"),
            Self::Io(s) => write!(f, "Pack banner scan failed: {s}"),
            Self::Budget => f.write_str("Pack banner directory exceeds metadata scan budget"),
            Self::NonUnicode => {
                f.write_str("Pack banner filename cannot be represented as Unicode")
            }
        }
    }
}
impl std::error::Error for Error {}
/// Original FileInfoNameComparer uses StringComparison.Ordinal, not culture.
/// UTF-16 comparison matters for supplementary characters vs BMP filenames.
pub fn select<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let mut best: Option<(usize, &str)> = None;
    for name in names {
        let extension = Path::new(name).extension().and_then(OsStr::to_str);
        let priority = match extension {
            Some("png") => 0,
            Some("jpg") => 1,
            Some("jpeg") => 2,
            Some("gif") => 3,
            Some("bmp") => 4,
            _ => continue,
        };
        if best.is_none_or(|(rank, old)| {
            priority < rank
                || (priority == rank && name.encode_utf16().cmp(old.encode_utf16()).is_lt())
        }) {
            best = Some((priority, name));
        }
    }
    best.map(|(_, name)| name)
}
/// The shared pack metadata byte budget bounds scanned filename memory. Never
/// truncate and report a guessed winner: over-budget scans return an error.
pub fn infer(directory: &Path, cancelled: &AtomicBool) -> Result<Option<PathBuf>, Error> {
    infer_with_budget(directory, cancelled, super::MAX_BYTES)
}
fn infer_with_budget(
    directory: &Path,
    cancelled: &AtomicBool,
    budget: usize,
) -> Result<Option<PathBuf>, Error> {
    let check = || {
        if cancelled.load(Ordering::Acquire) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    };
    check()?;
    let entries = fs::read_dir(directory).map_err(|e| Error::Io(e.to_string()))?;
    let mut names = Vec::new();
    let mut bytes = 0usize;
    for entry in entries {
        check()?;
        let entry = entry.map_err(|e| Error::Io(e.to_string()))?;
        let name = entry.file_name();
        bytes = bytes
            .checked_add(
                name.as_encoded_bytes()
                    .len()
                    .saturating_add(std::mem::size_of::<String>()),
            )
            .ok_or(Error::Budget)?;
        if bytes > budget {
            return Err(Error::Budget);
        }
        let kind = entry.file_type().map_err(|e| Error::Io(e.to_string()))?;
        if kind.is_dir() {
            continue;
        }
        if kind.is_symlink() {
            match fs::metadata(entry.path()) {
                Ok(meta) if meta.is_dir() => continue,
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(Error::Io(error.to_string())),
            }
        }
        let name = name.into_string().map_err(|_| Error::NonUnicode)?;
        names.push(name);
    }
    check()?;
    Ok(select(names.iter().map(String::as_str)).map(|name| directory.join(name)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_priority_case_and_utf16_ordinal_are_exact() {
        assert_eq!(
            select(["0.JPG", "z.jpg", "z.png", "A.png", "a.PNG"]),
            Some("A.png")
        );
        assert_eq!(
            select(["\u{e000}.png", "\u{10000}.png"]),
            Some("\u{10000}.png")
        );
        assert_eq!(select(["a.PNG", "b.Jpg"]), None);
    }
    #[test]
    fn budget_never_returns_a_partial_winner() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.png"), b"x").unwrap();
        assert_eq!(
            infer_with_budget(root.path(), &AtomicBool::new(false), 1),
            Err(Error::Budget)
        );
    }
    #[test]
    fn real_scan_ignores_directories_and_cancels() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("0.png")).unwrap();
        fs::write(root.path().join("é.png"), b"not decoded by scan").unwrap();
        fs::write(root.path().join("z.jpg"), b"x").unwrap();
        let flag = AtomicBool::new(false);
        assert_eq!(
            infer(root.path(), &flag).unwrap(),
            Some(root.path().join("é.png"))
        );
        flag.store(true, Ordering::Release);
        assert_eq!(infer(root.path(), &flag), Err(Error::Cancelled));
    }
}
#[cfg(test)]
mod original_reference_tests {
    use super::*;
    #[test]
    fn original_directory_scan_choices_match_all_recorded_cultures() {
        let rows: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/pack-banner/expected.json"
        ))
        .unwrap();
        for row in rows.as_array().unwrap() {
            if row["culture"] == "links" {
                continue;
            }
            let names = row["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap());
            assert_eq!(
                select(names),
                row["selected"].as_str(),
                "{}",
                row["culture"]
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn original_symlink_directory_and_dangling_file_rules() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        symlink(target.path(), root.path().join("0.png")).unwrap();
        symlink(root.path().join("missing"), root.path().join("1.png")).unwrap();
        assert_eq!(
            infer(root.path(), &AtomicBool::new(false)).unwrap(),
            Some(root.path().join("1.png"))
        );
    }
}
