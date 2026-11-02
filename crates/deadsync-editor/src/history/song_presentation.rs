//! Opaque shared metadata proof, minted only by real validated song/chart commits.
use super::{ContentToken, Session};
use crate::{Error, generation::metadata::Guid, song_properties::Draft};
use std::sync::Arc;
pub struct Application {
    changed: bool,
    receipt: Option<PresentationReceipt>,
}
impl Application {
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn presentation_receipt(&self) -> Option<&PresentationReceipt> {
        self.receipt.as_ref()
    }
}
/// Opaque capture of the real pre-commit session, never a caller-made token.
pub struct PresentationOrigin(ContentToken);
impl PresentationOrigin { pub fn capture(session:&Session)->Self{Self(session.content_token())} }
pub struct PresentationReceipt {
    before: ContentToken,
    after: ContentToken,
    charts: Vec<Guid>,
    audio: bool,
}
impl PresentationReceipt {
    /// Check an opaque before/after content proof without inventing a cursor.
    pub fn validate_transition(&self, before:&PresentationOrigin, session:&Session)->Result<(),Error>{
        self.rebind_token(&before.0,session).map(|_|())
    }
    pub fn preserves_audio(&self) -> bool {
        self.audio
    }
    pub(crate) fn accepts_before(&self, token: &ContentToken) -> bool {
        Arc::ptr_eq(&self.before.identity, &token.identity)
            && self.before.revision == token.revision
    }
    pub(crate) fn rebind_token(
        &self,
        token: &ContentToken,
        session: &Session,
    ) -> Result<ContentToken, Error> {
        if !self.accepts_before(token)
            || !session.accepts_content_token(&self.after)
            || session.chart_ids() != self.charts.as_slice()
        {
            return Err(Error::InvalidEdit(
                "Song presentation proof is stale or belongs to another document/chart".into(),
            ));
        }
        Ok(self.after.clone())
    }
}
pub(crate) fn apply(draft: &Draft, session: &mut Session) -> Result<Application, Error> {
    let before = session.content_token();
    let charts = session.chart_ids().to_vec();
    let clock = draft.preserves_presentation_clock();
    let audio = draft.preserves_presentation_audio();
    let changed = draft.apply(session)?;
    if changed && clock && audio {session.wrap_song_properties_metadata();}
    let receipt = (changed && clock && charts.as_slice() == session.chart_ids()).then(|| {
        PresentationReceipt {
            before,
            after: session.content_token(),
            charts,
            audio,
        }
    });
    Ok(Application { changed, receipt })
}

/// Second typed producer: chart properties cannot alter timing events, song
/// offsets or chart identities. Only their effective offset may change clocks.
pub(crate) fn apply_chart(
    draft: &crate::chart_properties::Draft,
    session: &mut Session,
    chart: usize,
) -> Result<Application, Error> {
    let before = session.content_token();
    let charts = session.chart_ids().to_vec();
    let clock = draft.preserves_presentation_clock();
    let audio = draft.preserves_presentation_audio(session.document());
    let changed = draft.apply(session, chart)?;
    let receipt = (changed && clock && charts.as_slice() == session.chart_ids()).then(|| {
        PresentationReceipt {
            before,
            after: session.content_token(),
            charts,
            audio,
        }
    });
    Ok(Application { changed, receipt })
}

/// Typed mixed clipboard producer. A clipboard with no rate/timing payload can
/// only change lane notes, attacks, patterns and the two song widget fields.
/// Those operations cannot change any chart clock, music path or chart identity.
pub(crate) fn apply_clipboard(clipboard:&crate::event_clipboard::Clipboard,session:&mut Session,chart:usize,row:u64,player:u32)->Result<(crate::event_clipboard::PasteReport,Application),Error>{
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let clock=clipboard.preserves_presentation_clock();
    let report=clipboard.paste_with_report(session,chart,row,player)?;
    let receipt=(report.changed&&clock&&charts.as_slice()==session.chart_ids()).then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok((report,Application{changed:report.changed,receipt}))
}

/// History proof exists only for the private pure-song-timing change wrapper.
pub(crate) fn apply_timing_history(session:&mut Session,redo:bool)->Result<Application,Error>{
    if !session.song_timing_history_preserves_presentation(redo){return Err(Error::InvalidEdit("History entry is not a pure song timing choice".into()));}
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=if redo{session.redo()?}else{session.undo()?};
    let receipt=changed.then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});Ok(Application{changed,receipt})
}

pub(crate) fn preserved_authoring(before:ContentToken,charts:Vec<Guid>,session:&Session)->Application{
    debug_assert_eq!(charts.as_slice(),session.chart_ids());
    Application{changed:true,receipt:Some(PresentationReceipt{before,after:session.content_token(),charts,audio:true})}
}

/// A validated pattern preview changes lane notes and its captured GA marker
/// metadata, never chart clocks, media paths or chart identities. Keep the exact
/// presentation coordinate, including stop interior/equal-time warp positions.
pub(crate) fn apply_pattern(preview:&crate::generation::pattern_job::Preview,session:&mut Session,chart:usize)->Result<Application,Error>{
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=preview.apply(session,chart)?;
    let receipt=(changed&&charts.as_slice()==session.chart_ids()).then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}

/// Validated saved-pattern note operation; no clock or media payload exists.
pub(crate) fn apply_pattern_set(preview:&crate::generation::pattern_batch::Preview,session:&mut Session,chart:usize)->Result<Application,Error>{
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=preview.apply(session,chart)?;
    let receipt=(changed&&charts.as_slice()==session.chart_ids()).then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}

/// Validated saved-pattern note operation; no clock or media payload exists.
pub(crate) fn apply_cleared_pattern(preview:&crate::generation::clear_patterns::Preview,session:&mut Session,chart:usize)->Result<Application,Error>{
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=preview.apply(session,chart)?;
    let receipt=(changed&&charts.as_slice()==session.chart_ids()).then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}

/// Seed callbacks can only change the captured marker's random seed; no clock,
/// media path, chart identity, cursor or note payload participates.
pub(crate) fn reseed_marker(draft:&crate::generation::marker_edit::Draft,session:&mut Session,chart:usize,seed:i32)->Result<Application,Error>{
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    draft.reseed(session,chart,seed)?;
    Ok(preserved_authoring(before,charts,session))
}

/// Only the reviewed migration producer may mint this clock-preserving proof.
pub(crate) fn apply_timing_migration(draft:&crate::timing_edit::migration::Draft,session:&mut Session)->Result<Application,Error>{
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=draft.apply(session)?;
    let receipt=changed.then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}
pub(crate) fn apply_timing_migration_history(session:&mut Session,redo:bool)->Result<Application,Error>{
    if !session.history_preserves_timing_migration(redo){return Err(Error::InvalidEdit("History is not a clock-preserving SSC migration".into()));}
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=if redo{session.redo()?}else{session.undo()?};
    let receipt=changed.then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}

/// Only the real typed ChartProperties entry supplies these effect flags.
pub(super) fn apply_chart_properties_history(session:&mut Session,redo:bool,clock:bool,audio:bool)->Result<Application,Error>{
 let before=session.content_token();let charts=session.chart_ids().to_vec();
 let changed=if redo{session.redo()?}else{session.undo()?};
 let receipt=(changed&&clock&&charts.as_slice()==session.chart_ids()).then(||PresentationReceipt{before,after:session.content_token(),charts,audio});
 Ok(Application{changed,receipt})
}

/// Validated marker editing only changes pattern/config metadata in the existing
/// chart owner; it cannot change SyncOffset, timing, music or chart identities.
pub(crate) fn apply_marker_metadata(draft:&crate::generation::marker_edit::Draft,session:&mut Session,chart:usize,delete:bool)->Result<Application,Error>{
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=if delete{draft.delete(session,chart)?}else{draft.apply(session,chart)?};
    session.wrap_marker_metadata(changed);
    let receipt=changed.then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}
pub(crate) fn apply_marker_metadata_history(session:&mut Session,redo:bool)->Result<Application,Error>{
    if !session.history_preserves_marker_metadata(redo){return Err(Error::InvalidEdit("History is not marker metadata".into()));}
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=if redo{session.redo()?}else{session.undo()?};
    let receipt=changed.then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}

/// Private Draft-produced metadata has an unchanged main music path and clock.
/// Dedicated Preview and sample-region notifications are applied by the native
/// registration/region consumers after the exact presentation rebind.
pub(crate) fn apply_song_metadata_history(session:&mut Session,redo:bool)->Result<Application,Error>{
    session.validate_song_properties_metadata_history(redo)?;
    let before=session.content_token();let charts=session.chart_ids().to_vec();
    let changed=if redo{session.redo()?}else{session.undo()?};
    let receipt=changed.then(||PresentationReceipt{before,after:session.content_token(),charts,audio:true});
    Ok(Application{changed,receipt})
}
