//! Session-scoped equivalent of Editor.PlayerPerChart. This is presentation
//! state, independent of chart Undo. Deleting a chart removes its cache entry;
//! closing a view retains it. Explicit user player selection clears all peers.
use super::*;
use std::collections::BTreeMap;
pub(super) struct Memory {
    session: crate::history::ContentToken,
    values: BTreeMap<crate::generation::metadata::Guid, u32>,
}
impl Memory {
    pub(super) fn new(session: &crate::history::Session, chart: usize, player: u32) -> Self {
        let mut values = BTreeMap::new();
        if let Some(id) = session.chart_ids().get(chart) {
            values.insert(id.clone(), player);
        }
        Self {
            session: session.content_token(),
            values,
        }
    }
    pub(super) fn prune(&mut self, session: &crate::history::Session) {
        if !session.owns_content_token(&self.session) {
            self.values.clear();
            self.session = session.content_token();
        }
        self.values.retain(|id, _| session.chart_ids().contains(id));
    }
    pub(super) fn explicit(
        &mut self,
        session: &crate::history::Session,
        chart: usize,
        player: u32,
    ) {
        self.prune(session);
        self.values.clear();
        if let Some(id) = session.chart_ids().get(chart) {
            self.values.insert(id.clone(), player);
        }
    }
}
impl Workspace {
    // FinishAddingChart initializes every new chart, even nonfocused batch
    // peers. Only structural additions/readditions qualify, never untouched
    // parsed charts whose cache may intentionally be absent after SetPlayer.
    pub(super) fn initialize_added_chart_players(&mut self,before:&[crate::generation::metadata::Guid])->Result<(),Error>{
        self.player_memory.prune(&self.file.session);
        let additions=self.file.session.chart_ids().iter().enumerate()
            .filter(|(_,id)|!before.contains(id)).map(|(index,id)|(index,id.clone())).collect::<Vec<_>>();
        for (index,id) in additions {
            let players=self.file.session.grid(index)?.players();
            self.player_memory.values.entry(id).or_insert(self.player.min(players-1));
        }
        Ok(())
    }
    pub(super) fn remembered_chart_player(&mut self, chart: usize) -> Result<u32, Error> {
        let players = self.file.session.grid(chart)?.players();
        self.player_memory.prune(&self.file.session);
        let id = self
            .file
            .session
            .chart_ids()
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Missing chart player identity".into()))?
            .clone();
        let player = self.player_memory.values.entry(id).or_insert(self.player);
        *player = (*player).min(players - 1);
        Ok(*player)
    }
}

#[cfg(test)]
mod addition_tests {
 use super::*;
 #[test]
 fn nonfocused_mixed_player_additions_keep_add_time_player_through_focus_and_history(){
  let root=tempfile::tempdir().unwrap();let path=root.path().join("players.ssc");
  std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000;").unwrap();
  let mut w=Workspace::open(&path,0).unwrap();w.execute(Command::SelectPlayer(1)).unwrap();
  let before=w.file.session.chart_ids().to_vec();
  let routine=w.grid().clone();let solo=crate::notes::Grid::parse_layout(b"0000","dance-single").unwrap();
  let revision=w.file.session.revision();
  let indices=w.file.session.append_charts(&[
   crate::history::ChartAddition{source_chart:0,layout:"dance-single",grid:&solo},
   crate::history::ChartAddition{source_chart:0,layout:"dance-routine",grid:&routine}],revision,"Batch fixture").unwrap();
  w.initialize_added_chart_players(&before).unwrap();
  w.execute(Command::SelectChart(indices[0])).unwrap();assert_eq!(w.player(),0);
  w.execute(Command::SelectChart(indices[1])).unwrap();assert_eq!(w.player(),1);
  w.execute(Command::SelectChart(indices[0])).unwrap();
  w.execute(Command::Undo).unwrap();
  w.execute(Command::SelectChart(0)).unwrap();w.execute(Command::SelectPlayer(0)).unwrap();
  // Redo readds both at the current global player0; old removed caches cannot leak.
  w.execute(Command::Redo).unwrap();w.execute(Command::SelectChart(indices[1])).unwrap();assert_eq!(w.player(),0);
  // Explicit intent must still clear a peer instead of filling every absent ID.
  w.execute(Command::SelectPlayer(1)).unwrap();w.execute(Command::SelectChart(indices[0])).unwrap();w.execute(Command::SelectPlayer(0)).unwrap();
  w.execute(Command::SelectChart(indices[1])).unwrap();assert_eq!(w.player(),0);
 }
}
