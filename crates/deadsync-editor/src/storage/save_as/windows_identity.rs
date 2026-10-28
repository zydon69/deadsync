//! Windows identity uses the opened object, never a canonical path fallback.
//! This module is intentionally undeclared until its dependency/adapter patch is reviewed.
use std::{
    fs::{File, OpenOptions},
    io,
    mem::MaybeUninit,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub(super) volume: u64,
    pub(super) file: u128,
}

pub(super) fn file(file: &File) -> io::Result<Identity> {
    let mut info = MaybeUninit::<FILE_ID_INFO>::uninit();
    // SAFETY: File owns a valid live handle for this call. The writable buffer
    // has precisely the documented FILE_ID_INFO size; assume_init follows success only.
    let success = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            info.as_mut_ptr().cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful FileIdInfo query initialized the complete structure.
    let info = unsafe { info.assume_init() };
    Ok(Identity {
        volume: info.VolumeSerialNumber,
        file: u128::from_le_bytes(info.FileId.Identifier),
    })
}

pub(super) fn directory(path: &Path) -> io::Result<Identity> {
    // BACKUP_SEMANTICS permits a directory handle. Default std sharing retains
    // read/write/delete sharing; retaining a review does not block legitimate writers.
    let handle = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    if !handle.metadata()?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Save As parent is not a directory",
        ));
    }
    file(&handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn same_object_and_hardlink_share_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("original.ssc");
        fs::write(&path, b"same bytes").unwrap();
        let link = dir.path().join("alias.ssc");
        fs::hard_link(&path, &link).unwrap();
        let first = file(&File::open(&path).unwrap()).unwrap();
        assert_eq!(first, file(&File::open(&link).unwrap()).unwrap());
        assert_eq!(
            directory(dir.path()).unwrap(),
            directory(dir.path()).unwrap()
        );
    }

    #[test]
    fn replacement_with_identical_bytes_has_another_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("destination.ssc");
        fs::write(&path, b"same bytes").unwrap();
        let original = File::open(&path).unwrap();
        let first = file(&original).unwrap();
        fs::rename(&path, dir.path().join("retained.ssc")).unwrap();
        fs::write(&path, b"same bytes").unwrap();
        assert_ne!(first, file(&File::open(&path).unwrap()).unwrap());
        assert_eq!(first, file(&original).unwrap());
    }

    #[test]
    fn replaced_parent_is_not_the_reviewed_directory() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let identity = directory(&parent).unwrap();
        fs::rename(&parent, dir.path().join("retained-parent")).unwrap();
        fs::create_dir(&parent).unwrap();
        assert_ne!(identity, directory(&parent).unwrap());
        assert!(directory(&dir.path().join("missing")).is_err());
        let regular = parent.join("not-directory");
        fs::write(&regular, b"x").unwrap();
        assert!(directory(&regular).is_err());
    }
}
