//! Closed timing transactions validate the prospective focused clock before publication.
pub use crate::history::timing_live::{Application, Plan};
use super::*;
#[derive(Clone, Copy)]
pub enum Anchor { MusicTime(f64), Row(f64) }
impl Anchor {
    pub(crate) fn position(self, session: &Session, chart: usize) -> Result<crate::chart_focus::Position, Error> {
        match self { Self::MusicTime(time)=>crate::chart_focus::Position::at_time(session,chart,time), Self::Row(row)=>crate::chart_focus::Position::at_fractional_row(session,chart,row) }
    }
}
/// Only these typed, in-memory timing edits may participate. No arbitrary
/// callback, file write, profile action or structural operation can be replayed.
#[derive(Clone, Copy)]
pub enum Edit<'a> {
    Tempo(&'a TempoDraft), Signatures(&'a signature::Draft), Scrolls(&'a scroll::Draft),
    Ticks(&'a tick::Draft), Combos(&'a combo::Draft), Speeds(&'a speed::Draft),
    Labels(&'a label::Draft), Pauses(&'a pause::Draft), Regions(&'a region::Draft),
}
impl Edit<'_> {
    pub(crate) fn apply(self, session: &mut Session, chart:usize)->Result<bool,Error>{
        match self { Self::Tempo(d)=>d.apply(session,chart), Self::Signatures(d)=>d.apply(session,chart), Self::Scrolls(d)=>d.apply(session,chart), Self::Ticks(d)=>d.apply(session,chart), Self::Combos(d)=>d.apply(session,chart), Self::Speeds(d)=>d.apply(session,chart), Self::Labels(d)=>d.apply(session,chart), Self::Pauses(d)=>d.apply(session,chart), Self::Regions(d)=>d.apply(session,chart) }
    }
    pub fn prepare(self, session:&Session, chart:usize, anchor:Anchor)->Result<Plan,Error>{Plan::prepare_edit(session,chart,anchor,self)}
}
