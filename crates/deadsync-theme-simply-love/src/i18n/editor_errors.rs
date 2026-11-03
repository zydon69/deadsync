//! User-facing editor diagnostics. Core errors remain suitable for logs and APIs.
//! Only known semantic messages are translated; unknown details remain intact.
use deadsync_editor::Error;
use std::{fmt::Display, io};

pub(crate) trait EditorDiagnostic: Display {
    fn localized(&self) -> String;
}

pub(crate) fn editor_error(error: &(impl EditorDiagnostic + ?Sized)) -> String {
    error.localized()
}

fn translated(key: &str) -> String {
    super::tr("Edition", key).to_string()
}

impl EditorDiagnostic for Error {
    fn localized(&self) -> String {
        match self {
            Self::ExternalChange => translated("DiagnosticExternalChange"),
            Self::RecoveryConflict => translated("DiagnosticRecoveryConflict"),
            Self::OutputReviewRequired => translated("DiagnosticOutputReview"),
            Self::Io(error) => error.localized(),
            Self::InvalidFormat(message)
            | Self::InvalidEdit(message)
            | Self::Unsupported(message) => legacy(message),
        }
    }
}

impl EditorDiagnostic for io::Error {
    fn localized(&self) -> String {
        // Custom payloads may contain additional user-relevant context. Do not
        // replace them with a generic kind and lose that information.
        if self.raw_os_error().is_none() && self.get_ref().is_some() {
            return legacy(&self.to_string());
        }
        let key = match self.kind() {
            io::ErrorKind::NotFound => "DiagnosticIoNotFound",
            io::ErrorKind::PermissionDenied => "DiagnosticIoPermission",
            io::ErrorKind::AlreadyExists => "DiagnosticIoExists",
            io::ErrorKind::InvalidData => "DiagnosticIoData",
            io::ErrorKind::InvalidInput => "DiagnosticIoInput",
            io::ErrorKind::Interrupted => "DiagnosticIoInterrupted",
            io::ErrorKind::TimedOut => "DiagnosticIoTimeout",
            io::ErrorKind::WriteZero => "DiagnosticIoWriteZero",
            io::ErrorKind::UnexpectedEof => "DiagnosticIoEof",
            io::ErrorKind::Unsupported => "DiagnosticIoUnsupported",
            io::ErrorKind::StorageFull => "DiagnosticIoFull",
            io::ErrorKind::ReadOnlyFilesystem => "DiagnosticIoReadOnly",
            _ => return legacy(&self.to_string()),
        };
        translated(key)
    }
}

impl EditorDiagnostic for String {
    fn localized(&self) -> String {
        legacy(self)
    }
}
impl EditorDiagnostic for str {
    fn localized(&self) -> String {
        legacy(self)
    }
}

impl<T: EditorDiagnostic + ?Sized> EditorDiagnostic for &T {
    fn localized(&self) -> String {
        T::localized(*self)
    }
}

impl EditorDiagnostic for crate::editor_audio::LoadError {
    fn localized(&self) -> String {
        match self {
            Self::Busy => translated("DiagnosticWaveformBusy"),
            Self::Cancelled => translated("DiagnosticWaveformCancelled"),
            Self::WorkerStopped => translated("DiagnosticWaveformStopped"),
            Self::Decode(details) => legacy(details),
        }
    }
}

fn legacy(message: &str) -> String {
    if let Some(raw) = message.strip_prefix("Unsafe source stream NoteType: ") {
        return translated("DiagnosticUnsafeStreamNoteType").replace("{value}", raw);
    }
    if let Some(details)=message.strip_prefix("Lossless authoring promotion refused: "){
        return super::tr_fmt("Edition","DiagnosticSmPromotionRefused",&[("details",details)]).to_string();
    }
    if let Some(field) = message.strip_prefix("Invalid General Options: ") {
        if deadsync_editor::options_preferences::General::NAMES.contains(&field) {
            return super::tr_fmt("Edition", "DiagnosticGeneralField", &[("field", field)])
                .to_string();
        }
    }
    let key = match message {
        "Pack.ini changed outside this editor" => "PackConflict",
        "Invalid pack property value" => "PackInvalidText",
        "Invalid pack year" => "PackInvalidYear",
        "Pack property history owner is no longer current" => "PackHistoryUnavailable",
        "Finish the pending durable pack save first" => "PackPending",
        "Profile resolution cancelled" => "DiagnosticProfileLoadCancelled",
        "Configured profile directory changed during song loading" => "DiagnosticProfileDirectoryChanged",
        "Generation profiles were already resolved for this load" => "DiagnosticProfilesAlreadyResolved",
        "Profile resolution belongs to another song or revision" => "DiagnosticProfileLoadStale",
        "Authoring promotion requires an SM document" => "DiagnosticSmPromotionRequiresSm",
        "Authoring promotion changed chart count" => "DiagnosticSmPromotionChartCount",
        "Authoring promotion changed chart metadata or order" => "DiagnosticSmPromotionMetadata",
        "Authoring promotion changed note attributes" => "DiagnosticSmPromotionNotes",
        "Authoring promotion changed the chart clock" => "DiagnosticSmPromotionClock",
        "History does not contain an authoring promotion" => "DiagnosticSmPromotionHistory",
        "Focused position is stale" => "DiagnosticFocusedPositionStale",
        "Focused chart is absent" => "DiagnosticFocusedChartMissing",
        "Focused chart view is absent" => "DiagnosticFocusedChartViewMissing",

        "This note kind is not available as a lane entry tool" => "DiagnosticLaneToolUnavailable",
        "This cell contains preserved note data; inserting here would discard it" => "DiagnosticOpaqueCellCollision",
        "Note reconstruction must preserve opaque source cells" => "DiagnosticOpaqueReconstruction",
        "Opaque standard cells cannot be converted to StepF2 notation" => "DiagnosticOpaqueStepF2",

        "Song timing chart no longer exists" => "DiagnosticSongTimingMissing",
        "Selected song timing chart no longer exists" => "DiagnosticSongTimingSelectedMissing",
        "Song timing chart no longer belongs to this song" => "DiagnosticSongTimingForeign",
        "Song timing chart is not loadable" => "DiagnosticSongTimingUnloaded",
        "Recovered song timing chart is not loadable" => "DiagnosticSongTimingRecovery",
        "SSC migration would change the editor's effective clock" => "DiagnosticMigrationClock",
        "History is not a clock-preserving SSC migration" => "DiagnosticMigrationHistory",
        "Migration changed the unfocused transport context" => "DiagnosticMigrationContext",
        "History is not a typed timing edit" => "DiagnosticTimingHistory",
        "Timing presentation belongs to another chart or revision" => "DiagnosticTimingPresentation",
        "Timing chart is absent from output order" => "DiagnosticSongTimingOrder",
        "Song properties draft is stale" => "DiagnosticSongPropertiesStale",
        "Song properties draft is stale or belongs to another document" => "DiagnosticSongPropertiesForeign",
        "Zero spacing rate has no unique inverse" => "DiagnosticSpacingInverse",
        "Nonfinite selected region geometry" => "DiagnosticRegionGeometry",
        "Invalid viewport event admission geometry" => "DiagnosticViewportGeometry",
        "Resident song preview path must be a single line" => "DiagnosticResidentPreviewPath",
        "Malformed resident song preview owner" => "DiagnosticResidentPreviewMalformed",
        "Unsupported resident song preview owner version" => "DiagnosticResidentPreviewVersion",
        "Repeated or chart-scoped resident song preview owners require explicit repair" => "DiagnosticResidentPreviewDuplicate",
        "Cannot encode resident song preview owner" => "DiagnosticResidentPreviewEncode",
        "Resident song preview owner requires SM authoring" => "DiagnosticResidentPreviewFormat",
        "No preview music selected" => "DiagnosticPreviewMissing",
        "Preview music file not found" => "DiagnosticPreviewFileMissing",
        "Preview length must be positive" => "DiagnosticPreviewLength",
        "GrooveAuthor song metadata must be an object" => "DiagnosticSongMetadataObject",
        "Staged chart edits cannot change visual preferences" => "DiagnosticVisualStaged",
        "Visual preference owners differ" => "DiagnosticVisualOwners",
        "Invalid waveform draw location" => "DiagnosticWaveformLocation",
        "Invalid waveform antialias parameter" => "DiagnosticWaveformAntialias",
        "Waveform value must be a finite 32-bit number" => "DiagnosticWaveformScalar",
        "Waveform RGBA components must be finite" | "Waveform colour components must be finite" => "DiagnosticWaveformColorFinite",
        "Waveform sparse colour scale must be finite" => "DiagnosticWaveformColorScaleFinite",
        "Song byte limit exceeded" => "DiagnosticSongSize",
        "Song opening cancelled" => "DiagnosticOpenCancelled",
        "Untitled song requires Save As" => "DiagnosticUntitledSaveAs",
        "Save As is required before creating a sidecar recovery" => "DiagnosticRecoveryNeedsSaveAs",
        "Untitled song has no saved baseline or recovery sidecar" => "DiagnosticUntitledNoBaseline",
        "Use Save for the current file" => "DiagnosticUseSave",
        "Save As destination must be a regular file or absent" => "DiagnosticSaveAsFile",
        "Generated output became stale" => "DiagnosticGeneratedStale",
        "Missing generated output" => "DiagnosticGeneratedMissing",
        "Save As source policy changed since review" => "DiagnosticSaveAsPolicyChanged",
        "External probe cancelled" => "DiagnosticProbeCancelled",
        "External change monitoring byte limit exceeded" => "DiagnosticProbeSize",
        "External song path is not a regular file" => "DiagnosticProbeFile",
        "Editor preferences must be an object" => "DiagnosticPreferencesObject",
        "Editor audio preferences must be an object" => "DiagnosticAudioObject",
        "Editor waveform preferences must be an object" => "DiagnosticWaveformObject",
        "Editor zoom preferences must be an object" => "DiagnosticZoomObject",
        "Editor scroll preferences must be an object" => "DiagnosticScrollObject",
        "Editor minimap preferences must be an object" => "DiagnosticMinimapObject",
        "Editor Receptors preferences must be an object" => "DiagnosticReceptorsObject",
        "Editor Options preferences must be an object" => "DiagnosticOptionsObject",
        "Unknown editor preferences version" => "DiagnosticPreferencesVersion",
        "Unknown GrooveAuthor chart metadata version" => "DiagnosticChartMetadataVersion",
        "Incomplete GrooveAuthor chart metadata" => "DiagnosticChartMetadataIncomplete",
        "Unknown GrooveAuthor song metadata version" => "DiagnosticSongMetadataVersion",
        "Incomplete GrooveAuthor song metadata" => "DiagnosticSongMetadataIncomplete",
        "Unknown native chart text version" => "DiagnosticChartTextVersion",
        "No chart selected for deletion" => "DiagnosticNoChartDelete",
        "No chart to transform" => "DiagnosticNoChartTransform",
        "No chart for note operation" => "DiagnosticNoChartNotes",
        "No attacks selected" => "DiagnosticNoAttacks",
        "No saved pattern markers in this chart" => "DiagnosticNoMarkers",
        "No saved pattern marker at the cursor" => "DiagnosticNoMarkerAtCursor",
        "No patterns selected for clearing" => "DiagnosticNoPatternsClear",
        "No patterns selected for generation" => "DiagnosticNoPatternsGenerate",
        "No source charts of the selected type" => "DiagnosticNoSourceCharts",
        "No tempo available for an attack duration" => "DiagnosticNoAttackTempo",
        "Attack inspector draft is stale" => "DiagnosticStaleAttackDraft",
        "Missing output draft" => "DiagnosticMissingOutputDraft",
        "Output settings became stale" => "DiagnosticStaleOutputDraft",
        "Select an existing timing chart" => "DiagnosticChooseTimingChart",
        "The output timing chart no longer exists; choose another chart explicitly" => {
            "DiagnosticMissingTimingChart"
        }
        "Source changed during export generation" => "DiagnosticChangedDuringExport",
        _ => return message.to_owned(),
    };
    translated(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(language: &str) {
        crate::i18n::init_for_tests();
        crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
    }

    #[test]
    fn typed_conflicts_and_reviews_preserve_distinct_reasons_in_both_languages() {
        for language in ["en", "fr"] {
            locale(language);
            let conflict = editor_error(&Error::ExternalChange);
            let recovery = editor_error(&Error::RecoveryConflict);
            let review = editor_error(&Error::OutputReviewRequired);
            assert_ne!(conflict, recovery);
            assert_ne!(review, recovery);
            assert_eq!(conflict, translated("DiagnosticExternalChange"));
            if language == "fr" {
                assert!(conflict.contains("fichier"));
                assert!(recovery.contains("récupération"));
                assert!(review.contains("revue"));
            }
        }
    }

    #[test]
    fn ordinary_io_is_localized_without_erasing_custom_payloads() {
        locale("fr");
        let error = io::Error::from(io::ErrorKind::PermissionDenied);
        assert_eq!(editor_error(&error), translated("DiagnosticIoPermission"));
        let custom = io::Error::new(
            io::ErrorKind::PermissionDenied,
            "details for /music/custom.ssc",
        );
        assert_eq!(editor_error(&custom), "details for /music/custom.ssc");
    }

    #[test]
    fn waveform_worker_states_keep_distinct_localized_reasons_and_decode_details() {
        use crate::editor_audio::LoadError;
        for language in ["en", "fr"] {
            locale(language);
            let busy = editor_error(&LoadError::Busy);
            let cancelled = editor_error(&LoadError::Cancelled);
            let stopped = editor_error(&LoadError::WorkerStopped);
            assert_eq!(busy, translated("DiagnosticWaveformBusy"));
            assert_eq!(cancelled, translated("DiagnosticWaveformCancelled"));
            assert_eq!(stopped, translated("DiagnosticWaveformStopped"));
            assert_ne!(busy, cancelled);
            assert_ne!(cancelled, stopped);
            let details = "decoder failed for /music/custom.ogg".to_owned();
            assert_eq!(editor_error(&LoadError::Decode(details.clone())), details);
        }
    }

    #[test]
    fn catalog_matches_whole_diagnostics_and_never_rewrites_user_values() {
        locale("fr");
        assert_eq!(
            editor_error(&Error::Unsupported(
                "Unknown editor preferences version".into()
            )),
            translated("DiagnosticPreferencesVersion")
        );
        let details = "Profile name: Unknown editor preferences version".to_owned();
        assert_eq!(editor_error(&details), details);
        assert_eq!(
            editor_error(&"unknown future diagnostic"),
            "unknown future diagnostic"
        );
    }

    #[test]
    fn storage_diagnostics_keep_the_required_recovery_action_in_both_languages() {
        for language in ["en", "fr"] {
            locale(language);
            for (message, key) in [
                ("Song byte limit exceeded", "DiagnosticSongSize"),
                ("Song opening cancelled", "DiagnosticOpenCancelled"),
                ("Untitled song requires Save As", "DiagnosticUntitledSaveAs"),
                ("Use Save for the current file", "DiagnosticUseSave"),
                ("Generated output became stale", "DiagnosticGeneratedStale"),
                (
                    "Save As source policy changed since review",
                    "DiagnosticSaveAsPolicyChanged",
                ),
                (
                    "External song path is not a regular file",
                    "DiagnosticProbeFile",
                ),
                (
                    "Editor Options preferences must be an object",
                    "DiagnosticOptionsObject",
                ),
            ] {
                let result = editor_error(&Error::InvalidEdit(message.into()));
                assert_eq!(result, translated(key));
                assert_ne!(result, key);
                if language == "fr" {
                    assert_ne!(result, message);
                }
            }
            let path = "Profile name: Generated output became stale";
            assert_eq!(editor_error(&path), path);
        }
    }

    #[test]
    fn general_validation_localizes_only_recognized_property_names() {
        for language in ["en", "fr"] {
            locale(language);
            for field in deadsync_editor::options_preferences::General::NAMES {
                let message = format!("Invalid General Options: {field}");
                let rendered = editor_error(&Error::InvalidFormat(message.clone()));
                assert!(rendered.contains(field));
                assert!(!rendered.contains("DiagnosticGeneralField"));
                if language == "fr" {
                    assert_ne!(rendered, message);
                }
            }
            let unknown = "Invalid General Options: user supplied unknown details";
            assert_eq!(editor_error(&unknown), unknown);
        }
    }
    #[test]
    fn song_timing_and_preview_errors_translate_without_rewriting_user_payloads() {
        for language in ["en", "fr"] {
            locale(language);
            for (message, key) in [
                ("Song timing chart no longer exists", "DiagnosticSongTimingMissing"),
                ("Selected song timing chart no longer exists", "DiagnosticSongTimingSelectedMissing"),
                ("Song timing chart no longer belongs to this song", "DiagnosticSongTimingForeign"),
                ("Song timing chart is not loadable", "DiagnosticSongTimingUnloaded"),
                ("Recovered song timing chart is not loadable", "DiagnosticSongTimingRecovery"),
                ("Timing chart is absent from output order", "DiagnosticSongTimingOrder"),
                ("Song properties draft is stale", "DiagnosticSongPropertiesStale"),
                ("Song properties draft is stale or belongs to another document", "DiagnosticSongPropertiesForeign"),
                ("Zero spacing rate has no unique inverse", "DiagnosticSpacingInverse"),
                ("Nonfinite selected region geometry", "DiagnosticRegionGeometry"),
                ("Invalid viewport event admission geometry", "DiagnosticViewportGeometry"),
                ("Resident song preview path must be a single line", "DiagnosticResidentPreviewPath"),
                ("Malformed resident song preview owner", "DiagnosticResidentPreviewMalformed"),
                ("Unsupported resident song preview owner version", "DiagnosticResidentPreviewVersion"),
                ("Repeated or chart-scoped resident song preview owners require explicit repair", "DiagnosticResidentPreviewDuplicate"),
                ("Cannot encode resident song preview owner", "DiagnosticResidentPreviewEncode"),
                ("Resident song preview owner requires SM authoring", "DiagnosticResidentPreviewFormat"),
                ("No preview music selected", "DiagnosticPreviewMissing"),
                ("Preview music file not found", "DiagnosticPreviewFileMissing"),
                ("Preview length must be positive", "DiagnosticPreviewLength"),
                ("GrooveAuthor song metadata must be an object", "DiagnosticSongMetadataObject"),
            ] {
                assert_eq!(editor_error(&Error::InvalidEdit(message.into())), translated(key));
                let user_value = format!("Chart description: {message}");
                assert_eq!(editor_error(&user_value), user_value);
            }
        }
    }

    #[test]
    fn opaque_note_and_lane_tool_diagnostics_are_localized_without_rewriting_payloads() {
        for language in ["en", "fr"] {
            locale(language);
            for (message, key) in [
                ("This note kind is not available as a lane entry tool", "DiagnosticLaneToolUnavailable"),
                ("This cell contains preserved note data; inserting here would discard it", "DiagnosticOpaqueCellCollision"),
                ("Note reconstruction must preserve opaque source cells", "DiagnosticOpaqueReconstruction"),
                ("Opaque standard cells cannot be converted to StepF2 notation", "DiagnosticOpaqueStepF2"),

            ] {
                let result = editor_error(&Error::Unsupported(message.into()));
                assert_eq!(result, translated(key));
                assert_ne!(result, key);
                if language == "fr" { assert_ne!(result, message); }
                let payload = format!("Chart description: {message}");
                assert_eq!(editor_error(&payload), payload);
            }
        }
    }

    #[test]
    fn sm_authoring_errors_localize_only_semantic_messages_and_keep_dynamic_details_exact() {
        for language in ["en", "fr"] {
            locale(language);
            for (message,key) in [
                ("Authoring promotion requires an SM document","DiagnosticSmPromotionRequiresSm"),
                ("Authoring promotion changed chart count","DiagnosticSmPromotionChartCount"),
                ("Authoring promotion changed chart metadata or order","DiagnosticSmPromotionMetadata"),
                ("Authoring promotion changed note attributes","DiagnosticSmPromotionNotes"),
                ("Authoring promotion changed the chart clock","DiagnosticSmPromotionClock"),
                ("History does not contain an authoring promotion","DiagnosticSmPromotionHistory"),
                ("Focused position is stale","DiagnosticFocusedPositionStale"),
                ("Focused chart is absent","DiagnosticFocusedChartMissing"),
                ("Focused chart view is absent","DiagnosticFocusedChartViewMissing"),

            ] {
                let rendered=editor_error(&Error::InvalidEdit(message.into()));
                assert_eq!(rendered,translated(key));assert_ne!(rendered,key);
                if language=="fr"{assert_ne!(rendered,message);}
                let user_value=format!("Chart description: {message}");assert_eq!(editor_error(&user_value),user_value);
            }
            let details="VENDOR_FIELD: authored {details}; \nMusic: chanson é\\;#exact";
            let message=format!("Lossless authoring promotion refused: {details}");
            let rendered=editor_error(&Error::Unsupported(message));
            assert_eq!(rendered,super::super::tr_fmt("Edition","DiagnosticSmPromotionRefused",&[("details",details)]).to_string());
            assert!(rendered.ends_with(details));assert!(!rendered.contains("DiagnosticSmPromotionRefused"));
            let user_value=format!("Chart description: Lossless authoring promotion refused: {details}");assert_eq!(editor_error(&user_value),user_value);
        }
    }

    #[test]
    fn unsafe_note_type_diagnostic_translates_without_translating_user_data() {
        for language in ["en","fr"] {
            locale(language);
            let message="Unsafe source stream NoteType: -99";
            let actual=editor_error(&Error::Unsupported(message.into()));
            assert_eq!(actual,translated("DiagnosticUnsafeStreamNoteType").replace("{value}","-99"));
            assert!(actual.contains("-99"));assert_ne!(actual,message);
            let user=format!("Description: {message}");assert_eq!(editor_error(&user),user);
        }
    }

    #[test]
    fn profile_loading_messages_are_localized_without_rewriting_user_text() {
        for language in ["en", "fr"] {
            locale(language);
            for (message, key) in [
                ("Profile resolution cancelled", "DiagnosticProfileLoadCancelled"),
                ("Configured profile directory changed during song loading", "DiagnosticProfileDirectoryChanged"),
                ("Generation profiles were already resolved for this load", "DiagnosticProfilesAlreadyResolved"),
                ("Profile resolution belongs to another song or revision", "DiagnosticProfileLoadStale"),

            ] {
                let result=editor_error(&Error::InvalidEdit(message.into()));
                assert_eq!(result,translated(key));
                assert_ne!(result,key);
                if language=="fr" {assert_ne!(result,message);}
                let user=format!("Description: {message}");
                assert_eq!(editor_error(&user),user);
            }
        }
    }

    #[test]
    fn timing_migration_and_live_diagnostics_are_translated_in_both_locales() {
        for language in ["en","fr"] {
            locale(language);
            for (message,key) in [
                ("SSC migration would change the editor's effective clock","DiagnosticMigrationClock"),
                ("History is not a clock-preserving SSC migration","DiagnosticMigrationHistory"),
                ("Migration changed the unfocused transport context","DiagnosticMigrationContext"),
                ("History is not a typed timing edit","DiagnosticTimingHistory"),
                ("Timing presentation belongs to another chart or revision","DiagnosticTimingPresentation"),
            ] {
                let result=editor_error(&Error::InvalidEdit(message.into()));
                assert_eq!(result,translated(key));assert_ne!(result,key);
                if language=="fr"{assert_ne!(result,message);}
            }
        }
    }

}
