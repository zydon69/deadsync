//! The shell installs the editor's own data path before workers are started.
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
#[derive(PartialEq, Eq)]
struct Paths {
    configs: PathBuf,
    preferences: PathBuf,
}
static PATHS: OnceLock<Paths> = OnceLock::new();
pub fn init_paths(configs: PathBuf, preferences: PathBuf) -> Result<(), &'static str> {
    if !configs.is_absolute() || !preferences.is_absolute() {
        return Err("Editor data paths must be absolute");
    }
    let paths = Paths {
        configs,
        preferences,
    };
    if let Some(current) = PATHS.get() {
        return if current == &paths {
            Ok(())
        } else {
            Err("Editor data paths already initialized")
        };
    }
    PATHS
        .set(paths)
        .map_err(|_| "Editor data paths already initialized")
}
pub fn configs_dir() -> Option<&'static Path> {
    PATHS.get().map(|paths| paths.configs.as_path())
}

pub fn preferences_path() -> Option<&'static Path> {
    PATHS.get().map(|paths| paths.preferences.as_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_paths_are_explicit_absolute_and_stable() {
        assert!(
            init_paths(
                PathBuf::from("relative-configs"),
                PathBuf::from("relative-prefs")
            )
            .is_err()
        );
        let root =
            std::env::temp_dir().join(format!("deadsync-editor-runtime-{}", std::process::id()));
        init_paths(root.clone(), root.join("editor-preferences.json")).unwrap();
        assert_eq!(configs_dir(), Some(root.as_path()));
        assert_eq!(
            preferences_path(),
            Some(root.join("editor-preferences.json").as_path())
        );
        assert!(init_paths(root.clone(), PathBuf::from("relative-prefs")).is_err());
        init_paths(root.clone(), root.join("editor-preferences.json")).unwrap();
        assert!(init_paths(root.join("other"), root.join("other.json")).is_err());
    }
}
