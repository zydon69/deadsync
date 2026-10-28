//! Windows publication barrier; no copy/delete or reboot fallback is permitted.
//! Undeclared until the separate Windows writer patch is integrated and validated.
use std::{fs::File, io, os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut result: Vec<u16> = path.as_os_str().encode_wide().collect();
    if result.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Save As path contains NUL",
        ));
    }
    result.push(0);
    Ok(result)
}

/// Returns only after the documented WRITE_THROUGH move has succeeded.
/// The caller supplies a temp in the same reviewed parent as destination.
/// This is not compare-and-swap protection against a noncooperating writer.
pub(super) fn publish(
    pending: tempfile::NamedTempFile,
    destination: &Path,
    absent: bool,
) -> io::Result<File> {
    // File::sync_all maps to FlushFileBuffers on Windows. Flush file contents
    // before the namespace operation; a flush failure leaves destination alone.
    pending.as_file().sync_all()?;
    let source = wide(pending.path())?;
    let destination = wide(destination)?;
    let flags = MOVEFILE_WRITE_THROUGH | if absent { 0 } else { MOVEFILE_REPLACE_EXISTING };
    // SAFETY: both buffers are NUL-terminated and live through this call.
    // The API does not retain pointers. No COPY_ALLOWED or DELAY_UNTIL_REBOOT.
    let success = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }
    let (file, mut old_path) = pending.into_parts();
    // The kernel consumed the old pathname. Never unlink a later unrelated
    // object that a noncooperating process could create at that old name.
    old_path.disable_cleanup(true);
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write};

    fn pending(parent: &Path, bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new_in(parent).unwrap();
        file.write_all(bytes).unwrap();
        file
    }

    #[test]
    fn absent_target_and_reviewed_overwrite_publish_exact_object() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("été.ssc");
        let file = publish(pending(dir.path(), b"first"), &dest, true).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"first");
        assert_eq!(
            super::super::windows_identity::file(&file).unwrap(),
            super::super::windows_identity::file(&File::open(&dest).unwrap()).unwrap()
        );
        publish(pending(dir.path(), b"second"), &dest, false).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"second");
    }

    #[test]
    fn no_clobber_failure_and_directory_failure_preserve_destination() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("existing.ssc");
        fs::write(&dest, b"original").unwrap();
        assert!(publish(pending(dir.path(), b"replacement"), &dest, true).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"original");
        assert!(publish(pending(dir.path(), b"replacement"), dir.path(), false).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"original");
    }

    #[test]
    fn nul_path_is_rejected_before_publication() {
        assert!(wide(Path::new("invalid\0name.ssc")).is_err());
    }
}
