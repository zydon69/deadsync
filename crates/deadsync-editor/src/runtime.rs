//! The shell installs the editor's own data path before workers are started.
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
static CONFIGS: OnceLock<PathBuf> = OnceLock::new();
pub fn init_paths(configs: PathBuf) -> Result<(), &'static str> {
    if !configs.is_absolute() {
        return Err("Editor configuration directory must be absolute");
    }
    if let Some(current) = CONFIGS.get() {
        return if current == &configs {
            Ok(())
        } else {
            Err("Editor configuration directory already initialized")
        };
    }
    CONFIGS
        .set(configs)
        .map_err(|_| "Editor configuration directory already initialized")
}
pub fn configs_dir() -> Option<&'static Path> {
    CONFIGS.get().map(PathBuf::as_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_paths_are_explicit_absolute_and_stable() {
        assert!(init_paths(PathBuf::from("relative-configs")).is_err());
        let root =
            std::env::temp_dir().join(format!("deadsync-editor-runtime-{}", std::process::id()));
        init_paths(root.clone()).unwrap();
        assert_eq!(configs_dir(), Some(root.as_path()));
        init_paths(root.clone()).unwrap();
        assert!(init_paths(root.join("other")).is_err());
    }
}
