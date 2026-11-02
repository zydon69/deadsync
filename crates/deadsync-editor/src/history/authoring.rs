//! Explicit in-memory promotion. Origin, source bytes and filesystem ownership
//! remain in FileSession; the document swap is one reversible history gesture.
use super::*;
use crate::{
    advanced_export::{self, Limits, Options},
    document::Format,
    output_policy::OutputPolicy,
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
pub struct Plan {
    output: advanced_export::Plan,
}
impl Plan {
    pub fn prepare(session: &mut Session) -> Result<Self, Error> {
        if session.document().format != Format::Sm {
            return Err(Error::InvalidEdit(
                "Authoring promotion requires an SM document".into(),
            ));
        }
        let options = Options {
            outfox_smx: false,
            ..Default::default()
        };
        let output = advanced_export::Plan::prepare_song_timing(
            session,
            Format::Ssc,
            options,
            Limits::default(),
            &AtomicBool::new(false),
        )?;
        // Promotion cannot acknowledge any loss, including unknown tags or
        // note attributes. Keep the original model and both history branches.
        if !output.issues().is_empty() {
            return Err(Error::Unsupported(format!(
                "Lossless authoring promotion refused: {}",
                output
                    .issues()
                    .iter()
                    .map(|issue| format!("{}: {}", issue.field, issue.detail))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        Ok(Self { output })
    }
    pub fn is_current(&self, session: &Session) -> bool {
        self.output.is_current(session)
    }
    pub(crate) fn apply(
        self,
        session: &mut Session,
        before_policy: OutputPolicy,
        after_policy: OutputPolicy,
    ) -> Result<crate::song_properties::Application, Error> {
        let output = self.output.generate(
            session,
            &BTreeSet::new(),
            Limits::default().bytes,
            &AtomicBool::new(false),
        )?;
        let document = Document::parse_with_context(
            output.bytes(),
            Format::Ssc,
            session.document().load_context(),
        )?;
        let mut candidate = session.detached_output_snapshot(document)?;
        if candidate.document().charts().len() != session.document().charts().len() {
            return Err(Error::InvalidEdit(
                "Authoring promotion changed chart count".into(),
            ));
        }
        let mut grids = BTreeMap::new();
        for chart in 0..session.chart_ids.len() {
            let before_key = crate::chart_order::Key::read(
                session.document(),
                chart,
                0,
                session.chart_ids[chart].clone(),
            )?;
            let after_key = crate::chart_order::Key::read(
                candidate.document(),
                chart,
                0,
                session.chart_ids[chart].clone(),
            )?;
            let before_text = crate::chart_text::Text::read(session.document(), chart)?;
            let after_text = crate::chart_text::Text::read(candidate.document(), chart)?;
            if before_key.steps_type != after_key.steps_type
                || before_key.difficulty != after_key.difficulty
                || before_key.rating != after_key.rating
                || before_key.name != after_key.name
                || before_key.description != after_key.description
                || crate::chart_text::Field::ALL
                    .into_iter()
                    .any(|field| before_text.get(field) != after_text.get(field))
            {
                return Err(Error::InvalidEdit(
                    "Authoring promotion changed chart metadata or order".into(),
                ));
            }
            let after = candidate.grid(chart)?.clone();
            if session.grid(chart)? != &after {
                return Err(Error::InvalidEdit(
                    "Authoring promotion changed note attributes".into(),
                ));
            }
            let before_clock = crate::playback::Clock::capture(session, chart)?;
            let after_clock = crate::playback::Clock::capture(&candidate, chart)?;
            if before_clock.rate_mappings().collect::<Vec<_>>()
                != after_clock.rate_mappings().collect::<Vec<_>>()
                || before_clock.music_time(0)?.to_bits() != after_clock.music_time(0)?.to_bits()
            {
                return Err(Error::InvalidEdit(
                    "Authoring promotion changed the chart clock".into(),
                ));
            }
            grids.insert(chart, after);
        }
        let before = session.content_token();
        let charts = session.chart_ids.clone();
        let timing = session.song_timing_choice()?;
        let document = candidate.snapshot_document()?;
        session.execute(
            "Enable full timing authoring".into(),
            Change::DocumentSwap(Box::new(DocumentState {
                document,
                grids,
                touched_grids: Default::default(),
                chart_ids: charts.clone(),
                song_timing: Some(timing),
            })),
        )?;
        session
            .undo
            .last_mut()
            .expect("promotion committed history")
            .authoring_policy = Some((before_policy, after_policy.clone()));
        session.authoring_policy_update = Some(after_policy);
        Ok(super::song_presentation::preserved_authoring(
            before, charts, session,
        ))
    }
}
impl Session {
    pub(crate) fn next_authoring_policy(&self, redo: bool) -> Option<&OutputPolicy> {
        let entry = if redo {
            self.redo.last()
        } else {
            self.undo.last()
        }?;
        entry
            .authoring_policy
            .as_ref()
            .map(|(before, after)| if redo { after } else { before })
    }
    pub(crate) fn pending_authoring_policy(&self) -> Option<&OutputPolicy> {
        self.authoring_policy_update.as_ref()
    }
    pub(crate) fn take_authoring_policy(&mut self) -> Option<OutputPolicy> {
        self.authoring_policy_update.take()
    }
    /// Publication changes the destination contract for every remaining history
    /// state. Undoing old in-memory promotion must never restore a prior path's
    /// output format after a successful Save As.
    pub(crate) fn rebase_authoring_policy_history(
        &mut self,
        policy: &OutputPolicy,
        format: Format,
    ) {
        // NativeLossless follows the current in-memory format. History crosses
        // SM/SSC boundaries, so pin only its publication contract to the actual
        // destination format; never reinterpret a .ssc destination as SM.
        let fixed = match policy {
            OutputPolicy::NativeLossless => OutputPolicy::AdvancedSongTiming {
                format,
                options: Options {
                    outfox_smx: false,
                    ..Default::default()
                },
            },
            policy => policy.clone(),
        };
        for entry in self.undo.iter_mut().chain(&mut self.redo) {
            if let Some((before, after)) = &mut entry.authoring_policy {
                *before = fixed.clone();
                *after = fixed.clone();
            }
        }
        self.authoring_policy_update = None;
    }
}
