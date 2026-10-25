//! A persistent pattern plan owns metadata and immutable profile dependencies.
//! Preview preparation is read-only; profile creation happens on acceptance.
use super::{
    metadata::{ChartData, Definition, Registry},
    pattern_job::Options,
    profiles::{Kind, Loaded, Profile},
};
use crate::Error;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
pub struct Plan {
    pub data: ChartData,
    directory: Option<PathBuf>,
    profiles: Vec<Profile>,
    dependencies: Vec<(PathBuf, Arc<[u8]>)>,
}
impl Plan {
    pub fn create(
        mut data: ChartData,
        options: &Options,
        loaded: &Loaded,
        directory: Option<PathBuf>,
    ) -> Result<Self, Error> {
        if data
            .patterns
            .as_ref()
            .is_some_and(|p| p.contains_key(&options.span.start))
        {
            return Err(Error::InvalidEdit(
                "A saved pattern already exists at this row; regenerate or edit it explicitly"
                    .into(),
            ));
        }
        let builtins = Registry::builtin();
        let mut profiles = Vec::new();
        let pattern = if let Some((id, _)) = builtins
            .patterns
            .iter()
            .find(|(_, cfg)| *cfg == &options.pattern)
        {
            id.clone()
        } else if let Some((id, _)) = loaded
            .registry
            .patterns
            .iter()
            .find(|(_, cfg)| *cfg == &options.pattern)
        {
            id.clone()
        } else {
            if directory.is_none() {
                return Err(Error::InvalidEdit(
                    "A profile directory is required for a custom pattern".into(),
                ));
            }
            let profile = Profile::new_pattern(
                &format!("1/{}", options.pattern.beat_sub_division * 4),
                options.pattern.clone(),
            )?;
            let id = profile.guid().clone();
            profiles.push(profile);
            id
        };
        let performed = builtins
            .performed
            .iter()
            .find(|(_, cfg)| *cfg == &options.performed)
            .or_else(|| {
                loaded
                    .registry
                    .performed
                    .iter()
                    .find(|(_, cfg)| *cfg == &options.performed)
            })
            .map(|(id, _)| id.clone())
            .ok_or_else(|| {
                Error::Unsupported(
                    "Register the custom performed profile before saving this marker".into(),
                )
            })?;
        let expressed = if loaded.registry.expressed.get(&data.expressed_chart_config)
            == Some(&options.expressed)
        {
            data.expressed_chart_config.clone()
        } else {
            builtins
                .expressed
                .iter()
                .find(|(_, cfg)| *cfg == &options.expressed)
                .or_else(|| {
                    loaded
                        .registry
                        .expressed
                        .iter()
                        .find(|(_, cfg)| *cfg == &options.expressed)
                })
                .map(|(id, _)| id.clone())
                .ok_or_else(|| {
                    Error::Unsupported(
                        "Register the custom expressed profile before saving this marker".into(),
                    )
                })?
        };
        let mut dependencies = Vec::new();
        for (kind, id) in [
            (Kind::Pattern, &pattern),
            (Kind::Performed, &performed),
            (Kind::Expressed, &expressed),
        ] {
            if let Some(file) = loaded.profiles.get(&(kind, id.clone())) {
                dependencies.push(file.snapshot());
            }
        }
        data.expressed_chart_config = expressed;
        data.patterns.get_or_insert_with(BTreeMap::new).insert(
            options.span.start,
            Definition {
                pattern_config_guid: pattern,
                performed_chart_config_guid: performed,
                length: options.span.end - options.span.start,
                random_seed: options.seed,
                start_position_inclusive: options.span.include_start,
                end_position_inclusive: options.span.include_end,
                ignore_preceding_distribution: options.ignore_preceding_distribution,
                ..Definition::default()
            },
        );
        data.validate()?;
        Ok(Self {
            data,
            directory,
            profiles,
            dependencies,
        })
    }
    pub fn ensure_profiles(&self) -> Result<(), Error> {
        let check = || -> Result<(), Error> {
            for (path, expected) in &self.dependencies {
                if fs::read(path)?.as_slice() != expected.as_ref() {
                    return Err(Error::ExternalChange);
                }
            }
            Ok(())
        };
        check()?;
        for profile in &self.profiles {
            let directory = self
                .directory
                .as_ref()
                .ok_or_else(|| Error::InvalidEdit("No profile directory".into()))?;
            let expected = profile.serialize()?;
            let path = directory.join(profile.filename());
            if path.exists() {
                if fs::read(&path)? != expected {
                    return Err(Error::ExternalChange);
                }
                continue;
            }
            if let Err(error) = profile.export_new(directory) {
                // A repeated acceptance after undo may reuse the same immutable
                // profile. Concurrent creation is allowed only for exact bytes.
                if fs::read(&path).ok().as_deref() != Some(expected.as_slice()) {
                    return Err(error);
                }
            }
        }
        check()
    }
}
