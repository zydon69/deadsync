//! Snapshot-bound deletion, including the final chart; no implicit save.
use crate::{
    Error,
    document::{Document, Format},
    history::{RevisionToken, Session},
    notes::layout_columns,
};
use std::ops::Range;
pub struct Draft {
    token: RevisionToken,
    intent:crate::history::file_intent::IntentGuard,
    timing_after:Option<crate::song_timing_chart::Choice>,
    chart: usize,
    layout: String,
    description: String,
    last: bool,
}
impl Draft {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let snapshot = session.snapshot_document()?;
        let descriptor = snapshot
            .charts()
            .get(chart)
            .ok_or_else(|| invalid("No chart selected for deletion"))?;
        Ok(Self {
            token: session.revision_token(),intent:crate::history::file_intent::IntentGuard::capture(session),timing_after:None,
            chart,
            layout: descriptor.steps_type.clone(),
            description: descriptor.description.clone(),
            last: session.document().charts().len() == 1,
        })
    }
    pub fn prepare_song_timing(&mut self,session:&Session,preferences:&crate::open_preferences::Preferences,cultural_order:&[usize])->Result<(),Error>{
        self.check(session,self.chart)?;
        let snapshot=session.song_timing_snapshot()?;
        // Validate the complete current order before removing our source index.
        preferences.select(&snapshot,cultural_order)?;
        let remaining=snapshot.without_source(self.chart);
        let order=cultural_order.iter().copied().filter(|index|*index!=self.chart).collect::<Vec<_>>();
        let mut choice=session.song_timing_choice()?;choice.reconcile_after_deletion(&remaining,preferences,&order)?;self.timing_after=Some(choice);Ok(())
    }
    pub fn layout(&self) -> &str {
        &self.layout
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn is_last(&self) -> bool {
        self.last
    }
    pub(crate) fn check(&self, session: &Session, chart: usize) -> Result<(), Error> {
        if chart != self.chart || !session.accepts_token(&self.token) || !self.intent.is_current(session) {
            return Err(invalid(
                "Chart deletion belongs to another chart or revision",
            ));
        }
        Ok(())
    }
    pub fn apply(&self, session: &mut Session) -> Result<(), Error> {
        self.check(session, self.chart)?;
        session.remove_chart(self.chart,self.timing_after.as_ref())
    }
}
/// Original SelectBestChart type/difficulty priorities, with file order for ties.
/// Live ChartComparer sorting/runtime GUID tie resolution remains separate.
pub(crate) fn successor(document: &Document, deleted: usize) -> Option<usize> {
    let source = document.charts().get(deleted)?;
    let available = || {
        document
            .charts()
            .iter()
            .enumerate()
            .filter(|(i, c)| *i != deleted && layout_columns(&c.steps_type).is_some())
    };
    if let Some((index, _)) = available()
        .find(|(_, c)| c.steps_type == source.steps_type && c.difficulty == source.difficulty)
    {
        return Some(index);
    }
    let difficulty = ["Challenge", "Hard", "Medium", "Easy", "Beginner", "Edit"];
    for kind in [
        Some(source.steps_type.as_str()),
        match source.steps_type.as_str() {
            "dance-single" => Some("dance-double"),
            "dance-double" => Some("dance-single"),
            "pump-single" => Some("pump-double"),
            "pump-double" => Some("pump-single"),
            _ => None,
        },
    ]
    .into_iter()
    .flatten()
    {
        for level in difficulty {
            if let Some((index, _)) =
                available().find(|(_, c)| c.steps_type == kind && c.difficulty == level)
            {
                return Some(index);
            }
        }
    }
    for kind in crate::chart_creation::LAYOUTS {
        if let Some((index, _)) = available().find(|(_, c)| c.steps_type == kind) {
            return Some(index);
        }
    }
    None
}
pub(crate) fn prepare(document: &Document, chart: usize) -> Result<Vec<u8>, Error> {
    let range = document.chart_block_range(chart)?;
    let mut edits = vec![(range, Vec::new())];
    if document.format == Format::Sm {
        for tag in document.tags().iter().filter(|tag| tag.chart.is_none()) {
            for (prefix, spelling) in [
                ("GACHARTDATAVERSIONCHART", "GAChartDataVersionCHART"),
                ("GACHARTDATACHART", "GAChartDataCHART"),
                ("DSATTACKROWSCHART", "DSAttackRowsCHART"),
                ("DSATTACKROWSLOCALCHART", "DSAttackRowsLocalCHART"),
                ("DSCHARTATTACKSCHART", "DSChartAttacksCHART"),
                ("DSTIMINGCHART", "DSTimingCHART"),
            ] {
                let Some(suffix) = tag.name.strip_prefix(prefix) else {
                    continue;
                };
                if suffix.len() < 4 || !suffix.bytes().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let index = suffix
                    .parse::<usize>()
                    .map_err(|_| invalid("Invalid SM chart metadata index"))?;
                if index >= document.charts().len() {
                    continue;
                }
                let marker = document.tag_marker_range(tag);
                if index == chart {
                    edits.push((marker, Vec::new()));
                } else if index > chart {
                    edits.push((
                        marker.start + 1..tag.value.start - 1,
                        format!("{spelling}{:04}", index - 1).into_bytes(),
                    ));
                }
            }
        }
    }
    splice(document.source(), edits)
}
fn splice(source: &[u8], mut edits: Vec<(Range<usize>, Vec<u8>)>) -> Result<Vec<u8>, Error> {
    edits.sort_by_key(|(range, _)| range.start);
    let mut cursor = 0;
    let mut result = Vec::with_capacity(source.len());
    for (range, value) in edits {
        if range.start < cursor || range.end > source.len() {
            return Err(invalid("Overlapping chart deletion spans"));
        }
        result.extend_from_slice(&source[cursor..range.start]);
        result.extend_from_slice(&value);
        cursor = range.end;
    }
    result.extend_from_slice(&source[cursor..]);
    Ok(result)
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
