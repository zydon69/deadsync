//! Read-only profile catalog for native selectors. Loading runs on a bounded job;
//! selection stages identities and retains byte guards for custom dependencies.
use super::{
    job::{Job, Phase, Progress},
    metadata::{Guid, Registry},
    profiles::{Issue, Kind, Loaded},
};
use crate::{
    Error,
    history::{RevisionToken, Session},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinLabel {
    Sixteenths,
    Eighths,
    Balanced,
    StaminaSingle,
    StaminaDouble,
    PatternBalanced,
    NoCandles,
    NoInward,
    NoTransitionLimit,
    Dynamic,
    AggressiveBrackets,
    NoBrackets,
}
pub mod description;
pub mod source_sort;
#[derive(Clone)]
pub struct Entry {
    pub guid: Guid,
    pub name: Option<String>,
    pub builtin: Option<BuiltinLabel>,
    pub config: super::profiles::Config,
    pub display: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Application,
    Chart(usize),
}
pub struct Task {
    token: RevisionToken,
    context: Context,
    directory: Option<PathBuf>,
}
pub struct Catalog {
    token: RevisionToken,
    context: Context,
    entries: BTreeMap<Kind, Vec<Entry>>,
    dependencies: BTreeMap<(Kind, Guid), (PathBuf, Arc<[u8]>)>,
    custom_profiles: BTreeMap<(Kind, Guid), super::profiles::Profile>,
    issues: Vec<Issue>,
}
pub struct ExpressionSelection {
    expressed: Guid,
    dependencies: Vec<(PathBuf, Arc<[u8]>)>,
}
pub struct Selection {
    pattern: Guid,
    performed: Guid,
    expressed: Option<Guid>,
    dependencies: Vec<(Kind, (PathBuf, Arc<[u8]>))>,
}
impl Task {
    pub fn capture(
        session: &Session,
        chart: usize,
        directory: Option<&Path>,
    ) -> Result<Self, Error> {
        if chart >= session.document().charts().len() {
            return Err(invalid("Profile selector chart does not exist"));
        }
        Self::capture_context(session, Context::Chart(chart), directory)
    }
    pub fn capture_application(session: &Session, directory: Option<&Path>) -> Result<Self, Error> {
        Self::capture_context(session, Context::Application, directory)
    }
    fn capture_context(
        session: &Session,
        context: Context,
        directory: Option<&Path>,
    ) -> Result<Self, Error> {
        let directory = directory
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_owned())
                } else {
                    std::env::current_dir().map(|cwd| cwd.join(path))
                }
            })
            .transpose()?;
        Ok(Self {
            token: session.revision_token(),
            context,
            directory,
        })
    }
    pub fn spawn(self) -> Result<Job<Catalog>, Error> {
        Job::spawn_with("editor-profile-catalog", move |cancelled, progress| {
            let catalog = self.run(cancelled)?;
            progress(Progress {
                phase: Phase::Ready,
                completed: 1,
                total: 1,
            });
            Ok(catalog)
        })
    }
    pub fn run(self, cancelled: &AtomicBool) -> Result<Catalog, Error> {
        check(cancelled)?;
        let loaded = if let Some(directory) = self.directory {
            Loaded::directory_checked(&directory, cancelled)?
        } else {
            Loaded {
                registry: Registry::builtin(),
                profiles: BTreeMap::new(),
                issues: Vec::new(),
            }
        };
        let mut entries = BTreeMap::new();
        let mut dependencies = BTreeMap::new();
        let mut custom_profiles = BTreeMap::new();
        for (kind, ids) in [
            (
                Kind::Pattern,
                loaded.registry.patterns.keys().collect::<Vec<_>>(),
            ),
            (Kind::Performed, loaded.registry.performed.keys().collect()),
            (Kind::Expressed, loaded.registry.expressed.keys().collect()),
        ] {
            let mut choices = Vec::new();
            for id in ids {
                check(cancelled)?;
                let (name, builtin) = if let Some(file) = loaded.profiles.get(&(kind, id.clone())) {
                    dependencies.insert((kind, id.clone()), file.snapshot());
                    custom_profiles.insert((kind, id.clone()), file.profile().clone());
                    (file.profile().name().map(str::to_owned), None)
                } else {
                    (
                        None,
                        Some(
                            builtin_label(kind, id)
                                .ok_or_else(|| invalid("Built-in profile has no catalog label"))?,
                        ),
                    )
                };
                let config = match kind {
                    Kind::Pattern => {
                        super::profiles::Config::Pattern(loaded.registry.patterns[id].clone())
                    }
                    Kind::Performed => {
                        super::profiles::Config::Performed(loaded.registry.performed[id].clone())
                    }
                    Kind::Expressed => {
                        super::profiles::Config::Expressed(loaded.registry.expressed[id].clone())
                    }
                };
                choices.push(Entry {
                    guid: id.clone(),
                    name,
                    builtin,
                    config,
                    display: None,
                });
            }
            // Built-ins first, then custom names; GUID breaks duplicate-name ties.
            choices.sort_by_cached_key(|entry| {
                (
                    entry.builtin.is_none(),
                    entry.name.as_deref().unwrap_or("").to_lowercase(),
                    entry.guid.clone(),
                )
            });
            entries.insert(kind, choices);
        }
        check(cancelled)?;
        Ok(Catalog {
            token: self.token,
            context: self.context,
            entries,
            dependencies,
            custom_profiles,
            issues: loaded.issues,
        })
    }
}
pub const KINDS: [Kind; 3] = [Kind::Pattern, Kind::Performed, Kind::Expressed];
/// Host resolves source CurrentCulture text/ordering without changing the catalog's dependencies.
pub trait Orderer {
    fn order(&mut self, catalog: &Catalog) -> Result<Vec<(Kind, Vec<(Guid, String)>)>, Error>;
}
impl Catalog {
    pub fn install_order(
        &mut self,
        presentation: Vec<(Kind, Vec<(Guid, String)>)>,
    ) -> Result<(), Error> {
        let mut next = BTreeMap::new();
        for (kind, rows) in presentation {
            if next.contains_key(&kind) || rows.len() != self.entries(kind).len() {
                return Err(invalid("Invalid profile presentation shape"));
            }
            let mut seen = std::collections::BTreeSet::new();
            let mut entries = Vec::new();
            for (id, text) in rows {
                if !seen.insert(id.clone()) {
                    return Err(invalid("Duplicate profile presentation identity"));
                }
                let mut entry = self
                    .entries(kind)
                    .iter()
                    .find(|entry| entry.guid == id)
                    .cloned()
                    .ok_or_else(|| invalid("Unknown profile presentation identity"))?;
                entry.display = Some(text);
                entries.push(entry);
            }
            next.insert(kind, entries);
        }
        if next.len() != KINDS.len() {
            return Err(invalid("Incomplete profile presentation"));
        }
        self.entries = next;
        Ok(())
    }

    pub fn entries(&self, kind: Kind) -> &[Entry] {
        self.entries.get(&kind).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        self.is_current_in(session, Context::Chart(chart))
    }
    pub fn is_current_in(&self, session: &Session, context: Context) -> bool {
        context == self.context && session.accepts_token(&self.token)
    }
    /// Capture an editable source only while its catalog and custom file still
    /// match. Presets retain their protected identity until explicitly duplicated.
    pub fn profile(
        &self,
        session: &Session,
        chart: usize,
        kind: Kind,
        id: &Guid,
    ) -> Result<super::profiles::Profile, Error> {
        self.profile_in(session, Context::Chart(chart), kind, id)
    }
    pub fn profile_in(
        &self,
        session: &Session,
        context: Context,
        kind: Kind,
        id: &Guid,
    ) -> Result<super::profiles::Profile, Error> {
        if !self.is_current_in(session, context) {
            return Err(invalid(
                "Profile catalog belongs to another context or revision",
            ));
        }
        if !self.entries(kind).iter().any(|entry| &entry.guid == id) {
            return Err(invalid("Selected profile is missing from the catalog"));
        }
        if let Some((path, expected)) = self.dependencies.get(&(kind, id.clone())) {
            if fs::read(path)?.as_slice() != expected.as_ref() {
                return Err(Error::ExternalChange);
            }
            return self
                .custom_profiles
                .get(&(kind, id.clone()))
                .cloned()
                .ok_or_else(|| invalid("Custom profile snapshot is missing"));
        }
        let registry = Registry::builtin();
        let config = match kind {
            Kind::Pattern => registry
                .patterns
                .get(id)
                .map(|c| super::profiles::Config::Pattern(c.clone())),
            Kind::Performed => registry
                .performed
                .get(id)
                .map(|c| super::profiles::Config::Performed(c.clone())),
            Kind::Expressed => registry
                .expressed
                .get(id)
                .map(|c| super::profiles::Config::Expressed(c.clone())),
        }
        .ok_or_else(|| invalid("Selected preset is missing"))?;
        let speed = (kind == Kind::Performed).then(|| {
            let mut speed = super::profiles::TravelSpeed::editor_defaults();
            if builtin_label(kind, id) == Some(BuiltinLabel::StaminaDouble) {
                speed.min_bpm = 99;
            }
            speed
        });
        super::profiles::Profile::preset(id, config, speed)
    }
    /// Open the same custom file captured by this catalog, retaining the file's
    /// byte guard for a later atomic save. Built-in presets have no writable file.
    pub fn profile_file(
        &self,
        session: &Session,
        chart: usize,
        kind: Kind,
        id: &Guid,
    ) -> Result<super::profiles::ProfileFile, Error> {
        self.profile_file_in(session, Context::Chart(chart), kind, id)
    }
    pub fn profile_file_in(
        &self,
        session: &Session,
        context: Context,
        kind: Kind,
        id: &Guid,
    ) -> Result<super::profiles::ProfileFile, Error> {
        let profile = self.profile_in(session, context, kind, id)?;
        let (path, expected) = self
            .dependencies
            .get(&(kind, id.clone()))
            .ok_or_else(|| invalid("Built-in editor profiles cannot be overwritten"))?;
        super::profiles::ProfileFile::capture_snapshot(path, expected.clone(), profile)
    }
    pub fn select(
        &self,
        session: &Session,
        chart: usize,
        pattern: &Guid,
        performed: &Guid,
    ) -> Result<Selection, Error> {
        if !self.is_current(session, chart) {
            return Err(invalid(
                "Profile catalog belongs to another chart or revision",
            ));
        }
        let mut dependencies = Vec::new();
        for (kind, id) in [(Kind::Pattern, pattern), (Kind::Performed, performed)] {
            if !self.entries(kind).iter().any(|entry| &entry.guid == id) {
                return Err(invalid("Selected profile is missing from the catalog"));
            }
            if let Some(dependency) = self.dependencies.get(&(kind, id.clone())) {
                dependencies.push((kind, dependency.clone()));
            }
        }
        Ok(Selection {
            pattern: pattern.clone(),
            performed: performed.clone(),
            expressed: None,
            dependencies,
        })
    }
    pub fn conversion(
        &self,
        session: &Session,
        chart: usize,
        selection: super::job::ProfileOverrides,
    ) -> Result<super::job::ProfileOverrides, Error> {
        if !self.is_current(session, chart) {
            return Err(invalid(
                "Profile catalog belongs to another chart or revision",
            ));
        }
        for (kind, id) in [
            (Kind::Performed, &selection.performed),
            (Kind::Expressed, &selection.expressed),
        ] {
            if let Some(id) = id {
                if !self.entries(kind).iter().any(|entry| &entry.guid == id) {
                    return Err(invalid(
                        "Selected conversion profile is missing from the catalog",
                    ));
                }
            }
        }
        Ok(selection)
    }
    pub fn select_expression(
        &self,
        session: &Session,
        chart: usize,
        expressed: &Guid,
    ) -> Result<ExpressionSelection, Error> {
        if !self.is_current(session, chart) {
            return Err(invalid(
                "Profile catalog belongs to another chart or revision",
            ));
        }
        if !self
            .entries(Kind::Expressed)
            .iter()
            .any(|entry| &entry.guid == expressed)
        {
            return Err(invalid(
                "Selected interpretation profile is missing from the catalog",
            ));
        }
        let dependencies = self
            .dependencies
            .get(&(Kind::Expressed, expressed.clone()))
            .cloned()
            .into_iter()
            .collect();
        Ok(ExpressionSelection {
            expressed: expressed.clone(),
            dependencies,
        })
    }
    pub fn select_generation(
        &self,
        session: &Session,
        chart: usize,
        pattern: &Guid,
        performed: &Guid,
        expressed: &Guid,
    ) -> Result<Selection, Error> {
        let mut selection = self.select(session, chart, pattern, performed)?;
        if !self
            .entries(Kind::Expressed)
            .iter()
            .any(|entry| &entry.guid == expressed)
        {
            return Err(invalid(
                "Selected interpretation profile is missing from the catalog",
            ));
        }
        if let Some(dependency) = self.dependencies.get(&(Kind::Expressed, expressed.clone())) {
            selection
                .dependencies
                .push((Kind::Expressed, dependency.clone()));
        }
        selection.expressed = Some(expressed.clone());
        Ok(selection)
    }
}
impl Selection {
    pub(crate) fn after_inline_creation(
        &mut self,
        receipt: &crate::history::profile_creation::Committed,
    ) {
        let kind = receipt.kind();
        let guid = receipt.guid().clone();
        match kind {
            Kind::Pattern => self.pattern = guid,
            Kind::Performed => self.performed = guid,
            Kind::Expressed => self.expressed = Some(guid),
        }
        self.dependencies.retain(|(existing, _)| *existing != kind);
        self.dependencies.push((kind, receipt.dependency().clone()));
    }
    pub(crate) fn check(
        &self,
        pattern: &Guid,
        performed: &Guid,
        expressed: &Guid,
    ) -> Result<(), Error> {
        if &self.pattern != pattern
            || &self.performed != performed
            || self
                .expressed
                .as_ref()
                .is_some_and(|expected| expected != expressed)
        {
            return Err(invalid(
                "Profile identities changed after catalog selection",
            ));
        }
        check_dependencies(
            &self
                .dependencies
                .iter()
                .map(|(_, dependency)| dependency.clone())
                .collect::<Vec<_>>(),
        )
    }
}
impl ExpressionSelection {
    pub(crate) fn after_inline_creation(
        &mut self,
        receipt: &crate::history::profile_creation::Committed,
    ) {
        self.expressed = receipt.guid().clone();
        self.dependencies = vec![receipt.dependency().clone()];
    }
    pub(crate) fn check(&self, expressed: &Guid) -> Result<(), Error> {
        if &self.expressed != expressed {
            return Err(invalid(
                "Interpretation profile changed after catalog selection",
            ));
        }
        check_dependencies(&self.dependencies)
    }
}
fn check_dependencies(dependencies: &[(PathBuf, Arc<[u8]>)]) -> Result<(), Error> {
    for (path, expected) in dependencies {
        if fs::read(path)?.as_slice() != expected.as_ref() {
            return Err(Error::ExternalChange);
        }
    }
    Ok(())
}

fn builtin_label(kind: Kind, id: &Guid) -> Option<BuiltinLabel> {
    use BuiltinLabel::*;
    let values: &[(&str, BuiltinLabel)] = match kind {
        Kind::Pattern => &[
            (super::metadata::SIXTEENTHS, Sixteenths),
            (super::metadata::EIGHTHS, Eighths),
        ],
        Kind::Performed => &[
            ("6276c906-ea8f-43b3-9500-0ddeac7bdc22", Balanced),
            ("aa41f2c5-4da1-41ad-b645-b18ed7c11d47", StaminaSingle),
            ("c0334922-6105-4703-add2-3de261b2ff19", StaminaDouble),
            (super::metadata::PATTERN_BALANCED, PatternBalanced),
            ("69d041be-ef12-4609-815e-b591d512e259", NoCandles),
            ("eba3802f-66ec-42d4-87cf-66e9dadc9d0a", NoInward),
            ("0dde2261-90c5-4413-82a6-9034dad80aa1", NoTransitionLimit),
        ],
        Kind::Expressed => &[
            (super::metadata::DYNAMIC, Dynamic),
            ("da3f6e12-49d1-416b-8db6-0ab413f740b6", AggressiveBrackets),
            ("0c0ba200-8f90-4060-8912-e9ea65831ebc", NoBrackets),
        ],
    };
    let text = id.to_string();
    values
        .iter()
        .find(|(candidate, _)| *candidate == text)
        .map(|(_, label)| *label)
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Profile catalog loading cancelled"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

impl Entry {
    pub fn source_name(&self) -> &str {
        self.name.as_deref().unwrap_or_else(|| match self.builtin {
            Some(BuiltinLabel::Sixteenths) => "Default 16ths",
            Some(BuiltinLabel::Eighths) => "Default 8ths",
            Some(BuiltinLabel::Balanced) => "Default Chart: Balanced",
            Some(BuiltinLabel::StaminaSingle) => "Default Chart: Stamina (SP)",
            Some(BuiltinLabel::StaminaDouble) => "Default Chart: Stamina (DP)",
            Some(BuiltinLabel::PatternBalanced) => "Default Pattern: Balanced",
            Some(BuiltinLabel::NoCandles) => "Default Pattern: No Candles",
            Some(BuiltinLabel::NoInward) => "Default Pattern: No Inward",
            Some(BuiltinLabel::NoTransitionLimit) => "Default Pattern: No Transition Limit",
            Some(BuiltinLabel::Dynamic) => "Dynamic",
            Some(BuiltinLabel::AggressiveBrackets) => "Aggressive Brackets",
            Some(BuiltinLabel::NoBrackets) => "No Brackets",
            None => "",
        })
    }
    pub fn source_text(
        &self,
        fixed2: &mut impl FnMut(f64) -> Result<String, Error>,
    ) -> Result<String, Error> {
        description::describe(self.source_name(), &self.config, fixed2)
    }
}
