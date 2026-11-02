use super::*;
use crate::{document::Format, history::Session, notes::Grid};

// One bounded entry per toolbox avoids retaining decoded PCM or unbounded song
// histories. Changes to the canonical path, length or modification time invalidate it.
pub(super) struct AnalysisCache {
    path: std::path::PathBuf,
    length: u64,
    modified: Option<std::time::SystemTime>,
    rhythm: deadsync_audio_analysis::rhythm::Rhythm,
}

pub(super) struct Preview {
    token: RevisionToken,
    grid: Grid,
    layout: String,
    bpm: f64,
    offset: f64,
    meter: u32,
}
impl Preview {
    pub(super) fn apply(&self, session: &mut Session) -> Result<usize, Error> {
        if !session.accepts_token(&self.token) {
            return invalid("Audio preview is stale; generate it again");
        }
        if session.document().format != Format::Ssc {
            return invalid("Audio chart generation requires SSC format");
        }
        let mut block=format!("#NOTEDATA:;\n#STEPSTYPE:{};\n#DESCRIPTION:Assistant audio proposal;\n#DIFFICULTY:Edit;\n#METER:{};\n#BPMS:0={};\n#OFFSET:{};\n#TIMESIGNATURES:0=4=4;\n#NOTES:\n",self.layout,self.meter,self.bpm,-self.offset).into_bytes();
        block.extend(self.grid.serialize()?);
        block.extend_from_slice(b";\n");
        session.insert_empty_chart(
            block,
            &self.grid,
            crate::generation::metadata::ChartData::default(),
            None,
        )
    }
}
impl Toolbox {
    pub(super) fn audio(
        &mut self,
        workspace: &mut Workspace,
        name: &str,
        args: &Value,
        cancelled: &AtomicBool,
    ) -> Result<Value, Error> {
        let base = workspace.file().resource_base().canonicalize()?;
        let music = workspace
            .file()
            .session
            .document()
            .text(None, "MUSIC")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::InvalidEdit("Song has no music file".into()))?;
        let path = base.join(music.replace('\\', "/")).canonicalize()?;
        if !path.starts_with(&base) {
            return invalid("Assistant audio must be inside the song directory");
        }
        let metadata = std::fs::metadata(&path)?;
        let modified = metadata.modified().ok();
        let reusable = modified.is_some()
            && self.audio_cache.as_ref().is_some_and(|cached| {
                cached.path == path
                    && cached.length == metadata.len()
                    && cached.modified == modified
            });
        if !reusable {
            let rhythm = deadsync_audio_analysis::rhythm::analyze_file(&path, cancelled)
                .map_err(Error::InvalidEdit)?;
            if cancelled.load(Ordering::Acquire) {
                return invalid("Assistant request cancelled");
            }
            // Never cache analysis if the source changed during decoding.
            let after = std::fs::metadata(&path)?;
            if after.len() != metadata.len() || after.modified().ok() != modified {
                return invalid("Audio changed during analysis; retry the request");
            }
            self.audio_cache = Some(AnalysisCache {
                path,
                length: metadata.len(),
                modified,
                rhythm,
            });
        }
        let rhythm = &self
            .audio_cache
            .as_ref()
            .expect("analysis cache populated")
            .rhythm;
        let mut description = json!({"duration_seconds":rhythm.duration_seconds,"estimated_bpm":rhythm.bpm,
            "beat_offset_seconds":rhythm.beat_offset_seconds,"tempo_confidence":rhythm.confidence,
            "detected_onsets":rhythm.onsets.len(),"timing_warning":"Constant-tempo estimate; verify tempo, phase, meter and musical phrasing before saving."});
        description["sections"]=json!(rhythm.sections.iter().map(|&(start,end,energy,onsets)|json!({"start_seconds":start,"end_seconds":end,"relative_rms_energy":energy,"onset_count":onsets})).collect::<Vec<_>>());
        if name == "analyze_audio" {
            return Ok(description);
        }
        if workspace.file().session.document().format != Format::Ssc {
            return invalid("Audio chart generation requires SSC format");
        }
        let bpm = number(args, "bpm", rhythm.bpm, 40.0, 300.0)?;
        let offset = number(
            args,
            "beat_offset_seconds",
            rhythm.beat_offset_seconds,
            0.0,
            rhythm.duration_seconds,
        )?;
        let max_nps = number(args, "max_nps", 6.0, 0.5, 30.0)?;
        let minimum_energy = number(args, "minimum_relative_energy", 0.05, 0.0, 1.0)?;
        let subdivision = unsigned(args, "subdivision", 4)?;
        if ![1, 2, 3, 4, 6, 8, 12, 16].contains(&subdivision) {
            return invalid("Unsupported beat subdivision");
        }
        let layout = args
            .get("target_layout")
            .and_then(Value::as_str)
            .unwrap_or("dance-single");
        if !crate::generation::job::TARGET_LAYOUTS.contains(&layout) {
            return invalid("Unsupported generation layout");
        }
        let seed = args
            .get("seed")
            .map(|v| {
                v.as_i64()
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or_else(|| Error::InvalidEdit("Invalid seed".into()))
            })
            .transpose()?
            .unwrap_or(0);
        let mut source = Grid::new(4, 1)?;
        let mut last = None;
        let mut count = 0usize;
        for &(second, _) in &rhythm.onsets {
            if rhythm
                .sections
                .iter()
                .find(|&&(start, end, _, _)| second >= start && second < end)
                .is_some_and(|&(_, _, energy, _)| energy < minimum_energy)
            {
                continue;
            }
            if cancelled.load(Ordering::Acquire) {
                return invalid("Audio generation cancelled");
            }
            let beat = (second - offset) * bpm / 60.0;
            if beat < 0.0 {
                continue;
            }
            let row =
                ((beat * subdivision as f64).round() * 48.0 / subdivision as f64).round() as u64;
            let snapped = offset + row as f64 / 48.0 * 60.0 / bpm;
            if row > i32::MAX as u64
                || (snapped - second).abs() > 0.09
                || last.is_some_and(|previous: f64| snapped - previous < 1.0 / max_nps)
            {
                continue;
            }
            source.set(
                Position {
                    row,
                    lane: [0, 3, 1, 2][count % 4],
                    player: 0,
                },
                Some(Note::tap()),
            )?;
            last = Some(snapped);
            count += 1;
        }
        if count < 4 {
            return invalid(
                "Too few onsets align to this tempo; adjust BPM/offset before generating",
            );
        }
        let mut bytes=format!("#VERSION:0.83;#BPMS:0={bpm};#OFFSET:{};#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:\n",-offset).into_bytes();
        bytes.extend(source.serialize()?);
        bytes.extend_from_slice(b";\n");
        let synthetic = Session::new(crate::document::Document::parse(
            Arc::<[u8]>::from(bytes),
            Format::Ssc,
        )?);
        let mut options = generation_options(args, layout)?;
        options.seed = seed;
        let generated = crate::generation::job::ChartTask::capture(&synthetic, 0, options)?
            .run(cancelled, |_| {})?;
        let grid = generated.grid().clone();
        description["type"] = json!("audio_chart");
        description["notes"] = json!(grid.head_count());
        description["target_layout"] = json!(layout);
        description["bpm"] = json!(bpm);
        description["onsets_not_used"] = json!(rhythm.onsets.len().saturating_sub(count));
        description["minimum_relative_energy"] = json!(minimum_energy);
        self.install(
            Change::Audio(Preview {
                token: workspace.file().session.revision_token(),
                grid,
                layout: layout.into(),
                bpm,
                offset,
                meter: unsigned(args, "meter", 1)?.clamp(1, 99) as u32,
            }),
            description,
        )
    }
}
fn number(args: &Value, key: &str, default: f64, min: f64, max: f64) -> Result<f64, Error> {
    let v = args
        .get(key)
        .map_or(Some(default), Value::as_f64)
        .filter(|v| v.is_finite() && *v >= min && *v <= max);
    v.ok_or_else(|| Error::InvalidEdit(format!("Invalid {key}; expected {min}..{max}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_origin::{NewSong, Origin};
    use crate::storage::FileSession;

    #[test]
    fn audio_analysis_reuses_the_same_source_and_invalidates_replaced_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("music.wav");
        // Invalid audio deliberately proves that a matching cached analysis is
        // returned without reopening the decoder.
        std::fs::write(&path, b"not audio").unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let document = crate::document::Document::parse(
            Arc::<[u8]>::from(b"#VERSION:0.83;#MUSIC:music.wav;#BPMS:0=120;".as_slice()),
            Format::Ssc,
        )
        .unwrap();
        let file = FileSession::from_new(NewSong {
            origin: Origin::Untitled {
                resource_base: directory.path().to_owned(),
            },
            session: Session::new(document),
        })
        .unwrap();
        let mut workspace = Workspace::from_file_unfocused(file);
        let mut toolbox = Toolbox::default();
        toolbox.audio_cache = Some(AnalysisCache {
            path: path.canonicalize().unwrap(),
            length: metadata.len(),
            modified: metadata.modified().ok(),
            rhythm: deadsync_audio_analysis::rhythm::Rhythm {
                duration_seconds: 10.0,
                bpm: 120.0,
                beat_offset_seconds: 0.0,
                confidence: 0.75,
                onsets: vec![(1.0, 1.0)],
                sections: vec![],
            },
        });
        let response = toolbox
            .audio(
                &mut workspace,
                "analyze_audio",
                &json!({}),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(response["estimated_bpm"], 120.0);
        assert_eq!(response["detected_onsets"], 1);
        std::fs::write(&path, b"different invalid audio bytes").unwrap();
        assert!(
            toolbox
                .audio(
                    &mut workspace,
                    "analyze_audio",
                    &json!({}),
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
}
