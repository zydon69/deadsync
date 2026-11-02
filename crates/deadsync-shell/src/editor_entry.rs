//! Product launch intent, parsed before argv conversion or the runtime chdir.
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorLaunch {
    Startup,
    Empty,
    File(PathBuf),
}
#[derive(Debug, PartialEq, Eq)]
pub struct LaunchArguments {
    pub editor: Option<EditorLaunch>,
    pub remaining: Vec<OsString>,
}
impl LaunchArguments {
    pub fn parse(
        argv: impl IntoIterator<Item = OsString>,
        caller_directory: &Path,
    ) -> Result<Self, String> {
        if !caller_directory.is_absolute() {
            return Err("Caller directory must be absolute".into());
        }
        let mut args = argv.into_iter();
        let mut remaining = vec![args.next().unwrap_or_else(|| OsString::from("deadsync"))];
        let mut editor = None;
        let mut live_case = false;
        while let Some(arg) = args.next() {
            let target = if arg == OsStr::new("--editor") {
                Some(EditorLaunch::Startup)
            } else if arg == OsStr::new("--editor-empty") {
                Some(EditorLaunch::Empty)
            } else if arg == OsStr::new("--editor-file") {
                let path = args
                    .next()
                    .ok_or_else(|| "--editor-file requires a path".to_string())?;
                if path.is_empty() {
                    return Err("--editor-file requires a nonempty path".into());
                }
                let path = PathBuf::from(path);
                Some(EditorLaunch::File(if path.is_absolute() {
                    path
                } else {
                    caller_directory.join(path)
                }))
            } else {
                None
            };
            if let Some(target) = target {
                if editor.replace(target).is_some() {
                    return Err("Choose exactly one editor launch option".into());
                }
            } else {
                if arg == OsStr::new("--perf-case")
                    || arg
                        .to_str()
                        .is_some_and(|arg| arg.starts_with("--perf-case="))
                {
                    live_case = true;
                }
                remaining.push(arg);
            }
        }
        if editor.is_some() && live_case {
            return Err("Editor launch cannot be combined with --perf-case".into());
        }
        Ok(Self { editor, remaining })
    }
    /// Updater/live-case flags remain Unicode APIs; the editor path was already
    /// separated losslessly and is never sent through this conversion.
    pub fn unicode_remaining(&self) -> Result<Vec<String>, String> {
        self.remaining
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                if index == 0 {
                    Ok("deadsync".into())
                } else {
                    // Unknown OS arguments historically did not prevent game
                    // startup. Editor paths were already extracted losslessly.
                    Ok(arg.to_string_lossy().into_owned())
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    #[test]
    fn no_editor_keeps_updater_and_console_flags() {
        let launch = LaunchArguments::parse(
            args(&["bin", "--console", "--no-update-check"]),
            Path::new("/caller"),
        )
        .unwrap();
        assert_eq!(launch.editor, None);
        assert_eq!(
            launch.unicode_remaining().unwrap(),
            ["deadsync", "--console", "--no-update-check"]
        );
    }
    #[test]
    fn relative_path_is_resolved_before_runtime_chdir_without_shell_interpretation() {
        let launch = LaunchArguments::parse(
            args(&["bin", "--editor-file", "Songs/a 'quoted' $() song.ssc"]),
            Path::new("/caller"),
        )
        .unwrap();
        assert_eq!(
            launch.editor,
            Some(EditorLaunch::File(PathBuf::from(
                "/caller/Songs/a 'quoted' $() song.ssc"
            )))
        );
        assert_eq!(launch.unicode_remaining().unwrap(), ["deadsync"]);
    }
    #[test]
    fn conflicts_missing_path_and_live_case_are_rejected() {
        for values in [
            vec!["bin", "--editor", "--editor-empty"],
            vec!["bin", "--editor-file"],
            vec!["bin", "--editor", "--perf-case", "case.json"],
        ] {
            assert!(LaunchArguments::parse(args(&values), Path::new("/caller")).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn native_non_unicode_path_survives_without_touching_filesystem() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let path = OsString::from_vec(vec![b'S', b'/', 0xff, b'.', b's', b's', b'c']);
        let launch = LaunchArguments::parse(
            vec!["bin".into(), "--editor-file".into(), path],
            Path::new("/caller"),
        )
        .unwrap();
        let Some(EditorLaunch::File(path)) = launch.editor.as_ref() else {
            panic!("file intent")
        };
        assert_eq!(path.as_os_str().as_bytes(), b"/caller/S/\xff.ssc");
        assert!(launch.unicode_remaining().is_ok());
    }
}

#[cfg(all(test, unix))]
mod audit_regressions {
    #[test]
    fn unknown_non_unicode_argument_does_not_prevent_normal_startup() {
        use std::os::unix::ffi::OsStringExt;
        let args = super::LaunchArguments::parse(
            vec!["deadsync".into(), std::ffi::OsString::from_vec(vec![255])],
            std::path::Path::new("/caller"),
        )
        .unwrap();
        assert!(args.unicode_remaining().is_ok());
    }
}
