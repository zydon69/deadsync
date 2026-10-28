//! MRU capture uses descriptor values and the exact displayed fractional row;
//! no source indexes or resident chart GUIDs are persisted as cross-load IDs.
use super::State;
use deadsync_editor::{Error, chart_order::Key, recent_songs::{SavedChart, Song}};
impl State {
    pub(super) fn capture_recent_song(&self) -> Result<Option<Song>, Error> {
        self.workspace.file().existing_path().map(|path|self.capture_recent_song_at(path)).transpose()
    }
    pub(super) fn capture_recent_song_at(&self,file:&std::path::Path)->Result<Song,Error>{
        let session=&self.workspace.file().session;
        let mut indices=Vec::new();
        if let Some(views)=&self.active_views {
            if !views.belongs_to(session) {return Err(Error::InvalidEdit("Recent chart views belong to another song".into()));}
            for view in views.entries() {
                if session.chart_ids().get(view.source_index()) != Some(view.identity()) {
                    return Err(Error::InvalidEdit("Recent chart view index is stale".into()));
                }
                indices.push(view.source_index());
            }
        } else if let Some(chart)=self.workspace.active_chart() {indices.push(chart);}
        let mut active=Vec::with_capacity(indices.len());
        for &index in &indices {
            let identity=session.chart_ids().get(index).ok_or_else(||Error::InvalidEdit("Recent chart index is stale".into()))?.clone();
            // SavedChart doesn't contain StepCount. Avoid decoding every chart
            // merely to capture metadata before a file transition.
            active.push(SavedChart::capture(&Key::read(session.document(),index,0,identity)?));
        }
        let focused=self.workspace.active_chart().and_then(|chart|indices.iter().position(|index|*index==chart));
        let row=self.playback_position().map(|(_,row)|row)
            .or_else(||self.focus_position.as_ref().filter(|position|position.is_current(&self.workspace)).map(|position|position.row()))
            .unwrap_or(self.workspace.row() as f64);
        if !row.is_finite() {return Err(Error::InvalidEdit("Recent chart row is not finite".into()));}
        let document=session.document();
        let pack=file.parent().and_then(std::path::Path::parent).and_then(std::path::Path::file_name).and_then(std::ffi::OsStr::to_str).map(str::to_owned);
        Ok(Song { file:file.to_owned(),title:document.text(None,"TITLE"),pack,spacing_zoom:self.zoom.spacing_zoom(),chart_position:row,active,focused })
    }

}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untitled_has_no_mru_entry_or_fabricated_filename() {
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();
        let state=State::from_workspace(deadsync_editor::workspace::Workspace::new_song(root.path(),0.009).unwrap()).unwrap();
        assert!(state.capture_recent_song().unwrap().is_none());assert_eq!(std::fs::read_dir(root.path()).unwrap().count(),0);
    }
    #[test]
    fn descriptors_and_fractional_warp_row_are_captured_without_reinverting_time() {
        crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let file=root.path().join("song.ssc");
        std::fs::write(&file,"#VERSION:0.83;#TITLE:Test;#BPMS:0=120;#WARPS:2=4;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:9;#NOTES:1000;").unwrap();
        let mut state=State::open(&file,0).unwrap();state.workspace.seek_preserving_selection(160).unwrap();state.focus_position=Some(deadsync_editor::chart_focus::Position::at_fractional_row(&state.workspace.file().session,0,160.5).unwrap());
        let song=state.capture_recent_song().unwrap().unwrap();assert_eq!(song.chart_position,160.5);assert_eq!(song.focused,Some(0));assert_eq!(song.active[0].rating,9);assert_eq!(song.file,file.canonicalize().unwrap());
    }
}
