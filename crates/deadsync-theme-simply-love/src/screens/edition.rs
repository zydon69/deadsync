//! Compact native editor screen. The document and every edit command live in
//! deadsync-editor; this module owns presentation, shortcuts, and exit prompts.
mod analysis_panel;
mod zoom_view;
mod spacing_view;
mod zoom_settings;
mod scroll_settings;
mod audio_settings;
mod batch;
mod chart_creation;
mod chart_deletion;
mod chart_rating;
mod display_tempo;
mod profile_edit;
mod text_fields;
mod clipboard;
mod music_file;
mod pattern_set;
mod playback;
mod song_properties;
mod sample_region;
mod waveform;
mod waveform_settings;
mod waveform_colors;
mod timing;
mod text_entry;

use crate::{act, i18n};
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
    space::{screen_center_x, screen_height, screen_width},
};
use deadsync_editor::{
    generation::{
        batch_job, clear_patterns,
        config::{PatternConfig, PerformedConfig, PerformedPreset},
        job::{
            ChartOptions, ChartPreview, Job, Phase, ProfileOverrides, Progress, Report,
            TARGET_LAYOUTS,
        },
        pattern_batch,
        pattern_job::{self, Span},
        profile_catalog::{self, BuiltinLabel},
        profiles::Kind as ProfileKind,
    },
    notes::{Kind, ROWS_PER_BEAT},
    workspace::{Command, SNAP_NAMES, SNAP_ROWS, TOOLS, Workspace},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy)]
enum Panel {
    Analysis,
    NewChart(usize),
    DeleteChart(usize),
    EmptyMenu(usize),
    Menu(usize),
    Transforms(usize),
    Unsaved(usize),
    ConfirmDiscard,
    Generation(usize),
    Batch(usize),
    Regeneration(usize),
    PatternSet(usize),
    Pattern(usize),
    Marker(usize),
    Profiles(usize),
    ChartProperties(usize),
    DisplayTempo(usize),
    ProfileEdit(usize),
    ProfileText,
    ProfileSpeed(usize),
    ProfileDefaults(usize),
    ProfileDelete(usize),
    ProfileWeights(usize),
    ChartText(usize),
    SongText(usize),
    SongProperties(usize),
    SongSample(usize),
    AudioSettings(usize),
    ZoomSettings(usize),
    ScrollSettings(usize),
    WaveformSettings(usize),
    WaveformColors(usize),
    Timing(usize),
    TimingMigration(usize),
    ChartExpression(usize),
    ConversionProfiles(usize),
    ConfirmMarkerDelete,
    Generating,
    GenerationPreview(usize),
}

struct GenerationSettings {
    target: usize,
    preset: usize,
    seed: i32,
    profiles: ProfileOverrides,
    performed_name: Option<Arc<str>>,
    expressed_name: Option<Arc<str>>,
}

const PATTERN_SUBDIVISIONS: [u32; 9] = [1, 2, 3, 4, 6, 8, 12, 16, 48];
const PATTERN_PRESETS: [PerformedPreset; 4] = [
    PerformedPreset::PatternBalanced,
    PerformedPreset::PatternNoCandles,
    PerformedPreset::PatternNoInward,
    PerformedPreset::PatternNoTransitionLimit,
];
struct PatternSettings {
    span: Span,
    subdivision: usize,
    preset: usize,
    seed: i32,
    ignore_distribution: bool,
}
#[derive(Clone, Copy)]
enum ProfileTarget {
    Marker,
    Chart,
    Conversion,
    Batch,
}

struct ProfileSettings {
    catalog: profile_catalog::Catalog,
    pattern: Option<usize>,
    performed: Option<usize>,
    expressed: Option<usize>,
}

enum GenerationJob {
    Chart(Job),
    Batch(Job<batch_job::Preview>),
    Pattern(Job<pattern_job::Preview>),
    PatternSet(Job<pattern_batch::Preview>),
    Clear(Job<clear_patterns::Preview>),
}
impl GenerationJob {
    fn cancel(&self) {
        match self {
            Self::Chart(job) => job.cancel(),
            Self::Batch(job) => job.cancel(),
            Self::Pattern(job) => job.cancel(),
            Self::PatternSet(job) => job.cancel(),
            Self::Clear(job) => job.cancel(),
        }
    }
    fn is_cancelled(&self) -> bool {
        match self {
            Self::Chart(job) => job.is_cancelled(),
            Self::Batch(job) => job.is_cancelled(),
            Self::Pattern(job) => job.is_cancelled(),
            Self::PatternSet(job) => job.is_cancelled(),
            Self::Clear(job) => job.is_cancelled(),
        }
    }
    fn progress(&self) -> Progress {
        match self {
            Self::Chart(job) => job.progress(),
            Self::Batch(job) => job.progress(),
            Self::Pattern(job) => job.progress(),
            Self::PatternSet(job) => job.progress(),
            Self::Clear(job) => job.progress(),
        }
    }
    fn poll(&mut self) -> Option<Result<GenerationPreview, deadsync_editor::Error>> {
        match self {
            Self::Chart(job) => job
                .poll()
                .map(|result| result.map(GenerationPreview::Chart)),
            Self::Batch(job) => job
                .poll()
                .map(|result| result.map(|preview| GenerationPreview::Batch { preview, entry: 0 })),
            Self::Pattern(job) => job
                .poll()
                .map(|result| result.map(GenerationPreview::Pattern)),
            Self::PatternSet(job) => job.poll().map(|result| {
                result.map(|preview| GenerationPreview::PatternSet { preview, entry: 0 })
            }),
            Self::Clear(job) => job
                .poll()
                .map(|result| result.map(GenerationPreview::Clear)),
        }
    }
}
enum GenerationPreview {
    Chart(ChartPreview),
    Batch {
        preview: batch_job::Preview,
        entry: usize,
    },
    Pattern(pattern_job::Preview),
    PatternSet {
        preview: pattern_batch::Preview,
        entry: usize,
    },
    Clear(clear_patterns::Preview),
}
impl GenerationPreview {
    fn grid(&self) -> &deadsync_editor::notes::Grid {
        match self {
            Self::Chart(p) => p.grid(),
            Self::Batch { preview, entry } => preview.grid(*entry),
            Self::Pattern(p) => p.grid(),
            Self::PatternSet { preview, .. } => preview.grid(),
            Self::Clear(p) => p.grid(),
        }
    }
    fn layout(&self) -> &str {
        match self {
            Self::Chart(p) => p.target_layout(),
            Self::Batch { preview, .. } => preview.target_layout(),
            Self::Pattern(p) => &p.generated().layout,
            Self::PatternSet { preview, .. } => preview.layout(),
            Self::Clear(p) => p.layout(),
        }
    }
}

pub struct State {
    spacing_mode: deadsync_editor::zoom::SpacingMode,
    reverse_display: bool,
    spacing_cache: Option<spacing_view::Cache>,
    zoom: deadsync_editor::zoom::Manager,
    zoom_epoch: std::time::Instant,
    zoom_preferences: deadsync_editor::zoom_preferences::Preferences,
    zoom_preferences_draft: Option<deadsync_editor::zoom_preferences::Preferences>,
    playback: Option<playback::Playing>,
    paused_playback: Option<playback::Paused>,
    scroll_preferences: deadsync_editor::scroll_preferences::Preferences,
    scroll_preferences_draft: Option<deadsync_editor::scroll_preferences::Preferences>,
    audio_preferences: deadsync_editor::audio_preferences::Audio,
    audio_preferences_file: Option<deadsync_editor::audio_preferences::File>,
    audio_preferences_path: Option<std::path::PathBuf>,
    audio_preferences_draft: Option<deadsync_editor::audio_preferences::Audio>,
    audio_commands: Vec<deadsync_gameplay::GameplayAudioCommand>,
    workspace: Workspace,
    generation: GenerationSettings,
    batch_source: Option<String>,
    pattern: PatternSettings,
    pattern_set: Option<pattern_set::Settings>,
    marker: Option<deadsync_editor::generation::marker_edit::Draft>,
    chart_properties: Option<deadsync_editor::chart_properties::Draft>,
    chart_rating_input: Option<String>,
    display_tempo_draft: Option<deadsync_editor::display_tempo::Value>,
    display_tempo_input: Option<(usize, String)>,
    profile_edit: Option<profile_edit::Draft>,
    profile_saved_choices: Option<profile_edit::Choices>,
    profile_history: std::collections::BTreeMap<PathBuf, deadsync_editor::generation::profiles::ProfileFile>,
    text_buffer: Option<deadsync_editor::text_buffer::Buffer>,
    text_clipboard_request: Option<clipboard::Request>,
    music_picker_request: Option<music_file::Request>,
    text_edit_serial: u64,
    song_properties: Option<deadsync_editor::song_properties::Draft>,
    sample_region: Option<sample_region::Cache>,
    waveform: waveform::View,
    waveform_preferences: deadsync_editor::waveform_preferences::Preferences,
    waveform_preferences_draft: Option<deadsync_editor::waveform_preferences::Preferences>,
    timing: Option<timing::Settings>,
    timing_migration: Option<deadsync_editor::timing_edit::migration::Draft>,
    catalog_target: ProfileTarget,
    profile_directory: Option<PathBuf>,
    saved_regions: Vec<(u64, u64)>,
    region_index: deadsync_editor::region_index::RegionIndex,
    signatures: Option<deadsync_editor::timing_edit::signature::Signatures>,
    labels: Option<deadsync_editor::timing_edit::label::Labels>,
    marker_cache: Option<(Arc<[u8]>, u64, usize)>,
    job: Option<GenerationJob>,
    catalog_job: Option<Job<profile_catalog::Catalog>>,
    analysis_job: Option<Job<deadsync_editor::analysis::task::Report>>,
    analysis_report: Option<deadsync_editor::analysis::task::Report>,
    analysis_bin: usize,
    new_chart: Option<chart_creation::Settings>,
    deletion: Option<deadsync_editor::chart_deletion::Draft>,
    profile_selection: Option<ProfileSettings>,
    preview: Option<GenerationPreview>,
    label_preedit: String,
    control: bool,
    control_keys: u8,
    shift: bool,
    shift_keys: u8,
    help: bool,
    panel: Option<Panel>,
    status: Arc<str>,
    toolbar: Arc<str>,
    chart_label: Arc<str>,
    last_recovery_revision: u64,
}

impl State {
    pub fn open(path: &Path, chart: usize) -> Result<Self, deadsync_editor::Error> {
        Self::from_workspace(Workspace::open(path, chart)?)
    }

    pub fn open_with_profiles(
        path: &Path,
        chart: usize,
        directory: &Path,
    ) -> Result<Self, deadsync_editor::Error> {
        let mut state = Self::open(path, chart)?;
        state.profile_directory = Some(directory.to_owned());
        Ok(state)
    }

    pub fn open_selected(
        path: &Path,
        chart_type: &str,
        difficulty: &str,
        description: &str,
        meter: u32,
    ) -> Result<Self, deadsync_editor::Error> {
        let file = deadsync_editor::storage::FileSession::open(path)?;
        if file.session.document().charts().iter()
            .all(|c| deadsync_editor::notes::layout_columns(&c.steps_type).is_none()) {
            return Self::from_workspace(Workspace::from_file(file, 0)?);
        }
        let matches: Vec<_> = file
            .session
            .document()
            .charts()
            .iter()
            .enumerate()
            .filter(|(_, chart)| {
                chart.steps_type == chart_type
                    && chart.difficulty.eq_ignore_ascii_case(difficulty)
                    && chart.description == description
                    && chart.meter.parse::<u32>().ok() == Some(meter)
            })
            .map(|(index, _)| index)
            .collect();
        if matches.len() != 1 {
            return Err(deadsync_editor::Error::InvalidEdit(
                "Cannot identify one source chart unambiguously; reload the song selection".into(),
            ));
        }
        Self::from_workspace(Workspace::from_file(file, matches[0])?)
    }

    fn from_workspace(workspace: Workspace) -> Result<Self, deadsync_editor::Error> {
        let layout = workspace.active_chart().and_then(|i| workspace.file().session.document().charts().get(i))
            .map_or("dance-single", |chart| chart.steps_type.as_str());
        let target = TARGET_LAYOUTS
            .iter()
            .position(|candidate| *candidate == layout)
            .unwrap_or(0);
        let mut state = Self {
            spacing_mode: deadsync_editor::zoom::SpacingMode::ConstantRow,
            reverse_display: false,
            spacing_cache: None,
            zoom: Default::default(),
            zoom_preferences: Default::default(),
            zoom_preferences_draft: None,
            zoom_epoch: std::time::Instant::now(),
            playback: None,
            paused_playback: None,
            scroll_preferences: Default::default(),
            scroll_preferences_draft: None,
            audio_preferences: Default::default(),
            audio_preferences_file: None,
            audio_preferences_path: None,
            audio_preferences_draft: None,
            audio_commands: Vec::new(),
            generation: GenerationSettings {
                target,
                preset: 0,
                seed: 0,
                profiles: ProfileOverrides::default(),
                performed_name: None,
                expressed_name: None,
            },
            batch_source: None,
            pattern_set: None,
            pattern: PatternSettings {
                span: Span {
                    start: 0,
                    end: 192,
                    include_start: true,
                    include_end: true,
                },
                subdivision: 3,
                preset: 0,
                seed: 0,
                ignore_distribution: false,
            },
            marker: None,
            chart_properties: None,
            chart_rating_input: None,
            display_tempo_draft: None,
            display_tempo_input: None,
            profile_edit: None,
            profile_history: Default::default(),
            profile_saved_choices: None,
            text_buffer: None,
            text_clipboard_request: None,
            music_picker_request: None,
            text_edit_serial: 0,
            song_properties: None,
            sample_region: None,
            waveform: waveform::View::default(),
            waveform_preferences: Default::default(),
            waveform_preferences_draft: None,
            timing: None,
            timing_migration: None,
            catalog_target: ProfileTarget::Marker,
            profile_directory: deadsync_editor::runtime::configs_dir().map(Path::to_path_buf),
            saved_regions: Vec::new(),
            region_index: Default::default(),
            signatures: None,
            labels: None,
            marker_cache: None,
            job: None,
            catalog_job: None,
            analysis_job: None,
            analysis_report: None,
            analysis_bin: 0,
            new_chart: None,
            deletion: None,
            profile_selection: None,
            preview: None,
            workspace,
            label_preedit: String::new(),
            control: false,
            control_keys: 0,
            shift: false,
            shift_keys: 0,
            help: false,
            panel: None,
            status: i18n::tr("Edition", "Ready"),
            toolbar: Arc::from(""),
            chart_label: Arc::from(""),
            last_recovery_revision: 0,
        };
        state.refresh();
        if let Some(path) = deadsync_editor::runtime::preferences_path() {
            if let Err(error) = state.configure_audio_preferences(path) {
                state.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
            }
        }
        Ok(state)
    }

    /// Focus loss cancels transient input state while retaining the editing draft.
    pub fn release_input(&mut self) {
        self.control = false;
        self.control_keys = 0;
        self.shift = false;
        self.shift_keys = 0;
        self.label_preedit.clear();
        self.text_clipboard_request = None;
        self.music_picker_request = None;
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn is_generating(&self) -> bool {
        self.job.is_some() || self.catalog_job.is_some()
    }
    pub fn generation_report(&self) -> Option<Report> {
        self.preview.as_ref().and_then(|p| match p {
            GenerationPreview::Chart(p) => Some(p.report()),
            _ => None,
        })
    }

    pub fn generation_expression_profile(
        &self,
    ) -> Option<&deadsync_editor::generation::metadata::Guid> {
        self.preview.as_ref().and_then(|preview| match preview {
            GenerationPreview::Chart(preview) => preview.expression_profile(),
            _ => None,
        })
    }

    pub fn generation_performed_profile(
        &self,
    ) -> Option<&deadsync_editor::generation::metadata::Guid> {
        self.preview.as_ref().and_then(|preview| match preview {
            GenerationPreview::Chart(preview) => preview.performed_profile(),
            _ => None,
        })
    }

    pub fn pattern_report(
        &self,
    ) -> Option<(deadsync_editor::generation::pattern_region::Deletions, f64)> {
        self.preview.as_ref().and_then(|p| match p {
            GenerationPreview::Pattern(p) => Some((p.deletions(), p.average_nps())),
            _ => None,
        })
    }

    fn open_pattern(&mut self) {
        let (start, end) = self.workspace.selection().unwrap_or_else(|| {
            let start = self.workspace.row();
            (start, start.saturating_add(ROWS_PER_BEAT * 4))
        });
        self.pattern.span.start = start;
        self.pattern.span.end = end;
        self.panel = Some(Panel::Pattern(0));
        self.status = i18n::tr("Edition", "GenerationOptionsHelp");
    }
    fn start_pattern(&mut self) {
        if !self.current_timing_for_generation() {
            return;
        }
        if self.is_generating() {
            return;
        }
        let options = pattern_job::Options {
            span: self.pattern.span,
            seed: self.pattern.seed,
            expressed: ChartOptions::balanced("dance-single").expressed,
            performed: PerformedConfig::preset(PATTERN_PRESETS[self.pattern.preset]),
            pattern: PatternConfig {
                beat_sub_division: PATTERN_SUBDIVISIONS[self.pattern.subdivision],
                ..PatternConfig::sixteenths()
            },
            ignore_preceding_distribution: self.pattern.ignore_distribution,
        };
        match self
            .workspace
            .persistent_pattern_task(options, self.profile_directory.as_deref())
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.preview = None;
                self.job = Some(GenerationJob::Pattern(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    fn open_marker(&mut self) {
        self.profile_selection = None;
        match self.workspace.edit_saved_pattern() {
            Ok(draft) => {
                self.marker = Some(draft);
                self.panel = Some(Panel::Marker(0));
                self.status = i18n::tr("Edition", "MarkerHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    fn commit_marker(&mut self, delete: bool) {
        let Some(draft) = &self.marker else {
            return;
        };
        let start = draft.start;
        match self.workspace.apply_marker_edit(draft, delete) {
            Ok(_) => {
                if !delete {
                    self.command(Command::Seek(start));
                }
                self.marker = None;
                self.panel = None;
                self.status = i18n::tr(
                    "Edition",
                    if delete {
                        "MarkerDeleted"
                    } else {
                        "MarkerEdited"
                    },
                );
                self.refresh();
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    fn open_chart_properties(&mut self) {
        if deadsync_editor::timing_edit::migration::needed(self.workspace.file().session.document())
        {
            self.open_timing_migration();
            return;
        }
        match self.workspace.chart_properties() {
            Ok(draft) => {
                self.chart_rating_input = None;
                self.display_tempo_draft = None;
                self.display_tempo_input = None;
                self.status = i18n::tr(
                    "Edition",
                    if draft.format() == deadsync_editor::document::Format::Sm {
                        "ChartPropertiesHelpSm"
                    } else {
                        "ChartPropertiesHelpSsc"
                    },
                );
                self.chart_properties = Some(draft);
                self.panel = Some(Panel::ChartProperties(0));
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn open_chart_expression(&mut self) {
        if self.is_generating() {
            return;
        }
        let Some(draft) = &self.chart_properties else {
            return;
        };
        match self
            .workspace
            .chart_expression_task(draft, self.profile_directory.as_deref())
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.profile_selection = None;
                self.catalog_target = ProfileTarget::Chart;
                self.catalog_job = Some(job);
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "ProfileLoading");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn stage_chart_expression(&mut self) {
        let (Some(settings), Some(draft)) = (&self.profile_selection, &mut self.chart_properties)
        else {
            return;
        };
        let Some(entry) = settings
            .expressed
            .and_then(|index| settings.catalog.entries(ProfileKind::Expressed).get(index))
        else {
            self.status = i18n::tr("Edition", "ChartChooseExpression");
            return;
        };
        match self
            .workspace
            .select_chart_expression(draft, &settings.catalog, &entry.guid)
        {
            Ok(()) => {
                self.profile_selection = None;
                self.panel = Some(Panel::ChartProperties(0));
                self.status = i18n::tr("Edition", "ChartExpressionStaged");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn apply_chart_properties(&mut self) {
        if !self.stage_chart_rating() {
            return;
        }
        let Some(draft) = &self.chart_properties else {
            return;
        };
        match self.workspace.apply_chart_properties(draft) {
            Ok(_) => {
                self.chart_properties = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ChartPropertiesApplied");
                self.refresh();
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn open_conversion_profiles(&mut self) {
        self.open_conversion_profiles_for(ProfileTarget::Conversion);
    }
    fn open_conversion_profiles_for(&mut self, target: ProfileTarget) {
        if self.is_generating() {
            return;
        }
        match self
            .workspace
            .conversion_profile_task(self.profile_directory.as_deref())
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.profile_selection = None;
                self.catalog_target = target;
                self.catalog_job = Some(job);
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "ProfileLoading");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn stage_conversion_profiles(&mut self) {
        let Some(settings) = &self.profile_selection else {
            return;
        };
        let selected = |kind, index: Option<usize>| match index? {
            0 => Some((None, None)),
            index => settings.catalog.entries(kind).get(index - 1).map(|entry| {
                (
                    Some(entry.guid.clone()),
                    Some(Arc::<str>::from(profile_entry_name(entry))),
                )
            }),
        };
        let (Some((performed, performed_name)), Some((expressed, expressed_name))) = (
            selected(ProfileKind::Performed, settings.performed),
            selected(ProfileKind::Expressed, settings.expressed),
        ) else {
            self.status = i18n::tr("Edition", "ConversionChooseMissing");
            return;
        };
        match self.workspace.select_conversion_profiles(
            &settings.catalog,
            ProfileOverrides {
                performed,
                expressed,
            },
        ) {
            Ok(profiles) => {
                self.generation.profiles = profiles;
                self.generation.performed_name = performed_name;
                self.generation.expressed_name = expressed_name;
                self.profile_selection = None;
                self.panel = Some(self.profile_return_panel());
                self.status = i18n::tr("Edition", "ConversionProfilesStaged");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    fn profile_return_panel(&self) -> Panel {
        match self.catalog_target {
            ProfileTarget::Marker => Panel::Marker(12),
            ProfileTarget::Chart => Panel::ChartProperties(0),
            ProfileTarget::Conversion => Panel::Generation(5),
            ProfileTarget::Batch => Panel::Batch(4),
        }
    }

    fn open_marker_profiles(&mut self) {
        if self.is_generating() {
            return;
        }
        let Some(draft) = &self.marker else {
            return;
        };
        match self
            .workspace
            .marker_profile_task(draft, self.profile_directory.as_deref())
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.profile_selection = None;
                self.catalog_target = ProfileTarget::Marker;
                self.catalog_job = Some(job);
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "ProfileLoading");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn stage_marker_profiles(&mut self) {
        let (Some(settings), Some(draft)) = (&self.profile_selection, &mut self.marker) else {
            return;
        };
        let pattern = settings
            .pattern
            .and_then(|index| settings.catalog.entries(ProfileKind::Pattern).get(index));
        let performed = settings
            .performed
            .and_then(|index| settings.catalog.entries(ProfileKind::Performed).get(index));
        let expressed = settings
            .expressed
            .and_then(|index| settings.catalog.entries(ProfileKind::Expressed).get(index));
        let (Some(pattern), Some(performed), Some(expressed)) = (pattern, performed, expressed)
        else {
            self.status = i18n::tr("Edition", "ProfileChooseMissing");
            return;
        };
        match self.workspace.select_marker_generation_profiles(
            draft,
            &settings.catalog,
            &pattern.guid,
            &performed.guid,
            &expressed.guid,
        ) {
            Ok(()) => {
                self.profile_selection = None;
                self.panel = Some(Panel::Marker(12));
                self.status = i18n::tr("Edition", "ProfileStaged");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn poll_profile_catalog(&mut self) {
        let Some(job) = &mut self.catalog_job else {
            return;
        };
        let cancelled = job.is_cancelled();
        let Some(result) = job.poll() else {
            return;
        };
        self.catalog_job = None;
        let saved_choice = self.profile_saved_choices.take();
        self.panel = Some(self.profile_return_panel());
        match result {
            Ok(catalog)
                if catalog.is_current(&self.workspace.file().session, self.workspace.chart())
                    && match self.catalog_target {
                        ProfileTarget::Marker => self.marker.is_some(),
                        ProfileTarget::Chart => self.chart_properties.is_some(),
                        ProfileTarget::Conversion | ProfileTarget::Batch => true,
                    } =>
            {
                let (mut pattern, mut performed, mut expressed) = match self.catalog_target {
                    ProfileTarget::Marker => {
                        let draft = self.marker.as_ref().unwrap();
                        (
                            catalog
                                .entries(ProfileKind::Pattern)
                                .iter()
                                .position(|entry| {
                                    entry.guid == draft.definition.pattern_config_guid
                                }),
                            catalog
                                .entries(ProfileKind::Performed)
                                .iter()
                                .position(|entry| {
                                    entry.guid == draft.definition.performed_chart_config_guid
                                }),
                            catalog
                                .entries(ProfileKind::Expressed)
                                .iter()
                                .position(|entry| entry.guid == draft.expressed_chart_config),
                        )
                    }
                    ProfileTarget::Chart => {
                        let draft = self.chart_properties.as_ref().unwrap();
                        (
                            None,
                            None,
                            catalog
                                .entries(ProfileKind::Expressed)
                                .iter()
                                .position(|entry| &entry.guid == draft.expression()),
                        )
                    }
                    ProfileTarget::Conversion | ProfileTarget::Batch => {
                        let index =
                            |kind, id: &Option<deadsync_editor::generation::metadata::Guid>| {
                                match id {
                                    None => Some(0),
                                    Some(id) => catalog
                                        .entries(kind)
                                        .iter()
                                        .position(|entry| &entry.guid == id)
                                        .map(|index| index + 1),
                                }
                            };
                        (
                            None,
                            index(ProfileKind::Performed, &self.generation.profiles.performed),
                            index(ProfileKind::Expressed, &self.generation.profiles.expressed),
                        )
                    }
                };
                if let Some(choices) = saved_choice {
                    let index = |kind, id: Option<&deadsync_editor::generation::metadata::Guid>| {
                        let offset = usize::from(matches!(self.catalog_target, ProfileTarget::Conversion | ProfileTarget::Batch));
                        match id {
                            Some(id) => catalog.entries(kind).iter().position(|entry| &entry.guid == id).map(|i| i + offset),
                            None if offset == 1 => Some(0),
                            None => None,
                        }
                    };
                    pattern = index(ProfileKind::Pattern, choices.pattern.as_ref());
                    performed = index(ProfileKind::Performed, choices.performed.as_ref());
                    expressed = index(ProfileKind::Expressed, choices.expressed.as_ref());
                }
                self.status = i18n::tr_fmt(
                    "Edition",
                    match self.catalog_target {
                        ProfileTarget::Marker => "ProfilePickerHelp",
                        ProfileTarget::Chart => "ChartExpressionPickerHelp",
                        ProfileTarget::Conversion | ProfileTarget::Batch => {
                            "ConversionProfilesHelp"
                        }
                    },
                    &[("issues", &catalog.issues().len().to_string())],
                );
                self.profile_selection = Some(ProfileSettings {
                    catalog,
                    pattern,
                    performed,
                    expressed,
                });
                self.panel = Some(match self.catalog_target {
                    ProfileTarget::Marker => Panel::Profiles(0),
                    ProfileTarget::Chart => Panel::ChartExpression(0),
                    ProfileTarget::Conversion | ProfileTarget::Batch => {
                        Panel::ConversionProfiles(0)
                    }
                });
            }
            Ok(_) => self.status = i18n::tr("Edition", "ProfileStale"),
            Err(error) => {
                self.status = if cancelled {
                    i18n::tr("Edition", "GenerationCancelled")
                } else {
                    i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
    }

    fn clear_marker_notes(&mut self) {
        if !self.current_timing_for_generation() {
            return;
        }
        if self.is_generating() {
            return;
        }
        let Some(draft) = &self.marker else {
            return;
        };
        if draft.has_changes() {
            self.status = i18n::tr("Edition", "MarkerPending");
            return;
        }
        match self
            .workspace
            .clear_pattern_task(draft, self.profile_directory.as_deref())
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.marker = None;
                self.preview = None;
                self.job = Some(GenerationJob::Clear(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    fn navigate_marker(&mut self, forward: bool) {
        if self
            .marker
            .as_ref()
            .is_some_and(|draft| draft.has_changes())
        {
            self.status = i18n::tr("Edition", "MarkerPending");
            return;
        }
        match self.workspace.navigate_saved_pattern(forward) {
            Ok(true) => {
                self.open_marker();
                self.refresh();
            }
            Ok(false) => {}
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    fn regenerate_saved_pattern(&mut self) {
        if !self.current_timing_for_generation() {
            return;
        }
        if self.is_generating() {
            return;
        }
        let result = if let Some(directory) = &self.profile_directory {
            self.workspace
                .saved_pattern_task_from_directory(directory)
                .and_then(|task| task.spawn())
        } else {
            self.workspace
                .saved_pattern_task(&deadsync_editor::generation::metadata::Registry::builtin())
                .and_then(|task| task.spawn())
        };
        match result {
            Ok(job) => {
                self.preview = None;
                self.job = Some(GenerationJob::Pattern(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    /// Polling is nonblocking and is called by Practice's normal update loop.
    pub fn update(&mut self) {
        self.update_with_time(std::time::Instant::now());
    }

    /// Update using the host's monotonic frame clock, also permitting
    /// deterministic animation checks without sleeps or hardware input.
    pub fn update_with_time(&mut self, now: std::time::Instant) {
        self.zoom.update(now.saturating_duration_since(self.zoom_epoch).as_secs_f64())
            .expect("finite monotonic frame clock");
        self.poll_waveform();
        self.poll_analysis();
        if self.catalog_job.is_some() {
            self.poll_profile_catalog();
            return;
        }
        let Some(job) = self.job.as_mut() else {
            return;
        };
        let cancelled = job.is_cancelled();
        if let Some(result) = job.poll() {
            self.job = None;
            match result {
                Ok(preview) => {
                    let batch = matches!(preview, GenerationPreview::Batch { .. });
                    let pattern_set = matches!(preview, GenerationPreview::PatternSet { .. });
                    if let GenerationPreview::PatternSet { preview, .. } = &preview {
                        if let Some(entry) = preview.entries().first() {
                            let _ = self.workspace.execute(Command::Seek(entry.row));
                        }
                    }
                    self.preview = Some(preview);
                    self.panel = Some(Panel::GenerationPreview(0));
                    self.status = i18n::tr(
                        "Edition",
                        if pattern_set {
                            "PatternSetPreviewHelp"
                        } else if batch {
                            "BatchPreviewHelp"
                        } else {
                            "GenerationPreviewHelp"
                        },
                    );
                }
                Err(error) => {
                    self.panel = None;
                    self.status = if cancelled {
                        i18n::tr("Edition", "GenerationCancelled")
                    } else {
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                    };
                }
            }
            self.refresh();
        } else if !cancelled {
            let progress = job.progress();
            let phase = match progress.phase {
                Phase::Preparing => "GenerationPreparing",
                Phase::Interpreting => "GenerationInterpreting",
                Phase::Converting
                    if matches!(
                        job,
                        GenerationJob::Pattern(_) | GenerationJob::PatternSet(_)
                    ) =>
                {
                    "PatternGenerating"
                }
                Phase::Converting if matches!(job, GenerationJob::Clear(_)) => "MarkerClearing",
                Phase::Converting => "GenerationConverting",
                Phase::Assembling => "GenerationAssembling",
                Phase::Ready => "GenerationReady",
            };
            self.status = i18n::tr_fmt(
                "Edition",
                if matches!(job, GenerationJob::PatternSet(_)) {
                    "PatternSetProgress"
                } else if matches!(job, GenerationJob::Batch(_)) {
                    "BatchProgress"
                } else {
                    "GenerationProgress"
                },
                &[
                    ("phase", &i18n::tr("Edition", phase)),
                    ("done", &progress.completed.to_string()),
                    ("total", &progress.total.to_string()),
                ],
            );
        }
    }

    fn adjust_generation_setting(&mut self, index: usize, forward: bool) {
        match index {
            0 => {
                self.generation.target = (self.generation.target
                    + if forward { 1 } else { TARGET_LAYOUTS.len() - 1 })
                    % TARGET_LAYOUTS.len()
            }
            1 => {
                self.generation.preset = (self.generation.preset + if forward { 1 } else { 2 }) % 3;
                self.generation.profiles.performed = None;
                self.generation.performed_name = None;
            }
            2 => {
                self.generation.seed =
                    self.generation
                        .seed
                        .wrapping_add(if forward { 1 } else { -1 })
            }
            _ => {}
        }
    }

    fn generation_label(&self, index: usize, key: &str) -> Arc<str> {
        let value = match index {
            0 => TARGET_LAYOUTS[self.generation.target].to_owned(),
            1 => self
                .generation
                .performed_name
                .as_ref()
                .map(|name| name.to_string())
                .unwrap_or_else(|| {
                    i18n::tr(
                        "Edition",
                        [
                            "GenerationBalanced",
                            "GenerationStaminaSingle",
                            "GenerationStaminaDouble",
                        ][self.generation.preset],
                    )
                    .to_string()
                }),
            2 => self.generation.seed.to_string(),
            6 => self
                .generation
                .expressed_name
                .as_ref()
                .map(|name| name.to_string())
                .unwrap_or_else(|| i18n::tr("Edition", "ConversionExpressionDefault").to_string()),
            _ => String::new(),
        };
        if index < 3 || index == 6 {
            i18n::tr_fmt("Edition", key, &[("value", &value)])
        } else {
            i18n::tr("Edition", key)
        }
    }

    fn generation_options(&self) -> ChartOptions {
        let mut options = ChartOptions::balanced(TARGET_LAYOUTS[self.generation.target]);
        options.seed = self.generation.seed;
        options.performed = PerformedConfig::preset(match self.generation.preset {
            1 => PerformedPreset::StaminaSingle,
            2 => PerformedPreset::StaminaDouble,
            _ => PerformedPreset::Balanced,
        });
        options
    }

    fn start_generation(&mut self) {
        if !self.current_timing_for_generation() {
            return;
        }
        if self.is_generating() {
            return;
        }
        let options = self.generation_options();
        match self
            .workspace
            .generation_task_with_profiles(
                options,
                self.profile_directory.as_deref(),
                self.generation.profiles.clone(),
            )
            .and_then(|task| task.spawn())
        {
            Ok(job) => {
                self.preview = None;
                self.job = Some(GenerationJob::Chart(job));
                self.panel = Some(Panel::Generating);
                self.status = i18n::tr("Edition", "GenerationPreparing");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }

    fn cancel_generation(&mut self) {
        if let Some(job) = &self.catalog_job {
            job.cancel();
            self.status = i18n::tr("Edition", "GenerationCancelling");
        }
        if let Some(job) = &self.job {
            job.cancel();
            self.status = i18n::tr("Edition", "GenerationCancelling");
        }
    }

    fn refresh(&mut self) {
        self.refresh_spacing_cache();
        self.refresh_waveform();
        self.refresh_sample_region();
        if !self.workspace.has_chart() {
            self.saved_regions.clear(); self.region_index=Default::default(); self.labels=None; self.signatures=None; self.marker_cache=None;
            self.chart_label=i18n::tr("Edition", "NoActiveChart");
            self.toolbar=i18n::tr_fmt("Edition", "NoChartToolbar", &[("dirty", if self.workspace.file().session.is_dirty(){" *"}else{""})]);
            return;
        }
        let session = &self.workspace.file().session;
        let chart = self.workspace.chart();
        let source = session.document().source();
        if self
            .marker_cache
            .as_ref()
            .is_none_or(|(old, revision, index)| {
                !Arc::ptr_eq(old, source) || *revision != session.revision() || *index != chart
            })
        {
            self.labels = deadsync_editor::timing_edit::label::Labels::from_document(
                session.document(), chart,
            ).ok();
            self.signatures = deadsync_editor::timing_edit::signature::Signatures::from_document(
                session.document(), chart,
            ).ok();
            self.saved_regions =
                deadsync_editor::generation::metadata::ChartData::read(session.document(), chart)
                    .ok()
                    .flatten()
                    .and_then(|data| data.patterns)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(row, marker)| (row, row + marker.length))
                    .collect();
            self.region_index = deadsync_editor::region_index::RegionIndex::new(self.saved_regions.clone())
                .expect("validated pattern metadata regions");
            self.marker_cache = Some((Arc::clone(source), session.revision(), chart));
        }
        let dirty = if self.workspace.file().session.is_dirty() {
            " *"
        } else {
            ""
        };
        self.toolbar = i18n::tr_fmt(
            "Edition",
            "Toolbar",
            &[
                ("dirty", dirty),
                (
                    "beat",
                    &format!("{:.3}", self.workspace.row() as f64 / ROWS_PER_BEAT as f64),
                ),
                ("snap", SNAP_NAMES[self.workspace.snap()]),
                ("player", &(self.workspace.player() + 1).to_string()),
                (
                    "tool",
                    &i18n::tr("Edition", tool_key(self.workspace.tool())),
                ),
            ],
        );
        let chart = &self.workspace.file().session.document().charts()[self.workspace.chart()];
        let identity = deadsync_editor::chart_identity::Identity::read(
            self.workspace.file().session.document(),
            self.workspace.chart(),
        )
        .ok();
        let difficulty = identity.as_ref().map_or(chart.difficulty.as_str(), |v| {
            v.difficulty().map_or(v.raw_difficulty(), |d| d.as_str())
        });
        let rating = identity
            .as_ref()
            .map_or_else(|| chart.meter.clone(), |v| v.rating().to_string());
        self.chart_label = Arc::from(format!(
            "{} / {}   {}   {}   [{}]",
            self.workspace.chart() + 1,
            self.workspace.file().session.document().charts().len(),
            chart.steps_type,
            difficulty,
            rating
        ));
    }

    fn command(&mut self, command: Command) {
        match self.workspace.execute(command) {
            Ok(()) => {
                self.status = match command {
                    Command::Save => i18n::tr("Edition", "Saved"),
                    Command::SaveRecovery => i18n::tr("Edition", "DraftSaved"),
                    Command::Recover => i18n::tr("Edition", "DraftRecovered"),
                    Command::DuplicateChart => i18n::tr("Edition", "ChartDuplicated"),
                    _ => i18n::tr("Edition", "Ready"),
                }
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
        self.refresh();
    }

    /// The enclosing Practice screen ignores its logical pad navigation while
    /// this screen is open, so an Escape cannot silently abandon edited data.
    /// Returns true only after an explicit, safe close decision.
    pub fn raw_key(&mut self, event: &RawKeyboardEvent) -> bool {
        match event.code {
            KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight => {
                let bit = match event.code {
                    KeyCode::ControlLeft => 1,
                    KeyCode::ControlRight => 2,
                    KeyCode::SuperLeft => 4,
                    KeyCode::SuperRight => 8,
                    _ => unreachable!(),
                };
                if event.pressed {
                    self.control_keys |= bit;
                } else {
                    self.control_keys &= !bit;
                }
                self.control = self.control_keys != 0;
                return false;
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                let bit = if event.code == KeyCode::ShiftLeft { 1 } else { 2 };
                if event.pressed {
                    self.shift_keys |= bit;
                } else {
                    self.shift_keys &= !bit;
                }
                self.shift = self.shift_keys != 0;
                return false;
            }
            _ => {}
        }
        if !event.pressed {
            return false;
        }
        if !self.label_preedit.is_empty() {
            return false;
        }
        let repeating_navigation = self.control
            && ((event.code == KeyCode::KeyL && self.panel.is_none())
                || (event.code == KeyCode::KeyP
                    && matches!(self.panel, None | Some(Panel::Marker(_)))));
        let repeating_label = self.wants_text_input()
            && matches!(event.code, KeyCode::Backspace | KeyCode::Delete);
        let repeating_text_cursor = matches!(
            self.panel, Some(Panel::ChartText(_) | Panel::SongText(_) | Panel::ProfileText)
        )
            && matches!(
                event.code,
                KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home | KeyCode::End
            );
        let repeating_analysis = matches!(self.panel, Some(Panel::Analysis))
            && matches!(event.code, KeyCode::ArrowLeft | KeyCode::ArrowRight);
        let repeating_offset = matches!(
            self.panel,
            Some(Panel::ChartProperties(2 | 8) | Panel::SongProperties(0 | 4) | Panel::SongSample(0 | 1 | 2) | Panel::AudioSettings(0 | 1) | Panel::ZoomSettings(0..=2) | Panel::WaveformSettings(2 | 3) | Panel::WaveformColors(1..=13))
        ) && matches!(event.code, KeyCode::ArrowLeft | KeyCode::ArrowRight);
        if event.repeat
            && !repeating_offset
            && !repeating_label
            && !repeating_text_cursor
            && !repeating_navigation
            && !repeating_analysis
            && !matches!(
                event.code,
                KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::PageUp | KeyCode::PageDown
            )
        {
            return false;
        }
        if self.control && event.code == KeyCode::KeyP
            && matches!(self.panel, None | Some(Panel::Marker(_)))
        {
            self.stop_audio();
            self.navigate_marker(!self.shift);
            return false;
        }
        if self.panel.is_some() {
            return self.panel_key(event.code);
        }
        if self.shift && !self.control && event.code == KeyCode::KeyS {
            self.open_scroll_settings(); return false;
        }
        if !self.workspace.has_chart() {
            self.stop_audio();
            return self.no_chart_key(event.code);
        }
        if self.shift && !self.control && event.code == KeyCode::KeyZ { self.open_zoom_settings(); return false; }
        if self.shift && !self.control && event.code == KeyCode::KeyV { self.toggle_reverse_display(); return false; }
        if self.control && event.code == KeyCode::KeyT { self.toggle_spacing_mode(self.shift); return false; }
        if self.zoom_key(event) { return false; }
        if event.code == KeyCode::KeyW && !self.control {
            if self.shift { self.open_waveform_settings(); } else { self.toggle_waveform(); }
            return false;
        }
        if matches!(event.code, KeyCode::Enter | KeyCode::NumpadEnter)
            && !self.control
            && !self.shift
        {
            self.toggle_audio();
            return false;
        }
        if !self.control && matches!(event.code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.navigate_snap((event.code == KeyCode::ArrowDown) != self.reverse_display);
            return false;
        }
        self.stop_audio();
        if self.control && self.shift && event.code == KeyCode::Backspace {
            self.open_chart_deletion();
            return false;
        }
        if self.control && event.code == KeyCode::KeyN {
            self.open_new_chart();
            return false;
        }
        if event.code == KeyCode::F12 {
            self.open_analysis();
            return false;
        }
        if event.code == KeyCode::F11 {
            self.open_song_properties();
            return false;
        }
        if event.code == KeyCode::Escape || event.code == KeyCode::F2 {
            return self.request_close();
        }
        if event.code == KeyCode::F10 {
            self.open_timing();
            return false;
        }
        if event.code == KeyCode::F9 {
            self.open_pattern_set();
            return false;
        }
        if event.code == KeyCode::F8 {
            self.open_batch();
            return false;
        }
        if event.code == KeyCode::F7 {
            self.open_chart_properties();
            return false;
        }
        if event.code == KeyCode::F6 {
            self.open_marker();
            return false;
        }
        if event.code == KeyCode::F5 {
            self.regenerate_saved_pattern();
            return false;
        }
        if event.code == KeyCode::F4 {
            self.open_pattern();
            return false;
        }
        if event.code == KeyCode::F3 {
            self.panel = Some(Panel::Generation(0));
            self.status = i18n::tr("Edition", "GenerationOptionsHelp");
            return false;
        }
        if event.code == KeyCode::F1 {
            self.panel = Some(Panel::Menu(0));
            return false;
        }
        let command = if self.control {
            match event.code {
                KeyCode::KeyS if self.shift => Some(Command::SaveRecovery),
                KeyCode::KeyS => Some(Command::Save),
                KeyCode::KeyZ if self.shift => Some(Command::Redo),
                KeyCode::KeyZ => Some(Command::Undo),
                KeyCode::KeyY => Some(Command::Redo),
                KeyCode::KeyC => Some(Command::Copy),
                KeyCode::KeyX => Some(Command::Cut),
                KeyCode::KeyV => Some(Command::Paste),
                KeyCode::KeyR => Some(Command::Recover),
                KeyCode::KeyD => Some(Command::DuplicateChart),
                KeyCode::KeyL if self.shift => Some(Command::PreviousLabel),
                KeyCode::KeyL => Some(Command::NextLabel),
                KeyCode::Tab => {
                    let count = self.workspace.file().session.document().charts().len();
                    let delta = if self.shift { count - 1 } else { 1 };
                    Some(Command::SelectChart(
                        (self.workspace.chart() + delta) % count,
                    ))
                }
                _ => None,
            }
        } else {
            match event.code {
                KeyCode::PageUp => Some(Command::PreviousMeasure),
                KeyCode::PageDown => Some(Command::NextMeasure),
                KeyCode::Home => Some(Command::Seek(0)),
                KeyCode::End => Some(Command::Seek(
                    self.workspace
                        .grid()
                        .iter()
                        .map(|(pos, note)| note.end.unwrap_or(pos.row))
                        .max()
                        .unwrap_or(0),
                )),
                KeyCode::ArrowLeft => {
                    Some(Command::SetSnap(self.workspace.snap().saturating_sub(1)))
                }
                KeyCode::ArrowRight => Some(Command::SetSnap(
                    (self.workspace.snap() + 1).min(SNAP_ROWS.len() - 1),
                )),
                KeyCode::Space => Some(Command::ToggleSelection),
                KeyCode::Delete | KeyCode::Backspace => Some(Command::Delete),
                KeyCode::KeyP => Some(Command::SelectPlayer(
                    (self.workspace.player() + 1) % self.workspace.grid().players(),
                )),
                KeyCode::KeyA => Some(Command::ToggleAutoAdvance),
                KeyCode::Tab => {
                    let index = TOOLS
                        .iter()
                        .position(|&tool| tool == self.workspace.tool())
                        .unwrap_or(0);
                    Some(Command::SelectTool(
                        TOOLS[(index + if self.shift { TOOLS.len() - 1 } else { 1 }) % TOOLS.len()],
                    ))
                }
                code => lane_for_key(code).map(Command::Place),
            }
        };
        if let Some(command) = command {
            if !event.repeat
                || matches!(command,
                    Command::Move(_) | Command::Seek(_) | Command::PreviousLabel | Command::NextLabel
                    | Command::PreviousMeasure | Command::NextMeasure
                )
            {
                self.command(command);
            }
        }
        false
    }

    fn request_close(&mut self) -> bool {
        if self.workspace.file().session.is_dirty() {
            self.panel = Some(Panel::Unsaved(0));
            false
        } else {
            true
        }
    }

    fn panel_key(&mut self, code: KeyCode) -> bool {
        let panel = self.panel.unwrap();
        if let Panel::ProfileWeights(index) = panel { self.profile_weights_key(index, code); return false; }
        if let Panel::ProfileDelete(index) = panel { self.profile_delete_key(index, code); return false; }
        if let Panel::ProfileDefaults(index) = panel { self.profile_defaults_key(index, code); return false; }
        if let Panel::ProfileSpeed(index) = panel { self.profile_speed_key(index, code); return false; }
        if matches!(panel, Panel::ProfileText) { self.profile_text_key(code); return false; }
        if let Panel::ProfileEdit(index) = panel { self.profile_edit_key(index, code); return false; }
        if let Panel::DisplayTempo(index) = panel { self.display_tempo_key(index, code); return false; }
        if let Panel::ChartText(index) | Panel::SongText(index) = panel {
            self.text_field_key(index, code);
            return false;
        }
        if let Panel::WaveformColors(index) = panel {
            self.waveform_colors_key(index, code);
            return false;
        }
        if let Panel::WaveformSettings(index) = panel {
            self.waveform_settings_key(index, code);
            return false;
        }
        if let Panel::ScrollSettings(index) = panel { self.scroll_settings_key(index, code); return false; }
        if let Panel::ZoomSettings(index) = panel { self.zoom_settings_key(index, code); return false; }
        if let Panel::AudioSettings(index) = panel {
            self.audio_settings_key(index, code);
            return false;
        }
        if let Panel::SongSample(index) = panel {
            self.song_sample_key(index, code);
            return false;
        }
        if matches!(panel, Panel::Analysis) {
            return self.analysis_key(code);
        }
        if matches!(panel, Panel::Generating) {
            if matches!(code, KeyCode::Escape | KeyCode::Enter) {
                self.cancel_generation();
            }
            return false;
        }
        if matches!(panel, Panel::GenerationPreview(_))
            && matches!(
                code,
                KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
            )
        {
            let command = match code {
                KeyCode::Home => Command::Seek(0),
                KeyCode::End => Command::Seek(
                    self.preview
                        .as_ref()
                        .map(|preview| {
                            preview
                                .grid()
                                .iter()
                                .map(|(pos, note)| note.end.unwrap_or(pos.row))
                                .max()
                                .unwrap_or(0)
                        })
                        .unwrap_or(0),
                ),
                KeyCode::PageUp => Command::Move(-16),
                _ => Command::Move(16),
            };
            self.command(command);
            self.status = i18n::tr(
                "Edition",
                match self.preview {
                    Some(GenerationPreview::PatternSet { .. }) => "PatternSetPreviewHelp",
                    Some(GenerationPreview::Batch { .. }) => "BatchPreviewHelp",
                    _ => "GenerationPreviewHelp",
                },
            );
            return false;
        }
        if code == KeyCode::Escape {
            if matches!(
                panel,
                Panel::Profiles(_) | Panel::ChartExpression(_) | Panel::ConversionProfiles(_)
            ) {
                self.profile_selection = None;
                self.panel = Some(self.profile_return_panel());
                return false;
            }
            if matches!(panel, Panel::NewChart(_) | Panel::DeleteChart(_)) {
                self.status = i18n::tr("Edition", "Ready");
            }
            if matches!(panel, Panel::Timing(_)) {
                self.status = i18n::tr("Edition", "TimingDiscarded");
            }
            if matches!(panel, Panel::TimingMigration(_)) {
                self.status = i18n::tr("Edition", "LegacyTimingDiscarded");
            }
            self.deletion = None;
            self.new_chart = None;
            self.timing_migration = None;
            self.timing = None;
            self.chart_properties = None;
            self.chart_rating_input = None;
            self.song_properties = None;
            self.refresh_sample_region();
            self.marker = None;
            self.preview = None;
            self.panel = None;
            return false;
        }
        match panel {
            Panel::ProfileWeights(_) => unreachable!("profile weights dispatched above"),
            Panel::ProfileDelete(_) => unreachable!("profile deletion dispatched above"),
            Panel::ProfileDefaults(_) => unreachable!("profile defaults dispatched above"),
            Panel::ProfileSpeed(_) => unreachable!("profile speed dispatched above"),
            Panel::ProfileText => unreachable!("profile name dispatched above"),
            Panel::ProfileEdit(_) => unreachable!("expression editing dispatched above"),
            Panel::DisplayTempo(_) => unreachable!("display tempo dispatched above"),
            Panel::ChartText(_) | Panel::SongText(_) => unreachable!("text field input dispatched above"),
            Panel::NewChart(index) => self.new_chart_key(index, code),
            Panel::DeleteChart(index) => self.deletion_key(index, code),
            Panel::EmptyMenu(index) => return self.empty_menu_key(index, code),
            Panel::ConversionProfiles(index) => {
                if code == KeyCode::KeyE && !self.control && index < 2 {
                    self.open_profile_edit([ProfileKind::Performed, ProfileKind::Expressed][index], Panel::ConversionProfiles(index)); return false;
                }
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::ConversionProfiles(
                        (index + if code == KeyCode::ArrowUp { 5 } else { 1 }) % 6,
                    ));
                } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    if let Some(settings) = &mut self.profile_selection {
                        let (selected, kind) = match index {
                            0 => (&mut settings.performed, ProfileKind::Performed),
                            1 => (&mut settings.expressed, ProfileKind::Expressed),
                            _ => return false,
                        };
                        let count = settings.catalog.entries(kind).len() + 1;
                        *selected = Some(match *selected {
                            Some(current) => {
                                (current
                                    + if code == KeyCode::ArrowRight {
                                        1
                                    } else {
                                        count - 1
                                    })
                                    % count
                            }
                            None => {
                                if code == KeyCode::ArrowRight {
                                    0
                                } else {
                                    count - 1
                                }
                            }
                        });
                    }
                } else if code == KeyCode::Enter {
                    match index {
                        2 => self.stage_conversion_profiles(),
                        4..=5 => self.open_profile_edit([ProfileKind::Performed, ProfileKind::Expressed][index-4], Panel::ConversionProfiles(index-4)),
                        3 => {
                            self.profile_selection = None;
                            self.panel = Some(self.profile_return_panel());
                        }
                        _ => {}
                    }
                }
            }
            Panel::Timing(index) => self.timing_key(index, code),
            Panel::SongProperties(index) => self.song_properties_key(index, code),
            Panel::SongSample(_) => unreachable!("sample input dispatched above"),
            Panel::WaveformColors(_) => unreachable!("waveform colours dispatched above"),
            Panel::WaveformSettings(_) => unreachable!("waveform settings dispatched above"),
            Panel::ScrollSettings(_) => unreachable!("scroll settings dispatched above"),
            Panel::ZoomSettings(_) => unreachable!("zoom settings dispatched above"),
            Panel::AudioSettings(_) => unreachable!("audio settings dispatched above"),
            Panel::TimingMigration(index) => self.timing_migration_key(index, code),
            Panel::ChartProperties(index) => {
                if index == 8 && self.rating_input(code) {
                    return false;
                }
                if index == 8 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    self.adjust_chart_rating(code == KeyCode::ArrowRight);
                    return false;
                }
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::ChartProperties(
                        (index + if code == KeyCode::ArrowUp { 10 } else { 1 }) % 11,
                    ));
                } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    if let Some(draft) = &mut self.chart_properties {
                        match index {
                            7 => {
                                let forward = code == KeyCode::ArrowRight;
                                let next = draft.difficulty().map(|d| d.step(forward)).unwrap_or(
                                    if forward {
                                        deadsync_editor::chart_identity::Difficulty::Beginner
                                    } else {
                                        deadsync_editor::chart_identity::Difficulty::Edit
                                    },
                                );
                                draft.set_difficulty(next);
                            }
                            1 => draft.set_use_chart_offset(!draft.uses_chart_offset()),
                            2 => {
                                let delta = if self.shift { 0.001 } else { 0.0001 }
                                    * if code == KeyCode::ArrowRight {
                                        1.0
                                    } else {
                                        -1.0
                                    };
                                if let Err(error) =
                                    draft.set_music_offset(draft.music_offset() + delta)
                                {
                                    self.status = i18n::tr_fmt(
                                        "Edition",
                                        "Error",
                                        &[("message", &error.to_string())],
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                } else if code == KeyCode::Enter {
                    match index {
                        0 => self.open_chart_expression(),
                        4 => self.apply_chart_properties(),
                        6 => self.open_timing(),
                        9 => self.open_chart_text(),
                        10 => self.open_display_tempo(),
                        8 => {
                            if self.stage_chart_rating() {
                                self.status = i18n::tr("Edition", "ChartRatingStaged");
                            }
                        }
                        5 => {
                            self.chart_properties = None;
                            self.chart_rating_input = None;
                            self.panel = None;
                        }
                        _ => {}
                    }
                }
            }
            Panel::ChartExpression(index) => {
                if index == 0 && code == KeyCode::KeyE && !self.control { self.open_profile_edit(ProfileKind::Expressed, Panel::ChartExpression(0)); return false; }
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::ChartExpression(
                        (index + if code == KeyCode::ArrowUp { 3 } else { 1 }) % 4,
                    ));
                } else if index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    if let Some(settings) = &mut self.profile_selection {
                        let count = settings.catalog.entries(ProfileKind::Expressed).len();
                        if count > 0 {
                            settings.expressed = Some(match settings.expressed {
                                Some(current) => {
                                    (current
                                        + if code == KeyCode::ArrowRight {
                                            1
                                        } else {
                                            count - 1
                                        })
                                        % count
                                }
                                None => {
                                    if code == KeyCode::ArrowRight {
                                        0
                                    } else {
                                        count - 1
                                    }
                                }
                            });
                        }
                    }
                } else if code == KeyCode::Enter {
                    match index {
                        1 => self.stage_chart_expression(),
                        3 => self.open_profile_edit(ProfileKind::Expressed, Panel::ChartExpression(0)),
                        2 => {
                            self.profile_selection = None;
                            self.panel = Some(Panel::ChartProperties(0));
                        }
                        _ => {}
                    }
                }
            }
            Panel::Profiles(index) => {
                if code == KeyCode::KeyE && !self.control && index < 3 {
                    self.open_profile_edit([ProfileKind::Pattern, ProfileKind::Performed, ProfileKind::Expressed][index], Panel::Profiles(index)); return false;
                }
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::Profiles(
                        (index + if code == KeyCode::ArrowUp { 7 } else { 1 }) % 8,
                    ));
                } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    if let Some(settings) = &mut self.profile_selection {
                        let (selected, kind) = match index {
                            0 => (&mut settings.pattern, ProfileKind::Pattern),
                            1 => (&mut settings.performed, ProfileKind::Performed),
                            2 => (&mut settings.expressed, ProfileKind::Expressed),
                            _ => return false,
                        };
                        let count = settings.catalog.entries(kind).len();
                        if count > 0 {
                            *selected = Some(match *selected {
                                Some(current) => {
                                    (current
                                        + if code == KeyCode::ArrowRight {
                                            1
                                        } else {
                                            count - 1
                                        })
                                        % count
                                }
                                None => {
                                    if code == KeyCode::ArrowRight {
                                        0
                                    } else {
                                        count - 1
                                    }
                                }
                            });
                        }
                    }
                } else if code == KeyCode::Enter {
                    match index {
                        3 => self.stage_marker_profiles(),
                        5..=7 => self.open_profile_edit([ProfileKind::Pattern, ProfileKind::Performed, ProfileKind::Expressed][index-5], Panel::Profiles(index-5)),
                        4 => {
                            self.profile_selection = None;
                            self.panel = Some(Panel::Marker(12));
                        }
                        _ => {}
                    }
                }
            }
            Panel::ConfirmMarkerDelete => {
                if code == KeyCode::Enter {
                    self.commit_marker(true);
                }
            }
            Panel::Marker(index) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::Marker(
                        (index + if code == KeyCode::ArrowUp { 12 } else { 1 }) % 13,
                    ));
                } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    if let Some(draft) = &mut self.marker {
                        let forward = code == KeyCode::ArrowRight;
                        let spacing = SNAP_ROWS[self.workspace.snap()];
                        let adjust = |value: u64| {
                            if forward {
                                value.saturating_add(spacing).min(i32::MAX as u64)
                            } else {
                                value.saturating_sub(spacing)
                            }
                        };
                        match index {
                            0 => draft.start = adjust(draft.start),
                            1 => draft.definition.length = adjust(draft.definition.length),
                            2 => {
                                draft.definition.random_seed = draft
                                    .definition
                                    .random_seed
                                    .wrapping_add(if forward { 1 } else { -1 })
                            }
                            3 => {
                                draft.definition.start_position_inclusive =
                                    !draft.definition.start_position_inclusive
                            }
                            4 => {
                                draft.definition.end_position_inclusive =
                                    !draft.definition.end_position_inclusive
                            }
                            5 => {
                                draft.definition.ignore_preceding_distribution =
                                    !draft.definition.ignore_preceding_distribution
                            }
                            _ => {}
                        }
                    }
                } else if code == KeyCode::Enter {
                    match index {
                        6 => self.commit_marker(false),
                        7 => self.panel = Some(Panel::ConfirmMarkerDelete),
                        8 => {
                            self.marker = None;
                            self.panel = None;
                        }
                        9 => self.clear_marker_notes(),
                        10 => self.navigate_marker(false),
                        11 => self.navigate_marker(true),
                        12 => self.open_marker_profiles(),
                        _ => {}
                    }
                }
            }
            Panel::Pattern(index) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::Pattern(
                        (index + if code == KeyCode::ArrowUp { 8 } else { 1 }) % 9,
                    ));
                } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    let forward = code == KeyCode::ArrowRight;
                    match index {
                        1 => {
                            self.pattern.subdivision =
                                (self.pattern.subdivision + if forward { 1 } else { 8 }) % 9
                        }
                        2 => {
                            self.pattern.preset =
                                (self.pattern.preset + if forward { 1 } else { 3 }) % 4
                        }
                        3 => {
                            self.pattern.seed =
                                self.pattern.seed.wrapping_add(if forward { 1 } else { -1 })
                        }
                        4 => self.pattern.span.include_start = !self.pattern.span.include_start,
                        5 => self.pattern.span.include_end = !self.pattern.span.include_end,
                        6 => self.pattern.ignore_distribution = !self.pattern.ignore_distribution,
                        _ => {}
                    }
                } else if code == KeyCode::Enter {
                    match index {
                        7 => self.start_pattern(),
                        8 => self.panel = None,
                        _ => {}
                    }
                }
            }
            Panel::Batch(index) => self.batch_key(index, code),
            Panel::PatternSet(index) => self.pattern_set_key(index, code),
            Panel::Regeneration(index) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::Regeneration(
                        (index + if code == KeyCode::ArrowUp { 2 } else { 1 }) % 3,
                    ));
                } else if code == KeyCode::Enter {
                    self.panel = None;
                    match index {
                        0 => self.regenerate_saved_pattern(),
                        1 => self.open_pattern_set(),
                        _ => {}
                    }
                }
            }
            Panel::Generation(index) => {
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::Generation(
                        (index + if code == KeyCode::ArrowUp { 6 } else { 1 }) % 7,
                    ));
                } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    let forward = code == KeyCode::ArrowRight;
                    self.adjust_generation_setting(index, forward);
                } else if code == KeyCode::Enter {
                    match index {
                        3 => self.start_generation(),
                        4 => self.panel = None,
                        5 => self.open_conversion_profiles(),
                        _ => {}
                    }
                }
            }
            Panel::GenerationPreview(index) => {
                let all_failed = matches!(&self.preview, Some(GenerationPreview::Batch {preview, ..}) if preview.report().generated == 0);
                if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                    self.change_batch_entry(code == KeyCode::ArrowRight);
                    self.change_pattern_set_entry(code == KeyCode::ArrowRight);
                } else if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    self.panel = Some(Panel::GenerationPreview(if all_failed {
                        0
                    } else {
                        1 - index
                    }));
                } else if code == KeyCode::Enter {
                    if index == 1 || all_failed {
                        self.preview = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "GenerationDiscarded");
                    } else if let Some(preview) = &self.preview {
                        let pattern = matches!(preview, GenerationPreview::Pattern(_));
                        let clearing = matches!(preview, GenerationPreview::Clear(_));
                        let pattern_set = matches!(preview, GenerationPreview::PatternSet { .. });
                        let no_changes = matches!(preview,GenerationPreview::PatternSet{preview,..} if !preview.has_changes());
                        let batch = matches!(preview, GenerationPreview::Batch { .. });
                        let result = match preview {
                            GenerationPreview::Chart(p) => {
                                self.workspace.apply_generated(p).map(|_| ())
                            }
                            GenerationPreview::Batch { preview, .. } => {
                                self.workspace.apply_batch_generated(preview).map(|_| ())
                            }
                            GenerationPreview::Pattern(p) => {
                                self.workspace.apply_generated_pattern(p).map(|_| ())
                            }
                            GenerationPreview::Clear(p) => {
                                self.workspace.apply_cleared_pattern(p).map(|_| ())
                            }
                            GenerationPreview::PatternSet { preview, .. } => {
                                self.workspace.apply_pattern_set(preview).map(|_| ())
                            }
                        };
                        match result {
                            Ok(_) => {
                                self.preview = None;
                                self.panel = None;
                                self.status = i18n::tr(
                                    "Edition",
                                    if pattern_set {
                                        if no_changes {
                                            "PatternSetNoChanges"
                                        } else {
                                            "PatternSetApplied"
                                        }
                                    } else if batch {
                                        "BatchApplied"
                                    } else if clearing {
                                        "MarkerCleared"
                                    } else if pattern {
                                        "PatternApplied"
                                    } else {
                                        "GenerationApplied"
                                    },
                                );
                                self.refresh();
                            }
                            Err(error) => {
                                self.status = i18n::tr_fmt(
                                    "Edition",
                                    "Error",
                                    &[("message", &error.to_string())],
                                )
                            }
                        }
                    }
                }
            }
            Panel::Menu(index) | Panel::Transforms(index) | Panel::Unsaved(index) => {
                let count = match panel {
                    Panel::Menu(_) => 16,
                    Panel::Transforms(_) => 4,
                    _ => 3,
                };
                if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
                    let next = (index
                        + if code == KeyCode::ArrowUp {
                            count - 1
                        } else {
                            1
                        })
                        % count;
                    self.panel = Some(match panel {
                        Panel::Menu(_) => Panel::Menu(next),
                        Panel::Transforms(_) => Panel::Transforms(next),
                        _ => Panel::Unsaved(next),
                    });
                } else if code == KeyCode::Enter {
                    self.panel = None;
                    if matches!(panel, Panel::Transforms(_)) {
                        use deadsync_editor::transform::Transform;
                        let transform = match index {
                            0 => Transform::Mirror,
                            1 => Transform::Flip,
                            2 => Transform::ShiftRows(-(SNAP_ROWS[self.workspace.snap()] as i64)),
                            _ => Transform::ShiftRows(SNAP_ROWS[self.workspace.snap()] as i64),
                        };
                        self.command(Command::Transform(transform));
                    } else if matches!(panel, Panel::Unsaved(_)) {
                        match index {
                            1 => {
                                self.command(Command::Save);
                                return !self.workspace.file().session.is_dirty();
                            }
                            2 => self.panel = Some(Panel::ConfirmDiscard),
                            _ => {}
                        }
                    } else {
                        match index {
                            1 => self.command(Command::Save),
                            2 => self.command(Command::Undo),
                            3 => self.command(Command::Redo),
                            4 => self.panel = Some(Panel::Transforms(0)),
                            5 => {
                                self.panel = Some(Panel::Generation(0));
                                self.status = i18n::tr("Edition", "GenerationOptionsHelp");
                            }
                            6 => self.open_pattern(),
                            7 => self.panel = Some(Panel::Regeneration(0)),
                            8 => self.open_marker(),
                            9 => self.open_chart_properties(),
                            10 => self.open_batch(),
                            11 => self.command(Command::SaveRecovery),
                            12 => self.help = !self.help,
                            13 => return self.request_close(),
                            14 => self.open_analysis(),
                            15 => self.command(Command::DuplicateChart),
                            _ => {}
                        }
                    }
                }
            }
            Panel::ConfirmDiscard if code == KeyCode::Enter => return true,
            _ => {}
        }
        false
    }

    /// Called at a low frequency by the screen. Durable drafts never write the
    /// original chart. No I/O is performed from actor construction.
    pub fn autosave(&mut self) {
        let session = &self.workspace.file().session;
        if session.is_dirty() && session.revision() != self.last_recovery_revision {
            match self.workspace.file().save_recovery() {
                Ok(()) => self.last_recovery_revision = session.revision(),
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
    }

    pub fn actors(&self, actors: &mut Vec<Actor>) {
        let width = screen_width();
        let height = screen_height();
        actors.push(act!(quad: xy(width * 0.5, height * 0.5): zoomto(width, height): diffuse(0.035, 0.045, 0.065, 1.0): z(8000)));
        text(
            actors,
            &i18n::tr("Edition", "Title"),
            18.0,
            15.0,
            0.85,
            [0.45, 0.85, 1.0, 1.0],
        );
        text(actors, &self.toolbar, 18.0, 45.0, 0.62, [1.0; 4]);
        text(
            actors,
            &self.chart_label,
            18.0,
            70.0,
            0.55,
            [0.65, 0.7, 0.8, 1.0],
        );
        let shortcuts = i18n::tr("Edition", if self.workspace.has_chart() { "Shortcuts" } else { "NoChartShortcuts" });
        let shortcut_width = (width - 36.0).max(1.0);
        actors.push(act!(text: font("miso"): settext(shortcuts): align(0.0,0.0): xy(18.0,height-70.0): zoom(0.5): wrapwidthpixels(shortcut_width/0.5): maxwidth(shortcut_width): maxheight(20.0): diffuse(0.65,0.7,0.8,1.0): z(8006)));
        // Reserve a bounded multiline footer for localized help and errors.
        let status_width = (width - 36.0).max(1.0);
        actors.push(act!(text: font("miso"): settext(self.status.clone()): align(0.0,0.0): xy(18.0,height-50.0): zoom(0.5): wrapwidthpixels(status_width/0.5): maxwidth(status_width): maxheight(42.0): diffuse(0.9,0.9,0.9,1.0): z(8006)));
        if !self.workspace.has_chart() {
            self.no_chart_actors(actors);
            return;
        }
        let grid = self
            .preview
            .as_ref()
            .map(GenerationPreview::grid)
            .unwrap_or_else(|| self.workspace.grid());
        let side_panel = self.pattern_set_side_panel();
        let field_center = if side_panel {
            (width - 390.0) * 0.5
        } else {
            screen_center_x()
        };
        let available_width = if side_panel {
            width - 570.0
        } else {
            width - 210.0
        };
        let lane_width = (available_width.max(80.0) / f32::from(grid.columns())).min(48.0);
        let field_width = lane_width * f32::from(grid.columns());
        let left = field_center - field_width * 0.5;
        let top = 105.0;
        let bottom = (height - 85.0).max(top);
        let cursor_y = if self.reverse_display { (bottom - 55.0).max(top) } else { (top + 55.0).min(bottom) };
        let projection = match self.display_projection(top, bottom, cursor_y) {
            Ok(projection) => projection,
            Err(error) => {
                text(
                    actors,
                    &i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]),
                    18.0,
                    top,
                    0.6,
                    [1.0, 0.5, 0.5, 1.0],
                );
                if let Some(panel) = self.panel { self.panel_actors(panel, actors); }
                return;
            }
        };
        let label_end = self.labels.as_ref().and_then(|labels| {
            labels.values().last_key_value().map(|(&row, _)| row)
        }).unwrap_or(0);
        let last_row = grid.last_row()
            .max(self.workspace.row().saturating_add(48 * 16))
            .max(self.region_index.last_row())
            .max(label_end);
        let ranges = match projection.visible_ranges(top, bottom, last_row) {
            Ok(ranges) => ranges,
            Err(error) => {
                text(
                    actors,
                    &i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]),
                    18.0,
                    top,
                    0.6,
                    [1.0, 0.5, 0.5, 1.0],
                );
                if let Some(panel) = self.panel { self.panel_actors(panel, actors); }
                return;
            }
        };
        self.waveform_actors(actors, field_center, field_width, top, bottom, &projection);
        self.sample_region_actors(actors, field_center, field_width, top, bottom, &projection);
        for lane in 0..=grid.columns() {
            actors.push(act!(quad: xy(left + f32::from(lane) * lane_width, (top + bottom) * 0.5): zoomto(1.0, bottom - top): diffuse(0.12, 0.16, 0.22, 1.0): z(8001)));
        }
        let overview_bins = ((bottom - top).ceil() as usize)
            .saturating_add(2)
            .clamp(1, 4096);
        let range_budget = (overview_bins / ranges.len().max(1)).max(1);
        let mut label_candidates = Vec::new();
        for &(start, end) in ranges.iter().take(overview_bins) {
            let bands = deadsync_editor::projection::RowBands::new(start, end, range_budget)
                .expect("ordered viewport bounds and bounded overview bins");
            for (a, b) in bands {
                // Query one representative boundary per pixel band. At detailed
                // scales each band contains at most two integer rows.
                if let Some(beat_row) = a.div_ceil(48).checked_mul(48).filter(|r| *r <= b) {
                    let is_measure = self.signatures.as_ref().is_some_and(|signatures| {
                        signatures
                            .measure_position(beat_row)
                            .is_ok_and(|position| position.row_in_measure == 0)
                    });
                    if !is_measure {
                        if let Some(y) = projection
                            .y(beat_row)
                            .filter(|y| *y >= f64::from(top) && *y <= f64::from(bottom))
                        {
                            let y = y as f32;
                            actors.push(act!(quad: xy(field_center,y): zoomto(field_width,1.0): diffuse(0.6,0.7,0.8,0.15): z(8001)));
                        }
                    }
                }
                if let Some(signatures) = &self.signatures {
                    if a <= i32::MAX as u64 {
                        if let Ok(mut starts) = signatures.measure_starts(a, b.min(i32::MAX as u64))
                        {
                            if let Some((measure_row, measure)) = starts.next() {
                                if let Some(y) = projection
                                    .y(measure_row)
                                    .filter(|y| *y >= f64::from(top) && *y <= f64::from(bottom))
                                {
                                    let y = y as f32;
                                    actors.push(act!(quad: xy(field_center,y): zoomto(field_width,1.0): diffuse(0.6,0.7,0.8,0.4): z(8001)));
                                    text(
                                        actors,
                                        &(measure + 1).to_string(),
                                        left - 30.0,
                                        y,
                                        0.45,
                                        [0.6, 0.7, 0.8, 1.0],
                                    );
                                }
                            }
                        }
                    }
                }
                if let Some(labels) = &self.labels {
                    if let Some((&label_row, label)) = labels.values().range(a..=b).next() {
                        let Some(y) = projection
                            .y(label_row)
                            .filter(|y| *y >= f64::from(top) && *y <= f64::from(bottom))
                        else {
                            continue;
                        };
                        let y = y as f32;
                        label_candidates.push((label_row, f64::from(y), label.clone()));
                    }
                }
            }
        }
        spacing_view::label_actors(actors, label_candidates, left + field_width + 12.0, width);
        let mut shown_regions = std::collections::BTreeSet::new();
        for &(start, end) in ranges.iter().take(overview_bins) {
            for (a, b) in self
                .region_index
                .visible(start, end, range_budget)
                .expect("ordered viewport bounds and bounded region budget")
            {
                if !shown_regions.insert((a, b)) {
                    continue;
                }
                let (Some(y1), Some(y2)) = (
                    projection.clipped_y(a, top, bottom),
                    projection.clipped_y(b, top, bottom),
                ) else {
                    continue;
                };
                let (y1, y2) = (y1 as f32, y2 as f32);
                actors.push(act!(quad: xy(left-9.0,(y1+y2)*0.5): zoomto(4.0,(y2-y1).abs().max(2.0)): diffuse(0.7,0.4,1.0,0.8): z(8002)));
                if a >= start {
                    text(
                        actors,
                        &i18n::tr("Edition", "PatternMarker"),
                        left - 85.0,
                        y1 - 9.0,
                        0.5,
                        [0.8, 0.6, 1.0, 1.0],
                    );
                }
            }
        }
        let selection = match &self.preview {
            Some(GenerationPreview::Pattern(p)) => Some((p.generated().start, p.generated().end)),
            Some(GenerationPreview::PatternSet { preview, entry }) => {
                preview.entries().get(*entry).and_then(|entry| {
                    self.saved_regions
                        .iter()
                        .find(|(row, _)| *row == entry.row)
                        .copied()
                })
            }
            _ => self.workspace.selection(),
        };
        if let Some((a, b)) = selection {
            if let (Some(y1), Some(y2)) = (
                projection.clipped_y(a, top, bottom),
                projection.clipped_y(b, top, bottom),
            ) {
                let (y1, y2) = (y1 as f32, y2 as f32);
                actors.push(act!(quad: xy(field_center, (y1 + y2) * 0.5): zoomto(field_width, (y2 - y1).abs().max(2.0)): diffuse(0.2, 0.6, 0.9, 0.2): z(8002)));
            }
        }
        let size_zoom = self.zoom.size_zoom() as f32;
        let note_bins = ((bottom - top).ceil() as usize).clamp(1, 4096);
        let bins_per_range = (note_bins / ranges.len().max(1)).max(1);
        let mut shown_notes = std::collections::BTreeSet::new();
        for &(start, end) in ranges.iter().take(note_bins) {
            let visible_notes = grid
                .visible_overview(start, end, bins_per_range)
                .expect("ordered viewport bounds and bounded overview bins");
            for (position, note) in visible_notes {
                if !shown_notes.insert(position) {
                    continue;
                }
                let x = left + (f32::from(position.lane) + 0.5) * lane_width;
                let Some(y) = projection.note_y(position.row) else {
                    continue;
                };
                let y = y as f32;
                let color = player_color(position.player);
                if let Some(tail) = note.end {
                    let a = y.clamp(top, bottom);
                    let Some(tail_y) = projection.note_y(tail) else {
                        continue;
                    };
                    let b = (tail_y as f32).clamp(top, bottom);
                    actors.push(act!(quad: xy(x, (a + b) * 0.5): zoomto(lane_width * 0.3 * size_zoom, (b - a).abs()): diffuse(color[0], color[1], color[2], 0.65): z(8003)));
                }
                if y < top || y > bottom {
                    continue;
                }
                let symbol = match note.kind {
                    Kind::Mine => "M",
                    Kind::Lift => "L",
                    Kind::Fake => "F",
                    Kind::Keysound => "K",
                    Kind::Attack => "A",
                    _ => "",
                };
                if symbol.is_empty() {
                    let rotation = lane_rotation(
                        self.preview
                            .as_ref()
                            .map(GenerationPreview::layout)
                            .unwrap_or_else(|| {
                                &self.workspace.file().session.document().charts()
                                    [self.workspace.chart()]
                                .steps_type
                            }),
                        position.lane,
                    );
                    actors.push(act!(sprite("test_input/highlightarrow.png"): xy(x, y): setsize(lane_width * 0.7 * size_zoom, lane_width * 0.7 * size_zoom): rotationz(rotation): diffuse(color[0], color[1], color[2], 1.0): z(8004)));
                } else {
                    text(
                        actors,
                        symbol,
                        x - 6.0 * size_zoom,
                        y - 9.0 * size_zoom,
                        0.65 * size_zoom,
                        color,
                    );
                }
            }
        }
        actors.push(act!(quad: xy(field_center, cursor_y): zoomto(field_width, 2.0): diffuse(1.0, 0.85, 0.25, 1.0): z(8005)));
        if self.help
            && self.preview.is_none()
            && !matches!(
                self.panel,
                Some(
                    Panel::Timing(_)
                        | Panel::TimingMigration(_)
                        | Panel::SongProperties(_)
                        | Panel::SongSample(_)
                        | Panel::ScrollSettings(_)
                        | Panel::ZoomSettings(_)
                        | Panel::AudioSettings(_)
                        | Panel::WaveformSettings(_)
                        | Panel::WaveformColors(_)
                        | Panel::SongText(_)
                )
            )
        {
            self.help_actors(actors);
        }
        if let Some(panel) = self.panel {
            self.panel_actors(panel, actors);
        }
    }

    fn row_projection(&self, top: f32, bottom: f32, cursor_y: f32) -> deadsync_editor::projection::RowProjection {
        deadsync_editor::projection::RowProjection::new(
            self.workspace.row(), f64::from(cursor_y), self.zoom.spacing_zoom(), f64::from(top), f64::from(bottom),
        ).expect("finite editor viewport with cursor inside bounds")
            .with_reverse(self.reverse_display)
            .with_row_offset(self.playback_position().map_or(0.0, |(_, row)| row - self.workspace.row() as f64))
            .expect("finite mapped playback row")
    }

    fn help_actors(&self, actors: &mut Vec<Actor>) {
        actors.push(act!(quad: xy(screen_width() * 0.5, screen_height() * 0.5): zoomto(screen_width() - 30.0, 280.0): diffuse(0.05, 0.08, 0.12, 0.97): z(8010)));
        let label = i18n::tr("Edition", "Help");
        actors.push(act!(text: font("miso"): settext(label): align(0.0, 0.0): xy(35.0, screen_height() * 0.5 - 120.0): zoom(0.65): z(8011)));
    }

    fn panel_actors(&self, panel: Panel, actors: &mut Vec<Actor>) {
        if matches!(panel, Panel::Analysis) {
            self.analysis_actors(actors);
            return;
        }
        let profile_keys: Vec<&str> = self.profile_edit.as_ref().map(|draft| {
            let mut keys = vec!["ProfileEditField"; draft.editor.fields().len()];
            keys.extend(["ExpressionEditSaveCopy", "ExpressionEditCancel", "ProfileEditName", "ProfileEditSaveOriginal", "ProfileEditDescription"]);
            if draft.supports_travel_speed() { keys.extend(["ProfileSpeedOpen", "ProfileWeightsOpen"]); }
            keys.extend(["ProfileRestoreDefaults", "ProfileUndoSave", "ProfileRedoSave", "ProfileDeleteOpen"]);
            keys
        }).unwrap_or_default();
        let keys: &[&str] = match panel {
            Panel::Analysis => &[],
            Panel::DeleteChart(_) => &["KeepEditing", "DeleteChartConfirm"],
            Panel::EmptyMenu(_) => &["NewChartApply", "Undo", "Redo", "Save", "SaveDraft", "Recover", "Close"],
            Panel::NewChart(_) => &["NewChartLayout", "NewChartApply", "KeepEditing"],
            Panel::Menu(_) => &[
                "Resume",
                "Save",
                "Undo",
                "Redo",
                "Transforms",
                "GenerateChart",
                "GeneratePattern",
                "RegeneratePatternsMenu",
                "EditPatternMarker",
                "ChartProperties",
                "GenerateChartSet",
                "SaveDraft",
                "ToggleHelp",
                "Close",
                "AnalysisTitle",
                "DuplicateChart",
            ],
            Panel::Transforms(_) => &["Mirror", "Flip", "ShiftEarlier", "ShiftLater"],
            Panel::Unsaved(_) => &["KeepEditing", "SaveAndClose", "Discard"],
            Panel::ConfirmDiscard => &["ConfirmDiscard"],
            Panel::Regeneration(_) => &["RegeneratePattern", "RegeneratePatternSet", "KeepEditing"],
            Panel::PatternSet(_) => &[
                "PatternSetMarker",
                "PatternSetSelected",
                "PatternSetAll",
                "PatternSetNone",
                "PatternSetArea",
                "PatternSetStart",
                "KeepEditing",
            ],
            Panel::Batch(_) => &[
                "BatchSource",
                "GenerationTarget",
                "GenerationPreset",
                "GenerationSeed",
                "GenerationProfiles",
                "BatchStart",
                "KeepEditing",
                "GenerationExpressionSummary",
            ],
            Panel::Generation(_) => &[
                "GenerationTarget",
                "GenerationPreset",
                "GenerationSeed",
                "GenerationStart",
                "KeepEditing",
                "GenerationProfiles",
                "GenerationExpressionSummary",
            ],
            Panel::Pattern(_) => &[
                "PatternRange",
                "PatternSubdivision",
                "GenerationPreset",
                "GenerationSeed",
                "PatternIncludeStart",
                "PatternIncludeEnd",
                "PatternIgnoreDistribution",
                "GenerationStart",
                "KeepEditing",
            ],
            Panel::Marker(_) => &[
                "MarkerStart",
                "MarkerLength",
                "GenerationSeed",
                "PatternIncludeStart",
                "PatternIncludeEnd",
                "PatternIgnoreDistribution",
                "MarkerSave",
                "MarkerDelete",
                "KeepEditing",
                "MarkerClearNotes",
                "MarkerPrevious",
                "MarkerNext",
                "MarkerProfiles",
            ],
            Panel::ConversionProfiles(_) => &[
                "GenerationPerformedProfile",
                "GenerationExpressionProfile",
                "ConversionProfileUse",
                "ProfileBack",
                "ProfileEditPerformed", "ExpressionEditOpen",
            ],
            Panel::TimingMigration(_) => &["LegacyTimingApply", "LegacyTimingCancel"],
            Panel::WaveformColors(_) => &["WaveformColorMode","WaveformColorDarken","WaveformDenseRed","WaveformDenseGreen","WaveformDenseBlue","WaveformDenseAlpha","WaveformSparseRed","WaveformSparseGreen","WaveformSparseBlue","WaveformSparseAlpha","WaveformBackgroundRed","WaveformBackgroundGreen","WaveformBackgroundBlue","WaveformBackgroundAlpha","WaveformColorsBack"],
            Panel::WaveformSettings(_) => &["WaveformSettingShow", "WaveformSettingEnabled", "WaveformSettingWidth", "WaveformSettingDense", "WaveformSettingScroll", "WaveformSettingsDefaults", "WaveformSettingsSave", "WaveformSettingsCancel"],
            Panel::ScrollSettings(_) => &["ScrollStopPlayback", "ScrollSettingsDefaults", "ScrollSettingsSave", "ScrollSettingsCancel"],
            Panel::ZoomSettings(_) => &["ZoomDuration", "ZoomMultiplier", "ZoomSizeCap", "ZoomLimited", "ZoomSettingsDefaults", "ZoomSettingsSave", "ZoomSettingsCancel"],
            Panel::AudioSettings(_) => &["AudioFadeIn", "AudioFadeOut", "AudioSettingsDefaults", "AudioSettingsApply", "AudioSettingsCancel"],
            Panel::SongSample(_) => &["SongSampleStart", "SongSampleLength", "SongLastSecondHint", "SongSampleBack"],
            Panel::SongProperties(_) => &[
                "SongSyncOffset", "SongPropertiesApply", "SongPropertiesCancel", "SongTextOpen", "SongMusicOffset", "SongItgApply",
            ],
            Panel::Timing(_) => &[
                "TimingScope",
                "TimingBeat",
                "TimingBpm",
                "TimingSet",
                "TimingRemove",
                "TimingMove",
                "TimingPrevious",
                "TimingNext",
                "TimingAccept",
                "TimingCancel",
            ],
            Panel::SongText(_) => &[
                "SongTitle", "SongSubtitle", "SongArtist", "SongCredit", "SongGenre", "SongOrigin",
                "SongTitleTranslit", "SongSubtitleTranslit", "SongArtistTranslit", "SongMusic", "SongPreviewMusic", "SongTextBack",
            ],
            Panel::ChartText(_) => &[
                "ChartName", "ChartDescription", "ChartCredit", "ChartStyle",
                "ChartMusic", "ChartTextBack",
            ],
            Panel::ProfileEdit(_) => &profile_keys,
            Panel::ProfileText => self.profile_text_keys(),
            Panel::ProfileWeights(_) => &["ProfileWeightsLayout", "ProfileWeightsState", "ProfileWeightsAdd", "ProfileWeightsRemove", "ProfileWeightsReturn"],
            Panel::ProfileDelete(_) => &["ProfileDeleteCancel", "ProfileDeleteConfirm"],
            Panel::ProfileDefaults(_) => &["ProfileDefaultsApply", "ProfileDefaultsCancel"],
            Panel::ProfileSpeed(_) => &["ProfileSpeedSubdivision", "ProfileSpeedMin", "ProfileSpeedMax", "ProfileSpeedMinSeconds", "ProfileSpeedMaxSeconds", "ProfileSpeedStage", "ProfileSpeedCancel"],
            Panel::DisplayTempo(_) => &["DisplayTempoMode", "DisplayTempoMin", "DisplayTempoRange", "DisplayTempoMax", "DisplayTempoStage", "DisplayTempoCancel"],
            Panel::ChartProperties(_) => &[
                "ChartChooseExpression",
                "ChartUseOffset",
                "ChartMusicOffset",
                "ChartEffectiveOffset",
                "ChartApplyProperties",
                "KeepEditing",
                "TimingOpen",
                "ChartDifficulty",
                "ChartRating",
                "ChartTextOpen",
                "DisplayTempoOpen",
            ],
            Panel::ChartExpression(_) => {
                &["ProfileExpression", "ChartStageExpression", "ProfileBack", "ExpressionEditOpen"]
            }
            Panel::Profiles(_) => &[
                "ProfilePattern",
                "ProfilePerformed",
                "ProfileExpression",
                "ProfileStage",
                "ProfileBack",
                "ProfileEditPattern", "ProfileEditPerformed", "ExpressionEditOpen",
            ],
            Panel::ConfirmMarkerDelete => &["MarkerConfirmDelete"],
            Panel::Generating => &["GenerationCancel"],
            Panel::GenerationPreview(_) if matches!(&self.preview,Some(GenerationPreview::Batch {preview,..}) if preview.report().generated == 0) => {
                &["GenerationDiscard"]
            }
            Panel::GenerationPreview(_) => &["GenerationApply", "GenerationDiscard"],
        };
        let selected = match panel {
            Panel::EmptyMenu(i)
            | Panel::DeleteChart(i)
            | Panel::NewChart(i)
            | Panel::Menu(i)
            | Panel::Transforms(i)
            | Panel::Unsaved(i)
            | Panel::Pattern(i)
            | Panel::Marker(i)
            | Panel::Profiles(i)
            | Panel::TimingMigration(i)
            | Panel::Timing(i)
            | Panel::ProfileWeights(i)
            | Panel::ProfileDelete(i)
            | Panel::ProfileDefaults(i)
            | Panel::ProfileSpeed(i)
            | Panel::ProfileEdit(i)
            | Panel::DisplayTempo(i)
            | Panel::ChartProperties(i)
            | Panel::ChartText(i)
            | Panel::SongText(i)
            | Panel::SongProperties(i)
            | Panel::SongSample(i)
            | Panel::ScrollSettings(i)
            | Panel::ZoomSettings(i)
            | Panel::AudioSettings(i)
            | Panel::WaveformSettings(i)
            | Panel::WaveformColors(i)
            | Panel::ChartExpression(i)
            | Panel::ConversionProfiles(i)
            | Panel::Generation(i)
            | Panel::Batch(i)
            | Panel::Regeneration(i)
            | Panel::PatternSet(i)
            | Panel::GenerationPreview(i) => i,
            _ => 0,
        };
        let menu_start = if matches!(panel, Panel::Menu(_) | Panel::SongText(_) | Panel::WaveformColors(_) | Panel::ProfileEdit(_)) {
            selected / 8 * 8
        } else { 0 };
        let visible_keys = if matches!(panel, Panel::Menu(_) | Panel::SongText(_) | Panel::WaveformColors(_) | Panel::ProfileEdit(_)) {
            keys.len().saturating_sub(menu_start).min(8)
        } else {
            keys.len()
        };
        let previewing = matches!(panel, Panel::GenerationPreview(_));
        let side_panel = self.pattern_set_side_panel();
        let panel_width = if side_panel {
            360.0
        } else if previewing {
            screen_width() - 30.0
        } else {
            520.0
        };
        let panel_x = if side_panel {
            screen_width() - 195.0
        } else {
            screen_width() * 0.5
        };
        let extra_height = if side_panel {
            220.0
        } else if previewing {
            128.0
        } else if matches!(panel, Panel::NewChart(_) | Panel::DeleteChart(_)) {
            85.0
        } else if matches!(panel, Panel::TimingMigration(_)) {
            125.0
        } else if matches!(panel, Panel::Timing(_)) {
            36.0
        } else {
            0.0
        };
        let menu_footer = if matches!(panel, Panel::Menu(_) | Panel::SongText(_) | Panel::WaveformColors(_) | Panel::ProfileEdit(_)) {
            24.0
        } else { 0.0 };
        let row_height = if matches!(panel, Panel::ChartProperties(_)) { 26.0 } else { 32.0 };
        let panel_height = visible_keys as f32 * row_height + 30.0 + extra_height + menu_footer;
        let panel_y = if previewing && !side_panel {
            screen_height() - panel_height * 0.5 - 8.0
        } else {
            screen_height() * 0.5
        };
        actors.push(act!(quad: xy(panel_x, panel_y): setsize(panel_width, panel_height): diffuse(0.08, 0.1, 0.15, 1.0): z(8020)));
        if matches!(panel, Panel::Menu(_) | Panel::SongText(_) | Panel::WaveformColors(_) | Panel::ProfileEdit(_)) {
            let page = i18n::tr_fmt("Edition",
                if matches!(panel, Panel::WaveformColors(_)) { "WaveformColorsPage" } else if matches!(panel, Panel::SongText(_)) { "TextFieldsPage" } else { "MenuPage" }, &[
                ("page", &(selected / 8 + 1).to_string()),
                ("pages", &keys.len().div_ceil(8).to_string()),
            ]);
            actors.push(act!(text: font("miso"): settext(page): align(0.5,0.5): xy(panel_x,panel_y+panel_height*0.5-10.0): zoom(0.45): z(8021)));
        }
        if matches!(panel, Panel::DeleteChart(_)) {
            let summary = self.deletion_summary();
            actors.push(act!(text: font("miso"): settext(summary): align(0.5,0.0): xy(panel_x,panel_y-panel_height*0.5+12.0): zoom(0.65): wrapwidthpixels((panel_width-30.0)/0.65): maxwidth(panel_width-30.0): maxheight(70.0): z(8021)));
        }
        if matches!(panel, Panel::NewChart(_)) {
            let summary = self.new_chart_summary();
            actors.push(act!(text: font("miso"): settext(summary): align(0.5,0.0): xy(panel_x,panel_y-panel_height*0.5+12.0): zoom(0.65): wrapwidthpixels((panel_width-30.0)/0.65): maxwidth(panel_width-30.0): maxheight(70.0): z(8021)));
        }
        if matches!(panel, Panel::TimingMigration(_)) {
            let summary = self.timing_migration_summary();
            actors.push(act!(text: font("miso"): settext(summary): align(0.5,0.0): xy(panel_x,panel_y-panel_height*0.5+10.0): zoom(0.65): wrapwidthpixels((panel_width-30.0)/0.65): maxwidth(panel_width-30.0): maxheight(110.0): z(8021)));
        }
        if matches!(panel, Panel::Timing(_)) {
            let heading = self.timing_heading();
            actors.push(act!(text: font("miso"): settext(heading): align(0.5,0.0): xy(panel_x,panel_y-panel_height*0.5+10.0): zoom(0.65): maxwidth(panel_width-30.0): z(8021)));
        }
        if let Some(preview) = self.preview.as_ref().filter(|_| previewing) {
            if let GenerationPreview::PatternSet { preview, entry } = preview {
                let summary = self.pattern_set_summary(preview, *entry);
                actors.push(act!(text: font("miso"): settext(summary): align(0.5,0.0): xy(panel_x,panel_y-panel_height*0.5+10.0): zoom(0.6): wrapwidthpixels((panel_width-30.0)/0.6): maxwidth(panel_width-30.0): maxheight(extra_height-20.0): z(8021)));
            } else if let GenerationPreview::Batch { preview, entry } = preview {
                let summary = self.batch_summary(preview, *entry);
                actors.push(act!(text: font("miso"): settext(summary): align(0.5,0.0): xy(screen_width()*0.5,panel_y-panel_height*0.5+10.0): zoom(0.6): wrapwidthpixels((screen_width()-60.0)/0.6): maxwidth(screen_width()-60.0): maxheight(110.0): z(8021)));
            } else if let GenerationPreview::Clear(preview) = preview {
                let report = preview.report();
                let summary = i18n::tr_fmt(
                    "Edition",
                    "MarkerClearSummary",
                    &[
                        ("removed", &report.removed.to_string()),
                        ("shortened", &report.shortened_holds.to_string()),
                        ("issues", &preview.profile_issues().len().to_string()),
                    ],
                );
                actors.push(act!(text: font("miso"): settext(summary): align(0.5, 0.0): xy(screen_width()*0.5,panel_y-panel_height*0.5+10.0): zoom(0.6): z(8021)));
            } else if let GenerationPreview::Chart(preview) = preview {
                let report = preview.report();
                let summary = i18n::tr_fmt(
                    "Edition",
                    "GenerationSummary",
                    &[
                        ("layout", preview.target_layout()),
                        ("seed", &preview.seed().to_string()),
                        ("notes", &report.notes.to_string()),
                        ("source_notes", &report.source_notes.to_string()),
                        ("source_mines", &report.source_mines.to_string()),
                        ("mines", &report.mines.to_string()),
                        ("skipped", &report.skipped_events.to_string()),
                        ("dropped", &report.dropped_mines.to_string()),
                        ("merged", &report.merged_mines.to_string()),
                        ("keysounds", &report.ignored_keysounds.to_string()),
                    ],
                );
                actors.push(act!(text: font("miso"): settext(summary): align(0.5, 0.0): xy(screen_width() * 0.5, panel_y - panel_height * 0.5 + 10.0): zoom(0.6): z(8021)));
                if !preview.profile_issues().is_empty() {
                    let issues = i18n::tr_fmt(
                        "Edition",
                        "PatternProfileIssues",
                        &[("count", &preview.profile_issues().len().to_string())],
                    );
                    actors.push(act!(text: font("miso"): settext(issues): align(0.5, 0.0): xy(screen_width()*0.5,panel_y-panel_height*0.5+86.0): zoom(0.6): z(8021)));
                }
                if report.empty {
                    actors.push(act!(text: font("miso"): settext(i18n::tr("Edition", "GenerationEmpty")): align(0.5, 0.5): xy(screen_width() * 0.5, panel_y - panel_height * 0.5 + 108.0): zoom(0.6): diffuse(1.0, 0.65, 0.3, 1.0): z(8021)));
                }
            } else if let GenerationPreview::Pattern(preview) = preview {
                let report = preview.deletions();
                let mut warnings = if preview.generated().warnings.is_empty() {
                    i18n::tr("Edition", "PatternNoWarnings").to_string()
                } else {
                    preview.generated().warnings.iter().map(|warning|i18n::tr("Edition",match warning {
                        deadsync_editor::generation::pattern::EndWarning::RelaxedToOneFoot=>"PatternRelaxed",
                        deadsync_editor::generation::pattern::EndWarning::FollowingFootingUnreachable=>"PatternUnreachable",
                    }).to_string()).collect::<Vec<_>>().join(" ")
                };
                if !preview.profile_issues().is_empty() {
                    warnings.push('\n');
                    warnings.push_str(&i18n::tr_fmt(
                        "Edition",
                        "PatternProfileIssues",
                        &[("count", &preview.profile_issues().len().to_string())],
                    ));
                }
                let summary = i18n::tr_fmt(
                    "Edition",
                    "PatternSummary",
                    &[
                        (
                            "start",
                            &format!("{:.3}", preview.generated().start as f64 / 48.0),
                        ),
                        (
                            "end",
                            &format!("{:.3}", preview.generated().end as f64 / 48.0),
                        ),
                        ("steps", &preview.generated().steps.len().to_string()),
                        ("removed", &report.removed.to_string()),
                        ("shortened", &report.shortened_holds.to_string()),
                        ("nps", &format!("{:.2}", preview.average_nps())),
                        ("warnings", &warnings),
                    ],
                );
                actors.push(act!(text: font("miso"): settext(summary): align(0.5, 0.0): xy(screen_width()*0.5,panel_y-panel_height*0.5+10.0): zoom(0.6): z(8021)));
            }
        }
        for (index, key) in keys.iter().enumerate().skip(menu_start).take(visible_keys) {
            let color = if index == selected {
                [1.0, 0.85, 0.25, 1.0]
            } else {
                [0.8, 0.8, 0.85, 1.0]
            };
            let label = if (matches!(panel, Panel::ChartText(_)) && index < 5)
                || (matches!(panel, Panel::SongText(_)) && index < deadsync_editor::song_text::Field::ALL.len())
            {
                self.text_field_label(index)
            } else if matches!(panel, Panel::ProfileText) && index == 0 {
                self.profile_text_label()
            } else if matches!(panel, Panel::ProfileWeights(_)) {
                self.profile_weights_label(index, key)
            } else if matches!(panel, Panel::ProfileSpeed(_)) {
                self.profile_speed_label(index, key)
            } else if matches!(panel, Panel::ProfileEdit(_)) {
                self.profile_edit_label(index, key)
            } else if matches!(panel, Panel::DisplayTempo(_)) {
                self.display_tempo_label(index, key)
            } else if matches!(panel, Panel::ChartProperties(_)) && index == 7 {
                self.chart_difficulty_label()
            } else if matches!(panel, Panel::ChartProperties(_)) && index == 8 {
                self.chart_rating_label()
            } else if matches!(panel, Panel::NewChart(_)) && index == 0 {
                self.new_chart_label()
            } else if matches!(panel, Panel::ConversionProfiles(_)) && index < 2 {
                let value = self
                    .profile_selection
                    .as_ref()
                    .and_then(|settings| {
                        let (selected, kind, default) = if index == 0 {
                            (
                                settings.performed,
                                ProfileKind::Performed,
                                "ConversionPresetDefault",
                            )
                        } else {
                            (
                                settings.expressed,
                                ProfileKind::Expressed,
                                "ConversionExpressionDefault",
                            )
                        };
                        match selected? {
                            0 => Some(i18n::tr("Edition", default).to_string()),
                            selected => settings
                                .catalog
                                .entries(kind)
                                .get(selected - 1)
                                .map(profile_entry_name),
                        }
                    })
                    .unwrap_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string());
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            } else if matches!(panel, Panel::ChartProperties(_)) && (1..=3).contains(&index) {
                let value = self
                    .chart_properties
                    .as_ref()
                    .map(|draft| match index {
                        1 => i18n::tr(
                            "Edition",
                            if draft.uses_chart_offset() {
                                "PatternYes"
                            } else {
                                "PatternNo"
                            },
                        )
                        .to_string(),
                        2 => format!("{:.6}", draft.music_offset()),
                        _ => format!("{:.6}", draft.effective_music_offset()),
                    })
                    .unwrap_or_default();
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            } else if (matches!(panel, Panel::Profiles(_)) && index < 3)
                || (matches!(panel, Panel::ChartExpression(_)) && index == 0)
            {
                let value = self
                    .profile_selection
                    .as_ref()
                    .and_then(|settings| {
                        let (selected, kind) = match index {
                            _ if matches!(panel, Panel::ChartExpression(_)) => {
                                (settings.expressed, ProfileKind::Expressed)
                            }
                            0 => (settings.pattern, ProfileKind::Pattern),
                            1 => (settings.performed, ProfileKind::Performed),
                            _ => (settings.expressed, ProfileKind::Expressed),
                        };
                        let entries = settings.catalog.entries(kind);
                        selected.and_then(|selected| {
                            entries.get(selected).map(|entry| {
                                format!(
                                    "{}/{} {}",
                                    selected + 1,
                                    entries.len(),
                                    profile_entry_name(entry)
                                )
                            })
                        })
                    })
                    .unwrap_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string());
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            } else if matches!(panel, Panel::Marker(_)) {
                let value = self
                    .marker
                    .as_ref()
                    .map(|draft| match index {
                        0 => format!("{:.3}", draft.start as f64 / 48.0),
                        1 => format!("{:.3}", draft.definition.length as f64 / 48.0),
                        2 => draft.definition.random_seed.to_string(),
                        3..=5 => i18n::tr(
                            "Edition",
                            if match index {
                                3 => draft.definition.start_position_inclusive,
                                4 => draft.definition.end_position_inclusive,
                                _ => draft.definition.ignore_preceding_distribution,
                            } {
                                "PatternYes"
                            } else {
                                "PatternNo"
                            },
                        )
                        .to_string(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                if index < 6 {
                    i18n::tr_fmt("Edition", key, &[("value", &value)])
                } else {
                    i18n::tr("Edition", key)
                }
            } else if matches!(panel, Panel::Pattern(_)) {
                let value = match index {
                    0 => format!(
                        "{:.3} - {:.3}",
                        self.pattern.span.start as f64 / 48.0,
                        self.pattern.span.end as f64 / 48.0
                    ),
                    1 => format!("1/{}", PATTERN_SUBDIVISIONS[self.pattern.subdivision] * 4),
                    2 => i18n::tr(
                        "Edition",
                        [
                            "PatternBalanced",
                            "PatternNoCandles",
                            "PatternNoInward",
                            "PatternNoTransitionLimit",
                        ][self.pattern.preset],
                    )
                    .to_string(),
                    3 => self.pattern.seed.to_string(),
                    4..=6 => i18n::tr(
                        "Edition",
                        if match index {
                            4 => self.pattern.span.include_start,
                            5 => self.pattern.span.include_end,
                            _ => self.pattern.ignore_distribution,
                        } {
                            "PatternYes"
                        } else {
                            "PatternNo"
                        },
                    )
                    .to_string(),
                    _ => String::new(),
                };
                if index < 7 {
                    i18n::tr_fmt("Edition", key, &[("value", &value)])
                } else {
                    i18n::tr("Edition", key)
                }
            } else if matches!(panel, Panel::GenerationPreview(_))
                && index == 0
                && matches!(self.preview, Some(GenerationPreview::Clear(_)))
            {
                i18n::tr("Edition", "MarkerClearApply")
            } else if matches!(panel, Panel::GenerationPreview(_))
                && index == 0
                && matches!(self.preview, Some(GenerationPreview::Pattern(_)))
            {
                i18n::tr(
                    "Edition",
                    if matches!(&self.preview,Some(GenerationPreview::Pattern(p)) if p.persists_marker())
                    {
                        "PatternApplyPersistent"
                    } else {
                        "PatternApply"
                    },
                )
            } else if matches!(panel, Panel::GenerationPreview(_))
                && index == 0
                && matches!(&self.preview,Some(GenerationPreview::Batch {preview,..}) if preview.report().generated > 0)
            {
                i18n::tr_fmt(
                    "Edition",
                    "BatchApply",
                    &[(
                        "count",
                        &self
                            .batch_preview()
                            .unwrap()
                            .0
                            .report()
                            .generated
                            .to_string(),
                    )],
                )
            } else if matches!(panel, Panel::GenerationPreview(_))
                && index == 0
                && matches!(&self.preview, Some(GenerationPreview::PatternSet { .. }))
            {
                i18n::tr("Edition", "PatternSetApply")
            } else if matches!(panel, Panel::Timing(_)) {
                self.timing_label(index, key)
            } else if matches!(panel, Panel::WaveformColors(_)) && index < 14 {
                self.waveform_colors_label(index,key)
            } else if matches!(panel, Panel::WaveformSettings(_)) && index < 5 {
                self.waveform_settings_label(index, key)
            } else if matches!(panel, Panel::ScrollSettings(_)) && index == 0 {
                self.scroll_settings_label(key)
            } else if matches!(panel, Panel::ZoomSettings(_)) && index < 4 {
                self.zoom_settings_label(index, key)
            } else if matches!(panel, Panel::AudioSettings(_)) && index < 2 {
                self.audio_settings_label(index, key)
            } else if matches!(panel, Panel::SongSample(_)) && index < 3 {
                self.song_sample_label(index, key)
            } else if matches!(panel, Panel::SongProperties(_)) && matches!(index, 0 | 4) {
                let offset = self.song_properties.as_ref().map_or(0.0, |draft| if index == 4 { draft.music_offset() } else { draft.sync_offset() });
                i18n::tr_fmt("Edition", key, &[("seconds", &format!("{offset:.6}"))])
            } else if matches!(panel, Panel::PatternSet(_)) {
                self.pattern_set_label(index, key)
            } else if matches!(panel, Panel::Batch(_)) {
                self.batch_label(index, key)
            } else if matches!(panel, Panel::Generation(_)) {
                self.generation_label(index, key)
            } else {
                i18n::tr("Edition", key)
            };
            actors.push(act!(text: font("miso"): settext(label): align(0.5, 0.5): xy(panel_x, panel_y - panel_height * 0.5 + extra_height + 30.0 + (index - menu_start) as f32 * row_height): zoom(0.8): diffuse(color[0], color[1], color[2], color[3]): z(8021)));
            if matches!(panel, Panel::WaveformColors(_)) && index<14 {
                let rgba=self.waveform_color_swatch(index);
                let y=panel_y-panel_height*0.5+extra_height+30.0+(index-menu_start) as f32*32.0;
                actors.push(act!(quad: xy(panel_x+panel_width*0.5-20.0,y): zoomto(16.0,16.0): diffuse(rgba[0],rgba[1],rgba[2],rgba[3]): z(8022)));
            }
            if side_panel || matches!(panel, Panel::ChartText(_) | Panel::SongText(_) | Panel::ProfileEdit(_) | Panel::ProfileText) {
                if let Some(Actor::Text { max_width, .. }) = actors.last_mut() {
                    *max_width = Some(panel_width - 30.0);
                }
            }
        }
    }
}

fn profile_entry_name(entry: &profile_catalog::Entry) -> String {
    if let Some(builtin) = entry.builtin {
        let key = match builtin {
            BuiltinLabel::Sixteenths => "ProfileSixteenths",
            BuiltinLabel::Eighths => "ProfileEighths",
            BuiltinLabel::Balanced => "GenerationBalanced",
            BuiltinLabel::StaminaSingle => "GenerationStaminaSingle",
            BuiltinLabel::StaminaDouble => "GenerationStaminaDouble",
            BuiltinLabel::PatternBalanced => "PatternBalanced",
            BuiltinLabel::NoCandles => "PatternNoCandles",
            BuiltinLabel::NoInward => "PatternNoInward",
            BuiltinLabel::NoTransitionLimit => "PatternNoTransitionLimit",
            BuiltinLabel::Dynamic => "ProfileDynamic",
            BuiltinLabel::AggressiveBrackets => "ProfileAggressiveBrackets",
            BuiltinLabel::NoBrackets => "ProfileNoBrackets",
        };
        return i18n::tr("Edition", key).to_string();
    }
    let name = entry
        .name
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() {
        return i18n::tr("Edition", "ProfileUnnamed").to_string();
    }
    let mut short: String = name.chars().take(32).collect();
    if name.chars().count() > 32 {
        short.push_str("...");
    }
    short
}

fn text(actors: &mut Vec<Actor>, label: &str, x: f32, y: f32, zoom: f32, color: [f32; 4]) {
    actors.push(act!(text: font("miso"): settext(label.to_owned()): align(0.0, 0.0): xy(x, y): zoom(zoom): diffuse(color[0], color[1], color[2], color[3]): z(8006)));
}

fn player_color(player: u32) -> [f32; 4] {
    const COLORS: [[f32; 4]; 6] = [
        [0.2, 0.65, 1.0, 1.0],
        [1.0, 0.35, 0.65, 1.0],
        [0.35, 1.0, 0.55, 1.0],
        [1.0, 0.75, 0.25, 1.0],
        [0.7, 0.4, 1.0, 1.0],
        [0.3, 1.0, 1.0, 1.0],
    ];
    COLORS[player as usize % COLORS.len()]
}

fn lane_rotation(layout: &str, lane: u16) -> f32 {
    if layout.starts_with("pump-") {
        [135.0, 225.0, 0.0, 45.0, 315.0][usize::from(lane % 5)]
    } else {
        [180.0, 90.0, 270.0, 0.0][usize::from(lane % 4)]
    }
}

fn lane_for_key(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::Digit1 => 0,
        KeyCode::Digit2 => 1,
        KeyCode::Digit3 => 2,
        KeyCode::Digit4 => 3,
        KeyCode::Digit5 => 4,
        KeyCode::Digit6 => 5,
        KeyCode::Digit7 => 6,
        KeyCode::Digit8 => 7,
        KeyCode::Digit9 => 8,
        KeyCode::Digit0 => 9,
        _ => return None,
    })
}

fn tool_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Tap => "Tap",
        Kind::Hold => "Hold",
        Kind::Roll => "Roll",
        Kind::Mine => "Mine",
        Kind::Lift => "Lift",
        Kind::Fake => "Fake",
        Kind::Keysound => "Keysound",
        Kind::Attack => "Attack",
    }
}
