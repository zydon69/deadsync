//! Source File preferences; reading launch intent never opens or rewrites a song.
use super::{Error, File, Path, PathBuf, Value};
#[derive(Clone, Debug, PartialEq)]
pub enum StartupTarget {
    Explicit(PathBuf),
    Recent(crate::recent_songs::Song),
    New,
}
impl File {
    pub fn open_last_on_launch(&self) -> Result<bool, Error> {
        Ok(self.settings.general_options.open_last)
    }
    pub fn initial_file_directory(&self) -> Result<Option<PathBuf>, Error> {
        match self.settings.initial_file_directory.as_ref() {
            None | Some(Value::Null) => Ok(default_directory()),
            Some(Value::String(value)) if value.is_empty() => Ok(default_directory()),
            Some(Value::String(value)) => Ok(Some(PathBuf::from(value))),
            _ => Err(Error::InvalidFormat(
                "Invalid OpenFileDialogInitialDirectory".into(),
            )),
        }
    }
    pub fn set_initial_file_directory(&mut self, directory: &Path) -> Result<(), Error> {
        let value = directory.to_str().ok_or_else(|| {
            Error::Unsupported("The initial file directory is not Unicode".into())
        })?;
        self.settings.initial_file_directory = Some(Value::String(value.into()));
        Ok(())
    }
    pub fn startup_target(&self, explicit: Option<&Path>) -> Result<StartupTarget, Error> {
        if let Some(path) = explicit {
            return Ok(StartupTarget::Explicit(path.to_owned()));
        }
        if self.open_last_on_launch()?
            && let Some(record) = self.recent_songs().entries.first()
        {
            return Ok(StartupTarget::Recent(record.song()?));
        }
        Ok(StartupTarget::New)
    }
}
fn default_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        return Some(PathBuf::from(r"C:\Games\StepMania 5\Songs\"));
    }
    #[cfg(target_os = "linux")]
    {
        return Some(PathBuf::from("/opt/itgmania/Songs/"));
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn prefs(value: &str) -> (tempfile::TempDir, File) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        std::fs::write(&path, value).unwrap();
        let file = File::open(&path).unwrap();
        (root, file)
    }
    #[test]
    fn explicit_launch_has_priority_and_recent_retains_source_descriptors() {
        let (_root, file) = prefs(
            r#"{"Version":1,"Options":{"OpenLastOpenedFileOnLaunch":true},"RecentFiles":[{"FileName":"/songs/recent.ssc","SongName":"Recent","SpacingZoom":2.5,"ChartPosition":160.5,"ActiveCharts":[],"FocusedChartIndex":-1}]}"#,
        );
        assert_eq!(
            file.startup_target(Some(Path::new("/songs/explicit.ssc")))
                .unwrap(),
            StartupTarget::Explicit(PathBuf::from("/songs/explicit.ssc"))
        );
        let StartupTarget::Recent(song) = file.startup_target(None).unwrap() else {
            panic!("expected recent")
        };
        assert_eq!(song.file, Path::new("/songs/recent.ssc"));
        assert_eq!(song.chart_position, 160.5);
        assert_eq!(song.spacing_zoom, 2.5);
    }
    #[test]
    fn launch_default_disabled_malformed_and_initial_directory_values() {
        let (_root, file) = prefs(r#"{"Version":1}"#);
        assert!(file.open_last_on_launch().unwrap());
        assert_eq!(file.startup_target(None).unwrap(), StartupTarget::New);
        let (_root, file) = prefs(
            r#"{"Version":1,"Options":{"OpenLastOpenedFileOnLaunch":false},"OpenFileDialogInitialDirectory":"/songs/custom/"}"#,
        );
        assert_eq!(file.startup_target(None).unwrap(), StartupTarget::New);
        assert_eq!(
            file.initial_file_directory().unwrap(),
            Some(PathBuf::from("/songs/custom/"))
        );
        let (_root, file) = prefs(
            r#"{"Version":1,"Options":{"OpenLastOpenedFileOnLaunch":false},"OpenFileDialogInitialDirectory":23}"#,
        );
        assert_eq!(file.startup_target(None).unwrap(), StartupTarget::New);
        assert!(file.initial_file_directory().is_err());
        assert!(matches!(
            file.startup_target(Some(Path::new("/explicit.ssc")))
                .unwrap(),
            StartupTarget::Explicit(_)
        ));
    }
}

#[cfg(test)]
mod strict_general_import {
    use super::*;
    #[test]
    fn malformed_launch_flag_refuses_import_without_rewriting() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        let bytes = br#"{"Version":1,"Options":{"OpenLastOpenedFileOnLaunch":"yes"}}"#;
        std::fs::write(&path, bytes).unwrap();
        assert!(File::open(&path).is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[cfg(test)]
mod directory_owner_tests {
    use super::*;
    #[test]
    fn selected_directory_is_a_real_mutation_cas_saved_and_reopened_without_touching_vendor() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("prefs.json");
        let initial =
            br#"{"Version":1,"Vendor":{"keep":7},"OpenFileDialogInitialDirectory":"old"}"#;
        std::fs::write(&path, initial).unwrap();
        let mut file = File::open(&path).unwrap();
        let chosen = root.path().join("chosen");
        file.set_initial_file_directory(&chosen).unwrap();
        assert!(file.has_changes());
        assert_eq!(file.initial_file_directory().unwrap(), Some(chosen.clone()));
        let other = br#"{"Version":1,"Vendor":{"keep":8},"OpenFileDialogInitialDirectory":"old"}"#;
        std::fs::write(&path, other).unwrap();
        assert!(matches!(file.save(), Err(Error::ExternalChange)));
        assert_eq!(file.initial_file_directory().unwrap(), Some(chosen.clone()));
        assert_eq!(std::fs::read(&path).unwrap(), other);
        std::fs::write(&path, initial).unwrap();
        file.save().unwrap();
        let reopened = File::open(&path).unwrap();
        assert_eq!(reopened.initial_file_directory().unwrap(), Some(chosen));
        let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["Vendor"]["keep"], 7);
    }
}
